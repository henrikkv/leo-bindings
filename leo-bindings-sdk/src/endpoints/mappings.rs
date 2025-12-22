use crate::config::Client;
use crate::error::{Error, Result};
use snarkvm::prelude::{Network, Value};

impl<N: Network> Client<N> {
    /// Query a mapping value from the network
    ///
    /// GET /{network}/program/{program}/mapping/{mapping}/{key}
    ///
    pub async fn mapping(
        &self,
        program_id: &str,
        mapping_name: &str,
        key: &Value<N>,
    ) -> Result<Option<Value<N>>> {
        let key_str = key.to_string().replace("\"", "");

        let url = format!(
            "{}/v2/{}/program/{}/mapping/{}/{}",
            self.endpoint,
            self.network_name(),
            program_id,
            mapping_name,
            key_str
        );

        let response = self.client.get(&url).send().await?;

        if response.status().is_success() {
            let json_text = response.text().await?;

            let value_opt: Option<Value<N>> = serde_json::from_str(&json_text)?;
            Ok(value_opt)
        } else if response.status() == 404 {
            Ok(None)
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
