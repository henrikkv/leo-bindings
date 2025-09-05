use crate::signature::SimplifiedBindings;
use crate::types::get_rust_type;
use proc_macro2::TokenStream;
use quote::quote;
use convert_case::{Case, Casing};

fn get_network_info(
    network_type: &syn::Path,
) -> (
    &'static str,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
) {
    let path_str = quote!(#network_type).to_string();
    match path_str.as_str() {
        s if s.contains("TestnetV0") => (
            "testnet",
            quote! { NetworkName::TestnetV0 },
            quote! { snarkvm::circuit::network::AleoTestnetV0 },
        ),
        s if s.contains("MainnetV0") => (
            "mainnet",
            quote! { NetworkName::MainnetV0 },
            quote! { snarkvm::circuit::network::AleoV0 },
        ),
        s if s.contains("CanaryV0") => (
            "canary",
            quote! { NetworkName::CanaryV0 },
            quote! { snarkvm::circuit::network::AleoCanaryV0 },
        ),
        _ => panic!(
            "Unsupported network type: {}. Supported types: TestnetV0, MainnetV0, CanaryV0",
            path_str
        ),
    }
}

pub fn generate_program_module(
    simplified: &SimplifiedBindings,
    network_type: syn::Path,
) -> TokenStream {
    let module_name = syn::Ident::new(
        &format!("{}_aleo", simplified.program_name.to_lowercase()),
        proc_macro2::Span::call_site(),
    );

    let dependency_modules: Vec<(String, String)> = simplified
        .imports
        .iter()
        .map(|import_name| {
            let module_name = format!("{}_aleo", import_name.to_lowercase());
            (import_name.clone(), module_name)
        })
        .collect();

    let program_code =
        generate_code_from_simplified(simplified, network_type, Some(dependency_modules));

    quote! {
        pub mod #module_name {
            #program_code
        }
    }
}

pub fn generate_code_from_simplified(
    simplified: &SimplifiedBindings,
    network_type: syn::Path,
    dependency_modules: Option<Vec<(String, String)>>,
) -> TokenStream {
    let program_name = syn::Ident::new(&simplified.program_name, proc_macro2::Span::call_site());

    let (network_path, network_name, aleo_type) = get_network_info(&network_type);

    let records = generate_records(&simplified.records);
    let structs = generate_structs(&simplified.structs);
    let imports = &simplified.imports;

    // Recursive deployment of dependencies
    let deployment_calls: Vec<proc_macro2::TokenStream> = imports
        .iter()
        .map(|import_name| {
            let dep_struct_name = syn::Ident::new(import_name, proc_macro2::Span::call_site());
            dependency_modules
                .as_ref()
                .and_then(|deps| deps.iter().find(|(name, _)| name == import_name))
                .map_or_else(
                    || quote! { #dep_struct_name::new(deployer, endpoint)?; },
                    |(_, module_name)| {
                        let dep_module_name = syn::Ident::new(module_name, proc_macro2::Span::call_site());
                        quote! { super::#dep_module_name::#dep_struct_name::new(deployer, endpoint)?; }
                    }
                )
        }).collect();
    // Add dependencies to SnarkVM
    let dep_additions: Vec<proc_macro2::TokenStream> = imports
        .iter()
        .map(|import_name| {
            let dep_program_id = format!("{}.aleo", import_name);
            quote! {
                let dep_program_id = ProgramID::<Nw>::from_str(#dep_program_id).unwrap();
                wait_for_program_availability(&dep_program_id.to_string(), endpoint, 60).map_err(|e| anyhow!(e.to_string()))?;
                let dep_program: Program<Nw> = {
                    let response = ureq::get(&format!("{}/{}/program/{}", endpoint, NETWORK_PATH, dep_program_id)).call().map_err(|e| anyhow!("Failed to fetch dependency program: {}", e))?;
                    let json_response: serde_json::Value = response.into_json()?;
                    json_response.as_str().ok_or_else(|| anyhow!("Expected program string in JSON response"))?.to_string().parse()?
                };
                process.write().add_program(&dep_program)?;
            }
        }).collect();

    let function_implementations = generate_function_implementations(
        &simplified.functions,
        &simplified.program_name,
        &aleo_type,
        &dep_additions,
    );

    let mapping_implementations = generate_mapping_implementations(
        &simplified.mappings,
        &simplified.program_name,
    );

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
        use snarkvm::circuit;
        use leo_package::Package;
        use leo_ast::NetworkName;
        use leo_span::create_session_if_not_set_then;
        use aleo_std::StorageMode;
        use serde_json;
        use std::str::FromStr;
        use std::fmt;
        use std::thread::sleep;
        use std::time::Duration;
        use leo_bindings::{ToValue, FromValue};
        use leo_bindings::utils::{Account, get_public_balance, broadcast_transaction, wait_for_transaction_confirmation, wait_for_program_availability};

        type Nw = #network_type;

        #(#records)*

        #(#structs)*

        pub struct #program_name {
            pub package: Package,
            pub endpoint: String,
        }

        const NETWORK_PATH: &str = #network_path;
        const NETWORK_NAME: NetworkName = #network_name;

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

                #(#deployment_calls)*

                let program_id = ProgramID::<Nw>::from_str(&format!("{}.aleo", stringify!(#program_name))).unwrap();
                let program_exists = {
                    let check_response = ureq::get(&format!("{}/{}/program/{}", endpoint, NETWORK_PATH, program_id))
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
                    let query = Query::<Nw, BlockMemory<Nw>>::from(endpoint);
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

                    let response = ureq::post(&format!("{}/{}/transaction/broadcast", endpoint, NETWORK_PATH))
                        .send_json(&transaction)?;

                    wait_for_transaction_confirmation::<Nw>(&transaction.id(), endpoint, NETWORK_PATH, 60)?;
                    wait_for_program_availability(&program_id.to_string(), endpoint, 60).map_err(|e| anyhow!(e.to_string()))?;
                }

                    Ok(Self {
                        package,
                        endpoint: endpoint.to_string(),
                    })
                });

                result
            }

            #(#function_implementations)*

            #(#mapping_implementations)*
        }
    };

    expanded
}

