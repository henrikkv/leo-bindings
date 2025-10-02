pub mod utils;

pub use leo_bindings_core::*;
pub use leo_bindings_macro::generate_network_bindings;

/// Generates Rust bindings for Leo programs across all networks.
///
/// # Parameters
/// - `snapshot_paths`: Array of relative path strings to dev.initial.json files generated with `leo build --enable-initial-ast-snapshot`
/// - `signature_paths`: Array of relative path strings to pre-processed signature JSON files
#[macro_export]
macro_rules! generate_bindings {
    ($snapshot_paths:expr, $signature_paths:expr) => {
        #[cfg(feature = "mainnet")]
        $crate::generate_network_bindings!("mainnet", $snapshot_paths, $signature_paths);

        #[cfg(feature = "testnet")]
        $crate::generate_network_bindings!("testnet", $snapshot_paths, $signature_paths);

        #[cfg(feature = "canary")]
        $crate::generate_network_bindings!("canary", $snapshot_paths, $signature_paths);

        #[cfg(feature = "interpreter")]
        $crate::generate_network_bindings!("interpreter", $snapshot_paths, $signature_paths);
    };
}

pub use aleo_std;
pub use anyhow;
pub use http;
pub use indexmap;
pub use leo_ast;
pub use leo_errors;
pub use leo_interpreter;
pub use leo_package;
pub use leo_parser;
pub use leo_span;
pub use rand;
pub use serde_json;
pub use snarkvm;
pub use ureq;
pub use walkdir;
