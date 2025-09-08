use anyhow::{anyhow, bail};
use snarkvm::prelude::*;
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
) -> Result<u64, anyhow::Error> {
    let credits = ProgramID::<N>::from_str("credits.aleo")?;
    let account_mapping = Identifier::<N>::from_str("account")?;

    let response = ureq::get(&format!(
        "{endpoint}/{network_path}/program/{credits}/mapping/{account_mapping}/{address}"
    ))
    .call();

    let balance: Result<Option<Value<N>>, anyhow::Error> = match response {
        Ok(response) => response.into_json().map_err(|err| err.into()),
        Err(err) => match err {
            ureq::Error::Status(_status, response) => {
                bail!(response
                    .into_string()
                    .unwrap_or("Response too large!".to_owned()))
            }
            err => bail!(err),
        },
    };

    match balance {
        Ok(Some(Value::Plaintext(Plaintext::Literal(Literal::<N>::U64(amount), _)))) => Ok(*amount),
        Ok(None) => Ok(0),
        Ok(Some(..)) => bail!("Failed to deserialize balance for {address}"),
        Err(err) => bail!("Failed to fetch balance for {address}: {err}"),
    }
}

pub fn broadcast_transaction<N: Network>(
    transaction: Transaction<N>,
    endpoint: &str,
    network_path: &str,
) -> Result<(), anyhow::Error> {
    ureq::post(&format!("{endpoint}/{network_path}/transaction/broadcast"))
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
            Ok(response) => {
                if let Ok(json) = response.into_json::<serde_json::Value>() {
                    if let Some(status) = json.get("status").and_then(|s| s.as_str()) {
                        match status {
                            "accepted" => return Ok(()),
                            "rejected" => return Err(anyhow!("❌ Transaction rejected: {json}")),
                            _ => return Err(anyhow!("⚠️ Status '{status}': {json}")),
                        }
                    }
                }
            }
            Err(_) => {
                sleep(Duration::from_secs(1));
            }
        }
    }
}

pub fn wait_for_program_availability(
    program_id: &str,
    endpoint: &str,
    timeout_secs: u64,
) -> Result<(), anyhow::Error> {
    let start_time = Instant::now();
    loop {
        if start_time.elapsed() > Duration::from_secs(timeout_secs) {
            return Err(anyhow!("Timeout waiting for program {program_id}"));
        }
        match ureq::get(&format!("{endpoint}/testnet/program/{program_id}")).call() {
            Ok(_) => return Ok(()),
            Err(_) => sleep(Duration::from_secs(1)),
        }
    }
}
