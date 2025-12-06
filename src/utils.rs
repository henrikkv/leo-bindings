use anyhow::{Result, anyhow};
use env_logger::{Builder, Env};
use rand::SeedableRng;
use rand_chacha::ChaChaRng;
use serde::Serialize;
use snarkvm::prelude::*;
use std::env;
use std::str::FromStr;
use std::thread::sleep;
use std::time::{Duration, Instant};

/// A helper struct for an Aleo account (from snarkOS).
#[derive(Clone, Debug)]
pub struct Account<N: Network> {
    private_key: PrivateKey<N>,
    view_key: ViewKey<N>,
    address: Address<N>,
}

impl<N: Network> Account<N> {
    pub fn new<R: Rng + CryptoRng>(rng: &mut R) -> Result<Self> {
        Self::try_from(PrivateKey::new(rng)?)
    }

    pub const fn private_key(&self) -> &PrivateKey<N> {
        &self.private_key
    }

    pub const fn view_key(&self) -> &ViewKey<N> {
        &self.view_key
    }

    pub const fn address(&self) -> Address<N> {
        self.address
    }
}

impl<N: Network> TryFrom<PrivateKey<N>> for Account<N> {
    type Error = Error;

    fn try_from(private_key: PrivateKey<N>) -> Result<Self, Self::Error> {
        Self::try_from(&private_key)
    }
}

impl<N: Network> TryFrom<&PrivateKey<N>> for Account<N> {
    type Error = Error;

    fn try_from(private_key: &PrivateKey<N>) -> Result<Self, Self::Error> {
        let view_key = ViewKey::try_from(private_key)?;
        let address = view_key.to_address();
        Ok(Self {
            private_key: *private_key,
            view_key,
            address,
        })
    }
}

impl<N: Network> FromStr for Account<N> {
    type Err = Error;

    fn from_str(private_key: &str) -> Result<Self, Self::Err> {
        Self::try_from(PrivateKey::from_str(private_key)?)
    }
}

pub fn get_public_balance<N: Network>(
    address: &Address<N>,
    endpoint: &str,
    network_path: &str,
) -> u64 {
    let credits = ProgramID::<N>::from_str("credits.aleo").unwrap();
    let account_mapping = Identifier::<N>::from_str("account").unwrap();

    let response = ureq::get(&format!(
        "{endpoint}/{network_path}/program/{credits}/mapping/{account_mapping}/{address}"
    ))
    .call();

    let balance: Option<Value<N>> = match response {
        Ok(mut response) => {
            let json_text = response.body_mut().read_to_string().unwrap();
            serde_json::from_str::<Option<Value<N>>>(&json_text).unwrap()
        }
        Err(err) => panic!("{}", err),
    };

    match balance {
        Some(Value::Plaintext(Plaintext::Literal(Literal::<N>::U64(amount), _))) => *amount,
        None => 0,
        Some(..) => panic!("Failed to deserialize balance for {address}"),
    }
}

pub fn get_development_key<N: Network>(index: u16) -> Result<PrivateKey<N>> {
    let mut rng = ChaChaRng::seed_from_u64(1234567890u64);
    for _ in 0..index {
        let _ = PrivateKey::<N>::new(&mut rng)?;
    }

    PrivateKey::<N>::new(&mut rng)
}

pub fn get_dev_account<N: Network>(index: u16) -> Result<Account<N>> {
    if index > 3 {
        return Err(anyhow!(
            "Development account index must be 0-3, got {}",
            index
        ));
    }
    let private_key = get_development_key(index)?;
    Account::try_from(private_key).map_err(|e| anyhow!("Failed to create account: {}", e))
}

pub fn get_account_from_env<N: Network>() -> Result<Account<N>> {
    dotenvy::dotenv().ok();
    let private_key_str =
        env::var("PRIVATE_KEY").map_err(|_| anyhow!("PRIVATE_KEY environment variable not set"))?;
    let private_key = PrivateKey::<N>::from_str(&private_key_str)
        .map_err(|e| anyhow!("Failed to parse PRIVATE_KEY: {}", e))?;
    Account::try_from(private_key).map_err(|e| anyhow!("Failed to create account: {}", e))
}

pub fn broadcast_transaction<N: Network>(
    transaction: Transaction<N>,
    endpoint: &str,
    network_path: &str,
) -> Result<(), anyhow::Error> {
    let url = format!("{endpoint}/{network_path}/transaction/broadcast");
    let mut request = ureq::post(&url);
    dotenvy::dotenv().ok();
    if let Ok(api_key) = env::var("PROVABLE_API_KEY") {
        request = request.header("X-Provable-API-Key", &api_key);
    }
    request
        .send_json(&transaction)
        .map(|_| ())
        .map_err(|error| anyhow!("Failed to broadcast transaction {error}"))
}

