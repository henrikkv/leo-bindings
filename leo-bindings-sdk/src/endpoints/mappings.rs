use crate::config::Client;
use crate::error::{Error, Result};
use snarkvm::prelude::{Address, Literal, Network, Plaintext, Value};

impl Client {
    /// Query a mapping value from the network
    ///
    /// GET /{network}/program/{program}/mapping/{mapping}/{key}
    ///
    pub async fn mapping<N: Network>(
        &self,
        program_id: &str,
        mapping_name: &str,
        key: &Value<N>,
    ) -> Result<Option<Value<N>>> {
        let key_str = key.to_string().replace("\"", "");

        let url = format!(
            "{}/v2/{}/program/{}/mapping/{}/{}",
            self.endpoint,
            N::SHORT_NAME,
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

    pub async fn public_balance<N: Network>(&self, address: &Address<N>) -> Result<u64> {
        let key = Value::from(Literal::Address(*address));
        let balance = self.mapping::<N>("credits.aleo", "account", &key).await?;

        match balance {
            Some(Value::Plaintext(Plaintext::Literal(Literal::U64(amount), _))) => Ok(*amount),
            None => Ok(0),
            Some(other) => Err(Error::BadResponse(format!(
                "Unexpected balance format: {:?}",
                other
            ))),
        }
    }
}
