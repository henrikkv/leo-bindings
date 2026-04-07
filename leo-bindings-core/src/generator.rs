use crate::types::{
    get_rust_type, get_rust_type_from_function_input, get_rust_type_from_function_output,
};
use convert_case::{Case::Pascal, Casing};
use itertools::Itertools;
use leo_abi_types::{Mode, Program, Record};
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::quote;

pub fn generate_program_module(abi: &Program, imports: &[String]) -> TokenStream {
    let program_id = abi.program.trim_end_matches(".aleo");
    let program_id_pascal = program_id.to_case(Pascal);

    let program_module = Ident::new(program_id, Span::call_site());
    let program_trait = Ident::new(&format!("{}Aleo", program_id_pascal), Span::call_site());
    let program_struct = Ident::new(&format!("{}Network", program_id_pascal), Span::call_site());

    let network_aliases = generate_network_aliases(&program_id_pascal, &program_struct);

    let records = generate_records(&abi.records);
    let structs = generate_structs(&abi.structs);

    let function_types = generate_function_types(&abi.functions);
    let mapping_types = generate_mapping_types(&abi.mappings);

    let trait_definition = generate_trait(&function_types, &mapping_types, &program_trait);

    let network_impl = generate_network_impl(
        imports,
        &function_types,
        &mapping_types,
        &program_trait,
        &program_struct,
        &Literal::string(abi.program.as_str()),
    );

    let type_imports = generate_type_imports(imports);

    quote! {
        pub mod #program_module {
            use leo_bindings::{anyhow, snarkvm, indexmap};
            use anyhow::{anyhow, Result};
            use snarkvm::prelude::*;
            use snarkvm::prelude::Network;
            use indexmap::IndexMap;
            use leo_bindings::{ToValue, FromValue};
            use leo_bindings::leo_bindings_sdk::{Client, VMManager, Account};

            #type_imports

            #network_aliases

            #(#structs)*

            #(#records)*

            #trait_definition

            /// Main bindings that connect to the Provable API or a local devnet.
            ///
            /// The network bindings can optionally use the Provable delegated proving service.
            pub mod network {
                use super::*;
                #network_impl
            }

            /// Faster bindings for testing Leo code locally.
            ///
            /// The interpreter state resets after the session.
            pub mod interpreter {
            }
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
        /// Program trait with network implementation.
        pub trait #program_trait<N: snarkvm::prelude::Network> {
            #(#function_signatures)*
            #(#mapping_signatures)*
        }
    }
}

fn generate_network_impl(
    imports: &[String],
    function_types: &[FunctionTypes],
    mapping_types: &[MappingTypes],
    program_trait: &Ident,
    program_struct: &Ident,
    program_id: &Literal,
) -> TokenStream {
    let (deployment_calls, trait_imports, dependency_ids): (Vec<TokenStream>, Vec<TokenStream>, Vec<TokenStream>) = imports
        .iter()
        .map(|import| {
            let import_pascal = import.to_case(Pascal);
            let import_module = Ident::new(import, Span::call_site());
            let import_struct = Ident::new(&format!("{}Network", import_pascal), Span::call_site());
            let import_trait = Ident::new(&format!("{}Aleo", import_pascal), Span::call_site());
            let import_crate_name = Ident::new(&format!("{}_bindings", import), Span::call_site());

            let deployment = quote! {
                let _ = #import_crate_name::#import_module::network::#import_struct::<N>::new(deployer, vm_manager.clone())?;
            };
            let trait_import = quote! { use #import_crate_name::#import_module::#import_trait; };
            let id = Literal::string(&format!("{}.aleo", import));
            let dependency_id = quote! { #id };

            (deployment, trait_import, dependency_id)
        })
        .multiunzip();

    let function_implementations: Vec<TokenStream> = function_types
        .iter()
        .map(|types| generate_function(&dependency_ids, types))
        .collect();

    let mapping_implementations: Vec<TokenStream> =
        mapping_types.iter().map(generate_mapping).collect();

    let new_implementation = generate_new(&deployment_calls, &trait_imports, &dependency_ids);

    quote! {
        use leo_bindings::leo_bindings_sdk::{Client, VMManager};
        use leo_bindings::{leo_package, leo_ast, leo_span, log};
        use snarkvm::console::program::{Record, Plaintext};
        use leo_package::Package;
        use leo_ast::NetworkName;
        use leo_span::create_session_if_not_set_then;
        use std::path::Path;
        use std::str::FromStr;

        #[derive(Debug, Clone)]
        pub struct #program_struct<N: Network> {
            pub vm_manager: VMManager<N>,
            pub package: Package,
            _network: std::marker::PhantomData<N>,
        }

        impl<N: Network> #program_struct<N> {
            const PROGRAM_ID: &str = #program_id;

            #new_implementation
        }

        impl<N: Network> #program_trait<N> for #program_struct<N> {
            #(#function_implementations)*

            #(#mapping_implementations)*
        }
    }
}

