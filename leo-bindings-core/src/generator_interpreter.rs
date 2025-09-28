use crate::generate_structs;
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
                    || quote! { #dep_struct_name::new(deployer, endpoint)?; },
                    |(_, module_name)| {
                        let dep_module_name = syn::Ident::new(module_name, proc_macro2::Span::call_site());
                        quote! { super::#dep_module_name::#dep_struct_name::new(deployer, endpoint)?; }
                    }
                )
        }).collect();
    let function_implementations = generate_interpreter_function_implementations(
        &simplified.functions,
        &simplified.program_name,
    );

    let mapping_implementations: Vec<proc_macro2::TokenStream> = vec![];

    let expanded = quote! {
        use leo_bindings::{anyhow, snarkvm, indexmap, leo_package, leo_ast, leo_span, leo_interpreter, leo_errors};

        use anyhow::{anyhow, bail, ensure};
        use snarkvm::prelude::*;
        use snarkvm::prelude::TestnetV0 as Nw;
        use indexmap::IndexMap;
        use leo_package::Package;
        use leo_ast::NetworkName;
        use leo_span::{SESSION_GLOBALS, SessionGlobals, Symbol};
        use leo_interpreter::{Frame, Element, StepResult};
        use leo_errors::Handler;
        use std::str::FromStr;
        use leo_bindings::{ToValue, FromValue};
        use leo_bindings::utils::{Account, collect_leo_paths, collect_aleo_paths};

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
                            let future_value = leo_ast::interpreter_value::Value::make_future(
                                function.program,
                                function.path.last().copied().unwrap_or(leo_span::Symbol::intern("unknown")),
                                arguments.clone().into_iter()
                            ).ok_or_else(|| anyhow!("Failed to create future value"))?;

                            match future_value.contents {
                                ValueVariants::Svm(svm_value) => Ok(vec![svm_value]),
                                _ => Err(anyhow!("Future did not create SVM value"))
                            }
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
            pub interpreter: std::cell::RefCell<leo_interpreter::Interpreter>,
            session: SessionGlobals,
        }

        impl #program_name {
            pub fn new(deployer: &Account<Nw>, endpoint: &str) -> Result<Self, anyhow::Error> {
                use leo_package::{Package, Manifest};
                use std::path::Path;
                use leo_interpreter::Interpreter;
                use leo_ast::interpreter_value::Value;

                let session = SessionGlobals::default();
                let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

                let interpreter = SESSION_GLOBALS.set(&session, || {
                    #(#deployment_calls)*

                    let package = Package::from_directory(
                        crate_dir,
                        crate_dir,
                        false,
                        false,
                        Some(NetworkName::from_str("testnet").unwrap()),
                        Some(endpoint),
                    ).map_err(|e| anyhow!("Failed to load package: {}", e))?;

                    let leo_files = collect_leo_paths(&package);
                    let aleo_files = collect_aleo_paths(&package);

                    if leo_files.is_empty() {
                        return Err(anyhow!("No Leo source files found in package"));
                    }

                    let signer: Value = deployer.address().into();
                    let block_height = 0u32;
                    let network = NetworkName::from_str("testnet")
                        .map_err(|e| anyhow!("Invalid network name: {}", e))?;

                    let mut interpreter = Interpreter::new(
                        &leo_files,
                        &aleo_files,
                        signer,
                        block_height,
                        network,
                    ).map_err(|e| anyhow!("Failed to create interpreter: {}", e))?;

                    interpreter.cursor.set_program(stringify!(#program_name));

                    Ok(interpreter)
                })?;

                Ok(Self {
                    endpoint: endpoint.to_string(),
                    interpreter: std::cell::RefCell::new(interpreter),
                    session,
                })
            }

            #(#function_implementations)*

            #(#mapping_implementations)*
        }
    };

    expanded
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
                let leo_result_value = SESSION_GLOBALS.set(&self.session, || {
                    let mut interpreter = self.interpreter.borrow_mut();

                    let function_args: Vec<leo_ast::interpreter_value::Value> = vec![#(#input_conversions),*];

                    let program_symbol = interpreter.cursor.current_program()
                        .ok_or_else(|| anyhow!("No current program set in interpreter"))?;
                    let function_name_symbol = leo_span::Symbol::intern(stringify!(#function_name));

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

                    Ok(function_outputs)
                })?;

                #function_return_conversions
            }
        }
    }).collect()
}
