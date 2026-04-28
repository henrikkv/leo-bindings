use leo_bindings_sdk::snarkvm::prelude::*;
use leo_bindings_sdk::{Account, Client, NetworkVm, ToValue, block_on};

#[test]
fn test_mapping_query() {
    let account = Account::<TestnetV0>::from_env().unwrap();
    let client = Client::new("https://api.explorer.provable.com", None).unwrap();
    let key = account.address().to_value();

    let _ = block_on(client.mapping::<TestnetV0>("credits.aleo", "account", &key)).unwrap();
}

#[test]
fn test_transfer_credits() {
    let account = Account::<TestnetV0>::from_env().unwrap();
    let client = Client::new("https://api.explorer.provable.com", None).unwrap();
    let vm = NetworkVm::new(&client).unwrap();

    let credits = block_on(client.program::<TestnetV0>("credits.aleo")).unwrap();
    let program = Program::from_str(&credits).unwrap();
    vm.add_program(&program).unwrap();

    let (tx, _) = vm
        .execute(
            account.private_key(),
            &"credits.aleo".try_into().unwrap(),
            &"transfer_public".try_into().unwrap(),
            vec![account.address().to_value(), 1u64.to_value()],
            None,
            0,
        )
        .unwrap();

    block_on(client.broadcast_wait(&tx)).unwrap();
}

#[test]
fn test_delegated_proving() {
    let account = Account::<TestnetV0>::from_env().unwrap();
    let client = Client::from_env().unwrap();
    let vm = NetworkVm::new(&client).unwrap();

    let src = block_on(client.program::<TestnetV0>("delegated_proving_test.aleo")).unwrap();
    let program = Program::from_str(&src).unwrap();
    vm.add_program(&program).unwrap();

    let auth = vm
        .authorize(
            account.private_key(),
            &"delegated_proving_test.aleo".try_into().unwrap(),
            &"divide".try_into().unwrap(),
            vec![
                1000u64.to_value(),
                10u64.to_value(),
                2u64.to_value(),
                1u64.to_value(),
            ],
        )
        .unwrap();

    let tx = block_on(client.prove(&auth)).unwrap();
    block_on(client.broadcast_wait(&tx)).unwrap();
}
