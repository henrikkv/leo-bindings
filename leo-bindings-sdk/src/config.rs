use crate::error::{Error, Result};
use reqwest_middleware::{ClientBuilder as MiddlewareClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use std::marker::PhantomData;
use std::time::Duration;

#[derive(Debug, Clone)]
pub(crate) struct ClientConfig {
    pub endpoint: String,
    pub consumer_id: Option<String>,
    pub api_key: Option<String>,
    pub timeout: Duration,
    pub confirmation_timeout: Duration,
    pub program_availability_timeout: Duration,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            consumer_id: None,
            api_key: None,
            timeout: Duration::from_secs(30),
            confirmation_timeout: Duration::from_secs(120),
            program_availability_timeout: Duration::from_secs(60),
        }
    }
}

pub struct ClientBuilder<N> {
    config: ClientConfig,
    _network: PhantomData<N>,
}

impl<N> Default for ClientBuilder<N> {
    fn default() -> Self {
        Self {
            config: ClientConfig::default(),
            _network: PhantomData,
        }
    }
}

impl<N: snarkvm::prelude::Network> ClientBuilder<N> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.config.endpoint = endpoint.into();
        self
    }

    pub fn consumer_id(mut self, consumer_id: impl Into<String>) -> Self {
        self.config.consumer_id = Some(consumer_id.into());
        self
    }

    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.config.api_key = Some(api_key.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    pub fn confirmation_timeout(mut self, timeout: Duration) -> Self {
        self.config.confirmation_timeout = timeout;
        self
    }

    pub fn program_timeout(mut self, timeout: Duration) -> Self {
        self.config.program_availability_timeout = timeout;
        self
    }

    pub(crate) fn build_http_client(&self) -> Result<ClientWithMiddleware> {
        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_millis(500), Duration::from_secs(10))
            .build_with_max_retries(3);

        let reqwest_client = reqwest::Client::builder()
            .timeout(self.config.timeout)
            .build()
            .map_err(Error::Http)?;

        let client = MiddlewareClientBuilder::new(reqwest_client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Ok(client)
    }

    pub(crate) fn get_config(&self) -> ClientConfig {
        self.config.clone()
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.config.endpoint.is_empty() {
            return Err(Error::Config("Endpoint is required".to_string()));
        }

        if self.config.consumer_id.is_some() != self.config.api_key.is_some() {
            return Err(Error::Config("consumer_id and api_key not set".to_string()));
        }

        Ok(())
    }

    pub fn build(self) -> Result<crate::client::ProvableClient<N>> {
        crate::client::ProvableClient::from_builder(self)
    }
}
