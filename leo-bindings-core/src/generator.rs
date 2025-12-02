use crate::generate_interpreter_impl;
use crate::signature::{FunctionBinding, SimplifiedBindings};
use crate::types::get_rust_type;
use convert_case::{Case::Pascal, Casing};
use itertools::Itertools;
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::quote;

pub fn generate_program_module(simplified: &SimplifiedBindings) -> TokenStream {
    let program_name_pascal = simplified.program_name.to_case(Pascal);

    let program_module = Ident::new(&simplified.program_name, Span::call_site());
    let program_trait = Ident::new(&format!("{}Aleo", program_name_pascal), Span::call_site());
    let program_struct = Ident::new(
        &format!("{}Network", program_name_pascal),
        Span::call_site(),
    );

    let network_aliases = generate_network_aliases(&program_name_pascal, &program_struct);

    let records = generate_records(&simplified.records);
    let structs = generate_structs(&simplified.structs);

    let function_types = generate_function_types(&simplified.functions);
    let mapping_types = generate_mapping_types(&simplified.mappings);

    let trait_definition = generate_trait(&function_types, &mapping_types, &program_trait);

    let network_impl = generate_network_impl(
        simplified,
        &function_types,
        &mapping_types,
        &program_trait,
        &program_struct,
    );

    let interpreter_impl =
        generate_interpreter_impl(simplified, &function_types, &mapping_types, &program_trait);

    let type_imports = generate_type_imports(&simplified.imports);

    quote! {
        pub mod #program_module {
            use leo_bindings::{anyhow, snarkvm, indexmap};
            use anyhow::{anyhow, Result};
            use snarkvm::prelude::*;
            use snarkvm::prelude::Network;
            use indexmap::IndexMap;
            use leo_bindings::{ToValue, FromValue};
            use leo_bindings::utils::Account;

            #type_imports

            #network_aliases

            #(#structs)*

            #(#records)*

            #trait_definition

            pub mod network {
                use super::*;
                #network_impl
            }

            #interpreter_impl
        }
    }
}