fn generate_records(records: &[crate::signature::RecordDef]) -> Vec<proc_macro2::TokenStream> {
    records.iter().map(|record| {
        let record_name = syn::Ident::new(&record.name.to_case(Case::Pascal), proc_macro2::Span::call_site());

        let member_definitions = record.members.iter().map(|member| {
            let member_name = syn::Ident::new(&member.name, proc_macro2::Span::call_site());
            if member.name == "owner" {
                quote! { #member_name: Owner<Nw, Plaintext<Nw>> }
            } else {
                let member_type = get_rust_type(&member.type_name);
                quote! { #member_name: #member_type }
            }
        });
        let extra_record_fields = quote! { __nonce: Group<Nw>, __version: U8<Nw> };


        let member_conversions = record.members.iter().filter(|member| member.name != "owner").map(|member| {
            let member_name = syn::Ident::new(&member.name, proc_macro2::Span::call_site());
            let mode = &member.mode;

            let entry_creation = match mode.to_lowercase().as_str() {
                "public" => quote! { Entry::Public(plaintext_value) },
                "private" => quote! { Entry::Private(plaintext_value) },
                _ => panic!("Unsupported mode '{}' for field '{}'. Only 'Private' and 'Public' modes are supported.", mode, member.name),
            };

            quote! {
                (
                    Identifier::try_from(stringify!(#member_name)).unwrap(),
                    {
                        let plaintext_value = match self.#member_name.to_value() {
                            Value::Plaintext(p) => p,
                            _ => panic!("Expected plaintext value from record member"),
                        };
                        #entry_creation
                    }
                )
            }
        });

        let member_extractions = record.members.iter().map(|member| {
            let member_name = syn::Ident::new(&member.name, proc_macro2::Span::call_site());
            let member_type = get_rust_type(&member.type_name);
            let field_name = &member.name;
            let mode = &member.mode;

            let entry_extraction = match mode.to_lowercase().as_str() {
                "public" => quote! {
                    let Entry::Public(plaintext) = entry else {
                        panic!("Expected Public entry for field '{}', but found different entry type", #field_name);
                    };
                },
                "private" => quote! {
                    let Entry::Private(plaintext) = entry else {
                        panic!("Expected Private entry for field '{}', but found different entry type", #field_name);
                    };
                },
                _ => panic!("Unsupported mode '{}' for field '{}'. Only 'Private' and 'Public' modes are supported.", mode, field_name),
            };

            if field_name == "owner" {
                quote! {
                    let #member_name = {
                        record.owner().clone()
                    };
                }
            } else {
                quote! {
                    let #member_name = {
                        let member_id = &Identifier::try_from(#field_name).unwrap();
                        let entry = record.data().get(member_id)
                            .expect(&format!("Field '{}' not found in record data", #field_name));
                        #entry_extraction
                        let value = Value::Plaintext(plaintext.clone());
                        <#member_type>::from_value(value)
                    };
                }
            }
        });

        let member_names: Vec<_> = record.members.iter().map(|member| {
            syn::Ident::new(&member.name, proc_macro2::Span::call_site())
        }).collect();
        let extra_member_inits = quote! { __nonce: record.nonce().clone(), __version: record.version().clone() };

        let getter_methods = record.members.iter().map(|member| {
            let member_name = syn::Ident::new(&member.name, proc_macro2::Span::call_site());
            let member_type = get_rust_type(&member.type_name);
            quote! {
                pub fn #member_name(&self) -> &#member_type {
                    &self.#member_name
                }
            }
        });

        quote! {
            #[derive(Debug, Clone)]
            pub struct #record_name {
                #(#member_definitions),*,
                #extra_record_fields
            }

            impl ToValue<Nw> for #record_name {
                fn to_value(&self) -> Value<Nw> {
                    match self.to_record() {
                        Ok(rec) => Value::Record(rec),
                        Err(e) => panic!("Failed to convert to Record: {}", e),
                    }
                }
            }

            impl FromValue<Nw> for #record_name {
                fn from_value(value: Value<Nw>) -> Self {
                    match value {
                        Value::Record(record) => {

                            #(#member_extractions)*

                            Self {
                                #(#member_names),*,
                                #extra_member_inits
                            }
                        },
                        _ => panic!("Expected record type"),
                    }
                }
            }

            impl #record_name {
                pub fn from_record(record: Record<Nw, Plaintext<Nw>>) -> Self {
                    Self::from_value(Value::Record(record))
                }

                pub fn from_encrypted_record(
                    record: Record<Nw, Ciphertext<Nw>>,
                    view_key: &ViewKey<Nw>
                ) -> Result<Self, anyhow::Error> {
                    match record.decrypt(view_key) {
                        Ok(decrypted_record) => Ok(Self::from_record(decrypted_record)),
                        Err(e) => Err(anyhow::anyhow!("Failed to decrypt record: {}", e))
                    }
                }

                pub fn to_record(&self) -> Result<Record<Nw, Plaintext<Nw>>, anyhow::Error> {
                    let data = IndexMap::from([
                        #(#member_conversions),*
                    ]);
                    let owner = self.owner.clone();
                    let nonce = self.__nonce.clone();
                    let version = self.__version.clone();

                    Record::<Nw, Plaintext<Nw>>::from_plaintext(
                        owner,
                        data,
                        nonce,
                        version
                    ).map_err(|e| anyhow::anyhow!("Failed to create record: {}", e))
                }

                #(#getter_methods)*
            }
        }
    }).collect()
}

fn generate_structs(structs: &[crate::signature::RecordDef]) -> Vec<proc_macro2::TokenStream> {
    structs
        .iter()
        .map(|struct_def| {
            let struct_name = syn::Ident::new(&struct_def.name.to_case(Case::Pascal), proc_macro2::Span::call_site());

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

            let member_names: Vec<_> = struct_def
                .members
                .iter()
                .map(|member| syn::Ident::new(&member.name, proc_macro2::Span::call_site()))
                .collect();

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
                #[derive(Debug, Clone, Copy)]
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
        })
        .collect()
}
fn generate_function_implementations(
    functions: &[crate::signature::FunctionBinding],
    program_name: &str,
    aleo_type: &proc_macro2::TokenStream,
    dep_additions: &[proc_macro2::TokenStream],
) -> Vec<proc_macro2::TokenStream> {
    functions.iter().map(|function| {
        let function_name = syn::Ident::new(&function.name, proc_macro2::Span::call_site());
        let input_params: Vec<TokenStream> = function.inputs.iter().map(|input| {
            let param_name = syn::Ident::new(&input.name, proc_macro2::Span::call_site());
            let param_type = crate::types::get_rust_type(&input.type_name);
            quote! { #param_name: #param_type }
        }).collect();

        let input_conversions: Vec<TokenStream> = function.inputs.iter().map(|input| {
                let param_name = syn::Ident::new(&input.name, proc_macro2::Span::call_site());
                quote! { (#param_name).to_value() }
            })
            .collect();

        let transaction_execution_body = quote! {
            let program_id = ProgramID::try_from(format!("{}.aleo", #program_name).as_str()).unwrap();
            let function_id = Identifier::from_str(&stringify!(#function_name).to_string()).unwrap();
            let function_args: Vec<Value<Nw>> = vec![
                #(#input_conversions),*
            ];
            let rng = &mut rand::thread_rng();

            let locator = Locator::<Nw>::new(program_id, function_id);

            let (execution_response, transaction): (Response<Nw>, Transaction<Nw>) = {
                let store = ConsensusStore::<Nw, ConsensusMemory<Nw>>::open(StorageMode::Production)?;
                let vm = VM::from(store)?;

                wait_for_program_availability(&program_id.to_string(), &self.endpoint, 60).map_err(|e| anyhow!(e.to_string()))?;
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

                let process = vm.process();

                #(#dep_additions)*

                process.write().add_program(&program)?;

                let authorization = process.read().authorize::<#aleo_type, _>(
                    account.private_key(),
                    program_id,
                    function_id,
                    function_args.iter(),
                    rng,
                )?;

                let (execution_response, _trace) = process.read().execute::<#aleo_type, _>(authorization.clone(), rng)?;
                let transaction = vm.execute(
                    account.private_key(),
                    (program_id, function_id),
                    function_args.iter(),
                    None,
                    0,
                    Some(&Query::<Nw, BlockMemory<Nw>>::from(self.endpoint.as_str()) as &dyn QueryTrait<Nw>),
                    rng,
                )?;
                (execution_response, transaction)
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

            match broadcast_transaction(transaction.clone(), &self.endpoint, NETWORK_PATH) {
                Ok(_) => {
                    wait_for_transaction_confirmation::<Nw>(&transaction.id(), &self.endpoint, NETWORK_PATH, 30)?;
                },
                Err(e) => {
                    eprintln!("❌ Failed to broadcast transaction for '{}': {}", locator.to_string(), e);
                    return Err(e);
                }
            }

            let function_outputs: Vec<Value<Nw>> = {
                let response_outputs = execution_response.outputs();
                let execution = match transaction {
                    Transaction::Execute(_, _, execution, _) => execution,
                    _ => panic!("Not an execution."),
                };

                let target_transition = execution.transitions()
                    .find(|transition| {
                        transition.function_name().to_string() == stringify!(#function_name)
                    })
                    .expect("Could not find transition for the target function");

                response_outputs.iter().enumerate().map(|(index, response_output)| {
                    let transition_output = target_transition.outputs().get(index)
                        .expect("Output index mismatch between response and transition");

                    match (response_output, transition_output) {
                        (Value::Record(_), snarkvm::ledger::block::Output::Record(_, _, Some(network_record), _)) => {
                            match network_record.decrypt(account.view_key()) {
                                Ok(plaintext_record) => Value::Record(plaintext_record),
                                Err(e) => {
                                    eprintln!("Failed to decrypt network record: {}", e);
                                    response_output.clone()
                                }
                            }
                        },
                        _ => response_output.clone()
                    }
                }).collect()
            };
        };

        let (function_return_type, function_return_conversions) = match function.outputs.len() {
            0 => (
                quote! { Result<(), anyhow::Error> },
                quote! { Ok(()) }
            ),
            1 => {
                let output_type = crate::types::get_rust_type(&function.outputs[0].type_name);
                let conversion = quote! {
                    <#output_type>::from_value(function_outputs.get(0).ok_or_else(|| anyhow!("Missing output at index 0"))?.clone())
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
                            <#output_type>::from_value(function_outputs.get(#i).ok_or_else(|| anyhow!("Missing output at index {}", #i))?.clone())
                        }
                    })
                    .collect();
                (
                    quote! { Result<(#(#output_types),*), anyhow::Error> },
                    quote! { Ok((#(#output_conversions),*)) }
                )
            }
        };

        let param_names_string = function.inputs.iter()
            .map(|input| input.name.clone())
            .collect::<Vec<_>>()
            .join(", ");

        quote! {
            pub fn #function_name(&self, account: &Account<Nw>, #(#input_params),*) -> #function_return_type {
                println!("Creating tx: {}.{}({})",
                    #program_name,
                    stringify!(#function_name),
                    #param_names_string);

                #transaction_execution_body
                #function_return_conversions
            }
        }
    }).collect()
}

fn generate_mapping_implementations(
    mappings: &[crate::signature::MappingBinding],
    program_name: &str,
) -> Vec<proc_macro2::TokenStream> {
    mappings.iter().map(|mapping| {
        let mapping_name = &mapping.name;
        let getter_name = syn::Ident::new(&format!("get_{}", mapping.name), proc_macro2::Span::call_site());
        let key_type = crate::types::get_rust_type(&mapping.key_type);
        let value_type = crate::types::get_rust_type(&mapping.value_type);

        quote! {
            pub fn #getter_name(&self, key: #key_type) -> Result<Option<#value_type>, anyhow::Error> {
                let program_id = format!("{}.aleo", #program_name);
                let mapping_name = #mapping_name;
                
                let key_value: Value<Nw> = key.to_value();
                let url = format!("{}/{}/program/{}/mapping/{}/{}", 
                    self.endpoint, NETWORK_PATH, program_id, mapping_name, 
                    key_value.to_string().replace("\"", ""));
                
                let response = ureq::get(&url).call();
                
                match response {
                    Ok(response) => {
                        let value: Option<Value<Nw>> = response.into_json()?;
                        match value {
                            Some(val) => Ok(Some(<#value_type>::from_value(val))),
                            None => Ok(None),
                        }
                    },
                    Err(ureq::Error::Status(404, _)) => Ok(None),
                    Err(e) => Err(anyhow!("Failed to fetch mapping value: {}", e)),
                }
            }
                
        }
    }).collect()
}
