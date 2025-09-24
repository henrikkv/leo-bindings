use anyhow::anyhow;
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
// Copyright (C) 2019-2025 Provable Inc.
// This file is part of the Leo library.

// The Leo library is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The Leo library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with the Leo library. If not, see <https://www.gnu.org/licenses/>.

use leo_package::{Package, ProgramData};
use leo_span::Symbol;

use indexmap::IndexSet;
use std::path::PathBuf;
use walkdir::WalkDir;

/// Collects paths to Leo source files for each program in the package.
///
/// For each non-test program, it searches the `src` directory for `.leo` files.
/// It separates the `main.leo` file from the rest and returns a tuple:
/// (`main.leo` path, list of other `.leo` file paths).
/// Test programs are included with an empty list of additional files.
/// Programs with bytecode data are ignored.
///
/// # Arguments
/// * `package` - Reference to the package containing programs.
///
/// # Returns
/// A vector of tuples with the main file and other source files.
pub fn collect_leo_paths(package: &Package) -> Vec<(PathBuf, Vec<PathBuf>)> {
    let mut partitioned_leo_paths = Vec::new();
    for program in &package.programs {
        match &program.data {
            ProgramData::SourcePath { directory, source } => {
                if program.is_test {
                    partitioned_leo_paths.push((source.clone(), vec![]));
                } else {
                    let src_dir = directory.join("src");
                    if !src_dir.exists() {
                        continue;
                    }

                    let mut all_files: Vec<PathBuf> = WalkDir::new(&src_dir)
                        .into_iter()
                        .filter_map(Result::ok)
                        .filter(|entry| {
                            entry.path().extension().and_then(|s| s.to_str()) == Some("leo")
                        })
                        .map(|entry| entry.into_path())
                        .collect();

                    if let Some(index) = all_files
                        .iter()
                        .position(|p| p.file_name().and_then(|s| s.to_str()) == Some("main.leo"))
                    {
                        let main = all_files.remove(index);
                        partitioned_leo_paths.push((main, all_files));
                    }
                }
            }
            ProgramData::Bytecode(..) => {}
        }
    }
    partitioned_leo_paths
}

/// Collects paths to `.aleo` files that are external (non-local) dependencies.
///
/// Scans the package's `imports` directory and filters out files that match
/// the names of local source-based dependencies.
/// Only retains `.aleo` files corresponding to true external dependencies.
///
/// # Arguments
/// * `package` - Reference to the package whose imports are being examined.
///
/// # Returns
/// A vector of paths to `.aleo` files not associated with local source dependencies.
pub fn collect_aleo_paths(package: &Package) -> Vec<PathBuf> {
    let local_dependency_symbols: IndexSet<Symbol> = package
        .programs
        .iter()
        .flat_map(|program| match &program.data {
            ProgramData::SourcePath { .. } => {
                // It's a local Leo dependency.
                Some(program.name)
            }
            ProgramData::Bytecode(..) => {
                // It's a network dependency or local .aleo dependency.
                None
            }
        })
        .collect();

    package
        .imports_directory()
        .read_dir()
        .ok()
        .into_iter()
        .flatten()
        .flat_map(|maybe_filename| maybe_filename.ok())
        .filter(|entry| {
            entry
                .file_type()
                .ok()
                .map(|filetype| filetype.is_file())
                .unwrap_or(false)
        })
        .flat_map(|entry| {
            let path = entry.path();
            if let Some(filename) = leo_package::filename_no_aleo_extension(&path) {
                let symbol = Symbol::intern(filename);
                if local_dependency_symbols.contains(&symbol) {
                    None
                } else {
                    Some(path)
                }
            } else {
                None
            }
        })
        .collect()
}
