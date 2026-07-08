use crate::config::Client;
use crate::error::{Error, Result};
use snarkvm::prelude::{Network, Value};

impl Client {
    /// Evaluate a view function against the latest network state.
    ///
    /// POST /{network}/program/{program}/view/{function}
    ///
    pub async fn evaluate_view<N: Network>(
        &self,
        program_id: &str,
        view_name: &str,
        inputs: &[Value<N>],
    ) -> Result<Vec<Value<N>>> {
        let url = format!(
            "{}/v2/{}/program/{}/view/{}",
            self.endpoint,
            N::SHORT_NAME,
            program_id,
            view_name
        );

        let input_strings: Vec<String> = inputs.iter().map(|v| v.to_string()).collect();

        let response = self.client.post(&url).json(&input_strings).send().await?;

        if response.status().is_success() {
            let output_strings: Vec<String> = response.json().await?;
            output_strings
                .into_iter()
                .map(|s| {
                    s.parse::<Value<N>>().map_err(|e| {
                        Error::Other(format!("Failed to parse view output '{s}': {e}"))
                    })
                })
                .collect()
        } else {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(Error::Other(format!("API error {status}: {message}")))
        }
    }
}
