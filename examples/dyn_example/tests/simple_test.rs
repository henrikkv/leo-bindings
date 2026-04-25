use dyn_example_bindings::dyn_example::DynExampleAleo;
use leo_bindings::leo_bindings_sdk::{Account, Client, LocalVM, NetworkVm, VMManager};
use leo_bindings::snarkvm::prelude::TestnetV0;

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

    let result = app.main(alice, 10u32, 5u32).unwrap();
    assert_eq!(result, 15u32);

    let result = app.static_import_two_combines(alice, 2u32, 3u32).unwrap();
    assert_eq!(result, (5u32, 6u32));

    let result = app
        .dynamic_combine_id_routing(
            alice,
            "dyn_plugin_add".try_into().unwrap(),
            "dyn_plugin_mul".try_into().unwrap(),
            10u32,
            4u32,
        )
        .unwrap();
    assert_eq!(result, (14u32, 40u32));

    let result = app
        .dynamic_combine_with_network(
            alice,
            "dyn_plugin_add".try_into().unwrap(),
            "aleo".try_into().unwrap(),
            2u32,
            3u32,
        )
        .unwrap();
    assert_eq!(result, 5u32);

    let result = app
        .dynamic_combine_field_target(alice, "dyn_plugin_add".try_into().unwrap(), 10u32, 4u32)
        .unwrap();
    assert_eq!(result, 14u32);

    let result = app
        .dyn_record_mint_then_double(alice, 25u64, "dyn_token_plugin".try_into().unwrap())
        .unwrap();
    assert_eq!(result, (25u64, 50u64));

    let (delta_out, _fin) = app
        .final_compose_import_and_local(alice, "dyn_plugin_add".try_into().unwrap(), 1u64, 12u32)
        .unwrap();
    assert_eq!(delta_out, 12u32);
}
