use crate::config::ClientBuilder;
use crate::error::{Error, Result};
use reqwest_middleware::ClientWithMiddleware;
use snarkvm::prelude::Network;
use std::marker::PhantomData;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub(crate) struct JwtToken {
    token: String,
    expires_at: u64,
}

pub struct ProvableClient<N: Network> {
    pub(crate) client: ClientWithMiddleware,
    pub(crate) endpoint: String,
    pub(crate) consumer_id: Option<String>,
    pub(crate) api_key: Option<String>,
    pub(crate) jwt_token: RwLock<Option<JwtToken>>,
    pub(crate) confirmation_timeout: Duration,
    pub(crate) program_availability_timeout: Duration,
    pub(crate) _network: PhantomData<N>,
}

impl<N: Network> ProvableClient<N> {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self::builder()
            .endpoint(endpoint)
            .build()
            .expect("Failed to build client")
    }

    pub fn with_jwt_credentials(
        endpoint: impl Into<String>,
        consumer_id: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        Self::builder()
            .endpoint(endpoint)
            .consumer_id(consumer_id)
            .api_key(api_key)
            .build()
            .expect("Failed to build client")
    }

    pub fn builder() -> ClientBuilder<N> {
        ClientBuilder::new()
    }

    pub(crate) fn from_builder(builder: ClientBuilder<N>) -> Result<Self> {
        builder.validate()?;
        let config = builder.get_config();
        let client = builder.build_http_client()?;

        Ok(Self {
            client,
            endpoint: config.endpoint,
            consumer_id: config.consumer_id,
            api_key: config.api_key,
            jwt_token: RwLock::new(None),
            confirmation_timeout: config.confirmation_timeout,
            program_availability_timeout: config.program_availability_timeout,
            _network: PhantomData,
        })
    }

    pub(crate) fn network_name(&self) -> &str {
        N::SHORT_NAME
    }

    async fn fetch_jwt(&self) -> Result<JwtToken> {
        let consumer_id = self
            .consumer_id
            .as_ref()
            .ok_or(Error::JwtCredentialsRequired)?;
        let api_key = self.api_key.as_ref().ok_or(Error::JwtCredentialsRequired)?;

        let url = format!("https://api.provable.com/jwts/{}", consumer_id);

        let response = self
            .client
            .post(&url)
            .header("X-Provable-API-Key", api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to fetch JWT token".to_string());
            return Err(Error::JwtFetchFailed { status, message });
        }

        let auth_header = response
            .headers()
            .get("Authorization")
            .ok_or_else(|| Error::BadResponse("Missing Authorization header".to_string()))?
            .to_str()
            .map_err(|_| Error::BadResponse("Invalid Authorization header".to_string()))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| Error::BadResponse("Invalid Authorization header format".to_string()))?
            .to_string();

        let body_text = response
            .text()
            .await
            .map_err(|e| Error::BadResponse(format!("Failed to read response body: {}", e)))?;

        let body_json: serde_json::Value = serde_json::from_str(&body_text)
            .map_err(|e| Error::BadResponse(format!("Failed to parse response body: {}", e)))?;

        let expires_at = body_json
            .get("exp")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| {
                Error::BadResponse("Missing or invalid 'exp' field in response".to_string())
            })?;

        Ok(JwtToken { token, expires_at })
    }

    pub(crate) async fn get_valid_jwt_token(&self) -> Result<String> {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        {
            let token_guard = self.jwt_token.read().await;
            if let Some(token) = token_guard.as_ref()
                && token.expires_at > current_time + 300
            {
                return Ok(token.token.clone());
            }
        }
        let new_token = self.fetch_jwt().await?;
        let token_string = new_token.token.clone();
        {
            let mut token_guard = self.jwt_token.write().await;
            *token_guard = Some(new_token);
        }

        Ok(token_string)
    }
}
