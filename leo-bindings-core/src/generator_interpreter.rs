use crate::interpreter_cheats::generate_interpreter_cheats_from_simplified;
use crate::signature::SimplifiedBindings;
use crate::{FunctionTypes, MappingTypes};
use convert_case::{Case::Pascal, Casing};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

pub fn generate_interpreter_impl(
    simplified: &SimplifiedBindings,
    function_types: &[FunctionTypes],
    mapping_types: &[MappingTypes],
    program_trait: &Ident,
) -> TokenStream {
    let program_name = &simplified.program_name;
    let program_name_pascal = simplified.program_name.to_case(Pascal);
    let program_struct = Ident::new(
        &format!("{}Interpreter", program_name_pascal),
        Span::call_site(),
    );

    let (deployment_calls, trait_imports): (Vec<TokenStream>, Vec<TokenStream>) = simplified
        .imports
        .iter()
        .map(|import| {
            let import_pascal = import.to_case(Pascal);
            let import_module = Ident::new(import, Span::call_site());
            let import_struct = Ident::new(&format!("{}Interpreter", import_pascal), Span::call_site());
            let import_trait = Ident::new(&format!("{}Aleo", import_pascal), Span::call_site());
            let import_crate_name = Ident::new(&format!("{}_bindings", import), Span::call_site());

            let deployment = quote! { #import_crate_name::#import_module::interpreter::#import_struct::new(deployer, endpoint).unwrap(); };
            let trait_import = quote! { use #import_crate_name::#import_module::#import_trait; };

            (deployment, trait_import)
        })
        .unzip();

    let function_implementations: Vec<TokenStream> = function_types
        .iter()
        .map(generate_interpreter_function)
        .collect();

    let mapping_implementations: Vec<TokenStream> = mapping_types
        .iter()
        .map(generate_interpreter_mapping)
        .collect();

    let cheats_module = generate_interpreter_cheats_from_simplified(simplified);

    let dev_account_funding = if simplified.program_name == "credits" {
        let cheats_module = Ident::new(
            &format!("{}_interpreter_cheats", program_name),
            Span::call_site(),
        );
        quote! {
            const balance: u64 = 1_500_000_000_000_000 / 8;
            #cheats_module::set_account(Address::from_str("aleo1rhgdu77hgyqd3xjj8ucu3jj9r2krwz6mnzyd80gncr5fxcwlh5rsvzp9px").unwrap(), balance).unwrap();
            #cheats_module::set_account(Address::from_str("aleo1s3ws5tra87fjycnjrwsjcrnw2qxr8jfqqdugnf0xzqqw29q9m5pqem2u4t").unwrap(), balance).unwrap();
            #cheats_module::set_account(Address::from_str("aleo1ashyu96tjwe63u0gtnnv8z5lhapdu4l5pjsl2kha7fv7hvz2eqxs5dz0rg").unwrap(), balance).unwrap();
            #cheats_module::set_account(Address::from_str("aleo12ux3gdauck0v60westgcpqj7v8rrcr3v346e4jtq04q7kkt22czsh808v2").unwrap(), balance).unwrap();
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        pub mod interpreter {
            use leo_bindings::{leo_package, leo_ast, leo_span, leo_interpreter, initialize_shared_interpreter, with_shared_interpreter, InterpreterExtensions};
            use leo_bindings::utils::*;
            use anyhow::anyhow;
            use snarkvm::prelude::TestnetV0;
            use snarkvm::prelude::TestnetV0 as N;
            use leo_package::Package;
            use leo_ast::NetworkName;
            use leo_span::{create_session_if_not_set_then, Symbol, SessionGlobals};
            use std::str::FromStr;
            use std::path::PathBuf;

            pub use super::*;

            pub struct #program_struct<N: Network> {
                pub endpoint: String,
                _network: std::marker::PhantomData<N>,
            }

            impl<N: Network> #program_struct<N> {
                const PROGRAM_NAME: &str = #program_name;
            }

            impl #program_trait<TestnetV0> for #program_struct<TestnetV0> {

            fn new(deployer: &Account<TestnetV0>, endpoint: &str) -> Result<Self, anyhow::Error> {
                use std::path::Path;
                use leo_ast::interpreter_value::Value;
                use leo_interpreter::Interpreter;
                #(#trait_imports)*

                let program_name = #program_name;
                let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

                create_session_if_not_set_then(|_| {
                    let interpreter_exists = with_shared_interpreter(|_| true).is_some();
                    if !interpreter_exists {
                        let block_height = 0u32;
                        let block_timestamp = 0i64;
                        let network = NetworkName::from_str("testnet").unwrap();

                        let interpreter = Interpreter::new(
                            &[] as &[(PathBuf, Vec<PathBuf>)],
                            &[] as &[PathBuf],
                            deployer.private_key().to_string(),
                            block_height,
                            block_timestamp,
                            network,
                        ).unwrap();

                        let session = SessionGlobals::default();
                        initialize_shared_interpreter(interpreter, session);
                        #(#deployment_calls)*
                    }

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
                });

                Ok(Self {
                    endpoint: endpoint.to_string(),
                    _network: std::marker::PhantomData,
                })
            }

            #(#function_implementations)*

            #(#mapping_implementations)*
        }

        impl #program_struct<TestnetV0> {
            pub fn configure_delegation(self, _config: DelegatedProvingConfig) -> Self {
                log::debug!("Not delegating when using interpreter");
                self
            }
            pub fn enable_delegation(self) -> Self {
                log::debug!("Not delegating when using interpreter");
                self
            }
            pub fn disable_delegation(self) -> Self {
                log::debug!("Not delegating when using interpreter");
                self
            }
        }

        #cheats_module
        }
    };

    expanded
}

