#[test]
fn token() {
    use leo_bindings::utils::*;
    use std::str::FromStr;
    use tokenexample::token_aleo::token;

    const ENDPOINT: &str = "http://localhost:3030";
    let alice: Account<snarkvm::console::network::TestnetV0> =
        Account::from_str("APrivateKey1zkp8CZNn3yeCseEtxuVPbDCwSyhGW6yZKUYKfgXmcpoGPWH").unwrap();

    let token = token::new(&alice, ENDPOINT).unwrap();

    wait_for_program_availability("token.aleo", ENDPOINT, 60).unwrap();

    let rec = token.mint_private(&alice, alice.address(), 100).unwrap();
    dbg!(&rec);
    let (rec1, rec2) = token
        .transfer_private(&alice, rec, alice.address(), 10)
        .unwrap();
    dbg!(&rec1);
    dbg!(&rec2);
}
