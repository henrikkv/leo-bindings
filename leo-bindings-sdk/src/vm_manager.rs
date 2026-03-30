use crate::account::Account;
use crate::config::Client;
use crate::error::{Error, Result};
use crate::stats::{print_deployment_stats, print_execution_stats};
use aleo_std::StorageMode;
use http::uri::Uri;
use snarkvm::ledger::block::Transaction;
use snarkvm::ledger::query::Query;
use snarkvm::ledger::store::ConsensusStore;
use snarkvm::ledger::store::helpers::memory::{BlockMemory, ConsensusMemory};
use snarkvm::prelude::*;
use snarkvm::synthesizer::VM;

pub const CONSENSUS_VERSION: ConsensusVersion = ConsensusVersion::V14;

#[derive(Clone)]
pub struct VMManager<N: Network> {
    vm: VM<N, ConsensusMemory<N>>,
    client: Client,
}

impl<N: Network> std::fmt::Debug for VMManager<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VMManager")
            .field("client", &self.client)
            .finish_non_exhaustive()
    }
}

impl<N: Network> VMManager<N> {
    pub fn new(client: &Client) -> Result<Self> {
        let store = ConsensusStore::<N, ConsensusMemory<N>>::open(StorageMode::Production)
            .map_err(|e| Error::Internal(format!("Failed to create consensus store: {}", e)))?;

        let vm =
            VM::from(store).map_err(|e| Error::Internal(format!("Failed to create VM: {}", e)))?;

        Ok(Self {
            vm,
            client: client.clone(),
        })
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn vm(&self) -> &VM<N, ConsensusMemory<N>> {
        &self.vm
    }

    pub async fn add_program(&self, program: &Program<N>) -> Result<()> {
        let program_id = program.id().to_string();
        let edition = self.client.program_edition::<N>(&program_id).await?;
        self.vm
            .process()
            .write()
            .add_program_with_edition(program, edition)
            .map_err(|e| Error::VmError(format!("Failed to add program '{}': {}", program.id(), e)))
    }

    pub fn contains_program(&self, program_id: &ProgramID<N>) -> bool {
        self.vm.process().read().contains_program(program_id)
    }

    pub async fn deploy(
        &self,
        private_key: &PrivateKey<N>,
        program: &Program<N>,
        priority_fee: u64,
        fee_record: Option<Record<N, Plaintext<N>>>,
    ) -> Result<Transaction<N>> {
        let vm = self.vm.clone();
        let endpoint = self.client.endpoint.clone();
        let private_key = *private_key;
        let program = program.clone();

        tokio::task::spawn_blocking(move || {
            let query = Self::create_query(&endpoint)?;
            let rng = &mut rand::thread_rng();
            let transaction = vm
                .deploy(
                    &private_key,
                    &program,
                    fee_record,
                    priority_fee,
                    Some(&query),
                    rng,
                )
                .map_err(|e| {
                    Error::VmError(format!("Failed to create deployment transaction: {}", e))
                })?;
            Ok(transaction)
        })
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
    }

    pub async fn execute(
        &self,
        private_key: &PrivateKey<N>,
        program_id: &str,
        function_name: &str,
        inputs: Vec<Value<N>>,
        fee_record: Option<Record<N, Plaintext<N>>>,
        priority_fee: u64,
    ) -> Result<(Transaction<N>, Vec<Value<N>>)> {
        let vm = self.vm.clone();
        let endpoint = self.client.endpoint.clone();
        let private_key = *private_key;
        let program_id: ProgramID<N> = program_id.parse()?;
        let function_name: Identifier<N> = function_name.parse()?;

        tokio::task::spawn_blocking(move || {
            let query = Self::create_query(&endpoint)?;
            let rng = &mut rand::thread_rng();
            let (transaction, response) = vm
                .execute_with_response(
                    &private_key,
                    (program_id, function_name),
                    inputs.iter(),
                    fee_record,
                    priority_fee,
                    Some(&query),
                    rng,
                )
                .map_err(|e| {
                    Error::VmError(format!("Failed to create execution transaction: {}", e))
                })?;

            let outputs = response.outputs().to_vec();

            Ok((transaction, outputs))
        })
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
    }

    pub async fn authorize(
        &self,
        private_key: &PrivateKey<N>,
        program_id: &str,
        function_name: &str,
        inputs: Vec<Value<N>>,
    ) -> Result<Authorization<N>> {
        let vm = self.vm.clone();
        let private_key = *private_key;

        let program_id: ProgramID<N> = program_id.parse()?;
        let function_name: Identifier<N> = function_name.parse()?;

        tokio::task::spawn_blocking(move || {
            let rng = &mut rand::thread_rng();
            let authorization = vm
                .authorize(&private_key, program_id, function_name, inputs.iter(), rng)
                .map_err(|e| Error::VmError(format!("Failed to create authorization: {}", e)))?;

            Ok(authorization)
        })
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
    }

    pub fn extract_outputs(
        &self,
        authorization: &Authorization<N>,
        view_key: &ViewKey<N>,
    ) -> Result<Vec<Value<N>>> {
        let request = authorization
            .peek_next()
            .map_err(|e| Error::VmError(format!("Failed to peek authorization: {}", e)))?;

        let function_id = snarkvm::console::program::compute_function_id(
            request.network_id(),
            request.program_id(),
            request.function_name(),
        )
        .map_err(|e| Error::VmError(format!("Failed to compute function ID: {}", e)))?;

        let num_inputs = request.inputs().len();

        let transitions = authorization.transitions();
        let main_transition = transitions
            .values()
            .last()
            .ok_or_else(|| Error::VmError("Authorization contains no transitions".to_string()))?;

        main_transition
            .outputs()
            .iter()
            .enumerate()
            .map(|(i, output)| {
                Self::decrypt_output(output, i, num_inputs, function_id, request.tvk(), view_key)
            })
            .collect()
    }

    fn decrypt_output(
        output: &snarkvm::ledger::block::Output<N>,
        output_index: usize,
        num_inputs: usize,
        function_id: Field<N>,
        tvk: &Field<N>,
        view_key: &ViewKey<N>,
    ) -> Result<Value<N>> {
        match output {
            Output::Constant(_, Some(plaintext)) | Output::Public(_, Some(plaintext)) => {
                Ok(Value::Plaintext(plaintext.clone()))
            }
            Output::Private(_, Some(ciphertext)) => {
                let index = Field::from_u16(
                    u16::try_from(num_inputs + output_index)
                        .map_err(|e| Error::Internal(format!("Index overflow: {}", e)))?,
                );
                let output_view_key = N::hash_psd4(&[function_id, *tvk, index]).map_err(|e| {
                    Error::VmError(format!("Failed to compute output view key: {}", e))
                })?;
                let plaintext = ciphertext.decrypt_symmetric(output_view_key).map_err(|e| {
                    Error::VmError(format!("Failed to decrypt private output: {}", e))
                })?;
                Ok(Value::Plaintext(plaintext))
            }
            Output::Record(_, _, Some(record_ciphertext), _) => {
                let record_plaintext = record_ciphertext
                    .decrypt(view_key)
                    .map_err(|e| Error::VmError(format!("Failed to decrypt record: {}", e)))?;
                Ok(Value::Record(record_plaintext))
            }
            Output::Future(_, Some(future)) => Ok(Value::Future(future.clone())),
            Output::ExternalRecord(_) => Err(Error::VmError(
                "External record outputs are not supported".to_string(),
            )),
            _ => Err(Error::VmError(
                "Output value is missing from transition".to_string(),
            )),
        }
    }

    pub fn calculate_cost(&self, transaction: &Transaction<N>) -> Result<(u64, (u64, u64))> {
        let execution = transaction
            .execution()
            .ok_or_else(|| Error::VmError("Transaction has no execution".to_string()))?;

        execution_cost(&self.vm.process().read(), execution, CONSENSUS_VERSION)
            .map_err(|e| Error::VmError(format!("Failed to calculate execution cost: {}", e)))
    }

    pub async fn ensure_program_loaded(
        &self,
        program_id: &str,
        dependencies: &[&str],
    ) -> Result<()> {
        let program_id_parsed: ProgramID<N> = program_id
            .parse()
            .map_err(|e| Error::Internal(format!("Invalid program ID '{}': {}", program_id, e)))?;

        if self.contains_program(&program_id_parsed) {
            return Ok(());
        }

        for dep_id in dependencies {
            let dep_program_id: ProgramID<N> = dep_id.parse().map_err(|e| {
                Error::Internal(format!("Invalid dependency ID '{}': {}", dep_id, e))
            })?;

            if !self.contains_program(&dep_program_id) {
                self.client
                    .wait_for_program::<N>(dep_id)
                    .await
                    .map_err(|e| {
                        Error::Internal(format!(
                            "Failed waiting for dependency '{}': {}",
                            dep_id, e
                        ))
                    })?;
                let bytecode = self.client.program::<N>(dep_id).await.map_err(|e| {
                    Error::Internal(format!("Failed to fetch dependency '{}': {}", dep_id, e))
                })?;
                let dep_program: Program<N> = bytecode.parse().map_err(|e| {
                    Error::Internal(format!("Failed to parse dependency '{}': {}", dep_id, e))
                })?;
                self.add_program(&dep_program).await.map_err(|e| {
                    Error::Internal(format!("Failed to add dependency '{}': {}", dep_id, e))
                })?;
            }
        }

        self.client
            .wait_for_program::<N>(program_id)
            .await
            .map_err(|e| {
                Error::Internal(format!(
                    "Failed waiting for program '{}': {}",
                    program_id, e
                ))
            })?;
        let bytecode = self.client.program::<N>(program_id).await.map_err(|e| {
            Error::Internal(format!("Failed to fetch program '{}': {}", program_id, e))
        })?;
        let program: Program<N> = bytecode.parse().map_err(|e| {
            Error::Internal(format!("Failed to parse program '{}': {}", program_id, e))
        })?;
        self.add_program(&program).await.map_err(|e| {
            Error::Internal(format!("Failed to add program '{}': {}", program_id, e))
        })?;

        Ok(())
    }

    pub async fn execute_and_broadcast(
        &self,
        account: &Account<N>,
        program_id: &str,
        function_name: &str,
        inputs: Vec<Value<N>>,
        dependencies: &[&str],
    ) -> Result<Vec<Value<N>>> {
        log::info!("Creating tx: {}.{}", program_id, function_name);

        self.ensure_program_loaded(program_id, dependencies).await?;

        let balance = self
            .client
            .public_balance::<N>(&account.address())
            .await
            .map_err(|e| Error::Internal(format!("Failed to get balance: {}", e)))?;

        let (transaction, function_outputs): (Transaction<N>, Vec<Value<N>>) = if self
            .client
            .has_credentials()
        {
            let auth = self
                .authorize(
                    account.private_key(),
                    program_id,
                    function_name,
                    inputs.clone(),
                )
                .await
                .map_err(|e| Error::Internal(format!("Failed to create authorization: {}", e)))?;

            let outputs = self
                .extract_outputs(&auth, account.view_key())
                .map_err(|e| Error::Internal(format!("Failed to extract outputs: {}", e)))?;

            let tx = self
                .client
                .prove(&auth)
                .await
                .map_err(|e| Error::Internal(format!("Delegated proving failed: {}", e)))?;

            if let Some(execution) = tx.execution() {
                print_execution_stats(self.vm(), program_id, execution, None, CONSENSUS_VERSION)
                    .map_err(|e| Error::Internal(format!("Failed to print stats: {}", e)))?;
            }

            let (total_cost, _) = self
                .calculate_cost(&tx)
                .map_err(|e| Error::Internal(format!("Failed to calculate cost: {}", e)))?;
            if balance < total_cost {
                return Err(Error::Internal(format!(
                    "Insufficient balance {} for total cost {} on `{}.{}`",
                    balance, total_cost, program_id, function_name
                )));
            }

            log::info!("✅ Received proved transaction: {}", tx.id());
            (tx, outputs)
        } else {
            let (tx, outputs) = self
                .execute(
                    account.private_key(),
                    program_id,
                    function_name,
                    inputs.clone(),
                    None,
                    0,
                )
                .await
                .map_err(|e| {
                    Error::Internal(format!("Failed to execute '{}': {}", function_name, e))
                })?;

            if let Some(execution) = tx.execution() {
                print_execution_stats(self.vm(), program_id, execution, None, CONSENSUS_VERSION)
                    .map_err(|e| Error::Internal(format!("Failed to print stats: {}", e)))?;
            }

            let (total_cost, _) = self
                .calculate_cost(&tx)
                .map_err(|e| Error::Internal(format!("Failed to calculate cost: {}", e)))?;
            if balance < total_cost {
                return Err(Error::Internal(format!(
                    "Insufficient balance {} for total cost {} on `{}.{}`",
                    balance, total_cost, program_id, function_name
                )));
            }

            (tx, outputs)
        };

        log::info!("📡 Broadcasting tx: {}", transaction.id());
        self.client
            .broadcast_wait(&transaction)
            .await
            .map_err(|e| Error::Internal(format!("Failed to broadcast transaction: {}", e)))?;

        Ok(function_outputs)
    }

    pub async fn deploy_and_broadcast(
        &self,
        deployer: &Account<N>,
        program: &Program<N>,
        dependencies: &[&str],
    ) -> Result<()> {
        let program_id = program.id().to_string();

        for dep_id in dependencies {
            let dep_program_id: ProgramID<N> = dep_id.parse().map_err(|e| {
                Error::Internal(format!("Invalid dependency ID '{}': {}", dep_id, e))
            })?;

            if !self.contains_program(&dep_program_id) {
                self.client
                    .wait_for_program::<N>(dep_id)
                    .await
                    .map_err(|e| {
                        Error::Internal(format!(
                            "Failed waiting for dependency '{}': {}",
                            dep_id, e
                        ))
                    })?;
                let bytecode = self.client.program::<N>(dep_id).await.map_err(|e| {
                    Error::Internal(format!("Failed to fetch dependency '{}': {}", dep_id, e))
                })?;
                let dep_program: Program<N> = bytecode.parse().map_err(|e| {
                    Error::Internal(format!("Failed to parse dependency '{}': {}", dep_id, e))
                })?;
                self.add_program(&dep_program).await.map_err(|e| {
                    Error::Internal(format!("Failed to add dependency '{}': {}", dep_id, e))
                })?;
            }
        }

        log::info!("📦 Creating deployment tx for '{}'...", program_id);

        let transaction = self
            .deploy(deployer.private_key(), program, 0, None)
            .await
            .map_err(|e| {
                Error::Internal(format!("Failed to create deployment transaction: {}", e))
            })?;

        if let Transaction::Deploy(_, _, _, deployment, _fee) = &transaction {
            print_deployment_stats(self.vm(), &program_id, deployment, None, CONSENSUS_VERSION)
                .map_err(|e| Error::Internal(format!("Failed to print stats: {}", e)))?;
        }

        let balance = self
            .client
            .public_balance::<N>(&deployer.address())
            .await
            .map_err(|e| Error::Internal(format!("Failed to get balance: {}", e)))?;
        let fee = transaction
            .fee_amount()
            .map_err(|e| Error::Internal(format!("Failed to get fee: {}", e)))?;
        if *fee > balance {
            return Err(Error::Internal(format!(
                "Insufficient balance {} for deployment cost {} on '{}'",
                balance, fee, program_id
            )));
        }

        log::info!(
            "📡 Broadcasting deployment tx: {} to {}",
            transaction.id(),
            self.client.endpoint()
        );

        self.client
            .broadcast_wait(&transaction)
            .await
            .map_err(|e| Error::Internal(format!("Failed to broadcast deployment: {}", e)))?;

        self.client
            .wait_for_program::<N>(&program_id)
            .await
            .map_err(|e| {
                Error::Internal(format!("Failed waiting for program availability: {}", e))
            })?;

        self.add_program(program)
            .await
            .map_err(|e| Error::Internal(format!("Failed to add deployed program to VM: {}", e)))?;

        Ok(())
    }

    fn create_query(endpoint: &str) -> Result<Query<N, BlockMemory<N>>> {
        let uri: Uri = endpoint
            .parse()
            .map_err(|e| Error::Config(format!("Invalid endpoint URI: {}", e)))?;
        Ok(Query::<N, BlockMemory<N>>::from(uri))
    }
}
