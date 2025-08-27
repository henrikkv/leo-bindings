use proc_macro2::TokenStream;
use quote::quote;
use crate::signature::SimplifiedBindings;
use crate::types::get_rust_type;


pub fn generate_code_from_simplified(simplified: &SimplifiedBindings, network_type: Option<syn::Path>) -> TokenStream {
    let program_name = syn::Ident::new(&simplified.program_name, proc_macro2::Span::call_site());
    
    let network_type_token = network_type
        .as_ref()
        .map(|path| quote! { #path })
        .expect("Network type must be specified in generate_bindings! macro");
    
    let path_str = quote!(#network_type_token).to_string();
    let (network_name_token, network_path) = match path_str.as_str() {
        s if s.contains("TestnetV0") => (quote! { NetworkName::TestnetV0 }, "testnet"),
        s if s.contains("MainnetV0") => (quote! { NetworkName::MainnetV0 }, "mainnet"),
        s if s.contains("CanaryV0") => (quote! { NetworkName::CanaryV0 }, "canary"),
        _ => panic!("Unsupported network type: {}. Supported types: TestnetV0, MainnetV0, CanaryV0", path_str),
    };
    
    
    let records = generate_records(&simplified.records);
    let structs = generate_structs(&simplified.structs);
    let function_implementations = generate_function_implementations(&simplified.functions, &simplified.program_name);
    
    let expanded = quote! {
        use anyhow::{anyhow, bail, ensure};
        use snarkvm::prelude::*;
        use indexmap::IndexMap;
        use snarkvm::ledger::query::*;
        use snarkvm::ledger::store::helpers::memory::{ConsensusMemory, BlockMemory};
        use snarkvm::ledger::store::ConsensusStore;
        use snarkvm::ledger::block::{Execution, Output, Transaction, Transition};
        use snarkvm::synthesizer::VM;
        use snarkvm::prelude::ConsensusVersion;
        use snarkvm::ledger::query::{QueryTrait, Query};
        use leo_package::Package;
        use leo_ast::NetworkName;
        use leo_span::create_session_if_not_set_then;
        use aleo_std::StorageMode;
        use serde_json;
        use std::str::FromStr;
        use std::fmt;
        use std::thread::sleep;
        use std::time::Duration;
        use leo_bindings::types::{ToValue, FromValue};
        use leo_bindings::utils::{Account, get_public_balance, broadcast_transaction};
        
        type Nw = #network_type_token;
        
        
        #(#records)*
        
        #(#structs)*
        
        pub struct #program_name {
            pub package: Package,
            pub endpoint: String,
        }
        
        const NETWORK_PATH: &str = #network_path;
        const NETWORK_NAME: NetworkName = #network_name_token;
        
        impl #program_name {
            pub fn new(deployer: &Account<Nw>, endpoint: &str) -> Result<Self, anyhow::Error> {
                use leo_package::Package;
                use leo_span::create_session_if_not_set_then;
                use std::path::Path;
                
                
                let result = create_session_if_not_set_then(|_| {
                let package = Package::from_directory(
                    Path::new("."),
                    Path::new("."),
                    false,
                    false,
                    NETWORK_NAME,
                    endpoint,
                )?;
                
                let main_program = package.programs.iter()
                    .find(|p| !p.is_test && p.is_local)
                    .ok_or_else(|| anyhow!("No main program found in package"))?;
                
                let aleo_name = format!("{}.aleo", main_program.name);
                let aleo_path = if package.manifest.program == aleo_name {
                    package.build_directory().join("main.aleo")
                } else {
                    package.imports_directory().join(aleo_name)
                };
                
                let bytecode = std::fs::read_to_string(aleo_path.clone())
                    .map_err(|e| anyhow!("Failed to read bytecode from {}: {}", aleo_path.display(), e))?;
                
                let program: Program<Nw> = bytecode.parse()
                    .map_err(|e| anyhow!("Failed to parse program: {}", e))?;
                
                let program_id = program.id();
                
                println!("📦 Creating deployment transaction for '{}'...", program_id);
                let rng = &mut rand::thread_rng();
                let vm = VM::from(ConsensusStore::<Nw, ConsensusMemory<Nw>>::open(StorageMode::Production)?)?;
                let query = Query::<Nw, BlockMemory<Nw>>::from(endpoint);
                
                let transaction = vm.deploy(
                    deployer.private_key(),
                    &program,
                    None,
                    0,
                    Some(&query),
                    rng,
                ).map_err(|e| anyhow!("Failed to generate deployment transaction: {}", e))?;
                
                println!("📡 Broadcasting deployment transaction...");
                let transaction_id = transaction.id();
                
                let response = ureq::post(&format!("{}/{}/transaction/broadcast", endpoint, NETWORK_PATH))
                    .send_json(&transaction)?;
                
                let response_string = response.into_string()?.trim_matches('\"').to_string();
                ensure!(
                    response_string == transaction_id.to_string(),
                    "Response ID mismatch: {} != {}", response_string, transaction_id
                );
                
                println!("✅ Deployment transaction {} broadcast successfully!", transaction_id);
                
                    Ok(Self { 
                        package,
                        endpoint: endpoint.to_string(),
                    })
                });
                
                result
            }
            
            #(#function_implementations)*
        }
    };
    
    expanded
}

fn generate_records(records: &[crate::signature::RecordDef]) -> Vec<proc_macro2::TokenStream> {
    records.iter().map(|record| {
        let record_name = syn::Ident::new(&record.name, proc_macro2::Span::call_site());
        
        let member_definitions = record.members.iter().map(|member| {
            let member_name = syn::Ident::new(&member.name, proc_macro2::Span::call_site());
            let member_type = get_rust_type(&member.type_name);
            quote! { pub #member_name: #member_type }
        });
        
        let member_extractions = record.members.iter().map(|member| {
            let member_name = syn::Ident::new(&member.name, proc_macro2::Span::call_site());
            let member_type = get_rust_type(&member.type_name);
            quote! {
                let #member_name = {
                    let member_id = &Identifier::try_from(stringify!(#member_name)).unwrap();
                    let entry = record.data().get(member_id).unwrap();
                    let value = Value::Plaintext(entry.clone());
                    <#member_type>::from_value(value)
                };
            }
        });
        
        let member_names = record.members.iter().map(|member| {
            syn::Ident::new(&member.name, proc_macro2::Span::call_site())
        });
        
        let _member_conversions = record.members.iter().map(|member| {
            let member_name = syn::Ident::new(&member.name, proc_macro2::Span::call_site());
            quote! {
                (
                    Identifier::try_from(stringify!(#member_name)).unwrap(),
                    self.#member_name.to_value().into_plaintext().unwrap()
                )
            }
        });
        
        quote! {
            #[derive(Debug, Clone, Copy, Default)]
            pub struct #record_name {
                #(#member_definitions),*
            }
            
            impl ToValue<Nw> for #record_name {
                fn to_value(&self) -> Value<Nw> {
                    // TODO: Implement proper Record construction with owner, gates, and nonce
                    // For now, this is not implemented as it requires additional metadata
                    panic!("ToValue for records with direct members needs proper Record construction with owner, gates, and nonce")
                }
            }
            
            impl FromValue<Nw> for #record_name {
                fn from_value(value: Value<Nw>) -> Self {
                    match value {
                        Value::Record(record) => {
                            #(#member_extractions)*
                            Self {
                                #(#member_names),*
                            }
                        },
                        _ => panic!("Expected record type"),
                    }
                }
            }
            
            impl #record_name {
                pub fn new(record: Record<Nw, Plaintext<Nw>>) -> Self {
                    Self::from_value(Value::Record(record))
                }
            }
        }
    }).collect()
}

fn generate_structs(structs: &[crate::signature::RecordDef]) -> Vec<proc_macro2::TokenStream> {
    structs.iter().map(|struct_def| {
        let struct_name = syn::Ident::new(&struct_def.name, proc_macro2::Span::call_site());
        
        let member_definitions = struct_def.members.iter().map(|member| {
            let member_name = syn::Ident::new(&member.name, proc_macro2::Span::call_site());
            let member_type = get_rust_type(&member.type_name);
            quote! { pub #member_name: #member_type }
        });
        
        let member_extractions = struct_def.members.iter().map(|member| {
            let member_name = syn::Ident::new(&member.name, proc_macro2::Span::call_site());
            let member_type = get_rust_type(&member.type_name);
            quote! {
                let #member_name = {
                    let member_id = &Identifier::try_from(stringify!(#member_name)).unwrap();
                    let entry = struct_members.get(member_id).unwrap();
                    <#member_type>::from_value(Value::Plaintext(entry.clone()))
                };
            }
        });
        
        let member_names: Vec<_> = struct_def.members.iter().map(|member| {
            syn::Ident::new(&member.name, proc_macro2::Span::call_site())
        }).collect();
        
        let member_definitions_for_constructor = struct_def.members.iter().map(|member| {
            let member_name = syn::Ident::new(&member.name, proc_macro2::Span::call_site());
            let member_type = get_rust_type(&member.type_name);
            quote! { #member_name: #member_type }
        });
        
        let member_conversions = struct_def.members.iter().map(|member| {
            let member_name = syn::Ident::new(&member.name, proc_macro2::Span::call_site());
            quote! {
                (
                    Identifier::try_from(stringify!(#member_name)).unwrap(),
                    match self.#member_name.to_value() {
                        Value::Plaintext(p) => p,
                        _ => panic!("Expected plaintext value"),
                    }
                )
            }
        });
        
        quote! {
            #[derive(Debug, Clone, Copy, Default)]
            pub struct #struct_name {
                #(#member_definitions),*
            }
            
            impl ToValue<Nw> for #struct_name {
                fn to_value(&self) -> Value<Nw> {
                    let members = IndexMap::from([
                        #(#member_conversions),*
                    ]);
                    Value::Plaintext(Plaintext::Struct(members, std::sync::OnceLock::new()))
                }
            }
            
            impl FromValue<Nw> for #struct_name {
                fn from_value(value: Value<Nw>) -> Self {
                    match value {
                        Value::Plaintext(Plaintext::Struct(struct_members, _)) => {
                            #(#member_extractions)*
                            Self {
                                #(#member_names),*
                            }
                        },
                        _ => panic!("Expected struct type"),
                    }
                }
            }
            
            impl #struct_name {
                pub fn new(#(#member_definitions_for_constructor),*) -> Self {
                    Self {
                        #(#member_names),*
                    }
                }
            }
        }
    }).collect()
}
fn generate_function_implementations(functions: &[crate::signature::FunctionBinding], program_name: &str) -> Vec<proc_macro2::TokenStream> {
    functions.iter().map(|function| {
        let function_name = syn::Ident::new(&function.name, proc_macro2::Span::call_site());
        
        let input_params = function.inputs.iter().map(|input| {
            let param_name = syn::Ident::new(&input.name, proc_macro2::Span::call_site());
            let param_type = get_rust_type(&input.type_name);
            quote! { #param_name: #param_type }
        });
        
        let output_types = function.outputs.iter().map(|output| {
            get_rust_type(&output.type_name)
        });
        
        let input_conversions = function.inputs.iter().map(|input| {
            let param_name = syn::Ident::new(&input.name, proc_macro2::Span::call_site());
            quote! { (#param_name).to_value() }
        });
        
        let output_conversions = function.outputs.iter().enumerate().map(|(i, output)| {
            let output_type = get_rust_type(&output.type_name);
            quote! { 
                <#output_type>::from_value(
                    outputs.get(#i).ok_or_else(|| anyhow!("Missing output at index {}", #i))?.clone()
                ) 
            }
        });
        
        let return_type = if function.outputs.len() == 1 {
            let single_type = get_rust_type(&function.outputs[0].type_name);
            quote! { Result<#single_type, anyhow::Error> }
        } else {
            quote! { Result<(#(#output_types),*), anyhow::Error> }
        };
        
        let return_value = if function.outputs.len() == 1 {
            let conversion = output_conversions.clone().next().unwrap();
            quote! { Ok(#conversion) }
        } else {
            quote! { Ok((#(#output_conversions),*)) }
        };
        
        quote! {
            pub fn #function_name(&self, account: &Account<Nw>, #(#input_params),*) -> #return_type {
                let program_id = ProgramID::try_from(format!("{}.aleo", #program_name).as_str()).unwrap();
                let function_id = Identifier::from_str(&stringify!(#function_name).to_string()).unwrap();
                let args: Vec<Value<Nw>> = vec![
                    #(#input_conversions),*
                ];
                let rng = &mut rand::thread_rng();
                println!("Transaction of function {}:", stringify!(#function_name));
                
                let locator = Locator::<Nw>::new(program_id, function_id);
                
                let transaction: Transaction<Nw> = {
                    let store = ConsensusStore::<Nw, ConsensusMemory<Nw>>::open(StorageMode::Production)?;
                    let vm = VM::from(store)?;
                    
                    let program: Program<Nw> = {
                        let response = ureq::get(&format!("{}/{}/program/{}", self.endpoint, NETWORK_PATH, program_id))
                            .call()
                            .map_err(|e| anyhow!("Failed to fetch program: {}", e))?;
                        let json_response: serde_json::Value = response.into_json()?;
                        json_response.as_str()
                            .ok_or_else(|| anyhow!("Expected program string in JSON response"))?
                            .to_string()
                            .parse()?
                    };
                    vm.process().write().add_program(&program)?;
                    vm.execute(
                        account.private_key(),
                        (program_id, function_id),
                        args.iter(),
                        None,
                        0,
                        Some(&Query::<Nw, BlockMemory<Nw>>::from(self.endpoint.as_str()) as &dyn QueryTrait<Nw>),
                        rng,)?
                };
                
                let public_balance = get_public_balance(&account.address(), &self.endpoint, NETWORK_PATH)?;
                let storage_cost = transaction
                    .execution()
                    .ok_or_else(|| anyhow!("The transaction does not contain an execution"))?
                    .size_in_bytes()?;
                
                if public_balance < storage_cost {
                    bail!(
                        "❌ The public balance of {} is insufficient to pay the base fee for `{}`",
                        public_balance,
                        locator.to_string()
                    );
                }
                
                println!("✅ Created execution transaction for '{}'", locator.to_string());
                println!("Response from transaction broadcast: {}", broadcast_transaction(transaction.clone(), &self.endpoint, NETWORK_PATH)?);  
                
                let execution = match transaction {
                    Transaction::Execute(_, _, execution, _) => execution,
                    _ => panic!("Not an execution."),
                };

                let outputs: Vec<Value<Nw>> = execution.transitions()
                    .find(|transition| {
                        transition.function_name().to_string() == stringify!(#function_name)
                    })
                    .expect("Could not find transition for the target function")
                    .outputs()
                    .iter()
                    .map(|output| {
                        match output {
                          Output::Constant(_, plaintext) | Output::Public(_, plaintext) => {
                              plaintext.as_ref().map(|pt| Value::Plaintext(pt.clone())).unwrap_or_else(|| {
                                  panic!("Expected plaintext output but found None")
                              })
                          },
                          Output::Private(_, _ciphertext) => {
                              panic!("Private outputs are not yet supported in generated bindings")
                          },
                          Output::Record(_, _, record_ciphertext, _) => {
                              record_ciphertext.as_ref().and_then(|rc| {
                                  rc.decrypt(account.view_key()).ok().map(|record| Value::Record(record))
                              }).unwrap_or_else(|| {
                                  panic!("Expected record output but found None or failed to decrypt")
                              })
                          },
                          Output::Future(_, future) => {
                              future.as_ref().map(|f| Value::Future(f.clone())).unwrap_or_else(|| {
                                  panic!("Expected future output but found None")
                              })
                          },
                          Output::ExternalRecord(external_record) => {
                              Value::Plaintext(Plaintext::from(Literal::Field(*external_record)))
                          },
                          _ => {
                              println!("Debug: Unexpected output type: {:?}", output);
                              panic!("Unexpected output type")
                          },
                        }
                    }).collect();

                #return_value
            }
        }
    }).collect()
}

