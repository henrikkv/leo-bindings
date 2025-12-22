use rand_chacha::ChaChaRng;
use rand_chacha::rand_core::SeedableRng;
use snarkvm::prelude::*;
use std::env;
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

impl<N: Network> Account<N> {
    pub fn from_env() -> Result<Account<N>> {
        dotenvy::dotenv().ok();
        let private_key_str = env::var("PRIVATE_KEY")
            .map_err(|_| anyhow!("PRIVATE_KEY environment variable not set"))?;
        let private_key = PrivateKey::<N>::from_str(&private_key_str)
            .map_err(|e| anyhow!("Failed to parse PRIVATE_KEY: {}", e))?;
        Account::try_from(private_key).map_err(|e| anyhow!("Failed to create account: {}", e))
    }

    pub fn dev_account(index: u16) -> Result<Account<N>> {
        if index > 3 {
            return Err(anyhow!(
                "Development account index must be 0-3, got {}",
                index
            ));
        }
        let private_key = Account::dev_private_key(index)?;
        Account::try_from(private_key).map_err(|e| anyhow!("Failed to create account: {}", e))
    }

    fn dev_private_key(index: u16) -> Result<PrivateKey<N>> {
        let mut rng = ChaChaRng::seed_from_u64(1234567890u64);
        for _ in 0..index {
            let _ = PrivateKey::<N>::new(&mut rng)?;
        }

        PrivateKey::new(&mut rng)
    }
}
