use delegated_proving_test_bindings::delegated_proving_test::*;
use leo_bindings::leo_bindings_sdk::{Account, Client, Credentials, VMManager};
use leo_bindings::utils::init_test_logger;
use snarkvm::prelude::*;

const ENDPOINT: &str = "https://api.explorer.provable.com";

const TEST_A: u64 = 1000;
const TEST_B: u64 = 10;
const TEST_C: u64 = 2;
const TEST_D: u64 = 1;
const EXPECTED: u64 = 150;

#[tokio::test]
async fn test_interpreter() {
    init_test_logger();
    let alice: Account<TestnetV0> = Account::dev_account(0).unwrap();

    let program = DelegatedProvingTestInterpreter::new(&alice)
        .await
        .unwrap();

    let result = program
        .divide(&alice, TEST_A, TEST_B, TEST_C, TEST_D)
        .await
        .unwrap();
    assert_eq!(result, EXPECTED);
    println!(
        "Interpreter test passed: divide({}, {}, {}, {}) = {}",
        TEST_A, TEST_B, TEST_C, TEST_D, result
    );
}

#[tokio::test]
async fn test_network_local_proving() {
    init_test_logger();

    let alice: Account<TestnetV0> = Account::from_env().unwrap();

    let client = Client::new(ENDPOINT, None).unwrap();
    let vm_manager = VMManager::new(&client).unwrap();
    let program = DelegatedProvingTestTestnet::new(&alice, vm_manager)
        .await
        .unwrap();

    let result = program
        .divide(&alice, TEST_A, TEST_B, TEST_C, TEST_D)
        .await
        .unwrap();
    assert_eq!(result, EXPECTED);
    println!(
        "Local proving test passed: divide({}, {}, {}, {}) = {}",
        TEST_A, TEST_B, TEST_C, TEST_D, result
    );
}

#[tokio::test]
async fn test_network_delegated_proving() {
    init_test_logger();

    let alice: Account<TestnetV0> = Account::from_env().unwrap();

    let credentials = Credentials::from_env().ok();
    let client = Client::new(ENDPOINT, credentials).unwrap();
    let vm_manager = VMManager::new(&client).unwrap();

    let program = DelegatedProvingTestTestnet::new(&alice, vm_manager)
        .await
        .unwrap();

    let result = program
        .divide(&alice, TEST_A, TEST_B, TEST_C, TEST_D)
        .await
        .unwrap();
    assert_eq!(result, EXPECTED);
    println!(
        "Delegated proving test passed: divide({}, {}, {}, {}) = {}",
        TEST_A, TEST_B, TEST_C, TEST_D, result
    );
}
