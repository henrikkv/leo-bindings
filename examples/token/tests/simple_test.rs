#[test]
fn token() {
    use leo_bindings::utils::*;
    use leo_bindings_credits::credits::*;
    use std::str::FromStr;
    use token_bindings::token::*;

    const ENDPOINT: &str = "http://localhost:3030";
    let rng = &mut rand::thread_rng();
    let alice =
        Account::from_str("APrivateKey1zkp8CZNn3yeCseEtxuVPbDCwSyhGW6yZKUYKfgXmcpoGPWH").unwrap();
    let bob = Account::new(rng).unwrap();

    let credits = CreditsTestnet::new(&alice, ENDPOINT).unwrap();
    let balance_before = credits.get_account(alice.address()).unwrap();
    dbg!(balance_before);
    credits
        .transfer_public(&alice, bob.address(), 1_000_000_000_000)
        .unwrap();
    let balance_after = credits.get_account(alice.address()).unwrap();
    dbg!(balance_after);

    let token = TokenTestnet::new(&alice, ENDPOINT).unwrap();

    let rec = token.mint_private(&alice, bob.address(), 100).unwrap();
    dbg!(&rec);
    let (rec1, rec2) = token
        .transfer_private(&bob, rec, bob.address(), 10)
        .unwrap();
    dbg!(&rec1);
    dbg!(&rec2);
}
