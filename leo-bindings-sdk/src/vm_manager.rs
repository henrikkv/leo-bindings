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

pub trait VMManager<N: Network>: Send + Sync + Clone {
    fn program_exists(&self, program_id: &str) -> Result<bool>;

    fn mapping_value(
        &self,
        program_id: &str,
        mapping_name: &str,
        key: &Value<N>,
    ) -> Result<Option<Value<N>>>;

    fn deploy_and_broadcast(
        &self,
        deployer: &Account<N>,
        program: &Program<N>,
        dependencies: &[&str],
    ) -> Result<()>;

    fn execute_and_broadcast(
        &self,
        account: &Account<N>,
        program_id: &str,
        function_name: &str,
        inputs: Vec<Value<N>>,
        dependencies: &[&str],
    ) -> Result<Vec<Value<N>>>;
}

#[derive(Clone)]
pub struct NetworkVm<N: Network> {
    vm: VM<N, ConsensusMemory<N>>,
    client: Client,
}

impl<N: Network> std::fmt::Debug for NetworkVm<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkVm")
            .field("client", &self.client)
            .finish_non_exhaustive()
    }
}

impl<N: Network> NetworkVm<N> {
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

    pub fn add_program(&self, program: &Program<N>) -> Result<()> {
        let program_id = program.id().to_string();
        let edition = crate::block_on(self.client.program_edition::<N>(&program_id))?;
        self.vm
            .process()
            .write()
            .add_program_with_edition(program, edition)
            .map_err(|e| Error::VmError(format!("Failed to add program '{}': {}", program.id(), e)))
    }

    pub fn contains_program(&self, program_id: &ProgramID<N>) -> bool {
        self.vm.process().read().contains_program(program_id)
    }

    pub fn deploy(
        &self,
        private_key: &PrivateKey<N>,
        program: &Program<N>,
        priority_fee: u64,
        fee_record: Option<Record<N, Plaintext<N>>>,
    ) -> Result<Transaction<N>> {
        let query = Self::create_query(&self.client.endpoint)?;
        let rng = &mut rand::thread_rng();
        self.vm
            .deploy(
                private_key,
                program,
                fee_record,
                priority_fee,
                Some(&query),
                rng,
            )
            .map_err(|e| Error::VmError(format!("Failed to create deployment transaction: {e}")))
    }

    pub fn execute(
        &self,
        private_key: &PrivateKey<N>,
        program_id: &str,
        function_name: &str,
        inputs: Vec<Value<N>>,
        fee_record: Option<Record<N, Plaintext<N>>>,
        priority_fee: u64,
    ) -> Result<(Transaction<N>, Vec<Value<N>>)> {
        let query = Self::create_query(&self.client.endpoint)?;
        let rng = &mut rand::thread_rng();
        let program_id: ProgramID<N> = program_id.parse()?;
        let function_name: Identifier<N> = function_name.parse()?;
        let (transaction, response) = self
            .vm
            .execute_with_response(
                private_key,
                (program_id, function_name),
                inputs.iter(),
                fee_record,
                priority_fee,
                Some(&query),
                rng,
            )
            .map_err(|e| Error::VmError(format!("Failed to create execution transaction: {e}")))?;
        let outputs = response.outputs().to_vec();
        Ok((transaction, outputs))
    }

