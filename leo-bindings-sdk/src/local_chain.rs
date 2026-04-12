use crate::account::Account;
use crate::error::{Error, Result};
use aleo_std::StorageMode;
use indexmap::IndexMap;
use rand::{CryptoRng, Rng};
use snarkvm::ledger::block::{Block, Header, Metadata, Ratifications, Transaction, Transactions};
use snarkvm::ledger::committee::{Committee, MIN_VALIDATOR_STAKE};
use snarkvm::ledger::store::ConsensusStorage;
use snarkvm::ledger::store::ConsensusStore;
use snarkvm::ledger::store::helpers::memory::ConsensusMemory;
use snarkvm::prelude::*;
use snarkvm::synthesizer::VM;
use snarkvm::synthesizer::program::{FinalizeGlobalState, FinalizeOperation};
use std::io::{Cursor, Read, Write};

pub const LOCAL_CHAIN_RNG_SEED: u64 = 1234567890;

pub fn local_chain_blob_path() -> Result<std::path::PathBuf> {
    std::env::current_dir()
        .map(|cwd| cwd.join(".consensusblocks"))
        .map_err(|e| Error::Internal(format!("local_chain: current_dir: {e}")))
}

pub fn parse_local_chain_blocks(data: &[u8]) -> Result<Vec<Block<TestnetV0>>> {
    if data.len() < 4 {
        return Err(Error::Internal("local_chain: blob too short".into()));
    }
    let mut c = Cursor::new(data);
    let n =
        u32::read_le(&mut c).map_err(|e| Error::Internal(format!("local_chain: {e}")))? as usize;
    let mut blocks = Vec::with_capacity(n);
    for _ in 0..n {
        let len = u32::read_le(&mut c).map_err(|e| Error::Internal(format!("local_chain: {e}")))?
            as usize;
        let mut buf = vec![0u8; len];
        c.read_exact(&mut buf)
            .map_err(|e| Error::Internal(format!("local_chain: {e}")))?;
        blocks.push(
            Block::<TestnetV0>::read_le(&mut Cursor::new(buf))
                .map_err(|e| Error::Internal(format!("local_chain: block decode: {e}")))?,
        );
    }
    Ok(blocks)
}

pub fn encode_local_chain_blocks<W: Write>(w: &mut W, blocks: &[Block<TestnetV0>]) -> Result<()> {
    (blocks.len() as u32)
        .write_le(&mut *w)
        .map_err(|e| Error::Internal(format!("local_chain: write: {e}")))?;
    for block in blocks {
        let bytes = block
            .to_bytes_le()
            .map_err(|e| Error::Internal(format!("block to_bytes: {e}")))?;
        (bytes.len() as u32)
            .write_le(&mut *w)
            .map_err(|e| Error::Internal(format!("local_chain: write: {e}")))?;
        w.write_all(&bytes)
            .map_err(|e| Error::Internal(format!("local_chain: write: {e}")))?;
    }
    Ok(())
}

pub fn vm_from_local_chain_blocks(
    blocks: &[Block<TestnetV0>],
) -> Result<VM<TestnetV0, ConsensusMemory<TestnetV0>>> {
    let store =
        ConsensusStore::<TestnetV0, ConsensusMemory<TestnetV0>>::open(StorageMode::new_test(None))
            .map_err(|e| Error::Internal(format!("consensus store: {e}")))?;
    let vm = VM::from(store).map_err(|e| Error::Internal(format!("VM: {e}")))?;
    for block in blocks {
        vm.add_next_block(block)
            .map_err(|e| Error::Internal(format!("add_next_block: {e}")))?;
    }
    let h = vm.block_store().current_block_height();
    let v = TestnetV0::CONSENSUS_VERSION(h).map_err(|e| Error::Internal(e.to_string()))?;
    if v < ConsensusVersion::V14 {
        return Err(Error::Internal(format!(
            "local_chain: need consensus >= V14 at height {h}, got {v:?}"
        )));
    }
    Ok(vm)
}

