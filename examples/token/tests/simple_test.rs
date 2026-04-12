use credits_bindings::credits::*;
use leo_bindings::leo_bindings_sdk::{Account, Client, LocalVM, NetworkVm, VMManager};
use snarkvm::prelude::TestnetV0;
use token_bindings::token::*;

const ENDPOINT: &str = "http://localhost:3030";

#[test]
fn token_net() {
    leo_bindings::utils::init_test_logger();
    let alice: Account<TestnetV0> = Account::dev_account(0).unwrap();
    let client = Client::new(ENDPOINT, None).unwrap();
    let vm_manager = NetworkVm::new(&client).unwrap();
    run_token_tests(vm_manager, &alice);
}

#[test]
fn token_sim() {
    leo_bindings::utils::init_test_logger();
    let alice: Account<TestnetV0> = Account::dev_account(0).unwrap();
    let sim_vm = LocalVM::new().unwrap();
    run_token_tests(sim_vm, &alice);
}

fn run_token_tests<V: VMManager<TestnetV0> + Clone>(vm: V, alice: &Account<TestnetV0>) {
    let rng = &mut rand::thread_rng();
    let bob = Account::new(rng).unwrap();

    let credits = CreditsAleo::new(alice, vm.clone()).unwrap();
    let account0 = Account::dev_account(0).unwrap();
    let account1 = Account::dev_account(1).unwrap();
    let account2 = Account::dev_account(2).unwrap();
    let account3 = Account::dev_account(3).unwrap();
    let b0 = credits.get_account(account0.address()).unwrap();
    let b1 = credits.get_account(account1.address()).unwrap();
    let b2 = credits.get_account(account2.address()).unwrap();
    let b3 = credits.get_account(account3.address()).unwrap();
    dbg!(&account0, &account1, &account2, &account3);
    dbg!(b0, b1, b2, b3);

    let balance_before = credits.get_account(alice.address()).unwrap();
    dbg!(balance_before);
    credits
        .transfer_public(alice, bob.address(), 1_000_000_000_000)
        .unwrap();
    let balance_after = credits.get_account(alice.address()).unwrap();
    dbg!(balance_after);

    let token = TokenAleo::new(alice, vm).unwrap();

    let rec = token.mint_private(alice, bob.address(), 100).unwrap();
    dbg!(&rec);
    let (rec1, rec2) = token
        .transfer_private(&bob, rec, bob.address(), 10)
        .unwrap();
    dbg!(&rec1);
    dbg!(&rec2);
}
