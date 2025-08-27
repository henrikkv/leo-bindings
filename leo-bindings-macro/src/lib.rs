use proc_macro::TokenStream;
use syn::{parse_macro_input, Token};
use leo_bindings_core::signature::{get_signatures, SimplifiedBindings};
use leo_bindings_core::generator::generate_code_from_simplified;

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
    
    let signatures_json = get_signatures(&json_content)
        .expect("Failed to extract signatures");
    
    let simplified: SimplifiedBindings = serde_json::from_str(&signatures_json)
        .expect("Failed to parse simplified JSON");
    
    generate_code_from_simplified(&simplified, args.network).into()
}