pub fn build_local_chain_blocks() -> Result<Vec<Block<TestnetV0>>> {
    use rand_chacha::ChaCha8Rng;
    use rand_chacha::rand_core::SeedableRng;

    let mut rng = ChaCha8Rng::seed_from_u64(LOCAL_CHAIN_RNG_SEED);
    let store =
        ConsensusStore::<TestnetV0, ConsensusMemory<TestnetV0>>::open(StorageMode::new_test(None))
            .map_err(|e| Error::Internal(format!("Failed to create consensus store: {e}")))?;
    let vm: VM<TestnetV0, ConsensusMemory<TestnetV0>> =
        VM::from(store).map_err(|e| Error::Internal(format!("Failed to create VM: {e}")))?;
    let (genesis, beacon_key) = genesis_dev_quorum(&vm, &mut rng)?;
    let mut blocks = vec![genesis.clone()];
    vm.add_next_block(&genesis)
        .map_err(|e| Error::Internal(format!("add_next_block genesis: {e}")))?;
    let n = TestnetV0::CONSENSUS_HEIGHT(ConsensusVersion::V14)
        .map_err(|e| Error::Internal(format!("CONSENSUS_HEIGHT(V14): {e}")))?;
    for _ in 0..n {
        let b = next_empty_block(&vm, &beacon_key, &mut rng)?;
        blocks.push(b.clone());
        vm.add_next_block(&b)
            .map_err(|e| Error::Internal(format!("add_next_block (local chain build): {e}")))?;
    }
    Ok(blocks)
}

pub fn build_local_chain_bytes() -> Result<Vec<u8>> {
    let blocks = build_local_chain_blocks()?;
    let mut v = Vec::new();
    encode_local_chain_blocks(&mut v, &blocks)?;
    Ok(v)
}

pub(crate) fn load_or_create_local_chain_bytes() -> Result<Vec<u8>> {
    let path = local_chain_blob_path()?;
    if let Ok(bytes) = std::fs::read(&path)
        && parse_local_chain_blocks(&bytes)
            .map(|b| !b.is_empty())
            .unwrap_or(false)
    {
        return Ok(bytes);
    }
    log::info!("LocalVM: writing local chain blob {}", path.display());
    let bytes = build_local_chain_bytes()?;
    std::fs::write(&path, &bytes)
        .map_err(|e| Error::Internal(format!("write {}: {e}", path.display())))?;
    Ok(bytes)
}

fn genesis_dev_quorum<R: Rng + CryptoRng>(
    vm: &VM<TestnetV0, ConsensusMemory<TestnetV0>>,
    rng: &mut R,
) -> Result<(Block<TestnetV0>, PrivateKey<TestnetV0>)> {
    const N: usize = 4;
    let mut private_keys = Vec::with_capacity(N);
    for i in 0..N {
        let i = i as u16;
        private_keys.push(
            *Account::<TestnetV0>::dev_account(i)
                .map_err(|e| Error::Internal(format!("dev_account({i}): {e}")))?
                .private_key(),
        );
    }
    let beacon_key = private_keys[0];

    let mut members = IndexMap::with_capacity(N);
    for key in &private_keys {
        let addr = Address::try_from(key)
            .map_err(|e| Error::Internal(format!("Address::try_from: {e}")))?;
        members.insert(addr, (MIN_VALIDATOR_STAKE, true, 0u8));
    }
    let committee = Committee::<TestnetV0>::new_genesis(members)
        .map_err(|e| Error::Internal(format!("Committee::new_genesis: {e}")))?;

    let remaining = TestnetV0::STARTING_SUPPLY
        .checked_sub(MIN_VALIDATOR_STAKE * (N as u64))
        .ok_or_else(|| {
            Error::Internal("Not enough starting supply for genesis validators".into())
        })?;

    let mut public_balances = IndexMap::with_capacity(N);
    for key in &private_keys {
        let addr = Address::try_from(key)
            .map_err(|e| Error::Internal(format!("Address::try_from: {e}")))?;
        public_balances.insert(addr, remaining / N as u64);
    }

    let bonded_balances = committee
        .members()
        .iter()
        .map(|(address, (amount, _, _))| (*address, (*address, *address, *amount)))
        .collect();

    let genesis = vm
        .genesis_quorum(
            &beacon_key,
            committee,
            public_balances,
            bonded_balances,
            rng,
        )
        .map_err(|e| Error::Internal(format!("genesis_quorum: {e}")))?;
    Ok((genesis, beacon_key))
}

fn next_empty_block<C: ConsensusStorage<TestnetV0>, R: Rng + CryptoRng>(
    vm: &VM<TestnetV0, C>,
    beacon_key: &PrivateKey<TestnetV0>,
    rng: &mut R,
) -> Result<Block<TestnetV0>> {
    let dt = TestnetV0::BLOCK_TIME as i64;
    let (ratifications, transactions, aborted, ratified_finalize) = vm
        .speculate(
            construct_finalize_global_state(vm, dt),
            dt,
            Some(0u64),
            vec![],
            &None.into(),
            [].into_iter(),
            rng,
        )
        .map_err(|e| Error::Internal(format!("speculate (advance chain): {e}")))?;
    if !aborted.is_empty() {
        return Err(Error::Internal(format!(
            "local_chain: empty advance aborted: {aborted:?}"
        )));
    }
    construct_next_block(
        vm,
        dt,
        beacon_key,
        ratifications,
        transactions,
        aborted,
        ratified_finalize,
        rng,
    )
}

