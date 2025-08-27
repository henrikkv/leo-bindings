use anyhow::{bail, ensure};
use snarkvm::prelude::*;
use std::str::FromStr;

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
        Ok(Self { private_key: *private_key, view_key, address })
    }
}

impl<N: Network> FromStr for Account<N> {
    type Err = Error;

    fn from_str(private_key: &str) -> Result<Self, Self::Err> {
        Self::try_from(PrivateKey::from_str(private_key)?)
    }
}

pub fn get_public_balance<N: Network>(address: &Address<N>, endpoint: &str, network_path: &str) -> Result<u64, anyhow::Error> {
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
        Ok(Some(Value::Plaintext(Plaintext::Literal(Literal::<N>::U64(amount), _)))) => {
            Ok(*amount)
        }
        Ok(None) => Ok(0),
        Ok(Some(..)) => bail!("Failed to deserialize balance for {address}"),
        Err(err) => bail!("Failed to fetch balance for {address}: {err}"),
    }
}

pub fn broadcast_transaction<N: Network>(transaction: Transaction<N>, endpoint: &str, network_path: &str) -> Result<String, anyhow::Error> {
    let transaction_id = transaction.id();
    ensure!(
        !transaction.is_fee(),
        "The transaction is a fee transaction and cannot be broadcast"
    );
    
    match ureq::post(&format!("{}/{}/transaction/broadcast", endpoint, network_path)).send_json(&transaction)
    {
        Ok(id) => {
            let response_string = id.into_string()?.trim_matches('\"').to_string();
            ensure!( response_string == transaction_id.to_string(), "The response does not match the transaction id. ({response_string} != {transaction_id})");
            println!(
                "⌛ Execution {transaction_id} has been broadcast to {}.",
                endpoint
            );
            
            Ok(response_string)
        }
        Err(error) => {
            let error_message = match error {
                ureq::Error::Status(code, response) => {
                    format!("(status code {code}: {:?})", response.into_string().unwrap_or_default())
                }
                ureq::Error::Transport(err) => format!("({err})"),
            };
            bail!(
                "❌ Failed to broadcast execution to {}: {}",
                endpoint,
                error_message
            )
        }
    }
}
pub fn wait_for_program_availability(
    program_id: &str,
    endpoint: &str,
    timeout_secs: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "🔄 Test: Waiting for program '{}' to become available via API...",
        program_id
    );
    let start = std::time::Instant::now();

    loop {
        if start.elapsed().as_secs() > timeout_secs {
            return Err(format!("Timeout waiting for program {}", program_id).into());
        }

        let response = ureq::get(&format!("{}/testnet/program/{}", endpoint, program_id)).call();
        match response {
            Ok(_) => {
                println!("✅ Test: Program '{}' is now available via API", program_id);
                return Ok(());
            }
            Err(_) => {
                if start.elapsed().as_secs() % 5 == 0 {
                    println!(
                        "⏳ Test: Still waiting for program availability... ({}/{})",
                        start.elapsed().as_secs(),
                        timeout_secs
                    );
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }
}

