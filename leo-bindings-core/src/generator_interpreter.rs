use crate::generate_records;
use crate::generate_structs;
use crate::signature::SimplifiedBindings;
use crate::types::get_rust_type;
use convert_case::{Case, Casing};
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
    // Add dependencies to the interpreter
    let dep_additions: Vec<proc_macro2::TokenStream> = imports
        .iter()
        .map(|import_name| {
            let dep_program_id = format!("{}.aleo", import_name);
            quote! {
                let dep_program_id = ProgramID::<N>::from_str(#dep_program_id)?;
                wait_for_program_availability(&dep_program_id.to_string(), endpoint, N::SHORT_NAME, 60).map_err(|e| anyhow!(e.to_string()))?;
                let dep_program: Program<N> = {
                    let mut response = ureq::get(&format!("{}/{}/program/{}", endpoint, N::SHORT_NAME, dep_program_id)).call().unwrap();
                    let json_text = response.body_mut().read_to_string().unwrap();
                    let json_response: serde_json::Value = serde_json::from_str(&json_text).unwrap();
                    json_response.as_str().unwrap().to_string().parse().unwrap()
                };
                vm.process().write().add_program(&dep_program)?;
            }
        }).collect();
    /*
        let function_implementations = generate_interpreter_function_implementations(
            &simplified.functions,
            &simplified.program_name,
            &dep_additions,
        );

        let mapping_implementations = generate_interpreter_mapping_implementations(
            &simplified.mappings,
            &simplified.program_name,
        );
    */
    let network_ident = syn::Ident::new(network_type, proc_macro2::Span::call_site());

    let expanded = quote! {
            use leo_bindings::{anyhow, snarkvm, indexmap, serde_json, leo_package, leo_ast, leo_span, aleo_std, http, ureq, rand};

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
            use leo_span::create_session_if_not_set_then;
            use aleo_std::StorageMode;
            use std::str::FromStr;
            use std::fmt;
            use std::thread::sleep;
            use std::time::Duration;
            use leo_bindings::{ToValue, FromValue};
            use leo_bindings::utils::{Account, get_public_balance, broadcast_transaction, wait_for_transaction_confirmation, wait_for_program_availability};

            #(#records)*

            #(#structs)*

            pub struct #program_name {
                pub package: Package,
                pub endpoint: String,
            }

            impl #program_name {
                pub fn new(deployer: &Account<Nw>, endpoint: &str) -> Result<Self, anyhow::Error> {
                    use leo_package::{Package, Manifest};
                    use leo_span::create_session_if_not_set_then;
                    use std::path::Path;

                    let result = create_session_if_not_set_then(|_| {
                    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

                    let package = match Package::from_directory(
                        crate_dir,
                        crate_dir,
                        false,
                        false,
                        Some(NetworkName::from_str(Nw::SHORT_NAME).unwrap()),
                        Some(endpoint),
                    ) {
                        Ok(pkg) => pkg,
                        Err(_) => {
                            let manifest = Manifest {
                                program: format!("{}.aleo", stringify!(#program_name)),
                                version: "0.1.0".to_string(),
                                description: "External binding".to_string(),
                                license: "MIT".to_string(),
                                leo: Default::default(),
                                dependencies: None,
                                dev_dependencies: None,
                            };
                            Package {
                                base_directory: crate_dir.canonicalize()?,
                                programs: Vec::new(),
                                manifest,
                            }
                        }
                    };

                    #(#deployment_calls)*

                    let program_id = ProgramID::<Nw>::from_str(&format!("{}.aleo", stringify!(#program_name)))?;
                    let program_exists = {
                        let check_response = ureq::get(&format!("{}/{}/program/{}", endpoint, Nw::SHORT_NAME, program_id))
                            .call();
                        match check_response {
                            Ok(_) => {
                                println!("✅ Found '{}', skipping deployment", program_id);
                                true
                            },
                            Err(_) => {
                                println!("📦 Deploying '{}'", program_id);
                                false
                            }
                        }
                    };

                    if !program_exists {
                        let target_program_name_symbol = leo_span::Symbol::intern(stringify!(#program_name));
                        let target_program = package.programs.iter()
                            .find(|p| p.name == target_program_name_symbol)
                            .ok_or_else(|| anyhow!("Program '{}' not found in package", stringify!(#program_name)))?;

                        let aleo_name = format!("{}.aleo", target_program.name);
                        let aleo_path = if package.manifest.program == aleo_name {
                            package.build_directory().join("main.aleo")
                        } else {
                            package.imports_directory().join(aleo_name)
                        };

                        let bytecode = std::fs::read_to_string(aleo_path.clone())
                            .map_err(|e| anyhow!("Failed to read bytecode from {}: {}", aleo_path.display(), e))?;

                        let program: Program<Nw> = bytecode.parse()
                            .map_err(|e| anyhow!("Failed to parse program: {}", e))?;

                        println!("📦 Creating deployment tx for '{}'...", program_id);
                        let rng = &mut rand::thread_rng();
                        let vm = VM::from(ConsensusStore::<Nw, ConsensusMemory<Nw>>::open(StorageMode::Production)?)?;
                        let query = Query::<Nw, BlockMemory<Nw>>::from(endpoint.parse::<http::uri::Uri>()?);
                        let process = vm.process();

                         #(#dep_additions)*

                        let transaction = vm.deploy(
                            deployer.private_key(),
                            &program,
                            None,
                            0,
                            Some(&query),
                            rng,
                        ).map_err(|e| anyhow!("Failed to generate deployment transaction: {}", e))?;

                        println!("📡 Broadcasting deployment tx: {} to {}",transaction.id(), endpoint);

                        let response = ureq::post(&format!("{}/{}/transaction/broadcast", endpoint, Nw::SHORT_NAME))
                            .send_json(&transaction)?;

                        wait_for_transaction_confirmation::<Nw>(&transaction.id(), endpoint, Nw::SHORT_NAME, 60)?;
                        wait_for_program_availability(&program_id.to_string(), endpoint, Nw::SHORT_NAME, 60).map_err(|e| anyhow!(e.to_string()))?;
                    }

                    Ok(Self {
                        package,
                        endpoint: endpoint.to_string(),
                    })
                    });

                    result
                }
    /*
                #(#function_implementations)*

                #(#mapping_implementations)*
    */
            }
        };

    expanded
}
