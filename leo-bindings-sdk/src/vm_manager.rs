use crate::account::Account;
use crate::config::Client;
use crate::error::{Error, Result};
use crate::local_chain::encode_local_chain_blocks;
use crate::stats::{print_deployment_stats, print_execution_stats};
use aleo_std::StorageMode;
use http::uri::Uri;
use snarkvm::ledger::block::Transaction;
use snarkvm::ledger::query::Query;
use snarkvm::ledger::store::ConsensusStore;
use snarkvm::ledger::store::helpers::memory::{BlockMemory, ConsensusMemory};
use snarkvm::prelude::*;
use snarkvm::synthesizer::VM;
use snarkvm::synthesizer::program::{FinalizeGlobalState, FinalizeStoreTrait, StackTrait};

pub const CONSENSUS_VERSION: ConsensusVersion = ConsensusVersion::V15;

pub trait VMManager<N: Network>: Send + Sync + Clone {
    fn program_exists(&self, program_id: &ProgramID<N>) -> Result<bool>;

    fn mapping_value(
        &self,
        program_id: &ProgramID<N>,
        mapping_name: &Identifier<N>,
        key: &Value<N>,
    ) -> Result<Option<Value<N>>>;

    fn evaluate_view(
        &self,
        program_id: &ProgramID<N>,
        view_name: &Identifier<N>,
        inputs: Vec<Value<N>>,
    ) -> Result<Vec<Value<N>>>;

    fn deploy_and_broadcast(
        &self,
        deployer: &Account<N>,
        program: &Program<N>,
        dependencies: &[ProgramID<N>],
    ) -> Result<()>;

    fn execute_and_broadcast(
        &self,
        account: &Account<N>,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
        inputs: Vec<Value<N>>,
        dependencies: &[ProgramID<N>],
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
            .map_err(|e| Error::Other(format!("Failed to create consensus store: {}", e)))?;

        let vm =
            VM::from(store).map_err(|e| Error::Other(format!("Failed to create VM: {}", e)))?;

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
            .lock()
            .add_program_with_edition(program, edition)
            .map_err(|e| Error::Other(format!("Failed to add program '{}': {}", program.id(), e)))
    }

    pub fn contains_program(&self, program_id: &ProgramID<N>) -> bool {
        self.vm.process().contains_program(program_id)
    }

    pub fn deploy(
        &self,
        private_key: &PrivateKey<N>,
        program: &Program<N>,
        priority_fee: u64,
        fee_record: Option<Record<N, Plaintext<N>>>,
    ) -> Result<Transaction<N>> {
        let query = Self::create_query(&self.client.endpoint)?;
        let rng = &mut rand::rng();
        self.vm
            .deploy(
                private_key,
                program,
                fee_record,
                priority_fee,
                Some(&query),
                rng,
            )
            .map_err(|e| Error::Other(format!("Failed to create deployment transaction: {e}")))
    }

    pub fn execute(
        &self,
        private_key: &PrivateKey<N>,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
        inputs: Vec<Value<N>>,
        fee_record: Option<Record<N, Plaintext<N>>>,
        priority_fee: u64,
    ) -> Result<(Transaction<N>, Vec<Value<N>>)> {
        let query = Self::create_query(&self.client.endpoint)?;
        let rng = &mut rand::rng();
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
            .map_err(|e| Error::Other(format!("Failed to create execution transaction: {e}")))?;
        let outputs = response.outputs().to_vec();
        Ok((transaction, outputs))
    }

    pub fn authorize(
        &self,
        private_key: &PrivateKey<N>,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
        inputs: Vec<Value<N>>,
    ) -> Result<Authorization<N>> {
        let rng = &mut rand::rng();
        self.vm
            .authorize(private_key, *program_id, *function_name, inputs.iter(), rng)
            .map_err(|e| Error::Other(format!("Failed to create authorization: {e}")))
    }

