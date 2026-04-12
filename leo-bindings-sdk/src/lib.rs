mod account;
mod config;
mod endpoints;
mod error;
pub mod local_chain;
mod stats;
mod utils;
mod vm_manager;

pub use account::Account;
pub use config::{Client, Credentials};
pub use endpoints::transactions::TransactionStatus;
pub use error::{Error, Result};
pub use local_chain::build_local_chain_bytes;
pub use stats::{print_deployment_stats, print_execution_stats};
pub use vm_manager::LocalVM;
pub use vm_manager::{CONSENSUS_VERSION, NetworkVm, VMManager};

pub fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime")
        .block_on(f)
}

pub use snarkvm;