pub fn generate_records(records: &[Record]) -> Vec<TokenStream> {
    records
        .iter()
        .map(|record| {
            let (n, _module_path) = record.path.split_last().unwrap();
            let record_name = Ident::new(&n.to_case(Pascal), Span::call_site());

            let member_definitions: Vec<TokenStream> = record
                .fields
                .iter()
                .map(|member| {
                    let member_name = Ident::new(&member.name, Span::call_site());
                    let member_type = get_rust_type(&member.ty);
                    quote! { #member_name: #member_type }
                })
                .collect();

            let extra_record_fields = quote! { __nonce: Group<N>, __version: U8<N> };

            let member_conversions = record.fields.iter().filter(|m| m.name != "owner").map(|member| {
                let member_name = Ident::new(&member.name, Span::call_site());

                let entry_creation = match member.mode {
                    Mode::Public => quote! { Entry::Public(plaintext_value) },
                    Mode::Constant => quote! { Entry::Constant(plaintext_value) },
                    Mode::Private | Mode::None => quote! { Entry::Private(plaintext_value) },
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

            let member_extractions: Vec<TokenStream> = record
                .fields
                .iter()
                .map(|member| {
                    let member_name = Ident::new(&member.name, Span::call_site());
                    let member_type = get_rust_type(&member.ty);
                    let field_name = &member.name;

                    if member.name == "owner" {
                        quote! {
                            let #member_name = match record.owner() {
                                Owner::Public(addr) => *addr,
                                Owner::Private(plaintext) => {
                                    <Address<N> as FromValue<N>>::from_value(Value::Plaintext(plaintext.clone()))
                                }
                            };
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
                                <#member_type>::from_value(Value::Plaintext(plaintext.clone()))
                            };
                        }
                    }
                })
                .collect();

            let record_owner = match record.fields.iter().find(|f| f.name == "owner") {
                Some(f) => match f.mode {
                    Mode::Public => quote! { Owner::Public(self.owner) },
                    Mode::Private | Mode::None | Mode::Constant => quote! {
                        Owner::Private(Plaintext::from(Literal::Address(self.owner)))
                    },
                },
                None => quote! {
                    Owner::Private(Plaintext::from(Literal::Address(self.owner)))
                },
            };

            let member_names: Vec<Ident> = record
                .fields
                .iter()
                .map(|member| Ident::new(&member.name, Span::call_site()))
                .collect();
            let extra_member_inits =
                quote! { __nonce: record.nonce().clone(), __version: record.version().clone() };

            let getter_methods: Vec<TokenStream> = record
                .fields
                .iter()
                .map(|member| {
                    let member_name = Ident::new(&member.name, Span::call_site());
                    let member_type = get_rust_type(&member.ty);
                    quote! {
                        pub fn #member_name(&self) -> &#member_type {
                            &self.#member_name
                        }
                    }
                })
                .collect();

            quote! {
                /// Record from Leo.
                #[derive(Debug, Clone)]
                pub struct #record_name<N: Network> {
                    #(#member_definitions),*,
                    #extra_record_fields
                }

                /// Convert to a SnarkVM Value.
                impl<N: Network> ToValue<N> for #record_name<N> {
                    fn to_value(&self) -> Value<N> {
                        match self.to_record() {
                            Ok(rec) => Value::Record(rec),
                            Err(e) => panic!("Failed to convert to Record: {}", e),
                        }
                    }
                }

                /// Create from a SnarkVM Value
                impl<N: Network> FromValue<N> for #record_name<N> {
                    fn from_value(value: Value<N>) -> Self {
                        match value {
                            Value::Record(record) => {
                                #(#member_extractions)*
                                Self {
                                    #(#member_names),*,
                                    #extra_member_inits
                                }
                            }
                            _ => panic!("Expected record value"),
                        }
                    }
                }

                impl<N: Network> #record_name<N> {
                    /// Convert to a SnarkVM Record.
                    pub fn to_record(&self) -> Result<Record<N, Plaintext<N>>, anyhow::Error> {
                        let data = IndexMap::from([
                            #(#member_conversions),*
                        ]);
                        let owner = #record_owner;
                        let nonce = self.__nonce.clone();
                        let version = self.__version.clone();

                        Ok(Record::<N, Plaintext<N>>::from_plaintext(
                            owner,
                            data,
                            nonce,
                            version
                        )?)
                    }

                    #(#getter_methods)*
                }
            }
        })
        .collect()
}

