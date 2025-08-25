use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Token};
use leo_signatures::SimplifiedBindings;


// Struct to parse macro arguments
struct MacroArgs {
    json_path: syn::LitStr,
    network: Option<syn::Path>,
}

impl syn::parse::Parse for MacroArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let json_path: syn::LitStr = input.parse()?;
        
        let network = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            Some(input.parse()?)
        } else {
            None
        };
        
        Ok(MacroArgs { json_path, network })
    }
}

#[proc_macro]
pub fn generate_bindings(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as MacroArgs);
    let json_path = args.json_path;
    
    let json_content = std::fs::read_to_string(json_path.value())
        .expect("Failed to read JSON file");
    
    let simplified: SimplifiedBindings = serde_json::from_str(&json_content)
        .expect("Failed to parse JSON");
    
    generate_code_from_simplified(&simplified, args.network)
}

fn generate_code_from_simplified(simplified: &SimplifiedBindings, network_type: Option<syn::Path>) -> TokenStream {
    let program_name = syn::Ident::new(&simplified.program_name, proc_macro2::Span::call_site());
    
    let network_type_token = network_type
        .as_ref()
        .map(|path| quote! { #path })
        .unwrap_or_else(|| quote! { snarkvm::console::network::MainnetV0 });
    
    let record_structs = simplified.records.iter().map(|record| {
        let record_name = syn::Ident::new(&record.name, proc_macro2::Span::call_site());
        let field_getters = record.fields.iter().map(|field| {
            let field_name = syn::Ident::new(&field.name, proc_macro2::Span::call_site());
            let field_type = get_rust_type(&field.type_name);
            
            quote! {
                pub fn #field_name(&self) -> #field_type {
                    let field = &Identifier::try_from(stringify!(#field_name)).unwrap();
                    let entry = self.record.data().get(field).unwrap();
                    let value = entry.to_value();
                    <#field_type>::from_value(value)
                }
            }
        });
        
        quote! {
            #[derive(Debug)]
            pub struct #record_name {
                pub record: Record<Nw, Plaintext<Nw>>,
            }
            
            impl ToValue<Nw> for #record_name {
                fn to_value(&self) -> Value<Nw> {
                    Value::Record(self.record.clone())
                }
            }
            
            impl FromValue<Nw> for #record_name {
                fn from_value(value: Value<Nw>) -> Self {
                    match value {
                        Value::Record(record) => {
                            Self { record }
                        },
                        _ => panic!("Expected record type"),
                    }
                }
            }
            
            impl #record_name {
                pub fn new(record: Record<Nw, Plaintext<Nw>>) -> Self {
                    #record_name { record }
                }
                
                #(#field_getters)*
            }
        }
    });
    
    let function_implementations = simplified.functions.iter().map(|function| {
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
                let program_name = stringify!(#program_name).to_string();
                let program_id_str = format!("{}.aleo", program_name);
                let program_id = ProgramID::try_from(program_id_str.as_str()).unwrap();
                let function_name = stringify!(#function_name).to_string();
                let function_id = Identifier::from_str(&function_name).unwrap();
                let args: Vec<Value<Nw>> = vec![
                    #(#input_conversions),*
                ];
                let rng = &mut rand::thread_rng();
                println!("Transaction of function {}:", function_name);
                let query = ENDPOINT.to_string();
                let private_key = account.private_key();
                let priority_fee = 0;
                let locator = Locator::<Nw>::new(program_id, function_id);
                
                let transaction: Transaction<Nw> = {
                    let rng = &mut rand::thread_rng();
                    let store = ConsensusStore::<Nw, ConsensusMemory<Nw>>::open(StorageMode::Production)?;
                    let vm = VM::from(store)?;
                    
                    let program_string = {
                        let response = ureq::get(&format!("{}/testnet/program/{}", query, program_id))
                            .call()
                            .map_err(|e| anyhow!("Failed to fetch program: {}", e))?;
                        let json_response: serde_json::Value = response.into_json()?;
                        json_response.as_str()
                            .ok_or_else(|| anyhow!("Expected program string in JSON response"))?
                            .to_string()
                    };
                    let program: Program<Nw> = program_string.parse()?;
                    vm.process().write().add_program(&program)?;
                    let fee_record = None;
                    vm.execute(
                        &private_key,
                        (program_id, function_id),
                        args.iter(),
                        fee_record,
                        priority_fee,
                        Some(&Query::<Nw, BlockMemory<Nw>>::from(query.as_str()) as &dyn QueryTrait<Nw>),
                        rng,)?
                };
                
                let public_balance = get_public_balance(&account.address(), &query)?;
                let storage_cost = transaction
                    .execution()
                    .ok_or_else(|| anyhow!("The transaction does not contain an execution"))?
                    .size_in_bytes()?;
                let base_fee = storage_cost.saturating_add(priority_fee);
                
                if public_balance < base_fee {
                    bail!(
                        "❌ The public balance of {} is insufficient to pay the base fee for `{}`",
                        public_balance,
                        locator.to_string()
                    );
                }
                
                println!("✅ Created execution transaction for '{}'", locator.to_string());
                println!("Response from transaction broadcast: {}", broadcast_transaction(transaction.clone())?);
                
                let execution = match transaction {
                    Transaction::Execute(_, _, execution, _) => execution,
                    _ => panic!("Not an execution."),
                };

                let mut transitions = execution.transitions();
                let target_transition = transitions.find(|transition| {
                    transition.function_name().to_string() == function_name
                }).expect("Could not find transition for the target function");
                let outputs_iter = target_transition.outputs().iter();
                let outputs: Vec<Value<Nw>> = outputs_iter.map(|output| {
                    match output {
                      Output::Constant(_, plaintext) | Output::Public(_, plaintext) => {
                          plaintext.as_ref().map(|pt| Value::Plaintext(pt.clone())).unwrap_or_else(|| {
                              panic!("Expected plaintext output but found None")
                          })
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
    });
    
    let expanded = quote! {
        use anyhow::{anyhow, bail, ensure};
        use snarkvm::prelude::*;
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
        
        type Nw = #network_type_token;
        
const ENDPOINT: &str = "http://localhost:3030";
        /// A helper struct for an Aleo account (from snarkOS).
        #[derive(Clone, Debug)]
        pub struct Account<N: Network> {
            private_key: PrivateKey<N>,
            view_key: ViewKey<N>,
            address: Address<N>,
        }

        impl<N: Network> Account<N> {
            pub fn new<R: Rng + CryptoRng>(rng: &mut R) -> Result<Self> {
                Self::try_from(PrivateKey::new(rng)?)
            }

            pub const fn private_key(&self) -> &PrivateKey<N> {
                &self.private_key
            }

            pub const fn view_key(&self) -> &ViewKey<N> {
                &self.view_key
            }

            pub const fn address(&self) -> Address<N> {
                self.address
            }
        }

        impl<N: Network> TryFrom<PrivateKey<N>> for Account<N> {
            type Error = Error;

            fn try_from(private_key: PrivateKey<N>) -> Result<Self, Self::Error> {
                Self::try_from(&private_key)
            }
        }

        impl<N: Network> TryFrom<&PrivateKey<N>> for Account<N> {
            type Error = Error;

            fn try_from(private_key: &PrivateKey<N>) -> Result<Self, Self::Error> {
                let view_key = ViewKey::try_from(private_key)?;
                let address = view_key.to_address();
                Ok(Self { private_key: *private_key, view_key, address })
            }
        }

        impl<N: Network> FromStr for Account<N> {
            type Err = Error;

            fn from_str(private_key: &str) -> Result<Self, Self::Err> {
                Self::try_from(PrivateKey::from_str(private_key)?)
            }
        }
        
        trait ToValue<N: Network> {
            fn to_value(&self) -> Value<N>;
        }
        
        trait FromValue<N: Network> {
            fn from_value(value: Value<N>) -> Self;
        }
        
        impl ToValue<Nw> for u8 {
            fn to_value(&self) -> Value<Nw> {
                Value::Plaintext(Plaintext::from(Literal::U8(U8::new(*self))))
            }
        }
        
        impl FromValue<Nw> for u8 {
            fn from_value(value: Value<Nw>) -> Self {
                match value {
                    Value::Plaintext(plaintext) => {
                        match plaintext {
                            Plaintext::Literal(literal, _) => {
                                match literal {
                                    Literal::U8(u8_val) => *u8_val,
                                    _ => panic!("Expected u8 type"),
                                }
                            },
                            _ => panic!("Expected literal plaintext"),
                        }
                    },
                    _ => panic!("Expected plaintext value"),
                }
            }
        }
        
        impl ToValue<Nw> for u16 {
            fn to_value(&self) -> Value<Nw> {
                Value::Plaintext(Plaintext::from(Literal::U16(U16::new(*self))))
            }
        }
        
        impl FromValue<Nw> for u16 {
            fn from_value(value: Value<Nw>) -> Self {
                match value {
                    Value::Plaintext(plaintext) => {
                        match plaintext {
                            Plaintext::Literal(literal, _) => {
                                match literal {
                                    Literal::U16(u16_val) => *u16_val,
                                    _ => panic!("Expected u16 type"),
                                }
                            },
                            _ => panic!("Expected literal plaintext"),
                        }
                    },
                    _ => panic!("Expected plaintext value"),
                }
            }
        }
        
        impl ToValue<Nw> for u32 {
            fn to_value(&self) -> Value<Nw> {
                Value::Plaintext(Plaintext::from(Literal::U32(U32::new(*self))))
            }
        }
        
        impl FromValue<Nw> for u32 {
            fn from_value(value: Value<Nw>) -> Self {
                match value {
                    Value::Plaintext(plaintext) => {
                        match plaintext {
                            Plaintext::Literal(literal, _) => {
                                match literal {
                                    Literal::U32(u32_val) => *u32_val,
                                    _ => panic!("Expected u32 type"),
                                }
                            },
                            _ => panic!("Expected literal plaintext"),
                        }
                    },
                    _ => panic!("Expected plaintext value"),
                }
            }
        }
        
        impl ToValue<Nw> for u64 {
            fn to_value(&self) -> Value<Nw> {
                Value::Plaintext(Plaintext::from(Literal::U64(U64::new(*self))))
            }
        }
        
        impl FromValue<Nw> for u64 {
            fn from_value(value: Value<Nw>) -> Self {
                match value {
                    Value::Plaintext(plaintext) => {
                        match plaintext {
                            Plaintext::Literal(literal, _) => {
                                match literal {
                                    Literal::U64(u64_val) => *u64_val,
                                    _ => panic!("Expected u64 type"),
                                }
                            },
                            _ => panic!("Expected literal plaintext"),
                        }
                    },
                    _ => panic!("Expected plaintext value"),
                }
            }
        }
        
        impl ToValue<Nw> for u128 {
            fn to_value(&self) -> Value<Nw> {
                Value::Plaintext(Plaintext::from(Literal::U128(U128::new(*self))))
            }
        }
        
        impl FromValue<Nw> for u128 {
            fn from_value(value: Value<Nw>) -> Self {
                match value {
                    Value::Plaintext(plaintext) => {
                        match plaintext {
                            Plaintext::Literal(literal, _) => {
                                match literal {
                                    Literal::U128(u128_val) => *u128_val,
                                    _ => panic!("Expected u128 type"),
                                }
                            },
                            _ => panic!("Expected literal plaintext"),
                        }
                    },
                    _ => panic!("Expected plaintext value"),
                }
            }
        }
        
        
        fn get_public_balance(address: &Address<Nw>, endpoint: &str) -> Result<u64, anyhow::Error> {
            let credits = ProgramID::<Nw>::from_str("credits.aleo")?;
            let account_mapping = Identifier::<Nw>::from_str("account")?;

            let response = ureq::get(&format!(
                "{endpoint}/testnet/program/{credits}/mapping/{account_mapping}/{address}"
            ))
            .call();

            let balance: Result<Option<Value<Nw>>, anyhow::Error> = match response {
                Ok(response) => response.into_json().map_err(|err| err.into()),
                Err(err) => match err {
                    ureq::Error::Status(_status, response) => {
                        bail!(response
                            .into_string()
                            .unwrap_or("Response too large!".to_owned()))
                    }
                    err => bail!(err),
                },
            };

            match balance {
                Ok(Some(Value::Plaintext(Plaintext::Literal(Literal::<Nw>::U64(amount), _)))) => {
                    Ok(*amount)
                }
                Ok(None) => Ok(0),
                Ok(Some(..)) => bail!("Failed to deserialize balance for {address}"),
                Err(err) => bail!("Failed to fetch balance for {address}: {err}"),
            }
        }
        
        fn broadcast_transaction(transaction: Transaction<Nw>) -> Result<String, anyhow::Error> {
            let transaction_id = transaction.id();
            ensure!(
                !transaction.is_fee(),
                "The transaction is a fee transaction and cannot be broadcast"
            );
            
            match ureq::post(&format!("{}/testnet/transaction/broadcast", ENDPOINT)).send_json(&transaction)
            {
                Ok(id) => {
                    let response_string = id.into_string()?.trim_matches('\"').to_string();
                    ensure!( response_string == transaction_id.to_string(), "The response does not match the transaction id. ({response_string} != {transaction_id})");
                    println!(
                        "⌛ Execution {transaction_id} has been broadcast to {}.",
                        ENDPOINT
                    );
                    
                    Ok(response_string)
                }
                Err(error) => {
                    let error_message = match error {
                        ureq::Error::Status(code, response) => {
                            format!("(status code {code}: {:?})", response.into_string().unwrap_or_default())
                        }
                        ureq::Error::Transport(err) => format!("({err})"),
                    };
                    bail!(
                        "❌ Failed to broadcast execution to {}: {}",
                        ENDPOINT,
                        error_message
                    )
                }
            }
        }
        
        
        #(#record_structs)*
        
        pub struct #program_name {
            pub package: Package,
        }
        
        impl #program_name {
            pub fn new(deployer: &Account<Nw>) -> Result<Self, anyhow::Error> {
                use leo_package::Package;
                use leo_span::create_session_if_not_set_then;
                use std::path::Path;
                
                // Initialize Leo session globals
                let result = create_session_if_not_set_then(|_| {
                    // Load the Leo package from the current directory
                let package = Package::from_directory(
                    Path::new("."),
                    Path::new("."),
                    false, // no_cache
                    false, // recursive
                    NetworkName::TestnetV0,
                    ENDPOINT,
                )?;
                
                // Get the main program
                let main_program = package.programs.iter()
                    .find(|p| !p.is_test && p.is_local)
                    .ok_or_else(|| anyhow!("No main program found in package"))?;
                
                // Read the compiled .aleo bytecode
                let aleo_name = format!("{}.aleo", main_program.name);
                let aleo_path = if package.manifest.program == aleo_name {
                    package.build_directory().join("main.aleo")
                } else {
                    package.imports_directory().join(aleo_name)
                };
                
                let bytecode = std::fs::read_to_string(aleo_path.clone())
                    .map_err(|e| anyhow!("Failed to read bytecode from {}: {}", aleo_path.display(), e))?;
                
                // Parse the program
                let program: Program<Nw> = bytecode.parse()
                    .map_err(|e| anyhow!("Failed to parse program: {}", e))?;
                
                let program_id = program.id();
                
                // Deploy the program
                println!("📦 Creating deployment transaction for '{}'...", program_id);
                let rng = &mut rand::thread_rng();
                let vm = VM::from(ConsensusStore::<Nw, ConsensusMemory<Nw>>::open(StorageMode::Production)?)?;
                let query = Query::<Nw, BlockMemory<Nw>>::from(ENDPOINT);
                
                let transaction = vm.deploy(
                    deployer.private_key(),
                    &program,
                    None, // fee_record
                    0,    // priority_fee
                    Some(&query),
                    rng,
                ).map_err(|e| anyhow!("Failed to generate deployment transaction: {}", e))?;
                
                // Broadcast the deployment transaction
                println!("📡 Broadcasting deployment transaction...");
                let transaction_id = transaction.id();
                let response = ureq::post(&format!("{}/testnet/transaction/broadcast", ENDPOINT))
                    .send_json(&transaction)?;
                
                let response_string = response.into_string()?.trim_matches('\"').to_string();
                ensure!(
                    response_string == transaction_id.to_string(),
                    "Response ID mismatch: {} != {}", response_string, transaction_id
                );
                
                println!("✅ Deployment transaction {} broadcast successfully!", transaction_id);
                
                    Ok(Self { package })
                });
                
                result
            }
            
            #(#function_implementations)*
        }
    };
    
    expanded.into()
}

fn get_rust_type(type_name: &str) -> proc_macro2::TokenStream {
    match type_name {
        "u8" => quote! { u8 },
        "u16" => quote! { u16 },
        "u32" => quote! { u32 },
        "u64" => quote! { u64 },
        "u128" => quote! { u128 },
        "i8" => quote! { i8 },
        "i16" => quote! { i16 },
        "i32" => quote! { i32 },
        "i64" => quote! { i64 },
        "i128" => quote! { i128 },
        "address" => quote! { Address<Nw> },
        "Address" => quote! { Address<Nw> },
        "Future" => quote! { Future<Nw> },
        other => {
            let type_ident = syn::Ident::new(other, proc_macro2::Span::call_site());
            quote! { #type_ident }
        }
    }
}
