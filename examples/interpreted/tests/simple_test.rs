#[test]
fn dev() {
    use interpreted_bindings::dev_interpreter::*;
    use leo_bindings::utils::*;
    use snarkvm::prelude::TestnetV0;
    use std::str::FromStr;

    const ENDPOINT: &str = "http://localhost:3030";
    let alice: Account<TestnetV0> =
        Account::from_str("APrivateKey1zkp8CZNn3yeCseEtxuVPbDCwSyhGW6yZKUYKfgXmcpoGPWH").unwrap();

    let dev = dev::new(&alice, ENDPOINT).unwrap();

    let user = dev.create_user(&alice, alice.address(), 0, 0).unwrap();
    dbg!(&user);
    let balance = dev.consume_user(&alice, user).unwrap();
    dbg!(balance);

    let a = A::new(1);
    let b = B::new(2, a);
    let container = dev.create_container(&alice, alice.address(), b).unwrap();
    dbg!(&container);
    let extracted_b = dev.consume_container(&alice, container).unwrap();
    dbg!(&extracted_b);

    let balance_before = dev.get_balances(0);
    dbg!(&balance_before);

    let (user, future) = dev.asynchronous(&alice, 60, 0).unwrap();
    dbg!(user);
    dbg!(future);

    let balance_after = dev.get_balances(0);
    dbg!(&balance_after);

    let result = dev.main(&alice, 10u32, 5u32).unwrap();
    assert_eq!(result, 15u32);
    let a = A::new(1);
    let b = B::new(2, a);
    dev.store_nested(&alice, b, 1).unwrap();
    dbg!(dev.get_bs(1));
    let result = dev.nested(&alice, b).unwrap();
    dbg!(result);
    let result = dev
        .nested_array(
            &alice,
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

    dev.test_all_types(&alice, field_a, scalar_a, group_a, false)
        .unwrap();
}
