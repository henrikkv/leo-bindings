use leo_bindings_core::generator::generate_program_module;
use leo_bindings_core::signature::{get_signatures, SimplifiedBindings};
use proc_macro::TokenStream;
use syn::{parse_macro_input, Token};

// Struct to parse macro arguments
struct MacroArgs {
    network: syn::Path,
    json_paths: syn::ExprArray,
}

impl syn::parse::Parse for MacroArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let network = input.parse()?;
        input.parse::<Token![,]>()?;
        let json_paths = input.parse()?;

        Ok(MacroArgs {
            network,
            json_paths,
        })
    }
}

#[proc_macro]
pub fn generate_bindings(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as MacroArgs);
    let network = args.network;

    let program_modules: Vec<proc_macro2::TokenStream> = args
        .json_paths
        .elems
        .iter()
        .map(|json_path_expr| {
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(json_path),
                ..
            }) = json_path_expr
            {
                let json_path_value = json_path.value();

                let json_content =
                    std::fs::read_to_string(&json_path_value).expect("Failed to read JSON");

                let signatures_json =
                    get_signatures(&json_content).expect("Failed to extract signatures");

                let simplified: SimplifiedBindings = serde_json::from_str(&signatures_json)
                    .expect("Failed to parse simplified JSON");

                generate_program_module(&simplified, network.clone())
            } else {
                panic!("Expected string literal for JSON path");
            }
        })
        .collect();

    let expanded = quote::quote! {
        #(#program_modules)*
    };

    expanded.into()
}
