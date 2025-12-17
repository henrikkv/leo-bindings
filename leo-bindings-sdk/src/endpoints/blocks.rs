use crate::client::ProvableClient;
use crate::error::{Error, Result};
use snarkvm::prelude::Network;

impl<N: Network> ProvableClient<N> {
    /// Get the latest block height
    ///
    /// GET /{network}/block/height/latest
    ///
    pub async fn height(&self) -> Result<u32> {
        let url = format!(
            "{}/v2/{}/block/height/latest",
            self.endpoint,
            self.network_name()
        );

        let response = self.client.get(&url).send().await?;

        if response.status().is_success() {
            let height_str = response.text().await?;
            height_str
                .trim()
                .parse()
                .map_err(|_| Error::BadResponse("Invalid block height format".to_string()))
        } else {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(Error::ApiError { status, message })
        }
    }

    /// Get a block by height
    ///
    /// GET /{network}/block/{height}
    ///
    pub async fn block(&self, height: u32) -> Result<String> {
        let url = format!(
            "{}/v2/{}/block/{}",
            self.endpoint,
            self.network_name(),
            height
        );

        let response = self.client.get(&url).send().await?;

        if response.status().is_success() {
            response.text().await.map_err(Error::Http)
        } else if response.status() == 404 {
            Err(Error::NotFound(format!("Block {} not found", height)))
        } else {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(Error::ApiError { status, message })
        }
    }
}