pub fn wait_for_transaction_confirmation<N: Network>(
    transaction_id: &N::TransactionID,
    endpoint: &str,
    network_path: &str,
    timeout_secs: u64,
) -> Result<(), anyhow::Error> {
    let start_time = Instant::now();
    loop {
        if start_time.elapsed() > Duration::from_secs(timeout_secs) {
            return Err(anyhow!("Transaction timeout after {timeout_secs} seconds"));
        }
        let url = &format!("{endpoint}/{network_path}/transaction/confirmed/{transaction_id}");
        match ureq::get(url).call() {
            Ok(mut response) => {
                let json_text = response.body_mut().read_to_string().unwrap();
                let json: serde_json::Value = serde_json::from_str(&json_text).unwrap();
                let status = json.get("status").and_then(|s| s.as_str()).unwrap();
                match status {
                    "accepted" => return Ok(()),
                    "rejected" => return Err(anyhow!("Transaction rejected: {json}")),
                    _ => return Err(anyhow!("Unexpected status '{status}': {json}")),
                }
            }
            Err(ureq::Error::StatusCode(500)) => {
                sleep(Duration::from_secs(1));
            }
            Err(ureq::Error::StatusCode(404)) => {
                sleep(Duration::from_secs(2));
            }
            Err(e) => {
                return Err(anyhow!("Failed to fetch transaction status: {}", e));
            }
        }
    }
}

pub fn wait_for_program_availability(
    program_id: &str,
    endpoint: &str,
    network_path: &str,
    timeout_secs: u64,
) -> Result<(), anyhow::Error> {
    let start_time = Instant::now();
    loop {
        if start_time.elapsed() > Duration::from_secs(timeout_secs) {
            return Err(anyhow!("Timeout waiting for program {program_id}"));
        }
        match ureq::get(&format!("{endpoint}/{network_path}/program/{program_id}")).call() {
            Ok(_) => return Ok(()),
            Err(_) => sleep(Duration::from_secs(1)),
        }
    }
}
#[derive(Debug, Clone)]
pub struct DelegatedProvingConfig {
    pub api_key: String,
    pub endpoint: String,
    pub enabled: bool,
}

impl DelegatedProvingConfig {
    pub fn new(api_key: &str, endpoint: Option<&str>) -> Self {
        Self {
            api_key: api_key.to_string(),
            endpoint: endpoint
                .map(|s| s.to_string())
                .unwrap_or_else(|| "https://api.explorer.provable.com".to_string()),
            enabled: false,
        }
    }
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();
        let api_key =
            env::var("PROVABLE_API_KEY").map_err(|_| anyhow!("PROVABLE_API_KEY not set"))?;
        let endpoint = env::var("PROVABLE_ENDPOINT")
            .unwrap_or_else(|_| "https://api.explorer.provable.com".to_string());

        Ok(Self {
            api_key,
            endpoint,
            enabled: false,
        })
    }

    pub fn enabled(mut self) -> Self {
        self.enabled = true;
        self
    }
}

#[derive(Debug, Serialize)]
struct ProvingRequest {
    authorization: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    fee_authorization: Option<serde_json::Value>,
    broadcast: bool,
}

pub fn execute_with_delegated_proving<N: Network>(
    config: &DelegatedProvingConfig,
    authorization: Authorization<N>,
) -> Result<Transaction<N>> {
    let authorization_json = serde_json::to_value(&authorization)
        .map_err(|e| anyhow!("Failed to serialize authorization: {}", e))?;

    let proving_request = ProvingRequest {
        authorization: authorization_json,
        fee_authorization: None,
        broadcast: false,
    };

    let url = format!("{}/v2/{}/prove", config.endpoint, N::SHORT_NAME);

    log::info!("Sending proving request to {}", url);
    log::debug!(
        "Proving request payload: {}",
        serde_json::to_string_pretty(&proving_request)
            .unwrap_or_else(|_| "Failed to serialize".to_string())
    );

    let response = ureq::post(&url)
        .header("X-Provable-API-Key", &config.api_key)
        .header("X-ALEO-METHOD", "submitProvingRequest")
        .header("Content-Type", "application/json")
        .send_json(&proving_request);

    let mut response = match response {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(status)) => {
            return Err(anyhow!("Request failed with status {}", status,));
        }
        Err(e) => {
            return Err(anyhow!("Failed to send request: {}", e));
        }
    };

    let response_text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| anyhow!("Failed to read response body: {}", e))?;

    let response_json: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| anyhow!("Failed to parse response as JSON: {}", e,))?;

    let transaction_value = response_json
        .get("transaction")
        .ok_or_else(|| anyhow!("'transaction' field missing: {}", response_text))?;

    let transaction_str = serde_json::to_string(transaction_value)
        .map_err(|e| anyhow!("Failed to serialize transaction: {}", e))?;

    let transaction: Transaction<N> = serde_json::from_str(&transaction_str)
        .map_err(|e| anyhow!("Failed to deserialize transaction: {}", e))?;

    log::info!("✅ Received proved transaction: {}", transaction.id());

    Ok(transaction)
}

