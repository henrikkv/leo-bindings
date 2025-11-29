use dev_bindings::dev::*;
use leo_bindings::utils::*;
use snarkvm::prelude::Network;
use std::str::FromStr;

const ENDPOINT: &str = "http://localhost:3030";

#[test]
fn dev_testnet() {
    leo_bindings::utils::init_test_logger();
    let alice = get_dev_account(0).unwrap();
    run_dev_tests(&DevTestnet::new(&alice, ENDPOINT).unwrap(), &alice);
}

#[test]
fn dev_interpreter() {
    leo_bindings::utils::init_test_logger();
    let alice = get_dev_account(0).unwrap();
    let dev = DevInterpreter::new(&alice, ENDPOINT).unwrap();
    run_dev_tests(&dev, &alice);

    leo_bindings::interpreter_cheats::set_block_height(1000);
    leo_bindings::interpreter_cheats::set_block_timestamp(1234567890);

    dev.store_block_info(&alice, 0).unwrap();
    assert_eq!(dev.get_block_heights(0), Some(1000u32));
    assert_eq!(dev.get_block_timestamps(0), Some(1234567890i64));
}

fn run_dev_tests<N: Network, P: DevAleo<N>>(dev: &P, alice: &Account<N>) {
    let user = dev.create_user(alice, alice.address(), 0, 0).unwrap();
    dbg!(&user);
    let balance = dev.consume_user(alice, user).unwrap();
    dbg!(balance);

    let a = A::new(1);
    let b = B::new(2, a);
    let container = dev.create_container(alice, alice.address(), b).unwrap();
    dbg!(&container);
    let extracted_b = dev.consume_container(alice, container).unwrap();
    dbg!(&extracted_b);

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
