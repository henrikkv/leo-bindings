#![feature(track_path)]

use leo_bindings_core::generator::generate_program_module;
use leo_bindings_core::signature::get_signatures;
use proc_macro::TokenStream;
use syn::{parse_macro_input, Expr, Token};

// Struct to parse macro arguments
struct MacroArgs {
    network: syn::Path,
    snapshot_paths: syn::ExprArray,
    signature_paths: syn::ExprArray,
}

impl syn::parse::Parse for MacroArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let network = input.parse()?;
        input.parse::<Token![,]>()?;
        let snapshot_paths = input.parse()?;
        input.parse::<Token![,]>()?;
        let signature_paths = input.parse()?;

        Ok(MacroArgs {
            network,
            snapshot_paths,
            signature_paths,
        })
    }
}

fn read_json_string_from_path_expr(expr: Expr) -> String {
    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(json_path),
        ..
    }) = expr
    {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let json_path = std::path::Path::new(&manifest_dir).join(json_path.value());
        // Ensures the bindings update after recompiling a leo program
        proc_macro::tracked_path::path(json_path.to_str().unwrap());
        std::fs::read_to_string(&json_path).expect("Failed to read JSON")
    } else {
        panic!("Path is not a string")
    }
}

/// Generates Rust bindings for Leo programs.
///
/// # Parameters
/// - `network`: Example: `snarkvm::console::network::TestnetV0`
/// - `snapshot_paths`: Array of relative path strings to dev.initial.json files generated with `leo build --enable-initial-ast-snapshot`
/// - `signature_paths`: Array of relative path strings to pre-processed signature JSON files
#[proc_macro]
pub fn generate_bindings(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as MacroArgs);
    let network = args.network;

    let program_modules: Vec<proc_macro2::TokenStream> = args
        .snapshot_paths
        .elems
        .into_iter()
        .map(read_json_string_from_path_expr)
        .map(get_signatures)
        .chain(
            args.signature_paths
                .elems
                .into_iter()
                .map(read_json_string_from_path_expr),
        )
        .map(|json| {
            generate_program_module(
                &serde_json::from_str(&json).expect("Failed to parse signatures from json"),
                network.clone(),
            )
        })
        .collect();

    quote::quote! {
        #(#program_modules)*
    }
    .into()
}
