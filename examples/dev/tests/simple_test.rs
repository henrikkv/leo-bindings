
#[test]
fn dev() {
    use devtest::*;
    use leo_bindings::utils::*;
    use std::str::FromStr;

    const ENDPOINT: &str = "http://localhost:3030";
    let alice: Account<snarkvm::console::network::TestnetV0> =
        Account::from_str("APrivateKey1zkp8CZNn3yeCseEtxuVPbDCwSyhGW6yZKUYKfgXmcpoGPWH").unwrap();

    let dev = dev::new(&alice, ENDPOINT).unwrap();

    wait_for_program_availability("dev.aleo", ENDPOINT, 60).unwrap();

    let result = dev.main(&alice, 10u32, 5u32).unwrap();
    assert_eq!(result, 15u32);
    println!("✅ Test passed - program is available on network");
    let a = A::new(1);
    let b = B::new(2, a);
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
    let user = dev.create_user(&alice, alice.address(), 0, 0).unwrap();
    dbg!(&user);
    let balance = dev.consume_user(&alice, user).unwrap();
    dbg!(balance);
}
