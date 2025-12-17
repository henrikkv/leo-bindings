use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Request middleware error: {0}")]
    Middleware(#[from] reqwest_middleware::Error),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Unauthorized: JWT token invalid or expired")]
    Unauthorized,

    #[error("Rate limited, retry after {0:?}")]
    RateLimited(Option<Duration>),

    #[error("Server error: {0}")]
    ServerError(String),

    #[error("Transaction {tx_id} was rejected: {reason}")]
    TransactionRejected { tx_id: String, reason: String },

    #[error("Transaction {0} not confirmed within timeout")]
    TransactionTimeout(String),

    #[error("Program {0} not available within timeout")]
    ProgramTimeout(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid configuration: {0}")]
    Config(String),

    #[error("JWT credentials (consumer_id and api_key) required")]
    JwtCredentialsRequired,

    #[error("Failed to fetch JWT token: {status} - {message}")]
    JwtFetchFailed { status: u16, message: String },

    #[error("JWT authentication failed: {0}")]
    JwtAuthFailed(String),

    #[error("Bad API response: {0}")]
    BadResponse(String),

    #[error("API error {status}: {message}")]
    ApiError { status: u16, message: String },

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, Error>;
