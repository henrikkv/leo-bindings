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

    let result = dev.main(&alice, 10u32, 5u32).unwrap();
    dbg!(result);

    let a = A::new(1);
    let b = B::new(2, a);
    let result = dev.nested(&alice, b).unwrap();
    dbg!(result);

    let user = dev.create_user(&alice, alice.address(), 0, 0).unwrap();
    dbg!(&user);
    let balance = dev.consume_user(&alice, user).unwrap();
    dbg!(balance);
}