pub fn extract_outputs_from_authorization<N: Network>(
    authorization: &Authorization<N>,
    view_key: &ViewKey<N>,
) -> Result<Vec<Value<N>>> {
    let request = authorization.peek_next()?;
    let function_id = snarkvm::console::program::compute_function_id(
        request.network_id(),
        request.program_id(),
        request.function_name(),
    )?;
    let num_inputs = request.inputs().len();

    let transitions = authorization.transitions();
    let main_transition = transitions
        .values()
        .last()
        .ok_or_else(|| anyhow!("Authorization contains no transitions"))?;

    main_transition
        .outputs()
        .iter()
        .enumerate()
        .map(|(i, output)| {
            decrypt_output(output, i, num_inputs, function_id, request.tvk(), view_key)
        })
        .collect()
}

fn decrypt_output<N: Network>(
    output: &snarkvm::ledger::block::Output<N>,
    output_index: usize,
    num_inputs: usize,
    function_id: Field<N>,
    tvk: &Field<N>,
    view_key: &ViewKey<N>,
) -> Result<Value<N>> {
    use snarkvm::ledger::block::Output;

    match output {
        Output::Constant(_, Some(plaintext)) | Output::Public(_, Some(plaintext)) => {
            Ok(Value::Plaintext(plaintext.clone()))
        }
        Output::Private(_, Some(ciphertext)) => {
            let index = Field::from_u16(u16::try_from(num_inputs + output_index)?);
            let output_view_key = N::hash_psd4(&[function_id, *tvk, index])?;
            let plaintext = ciphertext.decrypt_symmetric(output_view_key)?;
            Ok(Value::Plaintext(plaintext))
        }
        Output::Record(_, _, Some(record_ciphertext), _) => {
            let record_plaintext = record_ciphertext.decrypt(view_key)?;
            Ok(Value::Record(record_plaintext))
        }
        Output::Future(_, Some(future)) => Ok(Value::Future(future.clone())),
        Output::ExternalRecord(_) => Err(anyhow!("External record outputs are not supported")),
        _ => Err(anyhow!("Output value is missing from transition")),
    }
}

pub fn init_simple_logger() {
    use std::io::Write;
    let _ = Builder::from_env(Env::default().filter_or("RUST_LOG", "info"))
        .format(|buf, record| writeln!(buf, "{}", record.args()))
        .try_init();
}

pub fn init_test_logger() {
    use std::io::Write;
    let _ = Builder::from_env(Env::default().filter_or("RUST_LOG", "info"))
        .is_test(true)
        .format(|buf, record| writeln!(buf, "{}", record.args()))
        .try_init();
}

fn fetch_latest_block_height(endpoint: &str, network_path: &str) -> Result<u32> {
    let url = format!("{}/{}/block/height/latest", endpoint, network_path);
    let mut response = ureq::get(&url)
        .call()
        .map_err(|e| anyhow!("Failed to fetch latest block height: {}", e))?;
    let height_str = response
        .body_mut()
        .read_to_string()
        .map_err(|e| anyhow!("Failed to read response: {}", e))?;
    u32::from_str(&height_str).map_err(|e| anyhow!("Failed to parse block height: {}", e))
}

pub fn fetch_mapping_value(
    url: &str,
) -> Result<Option<String>> {
    let mut retries = 0;
    let max_retries = 3;

    loop {
        match ureq::get(url).call() {
            Ok(mut response) => {
                let json_text = response.body_mut().read_to_string()?;
                return Ok(Some(json_text));
            }
            Err(ureq::Error::StatusCode(404)) => {
                return Ok(None);
            }
            Err(ureq::Error::StatusCode(522)) | Err(ureq::Error::StatusCode(500)) => {
                if retries >= max_retries {
                    return Err(anyhow!(
                        "Failed to fetch mapping value after {} tries",
                        max_retries
                    ));
                }
                let backoff_ms = 100 * (2_u64.pow(retries));
                log::warn!(
                    "Server error fetching mapping (attempt {}/{}). Retrying in {}ms...",
                    retries + 1,
                    max_retries + 1,
                    backoff_ms
                );
                sleep(Duration::from_millis(backoff_ms));
                retries += 1;
            }
            Err(e) => {
                if retries >= max_retries {
                    return Err(anyhow!("Failed to fetch mapping value after {} tries", e));
                }
                let backoff_ms = 100 * (2_u64.pow(retries));
                log::warn!(
                    "Error fetching mapping (attempt {}/{}): {}. Retrying in {}ms...",
                    retries + 1,
                    max_retries + 1,
                    e,
                    backoff_ms
                );
                sleep(Duration::from_millis(backoff_ms));
                retries += 1;
            }
        }
    }
}
