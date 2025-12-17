use crate::client::ProvableClient;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use snarkvm::prelude::{Authorization, Network, Transaction};

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
}

impl<N: Network> ProvableClient<N> {
    /// Submit an authorization for delegated proving
    ///
    pub async fn prove(&self, authorization: &Authorization<N>) -> Result<Transaction<N>> {
        let jwt_token = self.get_valid_jwt_token().await?;

        let authorization_json = serde_json::to_value(authorization)
            .map_err(|e| Error::Internal(format!("Failed to serialize authorization: {}", e)))?;

        let proving_request = ProvingRequest {
            authorization: authorization_json,
            fee_authorization: None,
            broadcast: false,
        };

        let url = format!(
            "https://api.provable.com/prove/{}/prove",
            self.network_name()
        );

        let request_body = serde_json::to_string(&proving_request)
            .map_err(|e| Error::Internal(format!("Failed to serialize proving request: {}", e)))?;

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", jwt_token))
            .header("X-ALEO-METHOD", "submitProvingRequest")
            .header("Content-Type", "application/json")
            .body(request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Proving request failed".to_string());

            return match status {
                401 => Err(Error::JwtAuthFailed(message)),
                400 => Err(Error::BadRequest(message)),
                429 => Err(Error::RateLimited(None)),
                _ => Err(Error::ApiError { status, message }),
            };
        }

        let proving_response: ProvingResponse = response
            .json()
            .await
            .map_err(|e| Error::BadResponse(format!("Failed to parse proving response: {}", e)))?;

        let transaction: Transaction<N> = serde_json::from_value(proving_response.transaction)
            .map_err(|e| Error::BadResponse(format!("Failed to deserialize transaction: {}", e)))?;

        Ok(transaction)
    }
}
