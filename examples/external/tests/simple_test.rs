#[test]
fn external() {
    use external_bindings::war_game::*;
    use leo_bindings::utils::Account;
    use std::str::FromStr;

    const ENDPOINT: &str = "http://localhost:3030";
    let alice =
        Account::from_str("APrivateKey1zkp8CZNn3yeCseEtxuVPbDCwSyhGW6yZKUYKfgXmcpoGPWH").unwrap();
    let war = WarGameInterpreter::new(&alice, ENDPOINT).unwrap();

    war.create_game(&alice, 1, 1, 2, 3, 5, 29, 91).unwrap();
}