pub(crate) fn commit_transaction<R: Rng + CryptoRng>(
    vm: &VM<TestnetV0, ConsensusMemory<TestnetV0>>,
    beacon_key: &snarkvm::prelude::PrivateKey<TestnetV0>,
    transaction: &Transaction<TestnetV0>,
    rng: &mut R,
) -> Result<()> {
    let dt = TestnetV0::BLOCK_TIME as i64;
    let (ratifications, transactions, aborted, ratified_finalize) = vm
        .speculate_local_proofless(
            construct_finalize_global_state(vm, dt),
            dt,
            Some(0u64),
            vec![],
            &None.into(),
            std::iter::once(transaction),
            rng,
        )
        .map_err(|e| Error::Internal(format!("speculate_local_proofless: {e}")))?;
    if !aborted.is_empty() {
        return Err(Error::Internal(format!(
            "local_chain: proofless transaction aborted (ids): {aborted:?}"
        )));
    }
    let block = construct_next_block(
        vm,
        dt,
        beacon_key,
        ratifications,
        transactions,
        aborted,
        ratified_finalize,
        rng,
    )?;
    vm.add_next_block(&block)
        .map_err(|e| Error::Internal(format!("add_next_block: {e}")))?;
    Ok(())
}

fn construct_finalize_global_state<C: ConsensusStorage<TestnetV0>>(
    vm: &VM<TestnetV0, C>,
    time_since_last_block: i64,
) -> FinalizeGlobalState {
    let block_height = vm.block_store().max_height().unwrap();
    let latest_block_hash = vm
        .block_store()
        .get_block_hash(block_height)
        .unwrap()
        .unwrap();
    let latest_block = vm
        .block_store()
        .get_block(&latest_block_hash)
        .unwrap()
        .unwrap();
    let next_round = latest_block.round().saturating_add(1);
    let next_height = latest_block.height().saturating_add(1);
    let block_timestamp = match next_height
        >= TestnetV0::CONSENSUS_HEIGHT(ConsensusVersion::V12).unwrap_or_default()
    {
        true => Some(
            latest_block
                .timestamp()
                .saturating_add(time_since_last_block),
        ),
        false => None,
    };
    FinalizeGlobalState::new::<TestnetV0>(
        next_round,
        next_height,
        block_timestamp,
        latest_block.cumulative_weight(),
        0u128,
        latest_block.hash(),
    )
    .expect("FinalizeGlobalState::new")
}

fn construct_next_block<C: ConsensusStorage<TestnetV0>, R: Rng + CryptoRng>(
    vm: &VM<TestnetV0, C>,
    time_since_last_block: i64,
    private_key: &PrivateKey<TestnetV0>,
    ratifications: Ratifications<TestnetV0>,
    transactions: Transactions<TestnetV0>,
    aborted_transaction_ids: Vec<<TestnetV0 as Network>::TransactionID>,
    ratified_finalize_operations: Vec<FinalizeOperation<TestnetV0>>,
    rng: &mut R,
) -> Result<Block<TestnetV0>> {
    let block_hash = vm
        .block_store()
        .get_block_hash(vm.block_store().max_height().unwrap())
        .unwrap()
        .unwrap();
    let previous_block = vm.block_store().get_block(&block_hash).unwrap().unwrap();

    let metadata = Metadata::new(
        TestnetV0::ID,
        previous_block.round() + 1,
        previous_block.height() + 1,
        0,
        0,
        TestnetV0::GENESIS_COINBASE_TARGET,
        TestnetV0::GENESIS_PROOF_TARGET,
        previous_block.last_coinbase_target(),
        previous_block.last_coinbase_timestamp(),
        previous_block
            .timestamp()
            .saturating_add(time_since_last_block),
    )
    .map_err(|e| Error::Internal(format!("Metadata::new: {e}")))?;

    let header = Header::from(
        vm.block_store().current_state_root(),
        transactions
            .to_transactions_root()
            .map_err(|e| Error::Internal(e.to_string()))?,
        transactions
            .to_finalize_root(ratified_finalize_operations)
            .map_err(|e| Error::Internal(e.to_string()))?,
        ratifications
            .to_ratifications_root()
            .map_err(|e| Error::Internal(e.to_string()))?,
        Field::zero(),
        Field::zero(),
        metadata,
    )
    .map_err(|e| Error::Internal(format!("Header::from: {e}")))?;

    Block::new_beacon(
        private_key,
        previous_block.hash(),
        header,
        ratifications,
        None.into(),
        vec![],
        transactions,
        aborted_transaction_ids,
        rng,
    )
    .map_err(|e| Error::Internal(format!("Block::new_beacon: {e}")))
}
