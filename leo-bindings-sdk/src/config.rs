use crate::error::{Error, Result};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use snarkvm::prelude::Network;
use std::marker::PhantomData;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
struct JwtToken {
    token: String,
    expires_at: u64,
}

pub struct Credentials {
    consumer_id: String,
    api_key: String,
    jwt_token: RwLock<Option<JwtToken>>,
}

impl Credentials {
    pub fn new(consumer_id: &str, api_key: &str) -> Self {
        Self {
            consumer_id: consumer_id.to_string(),
            api_key: api_key.to_string(),
            jwt_token: RwLock::new(None),
        }
    }

    async fn fetch_jwt(&self, client: &ClientWithMiddleware) -> Result<JwtToken> {
        let url = format!("https://api.provable.com/jwts/{}", self.consumer_id);

        let response = client
            .post(&url)
            .header("X-Provable-API-Key", &self.api_key)
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

    pub(crate) async fn get_valid_jwt_token(
        &self,
        client: &ClientWithMiddleware,
    ) -> Result<String> {
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

        let new_token = self.fetch_jwt(client).await?;
        let token_string = new_token.token.clone();
        {
            let mut token_guard = self.jwt_token.write().await;
            *token_guard = Some(new_token);
        }

        Ok(token_string)
    }
}

pub struct Client<N: Network> {
    pub(crate) client: ClientWithMiddleware,
    pub(crate) endpoint: String,
    pub(crate) credentials: Option<Credentials>,
    pub(crate) _network: PhantomData<N>,
}

impl<N: Network> Client<N> {
    pub fn new(endpoint: &str, credentials: Option<Credentials>) -> Result<Self> {
        if endpoint.is_empty() {
            return Err(Error::Config("Endpoint is required".to_string()));
        }

        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_millis(500), Duration::from_secs(10))
            .build_with_max_retries(3);

        let reqwest_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(Error::Http)?;

        let client = ClientBuilder::new(reqwest_client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Ok(Self {
            client,
            endpoint: endpoint.to_string(),
            credentials,
            _network: PhantomData,
        })
    }

    pub fn from_env() -> Result<Self> {
        let _ = dotenvy::dotenv();

        let endpoint = std::env::var("ENDPOINT")
            .map_err(|_| Error::Config("ENDPOINT environment variable required".to_string()))?;

        let consumer_id = std::env::var("PROVABLE_CONSUMER_ID").ok();
        let api_key = std::env::var("PROVABLE_API_KEY").ok();

        let credentials = match (consumer_id, api_key) {
            (Some(cid), Some(key)) => Some(Credentials::new(&cid, &key)),
            (None, None) => None,
            _ => {
                return Err(Error::Config(
                    "Set PROVABLE_CONSUMER_ID and PROVABLE_API_KEY together".to_string(),
                ));
            }
        };

        Self::new(&endpoint, credentials)
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn has_credentials(&self) -> bool {
        self.credentials.is_some()
    }

    pub(crate) fn network_name(&self) -> &str {
        N::SHORT_NAME
    }

    pub(crate) async fn get_valid_jwt_token(&self) -> Result<String> {
        let credentials = self
            .credentials
            .as_ref()
            .ok_or(Error::JwtCredentialsRequired)?;

        credentials.get_valid_jwt_token(&self.client).await
    }
}
