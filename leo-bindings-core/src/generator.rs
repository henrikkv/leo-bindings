use crate::signature::SimplifiedBindings;
use crate::types::get_rust_type;
use proc_macro2::TokenStream;
use quote::quote;
use convert_case::{Case, Casing};
use crate::generate_interpreter_code_from_simplified;


pub fn generate_program_module(
    simplified: &SimplifiedBindings,
    network: &str,
) -> TokenStream {
    let network_type = match network {
        "mainnet" => "MainnetV0",
        "testnet" => "TestnetV0", 
        "canary" => "CanaryV0",
        "interpreter" => "TestnetV0",
        _ => panic!("Unsupported network: {}. Must be 'mainnet', 'testnet', 'canary', or 'interpreter'", network),
    };

    let module_name = syn::Ident::new(
        &format!("{}_{}", simplified.program_name.to_lowercase(), network),
        proc_macro2::Span::call_site(),
    );

    let dependency_modules: Vec<(String, String)> = simplified
        .imports
        .iter()
        .map(|import_name| {
            let module_name = format!("{}_{}", import_name.to_lowercase(), network);
            (import_name.clone(), module_name)
        })
        .collect();

    let program_code = match network {
        "interpreter" => generate_interpreter_code_from_simplified(simplified, dependency_modules, network_type),
        _             => generate_code_from_simplified(simplified, dependency_modules, network_type),
    };

    quote! {
        pub mod #module_name {
            #program_code
        }
    }
}

pub fn generate_code_from_simplified(
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
    // Add dependencies to SnarkVM
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

    let function_implementations = generate_function_implementations(
        &simplified.functions,
        &simplified.program_name,
        &dep_additions,
    );

    let mapping_implementations = generate_mapping_implementations(
        &simplified.mappings,
        &simplified.program_name,
    );

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

            #(#function_implementations)*

            #(#mapping_implementations)*
        }
    };

    expanded
}

pub fn generate_records(records: &[crate::signature::StructBinding]) -> Vec<proc_macro2::TokenStream> {
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
                "private" | "none" => quote! { Entry::Private(plaintext_value) },
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
                "private" | "none" => quote! {
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

pub fn generate_structs(structs: &[crate::signature::StructBinding]) -> Vec<proc_macro2::TokenStream> {
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
                let program_id = ProgramID::try_from(format!("{}.aleo", #program_name).as_str()).unwrap();

                let function_id = Identifier::from_str(&stringify!(#function_name).to_string()).unwrap();
                let function_args: Vec<Value<Nw>> = vec![#(#input_conversions),*];

                let rng = &mut rand::thread_rng();
                let locator = Locator::<Nw>::new(program_id, function_id);

                println!("Creating tx: {}.{}({})", #program_name, stringify!(#function_name), #param_names_string);

                let vm = VM::from(ConsensusStore::<Nw, ConsensusMemory<Nw>>::open(StorageMode::Production)?)?;
                let query = Query::<Nw, BlockMemory<Nw>>::from(self.endpoint.parse::<http::uri::Uri>()?);
                
                wait_for_program_availability(&program_id.to_string(), &self.endpoint, Nw::SHORT_NAME, 60).map_err(|e| anyhow!(e.to_string()))?;
                let program: Program<Nw> = {
                    let mut response = ureq::get(&format!("{}/{}/program/{}", self.endpoint, Nw::SHORT_NAME, program_id))
                        .call().unwrap();
                    let json_text = response.body_mut().read_to_string().unwrap();
                    let json_response: serde_json::Value = serde_json::from_str(&json_text).unwrap();
                    json_response.as_str().unwrap().parse().unwrap()
                };

                let endpoint = &self.endpoint;
                #(#dep_additions)*
                vm.process().write().add_programs_with_editions(&vec![(program, 1u16)])
                    .map_err(|e| anyhow!("Failed to add program '{}' to VM: {}", program_id, e))?;

                let (transaction, response) = vm.execute_with_response(
                    account.private_key(),
                    (program_id, function_id),
                    function_args.iter(),
                    None,
                    0,
                    Some(&query as &dyn QueryTrait<Nw>),
                    rng,
                ).map_err(|e| anyhow!("Failed to execute function '{}' in program '{}': {}", function_id, program_id, e))?;

                let public_balance = get_public_balance(&account.address(), &self.endpoint, Nw::SHORT_NAME);
                let execution = transaction.execution().ok_or_else(|| anyhow!("Missing execution"))?;
                let (total_cost, _) = execution_cost_v2(&vm.process().read(), execution)?;
                
                ensure!(public_balance >= total_cost, 
                    "❌ Insufficient balance {} for total cost {} on `{}`", public_balance, total_cost, locator);

                println!("📡 Broadcasting tx: {}",transaction.id());
                broadcast_transaction(transaction.clone(), &self.endpoint, Nw::SHORT_NAME)?;
                wait_for_transaction_confirmation::<Nw>(&transaction.id(), &self.endpoint, Nw::SHORT_NAME, 30)?;

                let function_outputs: Vec<Value<Nw>> = response.outputs().to_vec();

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
            pub fn #getter_name(&self, key: #key_type) -> Option<#value_type> {
                let program_id = format!("{}.aleo", #program_name);
                let mapping_name = #mapping_name;
                
                let key_value: Value<Nw> = key.to_value();
                let url = format!("{}/{}/program/{}/mapping/{}/{}", 
                    self.endpoint, Nw::SHORT_NAME, program_id, mapping_name, 
                    key_value.to_string().replace("\"", ""));
                
                let response = ureq::get(&url).call();
                
                match response {
                    Ok(mut response) => {
                        let json_text = response.body_mut().read_to_string().unwrap();
                        let value: Option<Value<Nw>> = serde_json::from_str(&json_text).unwrap();
                        match value {
                            Some(val) => Some(<#value_type>::from_value(val)),
                            None => None,
                        }
                    },
                    Err(ureq::Error::StatusCode(404)) => None,
                    Err(e) => panic!("Failed to fetch mapping value: {}", e),
                }
            }
                
        }
    }).collect()
}
