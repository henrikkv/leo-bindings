use arc20_bindings::IARC20;
use arc20_token_bindings::arc20_token::Arc20TokenAleo;
use leo_bindings::leo_bindings_sdk::{Account, Client, LocalVM, NetworkVm, VMManager};
use snarkvm::prelude::TestnetV0;

#[test]
fn arc20_net() {
    leo_bindings::utils::init_test_logger();
    let client = Client::new("http://localhost:3030", None).unwrap();
    let vm = NetworkVm::new(&client).unwrap();
    run(vm);
}

#[test]
fn arc20_sim() {
    leo_bindings::utils::init_test_logger();
    let vm = LocalVM::new().unwrap();
    run(vm);
}

fn run<V: VMManager<TestnetV0>>(vm: V) {
    let alice: Account<TestnetV0> = Account::dev_account(0).unwrap();
    let bob: Account<TestnetV0> = Account::dev_account(1).unwrap();

    let token = Arc20TokenAleo::new(&alice, vm.clone()).unwrap();
    let iarc20 = IARC20::from(vm.clone(), "arc20_token".try_into().unwrap()).unwrap();

    token
        .mint_public(&alice, alice.address(), 1_000u128)
        .unwrap();
    assert_eq!(iarc20.balance_of(alice.address()).unwrap(), 1_000u128);
    assert_eq!(iarc20.supply().unwrap(), 1_000u128);

    iarc20
        .transfer_public(&alice, bob.address(), 250u128)
        .unwrap();
    assert_eq!(iarc20.balance_of(alice.address()).unwrap(), 750u128);
    assert_eq!(iarc20.balance_of(bob.address()).unwrap(), 250u128);

    iarc20
        .approve_public(&alice, bob.address(), 100u128)
        .unwrap();
    assert_eq!(
        iarc20.allowance(alice.address(), bob.address()).unwrap(),
        100u128
    );

    iarc20
        .transfer_from_public(&bob, alice.address(), bob.address(), 40u128)
        .unwrap();
    assert_eq!(
        iarc20.allowance(alice.address(), bob.address()).unwrap(),
        60u128
    );
    assert_eq!(iarc20.balance_of(alice.address()).unwrap(), 710u128);
    assert_eq!(iarc20.balance_of(bob.address()).unwrap(), 290u128);

    iarc20
        .unapprove_public(&alice, bob.address(), 60u128)
        .unwrap();
    assert_eq!(
        iarc20.allowance(alice.address(), bob.address()).unwrap(),
        0u128
    );

    assert_eq!(iarc20.decimals().unwrap(), 6u8);
    assert_eq!(iarc20.max_supply().unwrap(), 1_000_000_000_000_000u128);
    assert_eq!(iarc20.supply().unwrap(), 1_000u128);
    assert_eq!(iarc20.name().unwrap().to_string(), "ExampleToken");
    assert_eq!(iarc20.symbol().unwrap().to_string(), "EXT");

    let minted = token
        .mint_private(&alice, alice.address(), 500u128)
        .unwrap();
    let minted = IARC20::Token::try_from(minted.to_dynamic_record().unwrap()).unwrap();
    assert_eq!(minted.amount(), 500u128);

    let (remainder, split_off) = iarc20.split(&alice, minted, 200u128).unwrap();
    assert_eq!(remainder.amount(), 300u128);
    assert_eq!(split_off.amount(), 200u128);

    let joined = iarc20.join(&alice, remainder, split_off).unwrap();
    assert_eq!(joined.amount(), 500u128);

    let (change, transferred) = iarc20
        .transfer_private(&alice, joined, bob.address(), 150u128)
        .unwrap();
    assert_eq!(change.amount(), 350u128);
    assert_eq!(transferred.amount(), 150u128);
}