    pub fn extract_outputs(
        &self,
        authorization: &Authorization<N>,
        view_key: &ViewKey<N>,
    ) -> Result<Vec<Value<N>>> {
        let request = authorization
            .peek_next()
            .map_err(|e| Error::Other(format!("Failed to peek authorization: {}", e)))?;

        let function_id = snarkvm::console::program::compute_function_id(
            request.network_id(),
            request.program_id(),
            request.function_name(),
        )
        .map_err(|e| Error::Other(format!("Failed to compute function ID: {}", e)))?;

        let num_inputs = request.inputs().len();

        let transitions = authorization.transitions();
        let main_transition = transitions
            .values()
            .last()
            .ok_or_else(|| Error::Other("Authorization contains no transitions".to_string()))?;

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
                        .map_err(|e| Error::Other(format!("Index overflow: {}", e)))?,
                );
                let output_view_key = N::hash_psd4(&[function_id, *tvk, index]).map_err(|e| {
                    Error::Other(format!("Failed to compute output view key: {}", e))
                })?;
                let plaintext = ciphertext.decrypt_symmetric(output_view_key).map_err(|e| {
                    Error::Other(format!("Failed to decrypt private output: {}", e))
                })?;
                Ok(Value::Plaintext(plaintext))
            }
            Output::Record(_, _, Some(record_ciphertext), _) => {
                let record_plaintext = record_ciphertext
                    .decrypt(view_key)
                    .map_err(|e| Error::Other(format!("Failed to decrypt record: {}", e)))?;
                Ok(Value::Record(record_plaintext))
            }
            Output::Future(_, Some(future)) => Ok(Value::Future(future.clone())),
            Output::ExternalRecord(_) => Err(Error::Other(
                "External record outputs are not supported".to_string(),
            )),
            _ => Err(Error::Other(
                "Output value is missing from transition".to_string(),
            )),
        }
    }

    pub fn calculate_cost(&self, transaction: &Transaction<N>) -> Result<(u64, (u64, u64))> {
        let execution = transaction
            .execution()
            .ok_or_else(|| Error::Other("Transaction has no execution".to_string()))?;

        execution_cost(&self.vm.process().lock(), execution, CONSENSUS_VERSION)
            .map_err(|e| Error::Other(format!("Failed to calculate execution cost: {}", e)))
    }

    fn load_missing_dependencies(&self, dependencies: &[ProgramID<N>]) -> Result<()> {
        for dep_id in dependencies {
            if self.contains_program(dep_id) {
                continue;
            }
            let dep_id_str = dep_id.to_string();
            crate::block_on(self.client.wait_for_program::<N>(&dep_id_str))?;
            let bytecode = crate::block_on(self.client.program::<N>(&dep_id_str))?;
            let dep_program: Program<N> = bytecode.parse().map_err(|e| {
                Error::Other(format!("Failed to parse dependency '{}': {}", dep_id, e))
            })?;
            self.add_program(&dep_program).map_err(|e| {
                Error::Other(format!("Failed to add dependency '{}': {}", dep_id, e))
            })?;
        }
        Ok(())
    }

    pub fn ensure_program_loaded(
        &self,
        program_id: &ProgramID<N>,
        dependencies: &[ProgramID<N>],
    ) -> Result<()> {
        if self.contains_program(program_id) {
            return Ok(());
        }

        self.load_missing_dependencies(dependencies)?;

        let program_id_str = program_id.to_string();
        crate::block_on(self.client.wait_for_program::<N>(&program_id_str))?;
        let bytecode = crate::block_on(self.client.program::<N>(&program_id_str))?;
        let program: Program<N> = bytecode.parse().map_err(|e| {
            Error::Other(format!("Failed to parse program '{}': {}", program_id, e))
        })?;
        self.add_program(&program)
            .map_err(|e| Error::Other(format!("Failed to add program '{}': {}", program_id, e)))?;

        Ok(())
    }

    pub fn execute_and_broadcast(
        &self,
        account: &Account<N>,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
        inputs: Vec<Value<N>>,
        dependencies: &[ProgramID<N>],
    ) -> Result<Vec<Value<N>>> {
        log::info!("Creating tx: {}.{}", program_id, function_name);

        self.ensure_program_loaded(program_id, dependencies)?;

        let balance = crate::block_on(self.client.public_balance::<N>(&account.address()))
            .map_err(|e| Error::Other(format!("Failed to get balance: {}", e)))?;

        let (transaction, function_outputs) = if self.client.has_credentials() {
            let auth = self.authorize(
                account.private_key(),
                program_id,
                function_name,
                inputs.clone(),
            )?;
            let outputs = self
                .extract_outputs(&auth, account.view_key())
                .map_err(|e| Error::Other(format!("Failed to extract outputs: {}", e)))?;
            let tx = crate::block_on(self.client.prove(&auth))
                .map_err(|e| Error::Other(format!("Delegated proving failed: {}", e)))?;
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
            .map_err(|e| Error::Other(format!("Failed to execute '{function_name}': {e}")))?
        };

        if let Some(execution) = transaction.execution() {
            print_execution_stats(
                self.vm(),
                &program_id.to_string(),
                execution,
                None,
                CONSENSUS_VERSION,
            )
            .map_err(|e| Error::Other(format!("Failed to print stats: {}", e)))?;
        }
        let (total_cost, _) = self
            .calculate_cost(&transaction)
            .map_err(|e| Error::Other(format!("Failed to calculate cost: {}", e)))?;
        if balance < total_cost {
            return Err(Error::Other(format!(
                "Insufficient balance {balance} for total cost {total_cost} on `{program_id}.{function_name}`"
            )));
        }

        log::info!("📡 Broadcasting tx: {}", transaction.id());
        crate::block_on(self.client.broadcast_wait(&transaction))
            .map_err(|e| Error::Other(format!("Failed to broadcast transaction: {}", e)))?;

        Ok(function_outputs)
    }

    pub fn deploy_and_broadcast(
        &self,
        deployer: &Account<N>,
        program: &Program<N>,
        dependencies: &[ProgramID<N>],
    ) -> Result<()> {
        let program_id = program.id();
        let program_id_str = program_id.to_string();
        self.load_missing_dependencies(dependencies)?;

        log::info!("📦 Creating deployment tx for '{}'...", program_id);

        let transaction = self
            .deploy(deployer.private_key(), program, 0, None)
            .map_err(|e| Error::Other(format!("Failed to create deployment transaction: {}", e)))?;

        if let Transaction::Deploy(_, _, _, deployment, _fee) = &transaction {
            print_deployment_stats(
                self.vm(),
                &program_id_str,
                deployment,
                None,
                CONSENSUS_VERSION,
            )
            .map_err(|e| Error::Other(format!("Failed to print stats: {}", e)))?;
        }

        let balance = crate::block_on(self.client.public_balance::<N>(&deployer.address()))
            .map_err(|e| Error::Other(format!("Failed to get balance: {}", e)))?;
        let fee = transaction
            .fee_amount()
            .map_err(|e| Error::Other(format!("Failed to get fee: {}", e)))?;
        if *fee > balance {
            return Err(Error::Other(format!(
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
            .map_err(|e| Error::Other(format!("Failed to broadcast deployment: {}", e)))?;

        crate::block_on(self.client.wait_for_program::<N>(&program_id_str))?;

        self.add_program(program)
            .map_err(|e| Error::Other(format!("Failed to add deployed program to VM: {}", e)))?;

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
    fn program_exists(&self, program_id: &ProgramID<N>) -> Result<bool> {
        crate::block_on(self.client.program_exists::<N>(&program_id.to_string()))
    }

    fn mapping_value(
        &self,
        program_id: &ProgramID<N>,
        mapping_name: &Identifier<N>,
        key: &Value<N>,
    ) -> Result<Option<Value<N>>> {
        crate::block_on(self.client.mapping::<N>(
            &program_id.to_string(),
            &mapping_name.to_string(),
            key,
        ))
    }

    fn evaluate_view(
        &self,
        program_id: &ProgramID<N>,
        view_name: &Identifier<N>,
        inputs: Vec<Value<N>>,
    ) -> Result<Vec<Value<N>>> {
        crate::block_on(self.client.evaluate_view::<N>(
            &program_id.to_string(),
            &view_name.to_string(),
            &inputs,
        ))
    }

    fn deploy_and_broadcast(
        &self,
        deployer: &Account<N>,
        program: &Program<N>,
        dependencies: &[ProgramID<N>],
    ) -> Result<()> {
        NetworkVm::deploy_and_broadcast(self, deployer, program, dependencies)
    }

    fn execute_and_broadcast(
        &self,
        account: &Account<N>,
        program_id: &ProgramID<N>,
        function_name: &Identifier<N>,
        inputs: Vec<Value<N>>,
        dependencies: &[ProgramID<N>],
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
        self.vm.process().contains_program(program_id)
    }

    fn ensure_program_loaded(
        &self,
        program_id: &ProgramID<TestnetV0>,
        dependencies: &[ProgramID<TestnetV0>],
    ) -> Result<()> {
        if self.contains_program(program_id) {
            return Ok(());
        }

        for dep_id in dependencies {
            if !self.contains_program(dep_id) {
                return Err(Error::Other(format!(
                    "LocalVM: dependency '{dep_id}' not on ledger; deploy it first (missing program '{program_id}')"
                )));
            }
        }

        Err(Error::Other(format!(
            "LocalVM: program '{program_id}' not loaded; deploy via bindings::new first"
        )))
    }

    pub fn deploy_and_broadcast(
        &self,
        deployer: &Account<TestnetV0>,
        program: &Program<TestnetV0>,
        dependencies: &[ProgramID<TestnetV0>],
    ) -> Result<()> {
        let program_id = program.id();

        for dep_id in dependencies {
            if !self.contains_program(dep_id) {
                return Err(Error::Other(format!(
                    "LocalVM: missing dependency '{dep_id}' before deploying '{program_id}'"
                )));
            }
        }

        log::info!("📦 Deploy: creating proofless deployment tx for '{program_id}'");

        let mut rng = rand::rng();
        let transaction = self
            .vm
            .deploy_local_proofless(deployer.private_key(), program, None, 0, None, &mut rng)
            .map_err(|e| Error::Other(format!("deploy_local_proofless: {e}")))?;

        let beacon_account = Account::dev_account(0).map_err(|e| Error::Other(e.to_string()))?;
        let beacon_key = *beacon_account.private_key();
        crate::local_chain::commit_transaction(&self.vm, &beacon_key, &transaction, &mut rng)?;

        Ok(())
    }

    pub fn execute_and_broadcast(
        &self,
        account: &Account<TestnetV0>,
        program_id: &ProgramID<TestnetV0>,
        function_name: &Identifier<TestnetV0>,
        inputs: Vec<Value<TestnetV0>>,
        dependencies: &[ProgramID<TestnetV0>],
    ) -> Result<Vec<Value<TestnetV0>>> {
        log::info!("Creating local tx: {program_id}.{function_name}");

        self.ensure_program_loaded(program_id, dependencies)?;

        let mut rng = rand::rng();

        let (transaction, response) = self
            .vm
            .execute_with_response_local_proofless(
                account.private_key(),
                (*program_id, *function_name),
                inputs.into_iter(),
                None,
                0,
                None,
                &mut rng,
            )
            .map_err(|e| Error::Other(format!("execute_with_response_local_proofless: {e}")))?;

        let beacon_account = Account::dev_account(0).map_err(|e| Error::Other(e.to_string()))?;
        let beacon_key = *beacon_account.private_key();
        crate::local_chain::commit_transaction(&self.vm, &beacon_key, &transaction, &mut rng)?;

        Ok(response.outputs().to_vec())
    }

    fn block_at_height(&self, height: u32) -> Result<Block<TestnetV0>> {
        let hash = self
            .vm
            .block_store()
            .get_block_hash(height)
            .map_err(|e| Error::Other(format!("get_block_hash({height}): {e}")))?
            .ok_or_else(|| Error::Other(format!("no block at height {height}")))?;
        self.vm
            .block_store()
            .get_block(&hash)
            .map_err(|e| Error::Other(format!("get_block({height}): {e}")))?
            .ok_or_else(|| Error::Other(format!("block not found at height {height}")))
    }

    pub fn as_bytes(&self) -> Result<Vec<u8>> {
        let height = self.vm.block_store().current_block_height();
        let mut blocks = Vec::new();
        for h in 0..=height {
            blocks.push(self.block_at_height(h)?);
        }

        let mut bytes = Vec::new();
        encode_local_chain_blocks(&mut bytes, &blocks)?;
        Ok(bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let blocks = crate::local_chain::parse_local_chain_blocks(bytes)?;
        let vm = crate::local_chain::vm_from_local_chain_blocks(&blocks)?;
        Ok(Self { vm })
    }

    pub fn set_mapping_value<N: Network>(
        &self,
        program_id: &ProgramID<N>,
        mapping_name: &Identifier<N>,
        key: &Value<N>,
        value: &Value<N>,
    ) -> Result<()> {
        let program: ProgramID<TestnetV0> = program_id.to_string().parse()?;
        let mapping: Identifier<TestnetV0> = mapping_name.to_string().parse()?;
        let k = match key {
            Value::Plaintext(p) => p.to_string().parse::<Plaintext<TestnetV0>>()?,
            _ => return Err(Error::Other("Mapping key must be plaintext".to_string())),
        };
        let v: Value<TestnetV0> = value.to_string().parse()?;
        self.vm
            .finalize_store()
            .update_key_value(program, mapping, k, v)?;
        Ok(())
    }
}

pub struct LocalVMSnapshot {
    bytes: Vec<u8>,
    finalize_overlay: Vec<(
        ProgramID<TestnetV0>,
        Identifier<TestnetV0>,
        Plaintext<TestnetV0>,
        Value<TestnetV0>,
    )>,
}

impl LocalVMSnapshot {
    pub fn restore(&self) -> LocalVM {
        let vm = LocalVM::from_bytes(&self.bytes).unwrap();
        for (program_id, mapping_name, key, value) in &self.finalize_overlay {
            vm.vm
                .finalize_store()
                .update_key_value(*program_id, *mapping_name, key.clone(), value.clone())
                .expect("finalize overlay apply failed");
        }
        vm
    }
}

impl LocalVM {
    pub fn snapshot(&self) -> LocalVMSnapshot {
        let bytes = self.as_bytes().unwrap();
        let mut finalize_overlay = Vec::new();
        let process = self.vm.process();
        let finalize_store = self.vm.finalize_store();
        for program_id in process.program_ids() {
            let Ok(Some(mapping_names)) = finalize_store.get_mapping_names_confirmed(&program_id)
            else {
                continue;
            };
            for mapping_name in mapping_names {
                let Ok(entries) = finalize_store.get_mapping_confirmed(program_id, mapping_name)
                else {
                    continue;
                };
                for (key, value) in entries {
                    finalize_overlay.push((program_id, mapping_name, key, value));
                }
            }
        }
        LocalVMSnapshot {
            bytes,
            finalize_overlay,
        }
    }
}

pub struct SnapshotStore {
    current: LocalVM,
    snapshots: std::collections::HashMap<&'static str, LocalVMSnapshot>,
}

impl SnapshotStore {
    pub fn new() -> Result<Self> {
        Ok(Self {
            current: LocalVM::new()?,
            snapshots: Default::default(),
        })
    }

    /// Create a store, run `f` for setup and return the store.
    pub fn build<F: FnOnce(&mut Self)>(f: F) -> Result<Self> {
        let mut store = Self::new()?;
        f(&mut store);
        Ok(store)
    }

    /// The current VM
    pub fn vm(&self) -> &LocalVM {
        &self.current
    }

    /// Snapshot the current state under `name`
    pub fn save(&mut self, name: &'static str) -> &mut Self {
        self.snapshots.insert(name, self.current.snapshot());
        self
    }

    /// Restore a previously saved snapshot
    pub fn restore(&self, name: &'static str) -> LocalVM {
        self.snapshots
            .get(name)
            .unwrap_or_else(|| panic!("snapshot '{name}' not found"))
            .restore()
    }
}

/// Declares a global [`SnapshotStore`] for multiple tests.
///
/// ```
/// snapshot_store!(SETUP, |store| {
///     let alice = Account::dev_account(0).unwrap();
///     MyAleo::new(&alice, store.vm().clone()).unwrap();
///     store.save("deployed");
/// });
///
/// #[test]
/// fn test_snapshot() {
///     let alice = Account::dev_account(0).unwrap();
///     
///     let vm = SETUP.restore("deployed");
///     // Skips deployment
///     let dev_a = DevAleo::new(&alice, vm).unwrap();
/// }
/// ```
#[macro_export]
macro_rules! snapshot_store {
    ($name:ident, |$store:ident| $body:block) => {
        static $name: ::std::sync::LazyLock<$crate::SnapshotStore> =
            ::std::sync::LazyLock::new(|| $crate::SnapshotStore::build(|$store| $body).unwrap());
    };
}

impl VMManager<TestnetV0> for LocalVM {
    fn program_exists(&self, program_id: &ProgramID<TestnetV0>) -> Result<bool> {
        Ok(self.contains_program(program_id))
    }

    fn mapping_value(
        &self,
        program_id: &ProgramID<TestnetV0>,
        mapping_name: &Identifier<TestnetV0>,
        key: &Value<TestnetV0>,
    ) -> Result<Option<Value<TestnetV0>>> {
        let k = match key {
            Value::Plaintext(p) => p.clone(),
            _ => {
                return Err(Error::Other(
                    "LocalVM mapping keys must be plaintext values".to_string(),
                ));
            }
        };
        self.vm
            .finalize_store()
            .get_value_confirmed(*program_id, *mapping_name, &k)
            .map_err(|e| Error::Other(format!("Mapping lookup failed: {e}")))
    }

    fn evaluate_view(
        &self,
        program_id: &ProgramID<TestnetV0>,
        view_name: &Identifier<TestnetV0>,
        inputs: Vec<Value<TestnetV0>>,
    ) -> Result<Vec<Value<TestnetV0>>> {
        let height = self.vm.block_store().current_block_height();
        let block = self.block_at_height(height)?;

        let block_timestamp = Some(block.timestamp());
        let block_spend_limit = match block.authority() {
            snarkvm::ledger::authority::Authority::Quorum(subdag) => {
                subdag.spend_limit(block.height())
            }
            _ => None,
        };
        let state = FinalizeGlobalState::new::<TestnetV0>(
            block.round(),
            height,
            block_timestamp,
            block.cumulative_weight(),
            block.cumulative_proof_target(),
            block.previous_hash(),
            block_spend_limit,
        )
        .map_err(|e| Error::Other(format!("Failed to build finalize global state: {e}")))?;

        let stack = self
            .vm
            .process()
            .get_stack(*program_id)
            .map_err(|e| Error::Other(format!("get_stack({program_id}): {e}")))?;

        stack
            .evaluate_view(state, self.vm.finalize_store(), view_name, inputs)
            .map_err(|e| Error::Other(format!("evaluate_view({program_id}, {view_name}): {e}")))
    }

    fn deploy_and_broadcast(
        &self,
        deployer: &Account<TestnetV0>,
        program: &Program<TestnetV0>,
        dependencies: &[ProgramID<TestnetV0>],
    ) -> Result<()> {
        LocalVM::deploy_and_broadcast(self, deployer, program, dependencies)
    }

    fn execute_and_broadcast(
        &self,
        account: &Account<TestnetV0>,
        program_id: &ProgramID<TestnetV0>,
        function_name: &Identifier<TestnetV0>,
        inputs: Vec<Value<TestnetV0>>,
        dependencies: &[ProgramID<TestnetV0>],
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
