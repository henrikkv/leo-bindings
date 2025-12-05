use delegated_proving_test_bindings::delegated_proving_test::*;
use leo_bindings::utils::*;
use snarkvm::prelude::*;

const ENDPOINT: &str = "https://api.explorer.provable.com";

const TEST_A: u64 = 1000;
const TEST_B: u64 = 10;
const TEST_C: u64 = 2;
const TEST_D: u64 = 1;
const EXPECTED: u64 = 150;

#[test]
fn test_interpreter() {
    init_test_logger();
    let alice = get_dev_account(0).unwrap();

    let program = DelegatedProvingTestInterpreter::new(&alice, ENDPOINT).unwrap();

    let result = program
        .divide(&alice, TEST_A, TEST_B, TEST_C, TEST_D)
        .unwrap();
    assert_eq!(result, EXPECTED);
    println!(
        "✅ Interpreter test passed: divide({}, {}, {}, {}) = {}",
        TEST_A, TEST_B, TEST_C, TEST_D, result
    );
}

#[test]
fn test_network_local_proving() {
    init_test_logger();

    let alice: Account<TestnetV0> = get_account_from_env().unwrap();

    let program = DelegatedProvingTestTestnet::new(&alice, ENDPOINT).unwrap();

    let result = program
        .divide(&alice, TEST_A, TEST_B, TEST_C, TEST_D)
        .unwrap();
    assert_eq!(result, EXPECTED);
    println!(
        "✅ Local proving test passed: divide({}, {}, {}, {}) = {}",
        TEST_A, TEST_B, TEST_C, TEST_D, result
    );
}

#[test]
fn test_network_delegated_proving() {
    init_test_logger();

    let alice: Account<TestnetV0> = get_account_from_env().unwrap();

    let delegation_config = DelegatedProvingConfig::from_env().unwrap();

    let program = DelegatedProvingTestTestnet::new(&alice, ENDPOINT)
        .unwrap()
        .configure_delegation(delegation_config)
        .enable_delegation();

    let result = program
        .divide(&alice, TEST_A, TEST_B, TEST_C, TEST_D)
        .unwrap();
    assert_eq!(result, EXPECTED);
    println!(
        "✅ Delegated proving test passed: divide({}, {}, {}, {}) = {}",
        TEST_A, TEST_B, TEST_C, TEST_D, result
    );
}
