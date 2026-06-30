use leo_abi_types::Program;
use leo_bindings_core::generator::generate_program_module;
use proc_macro2::Span;
use std::path::PathBuf;
use syn::Error;

/// Generates Rust bindings for a Leo program.
///
/// Reads ABI from `{CARGO_MANIFEST_DIR}/build/abi.json` and optional `{CARGO_MANIFEST_DIR}/build/imports/*.abi.json`.
#[proc_macro]
pub fn generate_bindings(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match generate_bindings_inner(input) {
        Ok(ts) => ts,
        Err(e) => e.to_compile_error().into(),
    }
}
fn generate_bindings_inner(
    _input: proc_macro::TokenStream,
) -> syn::Result<proc_macro::TokenStream> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").map_err(|e| {
        Error::new(
            Span::call_site(),
            format!("CARGO_MANIFEST_DIR is not set: {e}"),
        )
    })?;
    let crate_name = std::env::var("CARGO_CRATE_NAME").unwrap();
    let program_name = crate_name
        .strip_suffix("_bindings")
        .ok_or_else(|| Error::new(Span::call_site(), format!("crate name '{crate_name}' does not end with '_bindings'")))?;
    let build_dir = PathBuf::from(&manifest_dir).join("build").join(program_name);
    let abi_path = build_dir.join("abi.json");
    let json = std::fs::read_to_string(&abi_path).map_err(|e| {
        Error::new(
            Span::call_site(),
            format!("failed to read ABI at {}: {e}", abi_path.display()),
        )
    })?;
    let abi: Program = serde_json::from_str(&json).map_err(|e| Error::new(Span::call_site(), e))?;
    let program_id = abi.program.trim_end_matches(".aleo");
    let expected_crate_name = format!("{}_bindings", program_id);
    if crate_name != expected_crate_name {
        return Err(Error::new(
            Span::call_site(),
            format!(
                "Naming convention violation: library '{}' should be named '{}' for program '{}'. \
                 Update your Cargo.toml [lib] name to follow the convention: {{program}}_bindings",
                crate_name, expected_crate_name, abi.program
            ),
        ));
    };

    let mut import_names: Vec<String> = Vec::new();
    let build_root = PathBuf::from(&manifest_dir).join("build");
    if build_root.is_dir() {
        for entry in std::fs::read_dir(&build_root).map_err(|e| {
            Error::new(Span::call_site(), format!("failed to read {}: {e}", build_root.display()))
        })? {
            let entry = entry.map_err(|e| Error::new(Span::call_site(), e))?;
            let name = entry.file_name();
            let name = name.to_str().unwrap();
            if name == program_id || !entry.path().is_dir() {
                continue;
            }
            if entry.path().join("abi.json").exists() {
                import_names.push(name.to_string());
            }
        }
    }

    let import_streams: Vec<proc_macro2::TokenStream> = import_names
        .iter()
        .map(|import| {
            let import_crate =
                proc_macro2::Ident::new(&format!("{}_bindings", import), Span::call_site());
            let import_module = proc_macro2::Ident::new(import, Span::call_site());
            quote::quote! {
                pub use ::#import_crate::#import_module;
            }
        })
        .collect();

    let program_module = generate_program_module(&abi, &import_names);

    Ok(quote::quote! {
        #(#import_streams)*

        #program_module
    }
    .into())
}