    pub fn authorize(
        &self,
        private_key: &PrivateKey<N>,
        program_id: &str,
        function_name: &str,
        inputs: Vec<Value<N>>,
    ) -> Result<Authorization<N>> {
        let rng = &mut rand::thread_rng();
        let program_id: ProgramID<N> = program_id.parse()?;
        let function_name: Identifier<N> = function_name.parse()?;
        self.vm
            .authorize(private_key, program_id, function_name, inputs.iter(), rng)
            .map_err(|e| Error::VmError(format!("Failed to create authorization: {e}")))
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

    fn load_missing_dependencies(&self, dependencies: &[&str]) -> Result<()> {
        for dep_id in dependencies {
            let dep_program_id: ProgramID<N> = dep_id.parse().map_err(|e| {
                Error::Internal(format!("Invalid dependency ID '{}': {}", dep_id, e))
            })?;
            if self.contains_program(&dep_program_id) {
                continue;
            }
            crate::block_on(self.client.wait_for_program::<N>(dep_id))?;
            let bytecode = crate::block_on(self.client.program::<N>(dep_id))?;
            let dep_program: Program<N> = bytecode.parse().map_err(|e| {
                Error::Internal(format!("Failed to parse dependency '{}': {}", dep_id, e))
            })?;
            self.add_program(&dep_program).map_err(|e| {
                Error::Internal(format!("Failed to add dependency '{}': {}", dep_id, e))
            })?;
        }
        Ok(())
    }

    pub fn ensure_program_loaded(&self, program_id: &str, dependencies: &[&str]) -> Result<()> {
        let program_id_parsed: ProgramID<N> = program_id
            .parse()
            .map_err(|e| Error::Internal(format!("Invalid program ID '{}': {}", program_id, e)))?;

        if self.contains_program(&program_id_parsed) {
            return Ok(());
        }

        self.load_missing_dependencies(dependencies)?;

        crate::block_on(self.client.wait_for_program::<N>(program_id))?;
        let bytecode = crate::block_on(self.client.program::<N>(program_id))?;
        let program: Program<N> = bytecode.parse().map_err(|e| {
            Error::Internal(format!("Failed to parse program '{}': {}", program_id, e))
        })?;
        self.add_program(&program).map_err(|e| {
            Error::Internal(format!("Failed to add program '{}': {}", program_id, e))
        })?;

        Ok(())
    }

    pub fn execute_and_broadcast(
        &self,
        account: &Account<N>,
        program_id: &str,
        function_name: &str,
        inputs: Vec<Value<N>>,
        dependencies: &[&str],
    ) -> Result<Vec<Value<N>>> {
        log::info!("Creating tx: {}.{}", program_id, function_name);

        self.ensure_program_loaded(program_id, dependencies)?;

        let balance = crate::block_on(self.client.public_balance::<N>(&account.address()))
            .map_err(|e| Error::Internal(format!("Failed to get balance: {}", e)))?;

        let (transaction, function_outputs) = if self.client.has_credentials() {
            let auth = self.authorize(
                account.private_key(),
                program_id,
                function_name,
                inputs.clone(),
            )?;
            let outputs = self
                .extract_outputs(&auth, account.view_key())
                .map_err(|e| Error::Internal(format!("Failed to extract outputs: {}", e)))?;
            let tx = crate::block_on(self.client.prove(&auth))
                .map_err(|e| Error::Internal(format!("Delegated proving failed: {}", e)))?;
            log::info!("✅ Received proved transaction: {}", tx.id());
            (tx, outputs)
        } else {
            self.execute(
                account.private_key(),
                program_id,
                function_name,
                inputs.clone(),
                None,
                0,
            )
            .map_err(|e| Error::Internal(format!("Failed to execute '{function_name}': {e}")))?
        };

        if let Some(execution) = transaction.execution() {
            print_execution_stats(self.vm(), program_id, execution, None, CONSENSUS_VERSION)
                .map_err(|e| Error::Internal(format!("Failed to print stats: {}", e)))?;
        }
        let (total_cost, _) = self
            .calculate_cost(&transaction)
            .map_err(|e| Error::Internal(format!("Failed to calculate cost: {}", e)))?;
        if balance < total_cost {
            return Err(Error::Internal(format!(
                "Insufficient balance {balance} for total cost {total_cost} on `{program_id}.{function_name}`"
            )));
        }

        log::info!("📡 Broadcasting tx: {}", transaction.id());
        crate::block_on(self.client.broadcast_wait(&transaction))
            .map_err(|e| Error::Internal(format!("Failed to broadcast transaction: {}", e)))?;

        Ok(function_outputs)
    }

    pub fn deploy_and_broadcast(
        &self,
        deployer: &Account<N>,
        program: &Program<N>,
        dependencies: &[&str],
    ) -> Result<()> {
        let program_id = program.id().to_string();
        self.load_missing_dependencies(dependencies)?;

        log::info!("📦 Creating deployment tx for '{}'...", program_id);

        let transaction = self
            .deploy(deployer.private_key(), program, 0, None)
            .map_err(|e| {
                Error::Internal(format!("Failed to create deployment transaction: {}", e))
            })?;

        if let Transaction::Deploy(_, _, _, deployment, _fee) = &transaction {
            print_deployment_stats(self.vm(), &program_id, deployment, None, CONSENSUS_VERSION)
                .map_err(|e| Error::Internal(format!("Failed to print stats: {}", e)))?;
        }

        let balance = crate::block_on(self.client.public_balance::<N>(&deployer.address()))
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

        crate::block_on(self.client.broadcast_wait(&transaction))
            .map_err(|e| Error::Internal(format!("Failed to broadcast deployment: {}", e)))?;

        crate::block_on(self.client.wait_for_program::<N>(&program_id))?;

        self.add_program(program)
            .map_err(|e| Error::Internal(format!("Failed to add deployed program to VM: {}", e)))?;

        Ok(())
    }

    fn create_query(endpoint: &str) -> Result<Query<N, BlockMemory<N>>> {
        let base = endpoint.trim_end_matches('/');
        let rest_base = if base.ends_with("/v2") {
            base.to_string()
        } else {
            format!("{base}/v2")
        };
        let uri: Uri = rest_base
            .parse()
            .map_err(|e| Error::Config(format!("Invalid endpoint URI: {}", e)))?;
        Ok(Query::<N, BlockMemory<N>>::from(uri))
    }
}

impl<N: Network> VMManager<N> for NetworkVm<N> {
    fn program_exists(&self, program_id: &str) -> Result<bool> {
        crate::block_on(self.client.program_exists::<N>(program_id))
    }

