use crate::generate_records;
use crate::generate_structs;
use crate::signature::SimplifiedBindings;
use proc_macro2::TokenStream;
use quote::quote;

pub fn generate_interpreter_code_from_simplified(
    simplified: &SimplifiedBindings,
    dependency_modules: Vec<(String, String)>,
    network_type: &str,
) -> TokenStream {
    let program_name = syn::Ident::new(&simplified.program_name, proc_macro2::Span::call_site());

    let records = generate_records(&simplified.records);
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
    let network_ident = syn::Ident::new(network_type, proc_macro2::Span::call_site());

    let expanded = quote! {
        use leo_bindings::{anyhow, snarkvm, indexmap, serde_json, leo_package, leo_ast, leo_span, aleo_std, http, ureq, rand, leo_interpreter, leo_parser, leo_errors};

        use anyhow::{anyhow, bail, ensure};
        use snarkvm::prelude::*;
        use snarkvm::prelude::#network_ident as Nw;
        use indexmap::IndexMap;
        use snarkvm::ledger::query::*;
        use snarkvm::ledger::store::helpers::memory::{ConsensusMemory, BlockMemory};
        use snarkvm::ledger::store::ConsensusStore;
        use snarkvm::ledger::block::{Execution, Output, Transaction, Transition};
        use snarkvm::console::program::{Record, Plaintext, Ciphertext};
        use snarkvm::synthesizer::VM;
        use snarkvm::synthesizer::process::execution_cost_v2;
        use snarkvm::prelude::ConsensusVersion;
        use snarkvm::ledger::query::{QueryTrait, Query};
        use snarkvm::circuit;
        use leo_package::Package;
        use leo_ast::NetworkName;
        use leo_span::{SESSION_GLOBALS, SessionGlobals, Symbol, source_map::FileName, with_session_globals};
        use leo_interpreter::{Frame, Element, StepResult};
        use leo_errors::Handler;
        use aleo_std::StorageMode;
        use std::str::FromStr;
        use std::fmt;
        use std::thread::sleep;
        use std::time::Duration;
        use leo_bindings::{ToValue, FromValue};
        use leo_bindings::utils::{Account, get_public_balance, broadcast_transaction, wait_for_transaction_confirmation, wait_for_program_availability, collect_leo_paths, collect_aleo_paths};

        #(#records)*

        #(#structs)*

        fn snarkvm_value_to_leo_value(value: &Value<Nw>) -> Result<leo_ast::interpreter_value::Value, anyhow::Error> {
            let leo_svm_value: leo_ast::interpreter_value::SvmValue = value.clone();
            Ok(leo_svm_value.into())
        }


        fn leo_value_to_snarkvm_value(leo_value: leo_ast::interpreter_value::Value) -> Result<Value<Nw>, anyhow::Error> {
            use leo_ast::interpreter_value::SvmValue;
            let interpreter_svm_value: SvmValue = leo_value.try_into()
                .map_err(|_| anyhow!("Failed to convert Leo value to SnarkVM value"))?;
            Ok(interpreter_svm_value)
        }


        pub struct #program_name {
            pub package: Package,
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

                let (package, interpreter) = SESSION_GLOBALS.set(&session, || {
                    #(#deployment_calls)*

                    let program_id = format!("{}.aleo", stringify!(#program_name));

                    let mut package = Package::from_directory(
                        crate_dir,
                        crate_dir,
                        false,
                        false,
                        Some(NetworkName::from_str(Nw::SHORT_NAME).unwrap()),
                        Some(endpoint),
                    ).map_err(|e| anyhow!("Failed to load package: {}", e))?;

                    let leo_files = collect_leo_paths(&package);
                    let aleo_files = collect_aleo_paths(&package);

                    if leo_files.is_empty() {
                        return Err(anyhow!("No Leo source files found in package"));
                    }

                    let signer: Value = deployer.address().into();
                    let block_height = 0u32;
                    let network = NetworkName::from_str(Nw::SHORT_NAME)
                        .map_err(|e| anyhow!("Invalid network name: {}", e))?;

                    let mut interpreter = Interpreter::new(
                        &leo_files,
                        &aleo_files,
                        signer,
                        block_height,
                        network,
                    ).map_err(|e| anyhow!("Failed to create interpreter: {}", e))?;

                    interpreter.cursor.set_program(stringify!(#program_name));

                    Ok::<_, anyhow::Error>((package, interpreter))
                })?;

                Ok(Self {
                    package,
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
    _program_name: &str,
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

        let input_value_conversions: Vec<_> = function.inputs.iter().map(|input| {
            let param_name = syn::Ident::new(&input.name, proc_macro2::Span::call_site());
            quote! { <_ as ToValue<Nw>>::to_value(&#param_name).to_string() }
        }).collect();

        quote! {
            pub fn #function_name(&self, account: &Account<Nw>, #(#input_params),*) -> #function_return_type {
                let leo_result_value = SESSION_GLOBALS.set(&self.session, || {
                    let mut interpreter = self.interpreter.borrow_mut();

                    let param_values = vec![#(#input_value_conversions),*];
                    let function_call = format!("{}({})", stringify!(#function_name), param_values.join(", "));

                    use leo_span::{source_map::FileName, with_session_globals};
                    use leo_interpreter::{Frame, Element};

                    let filename = FileName::Custom(format!("user_input_{}", stringify!(#function_name)));
                    let source_file = with_session_globals(|globals| globals.source_map.new_source(&function_call, filename));

                    let expression = leo_parser::parse_expression(
                        leo_errors::Handler::default(),
                        &interpreter.node_builder,
                        &function_call,
                        source_file.absolute_start,
                        leo_ast::NetworkName::TestnetV0,
                    ).map_err(|e| anyhow!("Failed to parse function call '{}': {}", function_call, e))?;

                    interpreter.cursor.frames.push(Frame {
                        step: 0,
                        element: Element::Expression(expression, None),
                        user_initiated: true,
                    });

                    let step_result = interpreter.cursor.over()
                        .map_err(|e| anyhow!("Failed to execute function '{}': {}", stringify!(#function_name), e))?;

                    let result = if step_result.finished { step_result.value } else { None };

                    let function_outputs: Vec<snarkvm::prelude::Value<Nw>> = match result {
                        Some(leo_value) => {
                            match leo_value.contents {
                                leo_ast::interpreter_value::ValueVariants::Svm(svm_value) => vec![svm_value],
                                leo_ast::interpreter_value::ValueVariants::Unit => vec![],
                                _ => return Err(anyhow!("Unsupported return value type: {:?}", leo_value.contents)),
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
