#[tokio::test]
async fn external() {
    leo_bindings::utils::init_test_logger();

    use leo_bindings::leo_bindings_sdk::Account;
    use snarkvm::prelude::TestnetV0;
    use war_game_bindings::war_game::*;

    let alice: Account<TestnetV0> = Account::dev_account(0).unwrap();
    let war = WarGameInterpreter::new(&alice).await.unwrap();

    war.create_game(&alice, 1, 1, 2, 3, 5, 29, 91)
        .await
        .unwrap();
}
