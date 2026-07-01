use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Rate limited, retry after {0:?}")]
    RateLimited(Option<Duration>),

    #[error("Transaction {tx_id} was rejected: {reason}")]
    TransactionRejected { tx_id: String, reason: String },

    #[error("Transaction {0} not confirmed within timeout")]
    TransactionTimeout(String),

    #[error("Program {0} not available within timeout")]
    ProgramTimeout(String),

    #[error("Invalid configuration: {0}")]
    Config(String),

    #[error("{0}")]
    Other(String),
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Other(e.to_string())
    }
}

impl From<reqwest_middleware::Error> for Error {
    fn from(e: reqwest_middleware::Error) -> Self {
        Error::Other(e.to_string())
    }
}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        Error::Other(e.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Other(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
