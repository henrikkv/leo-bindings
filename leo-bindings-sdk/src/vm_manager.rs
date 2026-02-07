use crate::config::Client;
use crate::error::{Error, Result};
use aleo_std::StorageMode;
use http::uri::Uri;
use snarkvm::ledger::query::Query;
use snarkvm::ledger::store::ConsensusStore;
use snarkvm::ledger::store::helpers::memory::{BlockMemory, ConsensusMemory};
use snarkvm::prelude::*;
use snarkvm::synthesizer::VM;

#[derive(Clone)]
pub struct VMManager<N: Network> {
    vm: VM<N, ConsensusMemory<N>>,
    client: Client,
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

        execution_cost(&self.vm.process().read(), execution, ConsensusVersion::V12)
            .map_err(|e| Error::VmError(format!("Failed to calculate execution cost: {}", e)))
    }

    fn create_query(endpoint: &str) -> Result<Query<N, BlockMemory<N>>> {
        let uri: Uri = endpoint
            .parse()
            .map_err(|e| Error::Config(format!("Invalid endpoint URI: {}", e)))?;
        Ok(Query::<N, BlockMemory<N>>::from(uri))
    }
}
