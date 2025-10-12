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

/// Generates Rust bindings for a Leo program.
///
/// # Parameters
/// - A single relative path string to either:
///   - `*.initial.json` file (AST snapshot from `leo build --enable-initial-ast-snapshot`)
///   - `*.json` file (pre-processed signature JSON file)
///
/// # Example
/// ```
/// generate_bindings!("outputs/token.initial.json");
/// ```
#[proc_macro]
pub fn generate_bindings(input: TokenStream) -> TokenStream {
    let input_str = input.to_string();
    let path = input_str.trim().trim_matches('"').to_string();

    let json = read_json_string_from_path_string(path.clone());
    let json = if path.ends_with("initial.json") {
        get_signatures(json)
    } else {
        json
    };

    let simplified = simplified_from_json_string(json);

    let crate_name = std::env::var("CARGO_CRATE_NAME").unwrap();
    let expected_crate_name = format!("{}_bindings", simplified.program_name);

    if crate_name != expected_crate_name {
        let error_msg = format!(
            "Naming convention violation: library '{}' should be named '{}' for program '{}.aleo'. \
            Update your Cargo.toml [lib] name to follow the convention: {{program}}_bindings",
            crate_name, expected_crate_name, simplified.program_name
        );
        return quote::quote! {
            compile_error!(#error_msg);
        }
        .into();
    }

    let exports: Vec<proc_macro2::TokenStream> = simplified
        .imports
        .iter()
        .map(|import| {
            let import_crate = proc_macro2::Ident::new(
                &format!("{}_bindings", import),
                proc_macro2::Span::call_site(),
            );
            let import_module = proc_macro2::Ident::new(import, proc_macro2::Span::call_site());
            quote::quote! {
                pub use ::#import_crate::#import_module;
            }
        })
        .collect();

    let program_module = generate_program_module(&simplified);

    quote::quote! {
        #(#exports)*

        #program_module
    }
    .into()
}
