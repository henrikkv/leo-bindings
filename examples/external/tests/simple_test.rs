#[test]
fn external() {
    use leo_bindings::utils::*;
    use snarkvm::prelude::TestnetV0;
    use war_game_bindings::war_game::*;

    const ENDPOINT: &str = "http://localhost:3030";
    let alice: Account<TestnetV0> = get_dev_account(0).unwrap();
    let war = WarGameInterpreter::new(&alice, ENDPOINT).unwrap();

    war.create_game(&alice, 1, 1, 2, 3, 5, 29, 91).unwrap();
}
