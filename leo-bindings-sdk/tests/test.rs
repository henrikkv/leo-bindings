use aleo_std::StorageMode;
use leo_bindings::utils::*;
use leo_bindings_sdk::ProvableClient;
use snarkvm::ledger::query::{Query, QueryTrait};
use snarkvm::ledger::store::ConsensusStore;
use snarkvm::ledger::store::helpers::memory::{BlockMemory, ConsensusMemory};
use snarkvm::prelude::*;
use snarkvm::synthesizer::VM;

const ENDPOINT: &str = "https://api.explorer.provable.com";

#[tokio::test]
async fn test_mapping_query() {
    let account = get_account_from_env().unwrap();

    let client = ProvableClient::<TestnetV0>::new(ENDPOINT);

    let key = Value::from(Literal::Address(account.address()));

    let result = client.mapping("credits.aleo", "account", &key).await;

    assert!(result.is_ok(), "Mapping query failed: {:?}", result.err());

    let _value = result.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_transfer_credits() {
    let account = get_account_from_env().unwrap();

    println!("🔑 Account address: {}", account.address());

    let client = ProvableClient::<TestnetV0>::new(ENDPOINT);

    let key = Value::from(Literal::Address(account.address()));
    let balance_before: Option<Value<TestnetV0>> = client
        .mapping("credits.aleo", "account", &key)
        .await
        .expect("Failed to query balance");

    println!("💰 Balance before: {:?}", balance_before);

    let vm = VM::from(
        ConsensusStore::<TestnetV0, ConsensusMemory<TestnetV0>>::open(StorageMode::Production)
            .unwrap(),
    )
    .unwrap();

    let query = Query::<TestnetV0, BlockMemory<TestnetV0>>::from(
        ENDPOINT.parse::<http::uri::Uri>().unwrap(),
    );

    let rng = &mut rand::thread_rng();

    let program_id = ProgramID::<TestnetV0>::from_str("credits.aleo").unwrap();
    let function_name = Identifier::<TestnetV0>::from_str("transfer_public").unwrap();

    let transfer_amount = 1u64;
    let inputs = [
        Value::from(Literal::Address(account.address())),
        Value::from(Literal::U64(U64::new(transfer_amount))),
    ];

    println!("📝 Creating transfer transaction...");

    let (transaction, _response) = vm
        .execute_with_response(
            account.private_key(),
            (program_id, function_name),
            inputs.iter(),
            None,
            0,
            Some(&query as &dyn QueryTrait<TestnetV0>),
            rng,
        )
        .expect("Failed to create transaction");

    println!("✅ Transaction created: {}", transaction.id());

    println!("📡 Broadcasting transaction...");
    client
        .broadcast_wait(&transaction)
        .await
        .expect("Failed to broadcast transaction");
}

#[tokio::test]
#[ignore]
async fn test_delegated_proving() {
    let account = get_account_from_env().unwrap();
    let consumer_id = std::env::var("PROVABLE_CONSUMER_ID").expect("PROVABLE_CONSUMER_ID not set");
    let api_key = std::env::var("PROVABLE_API_KEY").expect("PROVABLE_API_KEY not set");

    println!("🔑 Account address: {}", account.address());

    let client = ProvableClient::<TestnetV0>::with_jwt_credentials(ENDPOINT, consumer_id, api_key);

    let vm = VM::from(
        ConsensusStore::<TestnetV0, ConsensusMemory<TestnetV0>>::open(StorageMode::Production)
            .unwrap(),
    )
    .unwrap();

    let program_id = ProgramID::<TestnetV0>::from_str("delegated_proving_test.aleo").unwrap();

    println!("📥 Fetching program from network...");
    let program_text = client
        .program(&program_id.to_string())
        .await
        .expect("Failed to fetch program");

    let program = snarkvm::prelude::Program::<TestnetV0>::from_str(&program_text)
        .expect("Failed to parse program");

    vm.process()
        .write()
        .add_program(&program)
        .expect("Failed to add program to VM");

    let rng = &mut rand::thread_rng();
    let function_name = Identifier::<TestnetV0>::from_str("divide").unwrap();

    let inputs = [
        Value::from(Literal::U64(U64::new(1000))),
        Value::from(Literal::U64(U64::new(10))),
        Value::from(Literal::U64(U64::new(2))),
        Value::from(Literal::U64(U64::new(1))),
    ];

    println!("📝 Creating authorization for delegated proving...");

    let authorization = vm
        .authorize(
            account.private_key(),
            program_id,
            function_name,
            inputs.iter(),
            rng,
        )
        .expect("Failed to create authorization");

    println!("✅ Authorization created");

    let proved_transaction = client
        .prove(&authorization)
        .await
        .expect("Failed to get proved transaction");

    println!(
        "✅ Transaction proved remotely: {}",
        proved_transaction.id()
    );

    println!("📡 Broadcasting transaction...");
    client
        .broadcast_wait(&proved_transaction)
        .await
        .expect("Failed to broadcast transaction");

    println!("✅ Transaction confirmed");
}
