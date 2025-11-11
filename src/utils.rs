use anyhow::{Result, anyhow};
use rand::SeedableRng;
use rand_chacha::ChaChaRng;
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
            Ok(mut response) => {
                let json_text = response.body_mut().read_to_string().unwrap();
                let json: serde_json::Value = serde_json::from_str(&json_text).unwrap();
                let status = json.get("status").and_then(|s| s.as_str()).unwrap();
                match status {
                    "accepted" => return Ok(()),
                    "rejected" => panic!("❌ Transaction rejected: {json}"),
                    _ => panic!("⚠️ Status '{status}': {json}"),
                }
            }
            Err(ureq::Error::StatusCode(500)) => {
                sleep(Duration::from_secs(1));
            }
            Err(e) => panic!("❌ Error fetching transaction: {}", e),
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
