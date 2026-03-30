mod account;
mod config;
mod endpoints;
mod error;
mod stats;
mod utils;
mod vm_manager;

pub use account::Account;
pub use config::{Client, Credentials};
pub use endpoints::transactions::TransactionStatus;
pub use error::{Error, Result};
pub use stats::{print_deployment_stats, print_execution_stats};
pub use vm_manager::{VMManager, CONSENSUS_VERSION};

pub fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime")
        .block_on(f)
}

pub use snarkvm;
