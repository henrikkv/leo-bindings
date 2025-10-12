#[test]
fn external() {
    use leo_bindings::utils::Account;
    use std::str::FromStr;
    use war_game_bindings::war_game::*;

    const ENDPOINT: &str = "http://localhost:3030";
    let alice =
        Account::from_str("APrivateKey1zkp8CZNn3yeCseEtxuVPbDCwSyhGW6yZKUYKfgXmcpoGPWH").unwrap();
    let war = WarGameInterpreter::new(&alice, ENDPOINT).unwrap();

    war.create_game(&alice, 1, 1, 2, 3, 5, 29, 91).unwrap();
}
