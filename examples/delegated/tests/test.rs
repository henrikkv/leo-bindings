use delegated_proving_test_bindings::delegated_proving_test::*;
use leo_bindings::leo_bindings_sdk::{Account, Client, Credentials, NetworkVm, VMManager};
use leo_bindings::utils::init_test_logger;
use snarkvm::prelude::*;

const ENDPOINT: &str = "https://api.explorer.provable.com";

const TEST_A: u64 = 1000;
const TEST_B: u64 = 10;
const TEST_C: u64 = 2;
const TEST_D: u64 = 1;
const EXPECTED: u64 = 150;

#[test]
fn test_network_local_proving() {
    init_test_logger();
    let alice: Account<TestnetV0> = Account::from_env().unwrap();
    let client = Client::new(ENDPOINT, None).unwrap();
    let vm_manager = NetworkVm::new(&client).unwrap();
    run_delegated_tests(vm_manager, &alice);
}

#[test]
fn test_network_delegated_proving() {
    init_test_logger();
    let alice: Account<TestnetV0> = Account::from_env().unwrap();
    let credentials = Credentials::from_env().unwrap();
    let client = Client::new(ENDPOINT, Some(credentials)).unwrap();
    let vm_manager = NetworkVm::new(&client).unwrap();
    run_delegated_tests(vm_manager, &alice);
}

fn run_delegated_tests<V: VMManager<TestnetV0>>(vm: V, alice: &Account<TestnetV0>) {
    let program = DelegatedProvingTestAleo::new(alice, vm).unwrap();
    let result = program
        .divide(alice, TEST_A, TEST_B, TEST_C, TEST_D)
        .unwrap();
    assert_eq!(result, EXPECTED);
    println!(
        "Test passed: divide({}, {}, {}, {}) = {}",
        TEST_A, TEST_B, TEST_C, TEST_D, result
    );
}
