use crate::generate_structs;
use crate::interpreter_cheats::generate_interpreter_cheats_from_simplified;
use crate::signature::SimplifiedBindings;
use proc_macro2::TokenStream;
use quote::quote;

pub fn generate_interpreter_code_from_simplified(
    simplified: &SimplifiedBindings,
    dependency_modules: Vec<(String, String)>,
) -> TokenStream {
    let program_name = syn::Ident::new(&simplified.program_name, proc_macro2::Span::call_site());

    let records = generate_structs(&simplified.records);
    let structs = generate_structs(&simplified.structs);
    let imports = &simplified.imports;

    // Recursive deployment of dependencies
    let deployment_calls: Vec<proc_macro2::TokenStream> = imports
        .iter()
        .map(|import_name| {
            let dep_struct_name = syn::Ident::new(import_name, proc_macro2::Span::call_site());
            dependency_modules
                .iter().find(|(name, _)| name == import_name)
                .map_or_else(
                    || quote! { #dep_struct_name::new(deployer, endpoint).unwrap(); },
                    |(_, module_name)| {
                        let dep_module_name = syn::Ident::new(module_name, proc_macro2::Span::call_site());
                        quote! { super::#dep_module_name::#dep_struct_name::new(deployer, endpoint).unwrap(); }
                    }
                )
        }).collect();
    let function_implementations = generate_interpreter_function_implementations(
        &simplified.functions,
        &simplified.program_name,
    );

    let mapping_implementations = generate_interpreter_mapping_implementations(
        &simplified.mappings,
        &simplified.program_name,
    );

    let cheats_module = generate_interpreter_cheats_from_simplified(simplified);

    let dev_account_funding = if simplified.program_name == "credits" {
        let cheats_module = syn::Ident::new(
            &format!("{}_interpreter_cheats", simplified.program_name),
            proc_macro2::Span::call_site(),
        );
        quote! {
            const balance: u64 = 1_500_000_000_000_000 / 8;
            #cheats_module::set_account(Address::from_str("aleo1rhgdu77hgyqd3xjj8ucu3jj9r2krwz6mnzyd80gncr5fxcwlh5rsvzp9px")?, balance)?;
            #cheats_module::set_account(Address::from_str("aleo1s3ws5tra87fjycnjrwsjcrnw2qxr8jfqqdugnf0xzqqw29q9m5pqem2u4t")?, balance)?;
            #cheats_module::set_account(Address::from_str("aleo1ashyu96tjwe63u0gtnnv8z5lhapdu4l5pjsl2kha7fv7hvz2eqxs5dz0rg")?, balance)?;
            #cheats_module::set_account(Address::from_str("aleo12ux3gdauck0v60westgcpqj7v8rrcr3v346e4jtq04q7kkt22czsh808v2")?, balance)?;
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        use leo_bindings::{anyhow, snarkvm, indexmap, leo_package, leo_ast, leo_span, leo_interpreter, leo_errors};

        use anyhow::{anyhow, bail, ensure};
        use snarkvm::prelude::*;
        use snarkvm::prelude::TestnetV0 as Nw;
        use indexmap::IndexMap;
        use leo_package::Package;
        use leo_ast::NetworkName;
        use leo_span::{create_session_if_not_set_then, SESSION_GLOBALS, SessionGlobals, Symbol};
        use leo_interpreter::{Frame, Element, StepResult};
        use leo_errors::Handler;
        use std::str::FromStr;
        use std::path::{Path, PathBuf};
        use leo_bindings::{ToValue, FromValue};
        use leo_bindings::utils::{Account};
        use leo_bindings::{initialize_shared_interpreter, with_shared_interpreter, InterpreterExtensions, walkdir};

        #(#records)*

        #(#structs)*

        fn snarkvm_value_to_leo_value(value: &Value<Nw>) -> Result<leo_ast::interpreter_value::Value, anyhow::Error> {
            let leo_svm_value: leo_ast::interpreter_value::SvmValue = value.clone();
            Ok(leo_svm_value.into())
        }


        fn leo_value_to_snarkvm_values(leo_value: leo_ast::interpreter_value::Value) -> Result<Vec<Value<Nw>>, anyhow::Error> {
            use leo_ast::interpreter_value::{SvmValue, ValueVariants};

            match leo_value.contents {
                ValueVariants::Svm(svm_value) => {
                    Ok(vec![svm_value])
                },
                ValueVariants::Tuple(tuple_values) => {
                    let mut svm_values = Vec::new();
                    for tuple_element in tuple_values {
                        let element_values = leo_value_to_snarkvm_values(tuple_element)?;
                        svm_values.extend(element_values);
                    }
                    Ok(svm_values)
                },
                ValueVariants::Unit => {
                    Ok(vec![])
                },
                ValueVariants::Unsuffixed(_) => {
                    Err(anyhow!("Cannot convert Unsuffixed literals"))
                },
                ValueVariants::Future(futures) => {
                    match &futures[0] {
                        leo_ast::interpreter_value::AsyncExecution::AsyncFunctionCall { function, arguments } => {
                            let svm_args: Vec<Argument<Nw>> = arguments.iter()
                                .flat_map(|arg| leo_value_to_snarkvm_values(arg.clone()).unwrap())
                                .filter_map(|svm_val| match svm_val {
                                    Value::Plaintext(pt) => Some(Argument::Plaintext(pt)),
                                    Value::Future(f) => Some(Argument::Future(f)),
                                    _ => None
                                })
                                .collect();

                            let program_id = ProgramID::try_from(format!("{}.aleo", function.program))?;
                            let function_name = Identifier::try_from(
                                function.path.last().copied().unwrap_or(Symbol::intern("unknown")).to_string()
                            )?;

                            Ok(vec![Value::Future(Future::new(program_id, function_name, svm_args))])
                        },
                        leo_ast::interpreter_value::AsyncExecution::AsyncBlock { .. } => {
                            Err(anyhow!("AsyncBlock futures not supported"))
                        }
                    }
                }
            }
        }


        pub struct #program_name {
            pub endpoint: String,
        }

        impl #program_name {
            pub fn new(deployer: &Account<Nw>, endpoint: &str) -> Result<Self, anyhow::Error> {
                use leo_package::{Package, Manifest};
                use std::path::Path;
                use leo_interpreter::Interpreter;
                use leo_ast::interpreter_value::Value;
                use leo_span;

                let program_name = stringify!(#program_name);
                let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

                create_session_if_not_set_then(|_| {
                    let interpreter_exists = with_shared_interpreter(|_| true).is_some();
                    if !interpreter_exists {
                        let signer: Value = deployer.address().into();
                        let block_height = 0u32;
                        let network = NetworkName::from_str("testnet").unwrap();

                        let interpreter = Interpreter::new(
                            &[] as &[(PathBuf, Vec<PathBuf>)],
                            &[] as &[PathBuf],
                            signer,
                            block_height,
                            network,
                        ).unwrap();

                        let session = SessionGlobals::default();
                        initialize_shared_interpreter(interpreter, session);
                    }

                    #(#deployment_calls)*

                });

                let program_exists = with_shared_interpreter(|state| {
                    state.interpreter.borrow().is_program_loaded(program_name)
                }).unwrap_or(false);

                if !program_exists {
                    with_shared_interpreter(|state| {
                        let package = Package::from_directory(
                            crate_dir,
                            crate_dir,
                            false,
                            false,
                            Some(NetworkName::from_str("testnet").unwrap()),
                            Some(endpoint),
                        ).unwrap();

                        let target_program_name_symbol = leo_span::Symbol::intern(program_name);
                        let target_program = package.programs.iter()
                            .find(|p| p.name == target_program_name_symbol)
                            .unwrap();

                        let mut interpreter = state.interpreter.borrow_mut();

                        match &target_program.data {
                            leo_package::ProgramData::Bytecode(bytecode) => {
                                interpreter.load_aleo_program_from_string(bytecode).unwrap();
                            },
                            leo_package::ProgramData::SourcePath { directory: _, source } => {
                                interpreter.load_leo_program(source).unwrap();
                            }
                        }
                    }).unwrap();
                }

                with_shared_interpreter(|state| {
                    let mut interpreter = state.interpreter.borrow_mut();
                    interpreter.cursor.set_program(program_name);
                });

                #dev_account_funding

                Ok(Self {
                    endpoint: endpoint.to_string(),
                })
            }

            #(#function_implementations)*

            #(#mapping_implementations)*
        }
        #cheats_module
    };

    expanded
}

fn generate_interpreter_mapping_implementations(
    mappings: &[crate::signature::MappingBinding],
    _program_name: &str,
) -> Vec<proc_macro2::TokenStream> {
    mappings.iter().map(|mapping| {
        let getter_name = syn::Ident::new(&format!("get_{}", mapping.name), proc_macro2::Span::call_site());
        let key_type = crate::types::get_rust_type(&mapping.key_type);
        let value_type = crate::types::get_rust_type(&mapping.value_type);
        let mapping_name_str = &mapping.name;

        quote! {
            pub fn #getter_name(&self, key: #key_type) -> Option<#value_type> {
                with_shared_interpreter(|state| {
                    let interpreter = state.interpreter.borrow();
                    let program_symbol = interpreter.cursor.current_program()
                        .expect("No current program set in interpreter");
                    let mapping_name_symbol = Symbol::intern(#mapping_name_str);

                    let key_leo_value = snarkvm_value_to_leo_value(&key.to_value()).ok()?;

                    if let Some(mapping) = interpreter.cursor.lookup_mapping(Some(program_symbol), mapping_name_symbol) {
                        if let Some(value_leo) = mapping.get(&key_leo_value) {
                            let snarkvm_values = leo_value_to_snarkvm_values(value_leo.clone()).ok()?;
                            if let Some(snarkvm_value) = snarkvm_values.get(0) {
                                return Some(<#value_type>::from_value(snarkvm_value.clone()));
                            }
                        }
                    }
                    None
                }).flatten()
            }
        }
    }).collect()
}

fn generate_interpreter_function_implementations(
    functions: &[crate::signature::FunctionBinding],
    program_name: &str,
) -> Vec<proc_macro2::TokenStream> {
    functions.iter().map(|function| {
        let function_name = syn::Ident::new(&function.name, proc_macro2::Span::call_site());

        let input_params: Vec<TokenStream> = function.inputs.iter().map(|input| {
            let param_name = syn::Ident::new(&input.name, proc_macro2::Span::call_site());
            let param_type = crate::types::get_rust_type(&input.type_name);
            quote! { #param_name: #param_type }
        }).collect();

        let (function_return_type, function_return_conversions) = match function.outputs.len() {
            0 => (
                quote! { Result<(), anyhow::Error> },
                quote! { Ok(()) }
            ),
            1 => {
                let output_type = crate::types::get_rust_type(&function.outputs[0].type_name);
                let conversion = quote! {
                    match leo_result_value.get(0) {
                        Some(snarkvm_value) => <#output_type>::from_value(snarkvm_value.clone()),
                        None => return Err(anyhow!("Missing output at index 0")),
                    }
                };
                (
                    quote! { Result<#output_type, anyhow::Error> },
                    quote! { Ok(#conversion) }
                )
            },
            _ => {
                let output_types: Vec<_> = function.outputs.iter()
                    .map(|output| crate::types::get_rust_type(&output.type_name))
                    .collect();
                let output_conversions: Vec<_> = function.outputs.iter()
                    .enumerate()
                    .map(|(i, output)| {
                        let output_type = crate::types::get_rust_type(&output.type_name);
                        quote! {
                            match leo_result_value.get(#i) {
                                Some(snarkvm_value) => <#output_type>::from_value(snarkvm_value.clone()),
                                None => return Err(anyhow!("Missing output at index {}", #i)),
                            }
                        }
                    })
                    .collect();
                (
                    quote! { Result<(#(#output_types),*), anyhow::Error> },
                    quote! { Ok((#(#output_conversions),*)) }
                )
            }
        };

        let input_conversions: Vec<_> = function.inputs.iter().map(|input| {
            let param_name = syn::Ident::new(&input.name, proc_macro2::Span::call_site());
            quote! { snarkvm_value_to_leo_value(&<_ as ToValue<Nw>>::to_value(&#param_name))? }
        }).collect();

        let param_count = function.inputs.len();

        quote! {
            pub fn #function_name(&self, account: &Account<Nw>, #(#input_params),*) -> #function_return_type {
                let leo_result_value = with_shared_interpreter(|state| {
                    let mut interpreter = state.interpreter.borrow_mut();

                    let function_args: Vec<leo_ast::interpreter_value::Value> = vec![#(#input_conversions),*];

                    interpreter.cursor.set_program(#program_name);

                    let program_symbol = interpreter.cursor.current_program()
                        .ok_or_else(|| anyhow!("No current program set in interpreter"))?;
                    let function_name_symbol = Symbol::intern(stringify!(#function_name));

                    interpreter.cursor.values.extend(function_args);

                    let function_identifier = leo_ast::Identifier::new(function_name_symbol, interpreter.node_builder.next_id());
                    let function_path = leo_ast::Path::new(
                        vec![],
                        function_identifier,
                        None,
                        leo_span::Span::default(),
                        interpreter.node_builder.next_id(),
                    );

                    let call_expression = leo_ast::CallExpression {
                        function: function_path,
                        arguments: vec![leo_ast::Expression::Unit(leo_ast::UnitExpression {
                            span: leo_span::Span::default(),
                            id: interpreter.node_builder.next_id()
                        }); #param_count],
                        const_arguments: vec![],
                        program: Some(program_symbol),
                        span: leo_span::Span::default(),
                        id: interpreter.node_builder.next_id(),
                    };

                    interpreter.cursor.frames.push(leo_interpreter::Frame {
                        step: 1,
                        element: leo_interpreter::Element::Expression(
                            leo_ast::Expression::Call(Box::new(call_expression)),
                            None
                        ),
                        user_initiated: true,
                    });

                    let interpreter_result = interpreter.cursor.over()
                        .map_err(|e| anyhow!("Failed to execute function '{}': {}", stringify!(#function_name), e))?;

                    let function_outputs: Vec<snarkvm::prelude::Value<Nw>> = match interpreter_result.value {
                        Some(leo_value) => {
                            match leo_value_to_snarkvm_values(leo_value) {
                                Ok(svm_values) => svm_values,
                                Err(e) => return Err(anyhow!("Failed to convert Leo return value to SnarkVM type: {}", e)),
                            }
                        },
                        None => vec![],
                    };

                    if let Some(leo_ast::interpreter_value::AsyncExecution::AsyncFunctionCall { function, arguments }) = interpreter.cursor.futures.pop() {
                        interpreter.cursor.values.extend(arguments);
                        interpreter.cursor.frames.push(leo_interpreter::Frame {
                            step: 0,
                            element: leo_interpreter::Element::DelayedCall(function),
                            user_initiated: true,
                        });
                        interpreter.cursor.over()?;
                    }

                        Ok(function_outputs)
                }).ok_or_else(|| anyhow!("Shared interpreter not available"))??;

                #function_return_conversions
            }
        }
    }).collect()
}
