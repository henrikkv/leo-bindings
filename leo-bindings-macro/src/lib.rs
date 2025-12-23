use leo_bindings_core::SimplifiedBindings;
use leo_bindings_core::generator::generate_program_module;
use proc_macro::TokenStream;

fn read_json_string_from_path_string(path: String) -> String {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_path = std::path::Path::new(&manifest_dir);
    let path = manifest_path.join(path);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read JSON at {}: {}.", path.display(), e))
}

fn simplified_from_json_string(json: String) -> SimplifiedBindings {
    serde_json::from_str(&json).expect("Failed to parse signatures from json")
}

/// Generates Rust bindings for a Leo program.
///
/// The input is the path of a "signatures.json" file with types from the Leo program.
#[proc_macro]
pub fn generate_bindings(input: TokenStream) -> TokenStream {
    let input_str = input.to_string();
    let path = input_str.trim().trim_matches('"').to_string();
    let json = read_json_string_from_path_string(path);
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