    fn mapping_value(
        &self,
        program_id: &str,
        mapping_name: &str,
        key: &Value<N>,
    ) -> Result<Option<Value<N>>> {
        crate::block_on(self.client.mapping::<N>(program_id, mapping_name, key))
    }

    fn deploy_and_broadcast(
        &self,
        deployer: &Account<N>,
        program: &Program<N>,
        dependencies: &[&str],
    ) -> Result<()> {
        NetworkVm::deploy_and_broadcast(self, deployer, program, dependencies)
    }

    fn execute_and_broadcast(
        &self,
        account: &Account<N>,
        program_id: &str,
        function_name: &str,
        inputs: Vec<Value<N>>,
        dependencies: &[&str],
    ) -> Result<Vec<Value<N>>> {
        NetworkVm::execute_and_broadcast(
            self,
            account,
            program_id,
            function_name,
            inputs,
            dependencies,
        )
    }
}

#[derive(Clone)]
pub struct LocalVM {
    vm: VM<TestnetV0, ConsensusMemory<TestnetV0>>,
}

impl std::fmt::Debug for LocalVM {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalVM").finish_non_exhaustive()
    }
}

impl LocalVM {
    pub fn new() -> Result<Self> {
        let bytes = crate::local_chain::load_or_create_local_chain_bytes()?;
        let blocks = crate::local_chain::parse_local_chain_blocks(&bytes)?;
        let vm = crate::local_chain::vm_from_local_chain_blocks(&blocks)?;
        Ok(Self { vm })
    }

    pub fn vm(&self) -> &VM<TestnetV0, ConsensusMemory<TestnetV0>> {
        &self.vm
    }

    fn contains_program(&self, program_id: &ProgramID<TestnetV0>) -> bool {
        self.vm.process().read().contains_program(program_id)
    }

    fn ensure_program_loaded(&self, program_id: &str, dependencies: &[&str]) -> Result<()> {
        let program_id_parsed: ProgramID<TestnetV0> = program_id
            .parse()
            .map_err(|e| Error::Internal(format!("Invalid program ID '{program_id}': {e}")))?;

        if self.contains_program(&program_id_parsed) {
            return Ok(());
        }

        for dep_id in dependencies {
            let dep_program_id: ProgramID<TestnetV0> = dep_id
                .parse()
                .map_err(|e| Error::Internal(format!("Invalid dependency ID '{dep_id}': {e}")))?;
            if !self.contains_program(&dep_program_id) {
                return Err(Error::Internal(format!(
                    "LocalVM: dependency '{dep_id}' not on ledger; deploy it first (missing program '{program_id}')"
                )));
            }
        }

        Err(Error::Internal(format!(
            "LocalVM: program '{program_id}' not loaded; deploy via bindings::new first"
        )))
    }

