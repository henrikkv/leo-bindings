use dev_bindings::dev::{A, B, DevAleo};
use leo_bindings::leo_bindings_sdk::snapshot_store;
use leo_bindings::leo_bindings_sdk::{Account, Client, LocalVM, NetworkVm, VMManager};
use snarkvm::prelude::TestnetV0;
use std::str::FromStr;

const ENDPOINT: &str = "http://localhost:3030";

#[test]
fn test_dev_net() {
    leo_bindings::utils::init_test_logger();
    let alice: Account<TestnetV0> = Account::dev_account(0).unwrap();
    let client = Client::new(ENDPOINT, None).unwrap();
    let net_vm = NetworkVm::new(&client).unwrap();
    run_dev_tests(net_vm, &alice);
}

#[test]
fn test_dev_sim() {
    leo_bindings::utils::init_test_logger();
    let alice: Account<TestnetV0> = Account::dev_account(0).unwrap();
    let sim_vm = LocalVM::new().unwrap();
    run_dev_tests(sim_vm, &alice);
}

fn run_dev_tests<V: VMManager<TestnetV0>>(vm: V, alice: &Account<TestnetV0>) {
    let dev = DevAleo::new(alice, vm).unwrap();
    let user = dev.create_user(alice, alice.address(), 0, 0).unwrap();
    dbg!(&user);
    let balance = dev.consume_user(alice, user).unwrap();
    dbg!(balance);

    let a = A::new(1);
    let b = B::new(2, a);
    let container = dev.create_container(alice, alice.address(), b).unwrap();
    dbg!(&container);
    let extracted_b = dev.consume_container(alice, container).unwrap();
    dbg!(extracted_b);

    let balance_before = dev.get_balances(0);
    dbg!(&balance_before);

    let (user, future) = dev.asynchronous(alice, 60, 0).unwrap();
    dbg!(user);
    dbg!(future);

    let balance_after = dev.get_balances(0);
    dbg!(&balance_after);

    let result = dev.main(alice, 10u32, 5u32).unwrap();
    assert_eq!(result, 15u32);
    let a = A::new(1);
    let b = B::new(2, a);
    dev.store_nested(alice, b, 1).unwrap();
    dbg!(dev.get_bs(1));
    let result = dev.nested(alice, b).unwrap();
    dbg!(result);
    let result = dev
        .nested_array(
            alice,
            [
                [10, 10, 10, 10, 10],
                [10, 10, 10, 10, 10],
                [10, 10, 10, 10, 10],
                [10, 10, 10, 10, 10],
                [10, 10, 10, 10, 10],
            ],
        )
        .unwrap();
    dbg!(result);

    let field_a = snarkvm::prelude::Field::from_str("123field").unwrap();
    let scalar_a = snarkvm::prelude::Scalar::from_str("789scalar").unwrap();
    let group_generator = snarkvm::prelude::Group::generator();
    let group_a = group_generator * scalar_a;

    dev.test_all_types(alice, field_a, scalar_a, group_a, false)
        .unwrap();
}

#[test]
fn test_mapping_cheat() {
    leo_bindings::utils::init_test_logger();
    let alice: Account<TestnetV0> = Account::dev_account(0).unwrap();
    let sim_vm = LocalVM::new().unwrap();
    let dev = DevAleo::new(&alice, sim_vm).unwrap();

    assert_eq!(dev.get_balances(0u64), None);
    dev.set_balances(0u64, 999u64);
    assert_eq!(dev.get_balances(0u64), Some(999u64));
}

snapshot_store!(SETUP, |store| {
    let alice: Account<TestnetV0> = Account::dev_account(0).unwrap();
    let dev = DevAleo::new(&alice, store.vm().clone()).unwrap();
    store.save("deployed");

    dev.set_balances(0u64, 100u64);
    store.save("with_balance");
});

#[test]
fn test_snapshot_isolation() {
    leo_bindings::utils::init_test_logger();
    let alice: Account<TestnetV0> = Account::dev_account(0).unwrap();

    let dev_a = DevAleo::new(&alice, SETUP.restore("deployed")).unwrap();
    assert_eq!(dev_a.get_balances(0u64), None);
    dev_a.set_balances(0u64, 10u64);
    assert_eq!(dev_a.get_balances(0u64), Some(10u64));

    let dev_b = DevAleo::new(&alice, SETUP.restore("with_balance")).unwrap();
    assert_eq!(dev_a.get_balances(0u64), Some(10u64));
    assert_eq!(dev_b.get_balances(0u64), Some(100u64));

    let dev_c = DevAleo::new(&alice, SETUP.restore("deployed")).unwrap();
    assert_eq!(dev_a.get_balances(0u64), Some(10u64));
    assert_eq!(dev_b.get_balances(0u64), Some(100u64));
    assert_eq!(dev_c.get_balances(0u64), None);
}