fn generate_interpreter_mapping(types: &MappingTypes) -> TokenStream {
    let MappingTypes {
        getter_name,
        mapping_name_literal,
        key_type,
        value_type,
    } = types;

    quote! {
        fn #getter_name(&self, key: #key_type) -> Option<#value_type> {
            with_shared_interpreter(|state| {
                let interpreter = state.interpreter.borrow();
                let program_symbol = Symbol::intern(Self::PROGRAM_NAME);
                let mapping_name_symbol = Symbol::intern(#mapping_name_literal);

                let key_leo_value: leo_ast::interpreter_value::Value = leo_ast::interpreter_value::Value::from((key).to_value());

                if let Some(mapping) = interpreter.cursor.lookup_mapping(Some(program_symbol), mapping_name_symbol) {
                    if let Some(value_leo) = mapping.get(&key_leo_value) {
                        let snarkvm_value = value_leo.to_value();
                        return Some(<#value_type>::from_value(snarkvm_value));
                    }
                }
                None
            }).flatten()
        }
    }
}

fn generate_interpreter_function(types: &FunctionTypes) -> TokenStream {
    let FunctionTypes {
        name: function_name,
        input_params,
        input_conversions: _,
        return_type: function_return_type,
        return_conversions: function_return_conversions,
    } = types;

    let input_params_string = input_params.to_string();
    let input_params_vec: Vec<_> = input_params_string
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .collect();
    let param_count = input_params_vec.len();

    let interpreter_input_conversions: Vec<TokenStream> = input_params_vec
        .iter()
        .map(|param| {
            let param_name = param.trim().split(':').next().unwrap().trim();
            let param_ident = Ident::new(param_name, Span::call_site());
            quote! {
                leo_ast::interpreter_value::Value::from(ToValue::<TestnetV0>::to_value(&#param_ident))
            }
        })
        .collect();

    quote! {
        fn #function_name(&self, account: &Account<TestnetV0>, #input_params) -> #function_return_type {
            let function_outputs = with_shared_interpreter(|state| -> Result<Vec<snarkvm::prelude::Value<TestnetV0>>, anyhow::Error> {
                let mut interpreter = state.interpreter.borrow_mut();

                interpreter.set_signer(account.address());

                let mut function_args: Vec<leo_ast::interpreter_value::Value> = Vec::new();
                #(function_args.push(#interpreter_input_conversions);)*

                let program_symbol = Symbol::intern(Self::PROGRAM_NAME);
                interpreter.cursor.set_program(Self::PROGRAM_NAME);

                let function_name_symbol = Symbol::intern(stringify!(#function_name));

                interpreter.cursor.values.extend(function_args);

                let default_span = leo_span::Span::default();
                let function_identifier = leo_ast::Identifier::new(function_name_symbol, interpreter.node_builder.next_id());
                let function_path = leo_ast::Path::new(
                    Vec::<leo_ast::Identifier>::new(),
                    function_identifier,
                    false,
                    None,
                    default_span,
                    interpreter.node_builder.next_id(),
                );

                let call_expression = leo_ast::CallExpression {
                    function: function_path,
                    arguments: vec![leo_ast::Expression::Unit(leo_ast::UnitExpression {
                        span: default_span,
                        id: interpreter.node_builder.next_id()
                    }); #param_count],
                    const_arguments: Vec::<leo_ast::Expression>::new(),
                    program: Some(program_symbol),
                    span: default_span,
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

                let mut function_outputs: Vec<snarkvm::prelude::Value<TestnetV0>> = if let Some(leo_value) = interpreter_result.value {
                    use leo_ast::interpreter_value::ValueVariants;
                    match &leo_value.contents {
                        ValueVariants::Tuple(elements) => {
                            elements.iter()
                                .filter_map(|elem| match &elem.contents {
                                    ValueVariants::Svm(svm_value) => Some(svm_value.clone()),
                                    _ => None
                                })
                                .collect()
                        }
                        ValueVariants::Svm(svm_value) => vec![svm_value.clone()],
                        _ => Vec::new()
                    }
                } else {
                    Vec::new()
                };

                if let Some(leo_ast::interpreter_value::AsyncExecution::AsyncFunctionCall { function, arguments }) = interpreter.cursor.futures.pop() {
                    use snarkvm::prelude::{ProgramID, Identifier, Future};

                    let fake_future = Future::new(
                        ProgramID::from_str("future.aleo").unwrap(),
                        Identifier::from_str("noop").unwrap(),
                        Vec::new()
                    );
                    function_outputs.push(snarkvm::prelude::Value::Future(fake_future));

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
}
