#![feature(track_path)]

use leo_bindings_core::generator::generate_program_module;
use leo_bindings_core::signature::get_signatures;
use leo_bindings_core::SimplifiedBindings;
use proc_macro::TokenStream;
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, LitStr, Result, Token};

// Struct to parse macro arguments
struct MacroArgs {
    networks: Vec<String>,
    snapshot_paths: Vec<String>,
    signature_paths: Vec<String>,
}

fn parse_string_array(input: ParseStream) -> Result<Vec<String>> {
    let content;
    syn::bracketed!(content in input);
    let mut strings = Vec::new();
    while !content.is_empty() {
        let lit: LitStr = content.parse()?;
        strings.push(lit.value());
        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        }
    }
    Ok(strings)
}

impl Parse for MacroArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let networks = parse_string_array(input)?;
        input.parse::<Token![,]>()?;
        let snapshot_paths = parse_string_array(input)?;
        input.parse::<Token![,]>()?;
        let signature_paths = parse_string_array(input)?;

        Ok(MacroArgs {
            networks,
            snapshot_paths,
            signature_paths,
        })
    }
}

fn read_json_string_from_path_string(path: String) -> String {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = std::path::Path::new(&manifest_dir).join(path);

    // Ensures the bindings update after recompiling a leo program
    proc_macro::tracked_path::path(path.to_str().expect("Path must be valid unicode"));

    std::fs::read_to_string(&path).expect("Failed to read JSON")
}
fn simplified_from_json_string(json: String) -> SimplifiedBindings {
    serde_json::from_str(&json).expect("Failed to parse signatures from json")
}

/// Generates Rust bindings for Leo programs.
///
/// # Parameters
/// - `network`: Network type string ("mainnet", "testnet", or "canary")
/// - `snapshot_paths`: Array of relative path strings to dev.initial.json files generated with `leo build --enable-initial-ast-snapshot`
/// - `signature_paths`: Array of relative path strings to pre-processed signature JSON files
#[proc_macro]
pub fn generate_network_bindings(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as MacroArgs);

    let program_modules: Vec<proc_macro2::TokenStream> = args
        .snapshot_paths
        .into_iter()
        .map(read_json_string_from_path_string)
        .map(get_signatures)
        .chain(
            args.signature_paths
                .into_iter()
                .map(read_json_string_from_path_string),
        )
        .map(|json| generate_program_module(&simplified_from_json_string(json), &args.networks))
        .collect();

    quote::quote! {
        #(#program_modules)*
    }
    .into()
}
