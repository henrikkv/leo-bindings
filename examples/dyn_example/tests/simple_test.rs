use std::str::FromStr;

use dyn_example_bindings::dyn_example::DynExampleAleo;
use leo_bindings::leo_bindings_sdk::{Account, Client, LocalVM, NetworkVm, VMManager};
use leo_bindings::snarkvm::prelude::{Identifier, IdentifierLiteral, TestnetV0, ToField};

const ENDPOINT: &str = "http://localhost:3030";

#[test]
fn test_dyn_example_net() {
    leo_bindings::utils::init_test_logger();
    let alice: Account<TestnetV0> = Account::dev_account(0).unwrap();
    let client = Client::new(ENDPOINT, None).unwrap();
    let net_vm = NetworkVm::new(&client).unwrap();
    run_dyn_example_tests(net_vm, &alice);
}

#[test]
fn test_dyn_example_sim() {
    leo_bindings::utils::init_test_logger();
    let alice: Account<TestnetV0> = Account::dev_account(0).unwrap();
    let sim_vm = LocalVM::new().unwrap();
    run_dyn_example_tests(sim_vm, &alice);
}

fn run_dyn_example_tests<V: VMManager<TestnetV0> + Clone>(vm: V, alice: &Account<TestnetV0>) {
    let app = DynExampleAleo::new(alice, vm).unwrap();

    assert_eq!(app.main(alice, 10u32, 5u32).unwrap(), 15u32);

    assert_eq!(
        app.static_import_two_combines(alice, 2u32, 3u32).unwrap(),
        (5u32, 6u32)
    );

    let add_prog = IdentifierLiteral::<TestnetV0>::new("dyn_plugin_add").unwrap();
    let mul_prog = IdentifierLiteral::<TestnetV0>::new("dyn_plugin_mul").unwrap();
    assert_eq!(
        app.dynamic_combine_id_routing(alice, add_prog, mul_prog, 10u32, 4u32)
            .unwrap(),
        (14u32, 40u32)
    );

    let net_id = IdentifierLiteral::<TestnetV0>::new("aleo").unwrap();
    assert_eq!(
        app.dynamic_combine_with_network(alice, add_prog, net_id, 2u32, 3u32)
            .unwrap(),
        5u32
    );

    let add_field = Identifier::<TestnetV0>::from_str("dyn_plugin_add")
        .unwrap()
        .to_field()
        .unwrap();
    assert_eq!(
        app.dynamic_combine_field_target(alice, add_field, 10u32, 4u32)
            .unwrap(),
        14u32
    );

    let token_program = IdentifierLiteral::<TestnetV0>::new("dyn_token_plugin").unwrap();
    assert_eq!(
        app.dyn_record_mint_then_double(alice, 25u64, token_program)
            .unwrap(),
        (25u64, 50u64)
    );

    let counter_target = Identifier::<TestnetV0>::from_str("dyn_plugin_add")
        .unwrap()
        .to_field()
        .unwrap();
    let (delta_out, _fin) = app
        .final_compose_import_and_local(alice, counter_target, 1u64, 12u32)
        .unwrap();
    assert_eq!(delta_out, 12u32);
}
