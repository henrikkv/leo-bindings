use crate::client::ProvableClient;
use crate::error::{Error, Result};
use crate::utils::poll_until;
use snarkvm::prelude::{Network, Transaction};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionStatus {
    Accepted,
    Rejected(String),
    Pending,
}

impl<N: Network> ProvableClient<N> {
    /// Broadcast a transaction and wait for confirmation
    ///
    /// POST /{network}/transaction/broadcast
    ///
    pub async fn broadcast_wait(&self, transaction: &Transaction<N>) -> Result<N::TransactionID> {
        self.broadcast(transaction).await?;

        let tx_id = transaction.id();

        self.wait_for_transaction(&tx_id).await?;

        Ok(tx_id)
    }

    /// Broadcast a transaction without waiting for confirmation
    ///
    /// POST /{network}/transaction/broadcast
    ///
    pub async fn broadcast(&self, transaction: &Transaction<N>) -> Result<()> {
        let url = format!(
            "{}/v2/{}/transaction/broadcast",
            self.endpoint,
            self.network_name()
        );

        let jwt_token = self.get_valid_jwt_token().await?;

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", jwt_token))
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(transaction)?)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            match status {
                400 => Err(Error::BadRequest(message)),
                401 => Err(Error::JwtAuthFailed(message)),
                429 => Err(Error::RateLimited(None)),
                _ => Err(Error::ApiError { status, message }),
            }
        }
    }

    /// Query the status of a transaction
    ///
    /// GET /{network}/transaction/confirmed/{id}
    ///
    pub async fn transaction_status(&self, tx_id: &N::TransactionID) -> Result<TransactionStatus> {
        let url = format!(
            "{}/v2/{}/transaction/confirmed/{}",
            self.endpoint,
            self.network_name(),
            tx_id
        );

        let response = self.client.get(&url).send().await?;

        if response.status() == 404 || response.status() == 500 {
            return Ok(TransactionStatus::Pending);
        }

        if response.status().is_success() {
            let json: serde_json::Value = response.json().await?;

            let status = json
                .get("status")
                .and_then(|s| s.as_str())
                .ok_or_else(|| Error::BadResponse("Missing status field".to_string()))?;

            match status {
                "accepted" => Ok(TransactionStatus::Accepted),
                "rejected" => {
                    let reason = json.to_string();
                    Ok(TransactionStatus::Rejected(reason))
                }
                _ => Ok(TransactionStatus::Pending),
            }
        } else {
            let status_code = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(Error::ApiError {
                status: status_code,
                message,
            })
        }
    }

    pub async fn wait_for_transaction(&self, tx_id: &N::TransactionID) -> Result<()> {
        let tx_id_owned = tx_id.to_string();

        poll_until(
            || {
                let tx_id_str = tx_id_owned.clone();
                let url = format!(
                    "{}/v2/{}/transaction/confirmed/{}",
                    self.endpoint,
                    self.network_name(),
                    tx_id_str
                );
                let client = self.client.clone();

                async move {
                    match client.get(&url).send().await {
                        Ok(resp) if resp.status() == 404 || resp.status() == 500 => Ok(None),
                        Ok(resp) if resp.status().is_success() => {
                            let json: serde_json::Value = resp.json().await?;

                            let status =
                                json.get("status").and_then(|s| s.as_str()).ok_or_else(|| {
                                    Error::BadResponse("Missing status field".to_string())
                                })?;

                            match status {
                                "accepted" => Ok(Some(())),
                                "rejected" => Err(Error::TransactionRejected {
                                    tx_id: tx_id_str,
                                    reason: json.to_string(),
                                }),
                                _ => Ok(None),
                            }
                        }
                        Ok(resp) => {
                            let status = resp.status().as_u16();
                            let message = resp
                                .text()
                                .await
                                .unwrap_or_else(|_| "Unknown error".to_string());
                            Err(Error::ApiError { status, message })
                        }
                        Err(e) => Err(Error::Middleware(e)),
                    }
                }
            },
            self.confirmation_timeout,
            Duration::from_secs(1),
        )
        .await
        .map_err(|_| Error::TransactionTimeout(tx_id_owned))
    }
}