fn generate_trait(
    function_types: &[FunctionTypes],
    mapping_types: &[MappingTypes],
    program_trait: &Ident,
) -> TokenStream {
    let function_signatures: Vec<TokenStream> = function_types
        .iter()
        .map(|types| {
            let name = &types.name;
            let input_params = &types.input_params;
            let return_type = &types.return_type;
            quote! { fn #name (&self, account: &Account<N>, #input_params) -> #return_type; }
        })
        .collect();

    let mapping_signatures: Vec<TokenStream> = mapping_types
        .iter()
        .map(|types| {
            let getter_name = &types.getter_name;
            let key_type = &types.key_type;
            let value_type = &types.value_type;
            quote! { fn #getter_name(&self, key: #key_type) -> Option<#value_type>; }
        })
        .collect();

    quote! {
        pub trait #program_trait<N: snarkvm::prelude::Network> {
            fn new(deployer: &Account<N>, endpoint: &str) -> Result<Self, anyhow::Error> where Self: Sized;
            #(#function_signatures)*
            #(#mapping_signatures)*
        }
    }
}

fn generate_network_impl(
    simplified: &SimplifiedBindings,
    function_types: &[FunctionTypes],
    mapping_types: &[MappingTypes],
    program_trait: &Ident,
    program_struct: &Ident,
) -> TokenStream {
    let program_id = Literal::string(&format!("{}.aleo", &simplified.program_name));

    let (deployment_calls, trait_imports, dependency_additions): (Vec<_>, Vec<_>, Vec<_>) = simplified
        .imports
        .iter()
        .map(|import| {
            let import_pascal = import.to_case(Pascal);
            let import_module = Ident::new(import, Span::call_site());
            let import_struct = Ident::new(&format!("{}Network", import_pascal), Span::call_site());
            let import_trait = Ident::new(&format!("{}Aleo", import_pascal), Span::call_site());
            let dependency_id = format!("{}.aleo", import);
            let import_crate_name = Ident::new(&format!("{}_bindings", import), Span::call_site());

            let deployment = quote! { #import_crate_name::#import_module::network::#import_struct::<N>::new(deployer, endpoint)?; };
            let trait_import = quote! { use #import_crate_name::#import_module::#import_trait; };
            let dependency_addition = quote! {
                let dependency_id = ProgramID::<N>::from_str(#dependency_id)?;
                let api_endpoint = format!("{}/v2", endpoint);
                wait_for_program_availability(&dependency_id.to_string(), &api_endpoint, N::SHORT_NAME, 60).map_err(|e| anyhow!(e.to_string()))?;
                let dependency_program: Program<N> = {
                    let mut response = ureq::get(&format!("{}/{}/program/{}", api_endpoint, N::SHORT_NAME, dependency_id)).call().unwrap();
                    let json_text = response.body_mut().read_to_string().unwrap();
                    let json_response: serde_json::Value = serde_json::from_str(&json_text).unwrap();
                    json_response.as_str().unwrap().to_string().parse().unwrap()
                };
                vm.process().write().add_program(&dependency_program)?;
            };
            (deployment, trait_import, dependency_addition)
        })
        .multiunzip();
    let dependency_additions = quote! { #(#dependency_additions)* };

    let function_implementations: Vec<TokenStream> = function_types
        .iter()
        .map(|types| generate_function(&dependency_additions, types, &program_id))
        .collect();

    let mapping_implementations: Vec<TokenStream> = mapping_types
        .iter()
        .map(|types| generate_mapping(types, &program_id))
        .collect();

    let new_implementation = generate_new(
        &deployment_calls,
        &dependency_additions,
        &trait_imports,
        &simplified.program_name,
    );

    quote! {
        use leo_bindings::{serde_json, leo_package, leo_ast, leo_span, aleo_std, http, ureq, rand, print_execution_stats, print_deployment_stats};
        use leo_bindings::utils::*;
        use anyhow::ensure;
        use snarkvm::ledger::query::*;
        use snarkvm::ledger::store::helpers::memory::{ConsensusMemory, BlockMemory};
        use snarkvm::ledger::store::ConsensusStore;
        use snarkvm::ledger::block::Transaction;
        use snarkvm::console::program::{Record, Plaintext};
        use snarkvm::synthesizer::VM;
        use snarkvm::synthesizer::process::execution_cost;
        use snarkvm::prelude::ConsensusVersion;
        use snarkvm::ledger::query::{QueryTrait, Query};
        use leo_package::Package;
        use leo_ast::NetworkName;
        use leo_span::create_session_if_not_set_then;
        use aleo_std::StorageMode;
        use std::str::FromStr;

        #[derive(Debug)]
        pub struct #program_struct<N: Network> {
            pub package: Package,
            pub endpoint: String,
            pub delegated_proving_config: Option<leo_bindings::DelegatedProvingConfig>,
            _network: std::marker::PhantomData<N>,
        }

        impl<N: Network> #program_trait<N> for #program_struct<N> {
            #new_implementation

            #(#function_implementations)*

            #(#mapping_implementations)*
        }

        impl<N: Network> #program_struct<N> {
            pub fn configure_delegation(mut self, config: leo_bindings::DelegatedProvingConfig) -> Self {
                self.delegated_proving_config = Some(config);
                self
            }
            pub fn enable_delegation(mut self) -> Self {
                if let Some(config) = &mut self.delegated_proving_config {
                    config.enabled = true;
                    log::info!("✅ Delegated proving enabled");
                }
                self
            }
            pub fn disable_delegation(mut self) -> Self {
                if let Some(config) = &mut self.delegated_proving_config {
                    config.enabled = false;
                    log::info!("ℹ️ Delegated proving disabled");
                }
                self
            }
        }
    }
}

pub fn generate_records(records: &[crate::signature::StructBinding]) -> Vec<TokenStream> {
    records.iter().map(|record| {
        let record_name = Ident::new(&record.name.to_case(Pascal), Span::call_site());
        let member_definitions = record.members.iter().map(|member| {
            let member_name = Ident::new(&member.name, Span::call_site());
            if member.name == "owner" {
                quote! { #member_name: Owner<N, Plaintext<N>> }
            } else {
                let member_type = get_rust_type(&member.type_name);
                quote! { #member_name: #member_type }
            }
        });
        let extra_record_fields = quote! { __nonce: Group<N>, __version: U8<N> };

        let member_conversions = record.members.iter().filter(|member| member.name != "owner").map(|member| {
            let member_name = Ident::new(&member.name, Span::call_site());
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

        let (member_extractions, struct_member_extractions): (Vec<_>, Vec<_>) = record.members
            .iter()
            .map(|member| {
                let member_name = Ident::new(&member.name, Span::call_site());
                let member_type = get_rust_type(&member.type_name);
                let field_name = &member.name;

                let record_extraction = if field_name == "owner" {
                    quote! {
                        let #member_name = record.owner().clone();
                    }
                } else {
                    quote! {
                        let #member_name = {
                            let member_id = &Identifier::try_from(#field_name).unwrap();
                            let entry = record.data().get(member_id)
                                .expect(&format!("Field '{}' not found in record data", #field_name));
                            let plaintext = match entry {
                                Entry::Public(p) | Entry::Private(p) | Entry::Constant(p) => p,
                            };
                            let value = Value::Plaintext(plaintext.clone());
                            <#member_type>::from_value(value)
                        };
                    }
                };

                // Needed for interpreter compatibility
                let struct_extraction = if field_name == "owner" {
                    quote! {
                        let #member_name = {
                            let member_id = &Identifier::try_from(#field_name).unwrap();
                            let plaintext = struct_members.get(member_id)
                                .expect("Owner field not found in record struct");
                            match plaintext {
                                Plaintext::Literal(Literal::Address(addr), _) => Owner::Public(*addr),
                                _ => panic!("Expected address for owner field"),
                            }
                        };
                    }
                } else {
                    quote! {
                        let #member_name = {
                            let member_id = &Identifier::try_from(#field_name).unwrap();
                            let plaintext = struct_members.get(member_id)
                                .expect(&format!("Field '{}' not found in record data", #field_name));
                            <#member_type>::from_value(Value::Plaintext(plaintext.clone()))
                        };
                    }
                };

                (record_extraction, struct_extraction)
            })
            .unzip();

        let member_names: Vec<_> = record.members.iter().map(|member| {
            Ident::new(&member.name, Span::call_site())
        }).collect();
        let extra_member_inits = quote! { __nonce: record.nonce().clone(), __version: record.version().clone() };

        let getter_methods = record.members.iter().map(|member| {
            let member_name = Ident::new(&member.name, Span::call_site());

            if member.name == "owner" {
                quote! {
                    pub fn #member_name(&self) -> Address<N> {
                        match &self.#member_name {
                            Owner::Public(addr) => *addr,
                            Owner::Private(plaintext) => {
                                match plaintext {
                                    Plaintext::Literal(Literal::Address(addr), _) => *addr,
                                    _ => panic!("Expected address in private owner field"),
                                }
                            }
                        }
                    }
                }
            } else {
                let member_type = get_rust_type(&member.type_name);
                quote! {
                    pub fn #member_name(&self) -> &#member_type {
                        &self.#member_name
                    }
                }
            }
        });

        quote! {
            #[derive(Debug, Clone)]
            pub struct #record_name<N: Network> {
                #(#member_definitions),*,
                #extra_record_fields
            }

            impl<N: Network> ToValue<N> for #record_name<N> {
                fn to_value(&self) -> Value<N> {
                    match self.to_record() {
                        Ok(rec) => Value::Record(rec),
                        Err(e) => panic!("Failed to convert to Record: {}", e),
                    }
                }
            }

            impl<N: Network> FromValue<N> for #record_name<N> {
                fn from_value(value: Value<N>) -> Self {
                    match value {
                        Value::Record(record) => {
                            #(#member_extractions)*
                            Self {
                                #(#member_names),*,
                                #extra_member_inits
                            }
                        },
                        // Interpreter compatibility: records represented as structs
                        Value::Plaintext(Plaintext::Struct(struct_members, _)) => {
                            #(#struct_member_extractions)*

                            Self {
                                #(#member_names),*,
                                __nonce: Group::zero(),
                                __version: U8::new(0)
                            }
                        },
                        _ => panic!("Expected record or struct value"),
                    }
                }
            }

            impl<N: Network> #record_name<N> {
                pub fn to_record(&self) -> Result<Record<N, Plaintext<N>>, anyhow::Error> {
                    let data = IndexMap::from([
                        #(#member_conversions),*
                    ]);
                    let owner = self.owner.clone();
                    let nonce = self.__nonce.clone();
                    let version = self.__version.clone();

                    Record::<N, Plaintext<N>>::from_plaintext(
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

pub fn generate_structs(structs: &[crate::signature::StructBinding]) -> Vec<TokenStream> {
    structs
        .iter()
        .map(|struct_def| {
            let struct_name = Ident::new(&struct_def.name.to_case(Pascal), Span::call_site());
            let (definitions, extractions, names, constructor_definitions, conversions): (Vec<_>, Vec<_>, Vec<_>, Vec<_>, Vec<_>) = struct_def
                .members
                .iter()
                .map(|member| {
                    let member_name = Ident::new(&member.name, Span::call_site());
                    let member_type = get_rust_type(&member.type_name);

                    let definition = quote! { pub #member_name: #member_type, };

                    let extraction = quote! {
                        let #member_name = {
                            let member_id = &Identifier::try_from(stringify!(#member_name)).unwrap();
                            let entry = struct_members.get(member_id).unwrap();
                            <#member_type>::from_value(Value::Plaintext(entry.clone()))
                        };
                    };

                    let name = member_name.clone();

                    let constructor_definition = quote! { #member_name: #member_type, };

                    let conversion = quote! {
                        (
                            Identifier::try_from(stringify!(#member_name)).unwrap(),
                            match self.#member_name.to_value() {
                                Value::Plaintext(p) => p,
                                _ => panic!("Expected plaintext value"),
                            }
                        )
                    };

                    (definition, extraction, name, constructor_definition, conversion)
                })
                .multiunzip();

            quote! {
                #[derive(Debug, Clone, Copy)]
                pub struct #struct_name<N: Network> {
                    #(#definitions)*
                    _network: std::marker::PhantomData<N>
                }

                impl<N: Network> ToValue<N> for #struct_name<N> {
                    fn to_value(&self) -> Value<N> {
                        let members = IndexMap::from([
                            #(#conversions),*
                        ]);
                        Value::Plaintext(Plaintext::Struct(members, std::sync::OnceLock::new()))
                    }
                }

                impl<N: Network> FromValue<N> for #struct_name<N> {
                    fn from_value(value: Value<N>) -> Self {
                        match value {
                            Value::Plaintext(Plaintext::Struct(struct_members, _)) => {
                                #(#extractions)*
                                Self {
                                    #(#names,)*
                                    _network: std::marker::PhantomData
                                }
                            },
                            _ => panic!("Expected struct type"),
                        }
                    }
                }

                impl<N: Network> #struct_name<N> {
                    pub fn new(#(#constructor_definitions)*) -> Self {
                        Self {
                            #(#names,)*
                            _network: std::marker::PhantomData
                        }
                    }
                }
            }
        })
        .collect()
}

pub struct FunctionTypes {
    pub name: Ident,
    pub input_params: TokenStream,
    pub input_conversions: TokenStream,
    pub return_type: TokenStream,
    pub return_conversions: TokenStream,
}

pub struct MappingTypes {
    pub getter_name: Ident,
    pub mapping_name_literal: String,
    pub key_type: TokenStream,
    pub value_type: TokenStream,
}

fn generate_function_types(functions: &[FunctionBinding]) -> Vec<FunctionTypes> {
    functions.iter().map(|function| {
        let name = Ident::new(&function.name, Span::call_site());

        let (input_params, input_conversions): (Vec<_>, Vec<_>) = function.inputs.iter().map(|input| {
            let param_name = Ident::new(&input.name, Span::call_site());
            let param_type = get_rust_type(&input.type_name);
            let param = quote! { #param_name: #param_type };
            let conversion = quote! { (#param_name).to_value() };
            (param, conversion)
        }).unzip();
        let input_params = quote! { #(#input_params),* };
        let input_conversions = quote! { #(#input_conversions),* };

        let (return_type, return_conversions) = match function.outputs.len() {
            0 => (
                quote! { Result<(), anyhow::Error> },
                quote! { Ok(()) }
            ),
            1 => {
                let output_type = get_rust_type(&function.outputs[0].type_name);
                let conversion = quote! {
                    match function_outputs.get(0) {
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
                let (output_types, output_conversions): (Vec<_>, Vec<_>) = function.outputs.iter()
                    .enumerate()
                    .map(|(i, output)| {
                        let output_type = get_rust_type(&output.type_name);
                        let conversion = quote! {
                            match function_outputs.get(#i) {
                                Some(snarkvm_value) => <#output_type>::from_value(snarkvm_value.clone()),
                                None => return Err(anyhow!("Missing output at index {}", #i)),
                            }
                        };
                        (output_type, conversion)
                    })
                    .unzip();
                (
                    quote! { Result<(#(#output_types),*), anyhow::Error> },
                    quote! { Ok((#(#output_conversions),*)) }
                )
            }
        };
        FunctionTypes {
            name,
            input_params,
            input_conversions,
            return_type,
            return_conversions,
        }
    }).collect()
}

fn generate_mapping_types(mappings: &[crate::signature::MappingBinding]) -> Vec<MappingTypes> {
    mappings
        .iter()
        .map(|mapping| {
            let getter_name = Ident::new(&format!("get_{}", mapping.name), Span::call_site());
            let mapping_name_literal = mapping.name.clone();
            let key_type = get_rust_type(&mapping.key_type);
            let value_type = get_rust_type(&mapping.value_type);

            MappingTypes {
                getter_name,
                mapping_name_literal,
                key_type,
                value_type,
            }
        })
        .collect()
}

fn generate_new(
    deployment_calls: &[TokenStream],
    dependency_additions: &TokenStream,
    trait_imports: &[TokenStream],
    program_name: &str,
) -> TokenStream {
    quote! {
        fn new(deployer: &Account<N>, endpoint: &str) -> Result<Self, anyhow::Error> {
            use leo_package::Package;
            use leo_span::create_session_if_not_set_then;
            use std::path::Path;
            #(#trait_imports)*

            let result = create_session_if_not_set_then(|_| {
                let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

                let package = Package::from_directory(
                    crate_dir,
                    crate_dir,
                    false,
                    false,
                    Some(NetworkName::from_str(N::SHORT_NAME).unwrap()),
                    Some(endpoint),
                )?;

                let program_id = ProgramID::<N>::from_str(concat!(#program_name, ".aleo"))?;
                let api_endpoint = format!("{}/v2", endpoint);

                #(#deployment_calls)*

                let program_exists = {
                    let check_response = ureq::get(&format!("{}/{}/program/{}", api_endpoint, N::SHORT_NAME, program_id))
                        .call();
                    match check_response {
                        Ok(_) => {
                            log::info!("✅ Found '{}', skipping deployment", program_id);
                            true
                        },
                        Err(_) => {
                            log::info!("📦 Deploying '{}'", program_id);
                            false
                        }
                    }
                };

                if !program_exists {
                    let program_symbol = leo_span::Symbol::intern(#program_name);
                    let target_program = package.programs.iter()
                        .find(|p| p.name == program_symbol)
                        .ok_or_else(|| anyhow!("Program '{}' not found in package", #program_name))?;

                    let bytecode = match &target_program.data {
                        leo_package::ProgramData::Bytecode(bytecode) => {
                            bytecode.clone()
                        },
                        leo_package::ProgramData::SourcePath { directory, source: _ } => {
                            let aleo_path = directory.join("build").join("main.aleo");
                            std::fs::read_to_string(&aleo_path)
                                .map_err(|e| anyhow!("Failed to read bytecode from {}: {}", aleo_path.display(), e))?
                        }
                    };

                    let program: Program<N> = bytecode.parse()
                        .map_err(|e| anyhow!("Failed to parse program: {}", e))?;

                    log::info!("📦 Creating deployment tx for '{}'...", program_id);
                    let rng = &mut rand::thread_rng();
                    let vm = VM::from(ConsensusStore::<N, ConsensusMemory<N>>::open(StorageMode::Production)?)?;
                    let query = Query::<N, BlockMemory<N>>::from(endpoint.parse::<http::uri::Uri>()?);

                     #dependency_additions

                    let transaction = vm.deploy(
                        deployer.private_key(),
                        &program,
                        None,
                        0,
                        Some(&query),
                        rng,
                    ).map_err(|e| anyhow!("Failed to generate deployment transaction: {}", e))?;

                    match &transaction {
                        Transaction::Deploy(_, _, _, deployment, fee) => {
                            print_deployment_stats(&vm, &program_id.to_string(), deployment, None, ConsensusVersion::V10)?;
                        },
                        _ => panic!("Expected a deployment transaction."),
                    };

                    log::info!("📡 Broadcasting deployment tx: {} to {}",transaction.id(), endpoint);

                    broadcast_transaction(transaction.clone(), &api_endpoint, N::SHORT_NAME)?;

                    wait_for_transaction_confirmation::<N>(&transaction.id(), &api_endpoint, N::SHORT_NAME, 120)?;
                    wait_for_program_availability(&program_id.to_string(), &api_endpoint, N::SHORT_NAME, 60).map_err(|e| anyhow!(e.to_string()))?;
                }

                Ok(Self {
                    package,
                    endpoint: endpoint.to_string(),
                    delegated_proving_config: None,
                    _network: std::marker::PhantomData,
                })
            });
            result
        }
    }
}

fn generate_function(
    dependency_additions: &TokenStream,
    types: &FunctionTypes,
    program_id: &Literal,
) -> TokenStream {
    let FunctionTypes {
        name,
        input_params,
        input_conversions,
        return_type,
        return_conversions,
    } = types;

    quote! {
        fn #name(&self, account: &Account<N>, #input_params) -> #return_type {
            let endpoint = &self.endpoint;
            let api_endpoint = format!("{}/v2", endpoint);
            let program_id = ProgramID::try_from(#program_id).unwrap();
            let function_id = Identifier::try_from(stringify!(#name)).unwrap();
            let function_args: Vec<Value<N>> = vec![#input_conversions];

            let rng = &mut rand::thread_rng();
            let locator = Locator::<N>::new(program_id, function_id);

            log::info!("Creating tx: {}.{}({})", #program_id, stringify!(#name), stringify!(#input_params));
            let vm = VM::from(ConsensusStore::<N, ConsensusMemory<N>>::open(StorageMode::Production)?)?;
            let query = Query::<N, BlockMemory<N>>::from(endpoint.parse::<http::uri::Uri>()?);

            wait_for_program_availability(&program_id.to_string(), &api_endpoint, N::SHORT_NAME, 60).map_err(|e| anyhow!(e.to_string()))?;
            let program: Program<N> = {
                let mut response = ureq::get(&format!("{}/{}/program/{}", api_endpoint, N::SHORT_NAME, program_id))
                    .call().unwrap();
                let json_text = response.body_mut().read_to_string().unwrap();
                let json_response: serde_json::Value = serde_json::from_str(&json_text).unwrap();
                json_response.as_str().unwrap().parse().unwrap()
            };

            #dependency_additions

            vm.process().write().add_programs_with_editions(&vec![(program, 1u16)])
                .map_err(|e| anyhow!("Failed to add program '{}' to VM: {}", program_id, e))?;

            let delegated_result = self.delegated_proving_config.as_ref()
                .filter(|config| config.enabled)
                .and_then(|config| {
                    let authorization = vm
                        .authorize(account.private_key(), program_id, function_id, function_args.iter(), rng)
                        .map_err(|e| {
                            log::warn!("Failed to create authorization: {}", e);
                            e
                        })
                        .ok()?;

                    match execute_with_delegated_proving(
                        config,
                        authorization,
                    ) {
                        Ok(transaction) => {
                            Some((transaction, Vec::new()))
                        }
                        Err(e) => { None }
                    }
                });

            let (transaction, function_outputs): (Transaction<N>, Vec<Value<N>>) = match delegated_result {
                Some(result) => result,
                None => {
                    let (transaction, response) = vm.execute_with_response(
                        account.private_key(),
                        (program_id, function_id),
                        function_args.iter(),
                        None,
                        0,
                        Some(&query as &dyn QueryTrait<N>),
                        rng,
                    ).map_err(|e| anyhow!("Failed to execute function '{}' in program '{}': {}", function_id, program_id, e))?;
                    (transaction, response.outputs().to_vec())
                }
            };

            let public_balance = get_public_balance(&account.address(), &api_endpoint, N::SHORT_NAME);
            let execution = transaction.execution().ok_or_else(|| anyhow!("Missing execution"))?;
            let (total_cost, _) = execution_cost(&vm.process().read(), execution, ConsensusVersion::V10)?;

            match &transaction {
                Transaction::Execute(_, _, execution, fee) => {
                    print_execution_stats(&vm, &program_id.to_string(), &execution, None, ConsensusVersion::V10)?;
                },
                _ => panic!("Expected an execution transaction."),
            };

            ensure!(public_balance >= total_cost,
                "❌ Insufficient balance {} for total cost {} on `{}`", public_balance, total_cost, locator);

            log::info!("📡 Broadcasting tx: {}",transaction.id());
            broadcast_transaction(transaction.clone(), &api_endpoint, N::SHORT_NAME)?;
            wait_for_transaction_confirmation::<N>(&transaction.id(), &api_endpoint, N::SHORT_NAME, 30)?;

            #return_conversions
        }
    }
}

fn generate_mapping(types: &MappingTypes, program_id: &Literal) -> TokenStream {
    let MappingTypes {
        getter_name,
        mapping_name_literal,
        key_type,
        value_type,
    } = types;

    quote! {
        fn #getter_name(&self, key: #key_type) -> Option<#value_type> {
            let program_id = #program_id;
            let mapping_name = #mapping_name_literal;

            let key_value: Value<N> = key.to_value();
            let url = format!("{}/{}/program/{}/mapping/{}/{}",
                self.endpoint, N::SHORT_NAME, program_id, mapping_name,
                key_value.to_string().replace("\"", ""));

            let response = ureq::get(&url).call();

            match response {
                Ok(mut response) => {
                    let json_text = response.body_mut().read_to_string().unwrap();
                    let value: Option<Value<N>> = serde_json::from_str(&json_text).unwrap();
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
}

fn generate_network_aliases(program_name_pascal: &str, program_struct: &Ident) -> TokenStream {
    let testnet_struct = Ident::new(
        &format!("{}Testnet", program_name_pascal),
        Span::call_site(),
    );
    let mainnet_struct = Ident::new(
        &format!("{}Mainnet", program_name_pascal),
        Span::call_site(),
    );
    let canary_struct = Ident::new(&format!("{}Canary", program_name_pascal), Span::call_site());
    let interpreter_struct = Ident::new(
        &format!("{}Interpreter", program_name_pascal),
        Span::call_site(),
    );

    quote! {
        pub type #testnet_struct = network::#program_struct<snarkvm::prelude::TestnetV0>;

        pub type #mainnet_struct = network::#program_struct<snarkvm::prelude::MainnetV0>;

        pub type #canary_struct = network::#program_struct<snarkvm::prelude::CanaryV0>;

        pub type #interpreter_struct = interpreter::#interpreter_struct<snarkvm::prelude::TestnetV0>;
    }
}

fn generate_type_imports(imports: &[String]) -> TokenStream {
    let import_statements: Vec<TokenStream> = imports
        .iter()
        .map(|import| {
            let import_crate_name = Ident::new(&format!("{}_bindings", import), Span::call_site());
            let import_module = Ident::new(import, Span::call_site());
            quote! {
                pub use #import_crate_name::#import_module::*;
            }
        })
        .collect();

    quote! {
        #(#import_statements)*
    }
}
