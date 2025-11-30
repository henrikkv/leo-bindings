use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use snarkvm::prelude::*;
use std::env;

#[derive(Debug, Clone)]
pub struct DelegatedProvingConfig {
    pub api_key: String,
    pub endpoint: String,
}

impl DelegatedProvingConfig {
    pub fn new(api_key: String, endpoint: Option<String>) -> Self {
        Self {
            api_key,
            endpoint: endpoint
                .unwrap_or_else(|| "https://api.explorer.provable.com/v2".to_string()),
        }
    }
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let api_key = env::var("PROVABLE_API_KEY")
            .map_err(|_| anyhow!("PROVABLE_API_KEY environment variable not set"))?;

        let endpoint = env::var("PROVABLE_ENDPOINT")
            .unwrap_or_else(|_| "https://api.explorer.provable.com/v2".to_string());

        Ok(Self { api_key, endpoint })
    }
}

#[derive(Debug, Serialize)]
struct ProvingRequest {
    authorization: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    fee_authorization: Option<serde_json::Value>,
    broadcast: bool,
}

#[derive(Debug, Deserialize)]
struct ProvingResponse {
    transaction: serde_json::Value,
    #[serde(default)]
    broadcast: Option<bool>,
}

pub fn execute_with_delegated_proving<N: Network>(
    config: &DelegatedProvingConfig,
    authorization: Authorization<N>,
    broadcast: bool,
) -> Result<Transaction<N>> {
    let authorization_json = serde_json::to_value(&authorization)
        .map_err(|e| anyhow!("Failed to serialize authorization: {}", e))?;

    let proving_request = ProvingRequest {
        authorization: authorization_json,
        fee_authorization: None,
        broadcast,
    };

    let url = format!("{}/{}/prove", config.endpoint, N::SHORT_NAME);

    let response = ureq::post(&url)
        .header("X-Provable-API-Key", &config.api_key)
        .header("Content-Type", "application/json")
        .header("X-ALEO-METHOD", "submitProvingRequest")
        .send_json(&proving_request);

    let mut response = match response {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(status)) => {
            return Err(anyhow!("Request failed with status {}", status,));
        }
        Err(e) => {
            return Err(anyhow!("Failed to send request: {}", e));
        }
    };

    let proving_response: ProvingResponse = response
        .body_mut()
        .read_json()
        .map_err(|e| anyhow!("Failed to parse response: {}", e))?;

    let transaction_str = serde_json::to_string(&proving_response.transaction)
        .map_err(|e| anyhow!("Failed to serialize transaction: {}", e))?;

    let transaction: Transaction<N> = serde_json::from_str(&transaction_str)
        .map_err(|e| anyhow!("Failed to deserialize transaction: {}", e))?;

    log::info!("✅ Received proved transaction: {}", transaction.id());

    Ok(transaction)
}
