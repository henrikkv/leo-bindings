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

    pub fn add_dependency(&self, program: &Program<N>) -> Result<()> {
        self.vm.process().write().add_program(program).map_err(|e| {
            Error::VmError(format!(
                "Failed to add dependency program '{}': {}",
                program.id(),
                e
            ))
        })
    }

    pub fn add_program(&self, program: &Program<N>, edition: u16) -> Result<()> {
        self.vm
            .process()
            .write()
            .add_programs_with_editions(&[(program.clone(), edition)])
            .map_err(|e| Error::VmError(format!("Failed to add program '{}': {}", program.id(), e)))
    }

    pub fn add_programs(&self, programs: &[Program<N>]) -> Result<()> {
        for program in programs {
            self.add_dependency(program)?;
        }
        Ok(())
    }

    pub fn contains_program(&self, program_id: &ProgramID<N>) -> bool {
        self.vm.process().read().contains_program(program_id)
    }

    fn create_query(&self) -> Result<Query<N, BlockMemory<N>>> {
        let uri: Uri = self
            .client
            .endpoint()
            .parse()
            .map_err(|e| Error::Config(format!("Invalid endpoint URI: {}", e)))?;

        Ok(Query::<N, BlockMemory<N>>::from(uri))
    }

    pub fn deploy(
        &self,
        private_key: &PrivateKey<N>,
        program: &Program<N>,
        fee_record: Option<Record<N, Plaintext<N>>>,
        priority_fee: u64,
    ) -> Result<Transaction<N>> {
        let query = self.create_query()?;

        let rng = &mut rand::thread_rng();
        let transaction = self
            .vm
            .deploy(
                private_key,
                program,
                fee_record,
                priority_fee,
                Some(&query),
                rng,
            )
            .map_err(|e| {
                Error::VmError(format!("Failed to create deployment transaction: {}", e))
            })?;

        Ok(transaction)
    }

    pub fn execute(
        &self,
        private_key: &PrivateKey<N>,
        program_id: impl TryInto<ProgramID<N>>,
        function_name: impl TryInto<Identifier<N>>,
        inputs: impl ExactSizeIterator<Item = impl TryInto<Value<N>>>,
        fee_record: Option<Record<N, Plaintext<N>>>,
        priority_fee: u64,
    ) -> Result<(Transaction<N>, Vec<Value<N>>)> {
        let query = self.create_query()?;

        let rng = &mut rand::thread_rng();
        let (transaction, response) = self
            .vm
            .execute_with_response(
                private_key,
                (program_id, function_name),
                inputs,
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
    }

    pub fn authorize(
        &self,
        private_key: &PrivateKey<N>,
        program_id: impl TryInto<ProgramID<N>>,
        function_name: impl TryInto<Identifier<N>>,
        inputs: impl IntoIterator<IntoIter = impl ExactSizeIterator<Item = impl TryInto<Value<N>>>>,
    ) -> Result<Authorization<N>> {
        let rng = &mut rand::thread_rng();
        let authorization = self
            .vm
            .authorize(private_key, program_id, function_name, inputs, rng)
            .map_err(|e| Error::VmError(format!("Failed to create authorization: {}", e)))?;

        Ok(authorization)
    }
}