pub fn generate_structs(structs: &[leo_abi_types::Struct]) -> Vec<TokenStream> {
    structs
        .iter()
        .map(|struct_def| {
            let last = struct_def
                .path
                .last()
                .expect("Struct.path should have at least one segment");
            let struct_name = Ident::new(&last.to_case(Pascal), Span::call_site());

            let (definitions, extractions, names, constructor_definitions, conversions): (Vec<_>, Vec<_>, Vec<_>, Vec<_>, Vec<_>) = struct_def
                .fields
                .iter()
                .map(|field| {
                    let member_name = Ident::new(&field.name, Span::call_site());
                    let member_type = get_rust_type(&field.ty);

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
                /// Struct from Leo.
                #[derive(Debug, Clone, Copy, Default)]
                pub struct #struct_name<N: Network> {
                    #(#definitions)*
                    _network: std::marker::PhantomData<N>
                }

                /// Convert to a SnarkVM Value.
                impl<N: Network> ToValue<N> for #struct_name<N> {
                    fn to_value(&self) -> Value<N> {
                        let members = IndexMap::from([
                            #(#conversions),*
                        ]);
                        Value::Plaintext(Plaintext::Struct(members, std::sync::OnceLock::new()))
                    }
                }

                /// Create from a SnarkVM Value.
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

pub(crate) struct FunctionTypes {
    pub(crate) name: Ident,
    pub(crate) input_params: TokenStream,
    pub(crate) input_conversions: TokenStream,
    pub(crate) return_type: TokenStream,
    pub(crate) return_conversions: TokenStream,
}

pub(crate) struct MappingTypes {
    pub(crate) getter_name: Ident,
    pub(crate) mapping_name: String,
    pub(crate) key_type: TokenStream,
    pub(crate) value_type: TokenStream,
}

fn generate_function_types(functions: &[leo_abi_types::Function]) -> Vec<FunctionTypes> {
    functions.iter().map(|function| {
        let name = Ident::new(&function.name, Span::call_site());

        let (input_params, input_conversions): (Vec<_>, Vec<_>) = function.inputs.iter().map(|input| {
            let param_name = Ident::new(&input.name, Span::call_site());
            let param_type = get_rust_type_from_function_input(&input.ty);
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
                let output_type = get_rust_type_from_function_output(&function.outputs[0].ty);
                let conversion = quote! {
                    match function_outputs.get(0) {
                        Some(snarkvm_value) => <#output_type>::from_value(snarkvm_value.clone()),
                        None => return Err(anyhow!("Missing output")),
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
                        let output_type = get_rust_type_from_function_output(&output.ty);
                        let conversion = quote! {
                            match function_outputs.get(#i) {
                                Some(snarkvm_value) => <#output_type>::from_value(snarkvm_value.clone()),
                                None => return Err(anyhow!("Missing output")),
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

fn generate_mapping_types(mappings: &[leo_abi_types::Mapping]) -> Vec<MappingTypes> {
    mappings
        .iter()
        .map(|mapping| MappingTypes {
            getter_name: Ident::new(&format!("get_{}", mapping.name), Span::call_site()),
            mapping_name: mapping.name.clone(),
            key_type: get_rust_type(&mapping.key),
            value_type: get_rust_type(&mapping.value),
        })
        .collect()
}

fn generate_new(
    deployment_calls: &[TokenStream],
    trait_imports: &[TokenStream],
    dependency_ids: &[TokenStream],
) -> TokenStream {
    quote! {
        pub fn new(deployer: &Account<N>, vm_manager: VMManager<N>) -> Result<Self, anyhow::Error> {
            use leo_bindings::leo_bindings_sdk::block_on;
            #(#trait_imports)*

            #(#deployment_calls)*

            let package = create_session_if_not_set_then(|_| {
                let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
                Package::from_directory(
                    crate_dir,
                    crate_dir,
                    false,
                    false,
                    Some(NetworkName::from_str(N::SHORT_NAME).unwrap()),
                    Some(vm_manager.client().endpoint()),
                )
            })?;

            let program_id = ProgramID::<N>::from_str(Self::PROGRAM_ID)?;
            let program_exists = block_on(vm_manager.client().program_exists::<N>(&program_id.to_string()))?;

            if program_exists {
                log::info!("✅ Found '{}', skipping deployment", program_id);
            } else {
                log::info!("📦 Deploying '{}'", program_id);

                let bytecode = create_session_if_not_set_then(|_| {
                    let program_symbol = leo_span::Symbol::intern(Self::PROGRAM_ID);
                    let target_program = package.compilation_units.iter()
                        .find(|p| p.name == program_symbol)
                        .ok_or_else(|| anyhow!("Program not found in package"))?;

                    match &target_program.data {
                        leo_package::ProgramData::Bytecode(bytecode) => Ok(bytecode.clone()),
                        leo_package::ProgramData::SourcePath { directory, source: _ } => {
                            let aleo_path = directory.join("build").join("main.aleo");
                            std::fs::read_to_string(&aleo_path).map_err(anyhow::Error::from)
                        }
                    }
                })?;

                let program: Program<N> = bytecode.parse()?;

                let dependencies: Vec<&str> = vec![#(#dependency_ids),*];
                block_on(vm_manager.deploy_and_broadcast(deployer, &program, &dependencies))?;
            }

            Ok(Self {
                vm_manager,
                package,
                _network: std::marker::PhantomData,
            })
        }
    }
}

fn generate_function(dependency_ids: &[TokenStream], types: &FunctionTypes) -> TokenStream {
    let FunctionTypes {
        name,
        input_params,
        input_conversions,
        return_type,
        return_conversions,
    } = types;

    quote! {
        fn #name(&self, account: &Account<N>, #input_params) -> #return_type {
            use leo_bindings::leo_bindings_sdk::block_on;
            let program_id_str = Self::PROGRAM_ID;
            let function_name = stringify!(#name);
            let function_args: Vec<Value<N>> = vec![#input_conversions];
            let dependencies: Vec<&str> = vec![#(#dependency_ids),*];

            let function_outputs = block_on(self.vm_manager.execute_and_broadcast(
                account,
                program_id_str,
                function_name,
                function_args,
                &dependencies,
            ))?;

            #return_conversions
        }
    }
}

fn generate_mapping(types: &MappingTypes) -> TokenStream {
    let MappingTypes {
        getter_name,
        mapping_name,
        key_type,
        value_type,
    } = types;

    quote! {
        fn #getter_name(&self, key: #key_type) -> Option<#value_type> {
            use leo_bindings::leo_bindings_sdk::block_on;
            let key_value: Value<N> = key.to_value();

            match block_on(self.vm_manager.client().mapping::<N>(Self::PROGRAM_ID, #mapping_name, &key_value)) {
                Ok(Some(val)) => Some(<#value_type>::from_value(val)),
                Ok(None) => None,
                Err(e) => {
                    log::error!("Failed to fetch mapping value: {}", e);
                    None
                }
            }
        }
    }
}

fn generate_network_aliases(program_id_pascal: &str, program_struct: &Ident) -> TokenStream {
    let testnet_struct = Ident::new(&format!("{}Testnet", program_id_pascal), Span::call_site());
    let mainnet_struct = Ident::new(&format!("{}Mainnet", program_id_pascal), Span::call_site());
    let canary_struct = Ident::new(&format!("{}Canary", program_id_pascal), Span::call_site());

    quote! {
        pub type #testnet_struct = network::#program_struct<snarkvm::prelude::TestnetV0>;

        pub type #mainnet_struct = network::#program_struct<snarkvm::prelude::MainnetV0>;

        pub type #canary_struct = network::#program_struct<snarkvm::prelude::CanaryV0>;
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
