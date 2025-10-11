#![feature(track_path)]

use leo_bindings_core::generator::generate_program_module;
use leo_bindings_core::signature::get_signatures;
use leo_bindings_core::SimplifiedBindings;
use proc_macro::TokenStream;

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
/// - Array of relative path strings to either:
///   - `*.initial.json` files (AST snapshots from `leo build --enable-initial-ast-snapshot`)
///   - `*.json` files (pre-processed signature JSON files)
///
/// Networks are selected via cargo features: `testnet`, `mainnet`, `canary`, `interpreter`
///
/// # Example
/// ```
/// generate_bindings!(["outputs/dev.initial.json", "outputs/token.signatures.json"]);
/// ```
#[proc_macro]
pub fn generate_bindings(input: TokenStream) -> TokenStream {
    let input_str = input.to_string();

    let paths: Vec<String> = input_str
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let program_modules: Vec<proc_macro2::TokenStream> = paths
        .into_iter()
        .map(|path| {
            let json = read_json_string_from_path_string(path.clone());
            if path.ends_with("initial.json") {
                get_signatures(json)
            } else {
                json
            }
        })
        .map(simplified_from_json_string)
        .map(|simplified| generate_program_module(&simplified))
        .collect();

    quote::quote! {
        #(#program_modules)*
    }
    .into()
}
