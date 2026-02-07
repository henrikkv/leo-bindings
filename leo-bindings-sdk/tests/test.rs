use leo_bindings_sdk::{Account, Client, VMManager};
use snarkvm::prelude::*;

const ENDPOINT: &str = "https://api.explorer.provable.com";

#[tokio::test]
async fn test_mapping_query() {
    let account = Account::<TestnetV0>::from_env().unwrap();

    let client = Client::new(ENDPOINT, None).unwrap();

    let key = Value::from(Literal::Address(account.address()));

    let result = client
        .mapping::<TestnetV0>("credits.aleo", "account", &key)
        .await;

    assert!(result.is_ok(), "Mapping query failed: {:?}", result.err());

    let _value = result.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_transfer_credits() {
    let account = Account::<TestnetV0>::from_env().unwrap();

    println!("🔑 Account address: {}", account.address());

    let client = Client::new(ENDPOINT, None).unwrap();

    let vm_manager = VMManager::<TestnetV0>::new(&client).unwrap();

    let key = Value::from(Literal::Address(account.address()));
    let balance_before = client
        .mapping::<TestnetV0>("credits.aleo", "account", &key)
        .await
        .expect("Failed to query balance");

    println!("💰 Balance before: {:?}", balance_before);

    println!("📥 Fetching credits.aleo program...");
    let credits_program_str = client
        .program::<TestnetV0>("credits.aleo")
        .await
        .expect("Failed to fetch credits.aleo");

    let credits_program =
        Program::<TestnetV0>::from_str(&credits_program_str).expect("Failed to parse credits.aleo");

    vm_manager
        .add_program(&credits_program)
        .await
        .expect("Failed to add credits.aleo to VM");

    let transfer_amount = 1u64;
    let inputs = vec![
        Value::from(Literal::Address(account.address())),
        Value::from(Literal::U64(U64::new(transfer_amount))),
    ];

    println!("📝 Creating transfer transaction...");

    let (transaction, _outputs) = vm_manager
        .execute(
            account.private_key(),
            "credits.aleo",
            "transfer_public",
            inputs,
            None,
            0,
        )
        .await
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
    let account = Account::<TestnetV0>::from_env().unwrap();

    println!("🔑 Account address: {}", account.address());

    let client = Client::from_env().unwrap();

    let vm_manager = VMManager::<TestnetV0>::new(&client).unwrap();

    let program_id = "delegated_proving_test.aleo";

    println!("📥 Fetching program from network...");
    let program_text = client
        .program::<TestnetV0>(program_id)
        .await
        .expect("Failed to fetch program");

    let program = Program::<TestnetV0>::from_str(&program_text).expect("Failed to parse program");

    vm_manager
        .add_program(&program)
        .await
        .expect("Failed to add program to VM");

    let inputs = vec![
        Value::from(Literal::U64(U64::new(1000))),
        Value::from(Literal::U64(U64::new(10))),
        Value::from(Literal::U64(U64::new(2))),
        Value::from(Literal::U64(U64::new(1))),
    ];

    println!("📝 Creating authorization for delegated proving...");

    let authorization = vm_manager
        .authorize(account.private_key(), program_id, "divide", inputs)
        .await
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
