#![doc = include_str!("../README.md")]

//!
//! ## Generated Crates
//!
//! The credits bindings can be imported to interact with credits.aleo.
//! - `credits_bindings` - leo-bindings-credits/
//!
//! The documentation from the examples show what the macro expands to.
//! - `token_bindings` - examples/token/leo/
//! - `dev_bindings` - examples/dev/leo/
//! - `delegated_proving_test_bindings` - examples/delegated/leo/

pub mod interpreter_cheats;
pub mod utils;

pub use leo_bindings_core::*;
pub use leo_bindings_macro::generate_bindings;
pub use utils::DelegatedProvingConfig;

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
