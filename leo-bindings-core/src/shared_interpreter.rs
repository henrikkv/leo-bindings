use leo_ast::Location;
use leo_ast::NetworkName;
use leo_errors::{CompilerError, Result};
use leo_interpreter::{Element, Frame, FunctionVariant, Interpreter};
use leo_parser;
use leo_span::source_map::FileName;
use leo_span::{with_session_globals, SessionGlobals, Symbol, SESSION_GLOBALS};
use snarkvm::prelude::{Program, TestnetV0};
use std::{cell::RefCell, collections::HashMap, fs, path::Path, rc::Rc};

pub struct SharedInterpreterState {
    pub interpreter: RefCell<Interpreter>,
    pub session: SessionGlobals,
}

thread_local! {
    static SHARED_INTERPRETER: RefCell<Option<Rc<SharedInterpreterState>>> = const { RefCell::new(None) };
}

pub fn initialize_shared_interpreter(interpreter: Interpreter, session: SessionGlobals) -> bool {
    SHARED_INTERPRETER.with(|shared| {
        let mut state = shared.borrow_mut();
        if state.is_none() {
            *state = Some(Rc::new(SharedInterpreterState {
                interpreter: RefCell::new(interpreter),
                session,
            }));
            true
        } else {
            false
        }
    })
}

pub fn with_shared_interpreter<T, F>(f: F) -> Option<T>
where
    F: FnOnce(&SharedInterpreterState) -> T,
{
    SHARED_INTERPRETER.with(|shared| {
        shared
            .borrow()
            .as_ref()
            .map(|state| SESSION_GLOBALS.set(&state.session, || f(state)))
    })
}

pub trait InterpreterExtensions {
    fn load_leo_program(&mut self, path: &Path) -> Result<()>;

    fn load_aleo_program_from_string(&mut self, bytecode: &str) -> Result<()>;

    fn is_program_loaded(&self, program_name: &str) -> bool;

    fn get_loaded_programs(&self) -> Vec<String>;
}

impl InterpreterExtensions for Interpreter {
    fn load_leo_program(&mut self, path: &Path) -> Result<()> {
        let text = fs::read_to_string(path).map_err(|e| CompilerError::file_read_error(path, e))?;
        let source_file = with_session_globals(|s| {
            s.source_map
                .new_source(&text, FileName::Real(path.to_path_buf()))
        });

        let ast = leo_parser::parse_ast(
            self.handler.clone(),
            &self.node_builder,
            &source_file,
            &[],
            NetworkName::TestnetV0,
        )?;

        for (&program, scope) in ast.ast.program_scopes.iter() {
            self.filename_to_program
                .insert(path.to_path_buf(), program.to_string());

            for (name, function) in scope.functions.iter() {
                self.cursor.functions.insert(
                    Location::new(program, vec![*name]),
                    FunctionVariant::Leo(function.clone()),
                );
            }

            for (name, composite) in scope.structs.iter() {
                self.cursor.structs.insert(
                    vec![*name],
                    composite
                        .members
                        .iter()
                        .map(|m| (m.identifier.name, m.type_.clone()))
                        .collect(),
                );
            }

            for (name, _mapping) in scope.mappings.iter() {
                self.cursor
                    .mappings
                    .insert(Location::new(program, vec![*name]), HashMap::new());
            }

            for (name, const_declaration) in scope.consts.iter() {
                self.cursor.frames.push(Frame {
                    step: 0,
                    element: Element::Expression(
                        const_declaration.value.clone(),
                        Some(const_declaration.type_.clone()),
                    ),
                    user_initiated: false,
                });
                self.cursor.over()?;
                let value = self.cursor.values.pop().unwrap();
                self.cursor
                    .globals
                    .insert(Location::new(program, vec![*name]), value);
            }
        }

        Ok(())
    }

    fn load_aleo_program_from_string(&mut self, bytecode: &str) -> Result<()> {
        let aleo_program: Program<TestnetV0> = bytecode.parse()?;
        let program = Symbol::intern(&aleo_program.id().name().to_string());

        for (name, struct_type) in aleo_program.structs().iter() {
            self.cursor.structs.insert(
                vec![Symbol::intern(&name.to_string())],
                struct_type
                    .members()
                    .iter()
                    .map(|(id, type_)| {
                        (
                            leo_ast::Identifier::from(id).name,
                            leo_ast::Type::from_snarkvm(type_, None),
                        )
                    })
                    .collect(),
            );
        }

        for (name, record_type) in aleo_program.records().iter() {
            use snarkvm::prelude::EntryType;
            self.cursor.structs.insert(
                vec![Symbol::intern(&name.to_string())],
                record_type
                    .entries()
                    .iter()
                    .map(|(id, entry)| {
                        let inner_type = match entry {
                            EntryType::Public(t)
                            | EntryType::Private(t)
                            | EntryType::Constant(t) => t,
                        };
                        (
                            leo_ast::Identifier::from(id).name,
                            leo_ast::Type::from_snarkvm(inner_type, None),
                        )
                    })
                    .collect(),
            );
        }

        for (name, _mapping) in aleo_program.mappings().iter() {
            self.cursor.mappings.insert(
                Location::new(program, vec![Symbol::intern(&name.to_string())]),
                HashMap::new(),
            );
        }

        for (name, function) in aleo_program.functions().iter() {
            self.cursor.functions.insert(
                Location::new(program, vec![Symbol::intern(&name.to_string())]),
                FunctionVariant::AleoFunction(function.clone()),
            );
        }

        for (name, closure) in aleo_program.closures().iter() {
            self.cursor.functions.insert(
                Location::new(program, vec![Symbol::intern(&name.to_string())]),
                FunctionVariant::AleoClosure(closure.clone()),
            );
        }

        Ok(())
    }

    fn is_program_loaded(&self, program_name: &str) -> bool {
        let program_symbol = Symbol::intern(program_name);

        self.cursor
            .functions
            .keys()
            .any(|gid| gid.program == program_symbol)
            || self
                .cursor
                .mappings
                .keys()
                .any(|gid| gid.program == program_symbol)
            || self
                .cursor
                .globals
                .keys()
                .any(|gid| gid.program == program_symbol)
            || self
                .cursor
                .records
                .keys()
                .any(|(p, _)| *p == program_symbol)
    }

    fn get_loaded_programs(&self) -> Vec<String> {
        let mut programs = std::collections::HashSet::new();
        for global_id in self.cursor.functions.keys() {
            programs.insert(global_id.program.to_string());
        }
        for global_id in self.cursor.mappings.keys() {
            programs.insert(global_id.program.to_string());
        }
        for global_id in self.cursor.globals.keys() {
            programs.insert(global_id.program.to_string());
        }
        for (program, _) in self.cursor.records.keys() {
            programs.insert(program.to_string());
        }
        programs.into_iter().collect()
    }
}
