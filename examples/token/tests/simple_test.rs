#[test]
fn token() {
    use credits_bindings::credits::*;
    use leo_bindings::utils::*;
    use token_bindings::token::*;

    const ENDPOINT: &str = "http://localhost:3030";
    let rng = &mut rand::thread_rng();
    let alice = get_dev_account(0).unwrap();
    let bob = Account::new(rng).unwrap();

    let credits = CreditsTestnet::new(&alice, ENDPOINT).unwrap();
    let account0 = get_dev_account(0).unwrap();
    let account1 = get_dev_account(1).unwrap();
    let account2 = get_dev_account(2).unwrap();
    let account3 = get_dev_account(3).unwrap();
    let b0 = credits.get_account(account0.address()).unwrap();
    let b1 = credits.get_account(account1.address()).unwrap();
    let b2 = credits.get_account(account2.address()).unwrap();
    let b3 = credits.get_account(account3.address()).unwrap();
    dbg!(account0, account1, account2, account3);
    dbg!(b0, b1, b2, b3);

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