    pub fn deploy_and_broadcast(
        &self,
        deployer: &Account<TestnetV0>,
        program: &Program<TestnetV0>,
        dependencies: &[&str],
    ) -> Result<()> {
        let program_id = program.id().to_string();

        for dep_id in dependencies {
            let dep_program_id: ProgramID<TestnetV0> = dep_id
                .parse()
                .map_err(|e| Error::Internal(format!("Invalid dependency ID '{dep_id}': {e}")))?;
            if !self.contains_program(&dep_program_id) {
                return Err(Error::Internal(format!(
                    "LocalVM: missing dependency '{dep_id}' before deploying '{program_id}'"
                )));
            }
        }

        log::info!("📦 Deploy: creating proofless deployment tx for '{program_id}'");

        let mut rng = rand::thread_rng();
        let transaction = self
            .vm
            .deploy_local_proofless(deployer.private_key(), program, None, 0, None, &mut rng)
            .map_err(|e| Error::VmError(format!("deploy_local_proofless: {e}")))?;

        let beacon_account = Account::dev_account(0).map_err(|e| Error::Internal(e.to_string()))?;
        let beacon_key = *beacon_account.private_key();
        crate::local_chain::commit_transaction(&self.vm, &beacon_key, &transaction, &mut rng)?;

        Ok(())
    }

    pub fn execute_and_broadcast(
        &self,
        account: &Account<TestnetV0>,
        program_id: &str,
        function_name: &str,
        inputs: Vec<Value<TestnetV0>>,
        dependencies: &[&str],
    ) -> Result<Vec<Value<TestnetV0>>> {
        log::info!("Creating local tx: {program_id}.{function_name}");

        self.ensure_program_loaded(program_id, dependencies)?;

        let program_id_parsed: ProgramID<TestnetV0> = program_id
            .parse()
            .map_err(|e| Error::Internal(format!("Invalid program ID: {e}")))?;
        let function_name_parsed: Identifier<TestnetV0> = function_name
            .parse()
            .map_err(|e| Error::Internal(format!("Invalid function name: {e}")))?;

        let mut rng = rand::thread_rng();

        let (transaction, response) = self
            .vm
            .execute_with_response_local_proofless(
                account.private_key(),
                (program_id_parsed, function_name_parsed),
                inputs.into_iter(),
                None,
                0,
                None,
                &mut rng,
            )
            .map_err(|e| Error::VmError(format!("execute_with_response_local_proofless: {e}")))?;

        let beacon_account = Account::dev_account(0).map_err(|e| Error::Internal(e.to_string()))?;
        let beacon_key = *beacon_account.private_key();
        crate::local_chain::commit_transaction(&self.vm, &beacon_key, &transaction, &mut rng)?;

        Ok(response.outputs().to_vec())
    }
}

impl VMManager<TestnetV0> for LocalVM {
    fn program_exists(&self, program_id: &str) -> Result<bool> {
        let id: ProgramID<TestnetV0> = program_id
            .parse()
            .map_err(|e| Error::Internal(format!("Invalid program id: {e}")))?;
        Ok(self.contains_program(&id))
    }

    fn mapping_value(
        &self,
        program_id: &str,
        mapping_name: &str,
        key: &Value<TestnetV0>,
    ) -> Result<Option<Value<TestnetV0>>> {
        let pid = ProgramID::from_str(program_id).map_err(|e| Error::Internal(e.to_string()))?;
        let mid = Identifier::from_str(mapping_name).map_err(|e| Error::Internal(e.to_string()))?;
        let key_plain = match key {
            Value::Plaintext(p) => p.clone(),
            _ => {
                return Err(Error::Internal(
                    "LocalVM mapping keys must be plaintext values".to_string(),
                ));
            }
        };
        self.vm
            .finalize_store()
            .get_value_confirmed(pid, mid, &key_plain)
            .map_err(|e| Error::Internal(format!("Mapping lookup failed: {e}")))
    }

    fn deploy_and_broadcast(
        &self,
        deployer: &Account<TestnetV0>,
        program: &Program<TestnetV0>,
        dependencies: &[&str],
    ) -> Result<()> {
        LocalVM::deploy_and_broadcast(self, deployer, program, dependencies)
    }

    fn execute_and_broadcast(
        &self,
        account: &Account<TestnetV0>,
        program_id: &str,
        function_name: &str,
        inputs: Vec<Value<TestnetV0>>,
        dependencies: &[&str],
    ) -> Result<Vec<Value<TestnetV0>>> {
        LocalVM::execute_and_broadcast(
            self,
            account,
            program_id,
            function_name,
            inputs,
            dependencies,
        )
    }
}
