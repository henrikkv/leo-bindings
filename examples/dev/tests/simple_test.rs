#[test]
fn dev() {
    use devtest::dev_aleo::*;
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
    let field_b = snarkvm::prelude::Field::from_str("456field").unwrap();
    dev.test_field(&alice, field_a).unwrap();
    dev.test_field_tuple(&alice, field_a, field_b).unwrap();

    let scalar_a = snarkvm::prelude::Scalar::from_str("789scalar").unwrap();
    let scalar_b = snarkvm::prelude::Scalar::from_str("321scalar").unwrap();
    dev.test_scalar(&alice, scalar_a).unwrap();
    dev.test_scalar_tuple(&alice, scalar_a, scalar_b).unwrap();

    let group_generator = snarkvm::prelude::Group::generator();
    let group_a = group_generator * scalar_a;
    let group_b = group_generator * scalar_b;
    dev.test_group(&alice, group_a).unwrap();
    dev.test_group_tuple(&alice, group_a, group_b).unwrap();

    dev.test_simple_tuple(&alice, 100u32, 1000u64).unwrap();
    dev.test_mixed_tuple(&alice, field_a, true, 999u32).unwrap();
    dev.test_field_scalar_tuple(&alice, field_a, scalar_a)
        .unwrap();
    dev.test_group_field_tuple(&alice, group_a, field_a)
        .unwrap();
    dev.test_all_types(&alice, field_a, scalar_a, group_a, false)
        .unwrap();
}
