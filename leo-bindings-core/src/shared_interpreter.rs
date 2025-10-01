use leo_ast::interpreter_value::GlobalId;
use leo_ast::{Ast, NetworkName};
use leo_errors::{CompilerError, Handler, Result};
use leo_interpreter::{Element, Frame, FunctionVariant, Interpreter};
use leo_parser;
use leo_span::source_map::FileName;
use leo_span::{with_session_globals, SessionGlobals, Symbol, SESSION_GLOBALS};
use snarkvm::prelude::{Program, TestnetV0};
use std::{cell::RefCell, collections::HashMap, fs, path::PathBuf, rc::Rc};

pub struct SharedInterpreterState {
    pub interpreter: RefCell<Interpreter>,
    pub session: SessionGlobals,
}

thread_local! {
    static SHARED_INTERPRETER: RefCell<Option<Rc<SharedInterpreterState>>> = RefCell::new(None);
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

/// Extension trait that adds dynamic loading capabilities to the Leo interpreter
pub trait InterpreterExtensions {
    /// Dynamically load Leo programs into an existing interpreter instance
    fn load_leo_programs(&mut self, leo_source_files: &[(PathBuf, Vec<PathBuf>)]) -> Result<()>;

    /// Dynamically load Aleo programs into an existing interpreter instance
    fn load_aleo_programs<P: AsRef<std::path::Path>>(
        &mut self,
        aleo_source_files: impl IntoIterator<Item = P>,
    ) -> Result<()>;

    /// Check if a program is already loaded in the interpreter
    fn is_program_loaded(&self, program_name: &str) -> bool;

    /// Get a list of all loaded program names
    fn get_loaded_programs(&self) -> Vec<String>;
}

impl InterpreterExtensions for Interpreter {
    fn load_leo_programs(&mut self, leo_source_files: &[(PathBuf, Vec<PathBuf>)]) -> Result<()> {
        for (path, modules) in leo_source_files {
            let ast = get_ast(
                path,
                modules,
                &self.handler,
                &self.node_builder,
                NetworkName::TestnetV0,
            )?;

            for (&program, scope) in ast.ast.program_scopes.iter() {
                self.filename_to_program
                    .insert(path.to_path_buf(), program.to_string());

                for (name, function) in scope.functions.iter() {
                    self.cursor.functions.insert(
                        GlobalId {
                            program,
                            path: vec![*name],
                        },
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
                    self.cursor.mappings.insert(
                        GlobalId {
                            program,
                            path: vec![*name],
                        },
                        HashMap::new(),
                    );
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
                    self.cursor.globals.insert(
                        GlobalId {
                            program,
                            path: vec![*name],
                        },
                        value,
                    );
                }
            }

            for (mod_path, module) in ast.ast.modules.iter() {
                let program = module.program_name;
                let to_absolute_path = |name: Symbol| {
                    let mut full_name = mod_path.clone();
                    full_name.push(name);
                    full_name
                };

                for (name, function) in module.functions.iter() {
                    self.cursor.functions.insert(
                        GlobalId {
                            program,
                            path: to_absolute_path(*name),
                        },
                        FunctionVariant::Leo(function.clone()),
                    );
                }

                for (name, composite) in module.structs.iter() {
                    self.cursor.structs.insert(
                        to_absolute_path(*name),
                        composite
                            .members
                            .iter()
                            .map(|m| (m.identifier.name, m.type_.clone()))
                            .collect(),
                    );
                }

                for (name, const_declaration) in module.consts.iter() {
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
                    self.cursor.globals.insert(
                        GlobalId {
                            program,
                            path: to_absolute_path(*name),
                        },
                        value,
                    );
                }
            }
        }
        Ok(())
    }

    fn load_aleo_programs<P: AsRef<std::path::Path>>(
        &mut self,
        aleo_source_files: impl IntoIterator<Item = P>,
    ) -> Result<()> {
        for path in aleo_source_files {
            let path = path.as_ref();
            let text =
                fs::read_to_string(path).map_err(|e| CompilerError::file_read_error(path, e))?;
            let aleo_program: Program<TestnetV0> = text.parse()?;
            let program = Symbol::intern(&aleo_program.id().name().to_string());
            self.filename_to_program
                .insert(path.to_path_buf(), program.to_string());

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
                    GlobalId {
                        program,
                        path: vec![Symbol::intern(&name.to_string())],
                    },
                    HashMap::new(),
                );
            }

            for (name, function) in aleo_program.functions().iter() {
                self.cursor.functions.insert(
                    GlobalId {
                        program,
                        path: vec![Symbol::intern(&name.to_string())],
                    },
                    FunctionVariant::AleoFunction(function.clone()),
                );
            }

            for (name, closure) in aleo_program.closures().iter() {
                self.cursor.functions.insert(
                    GlobalId {
                        program,
                        path: vec![Symbol::intern(&name.to_string())],
                    },
                    FunctionVariant::AleoClosure(closure.clone()),
                );
            }
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

fn get_ast(
    path: &std::path::PathBuf,
    modules: &[std::path::PathBuf],
    handler: &Handler,
    node_builder: &leo_ast::NodeBuilder,
    network: NetworkName,
) -> Result<Ast> {
    let text = fs::read_to_string(path).map_err(|e| CompilerError::file_read_error(path, e))?;
    let source_file = with_session_globals(|s| {
        s.source_map
            .new_source(&text, FileName::Real(path.to_path_buf()))
    });

    let modules = modules
        .iter()
        .map(|filename| {
            let source = fs::read_to_string(filename).unwrap();
            with_session_globals(|s| {
                s.source_map
                    .new_source(&source, FileName::Real(filename.to_path_buf()))
            })
        })
        .collect::<Vec<_>>();

    leo_parser::parse_ast(
        handler.clone(),
        node_builder,
        &source_file,
        &modules,
        network,
    )
}
