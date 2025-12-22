use crate::config::Client;
use crate::error::{Error, Result};
use crate::utils::poll_until;
use snarkvm::prelude::Network;
use std::time::Duration;

impl<N: Network> Client<N> {
    /// Fetch a program's bytecode from the network
    ///
    /// GET /{network}/program/{id}
    ///
    pub async fn program(&self, program_id: &str) -> Result<String> {
        let url = format!(
            "{}/v2/{}/program/{}",
            self.endpoint,
            self.network_name(),
            program_id
        );

        let response = self.client.get(&url).send().await?;

        if response.status().is_success() {
            let json: serde_json::Value = response.json().await?;
            json.as_str()
                .ok_or_else(|| Error::BadResponse("Expected string program".to_string()))
                .map(|s| s.to_string())
        } else if response.status() == 404 {
            Err(Error::NotFound(format!("Program {} not found", program_id)))
        } else {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(Error::ApiError { status, message })
        }
    }

    /// Check if a program exists on the network
    ///
    /// GET /{network}/program/{id} (checking for 404)
    ///
    pub async fn program_exists(&self, program_id: &str) -> Result<bool> {
        match self.program(program_id).await {
            Ok(_) => Ok(true),
            Err(Error::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn wait_for_program(&self, program_id: &str) -> Result<()> {
        let program_id_owned = program_id.to_string();

        poll_until(
            || {
                let program_id = program_id_owned.clone();
                let url = format!(
                    "{}/v2/{}/program/{}",
                    self.endpoint,
                    self.network_name(),
                    program_id
                );
                let client = self.client.clone();

                async move {
                    match client.get(&url).send().await {
                        Ok(resp) if resp.status().is_success() => Ok(Some(())),
                        Ok(resp) if resp.status() == 404 => Ok(None),
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
            Duration::from_secs(60),
            Duration::from_secs(1),
        )
        .await
        .map_err(|_| Error::ProgramTimeout(program_id.to_string()))
    }
}
