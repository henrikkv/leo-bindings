use dyn_example_bindings::dyn_example::DynExampleAleo;
use dyn_example_bindings::{Doubler, MathOps};
use leo_bindings::leo_bindings_sdk::{Account, Client, LocalVM, NetworkVm, VMManager};
use leo_bindings::snarkvm::prelude::TestnetV0;

#[test]
fn test_dyn_example_net() {
    leo_bindings::utils::init_test_logger();
    let client = Client::new("http://localhost:3030", None).unwrap();
    let net_vm = NetworkVm::new(&client).unwrap();
    run_dyn_example_tests(net_vm);
}

#[test]
fn test_dyn_example_sim() {
    leo_bindings::utils::init_test_logger();
    let sim_vm = LocalVM::new().unwrap();
    run_dyn_example_tests(sim_vm);
}

fn run_dyn_example_tests<V: VMManager<TestnetV0>>(vm: V) {
    let alice: Account<TestnetV0> = Account::dev_account(0).unwrap();
    let app = DynExampleAleo::new(&alice, vm.clone()).unwrap();

    let result = app.main(&alice, 10u32, 5u32).unwrap();
    assert_eq!(result, 15u32);

    let result = app.static_import_two_combines(&alice, 2u32, 3u32).unwrap();
    assert_eq!(result, (5u32, 6u32));

    let result = app
        .dynamic_combine_id_routing(
            &alice,
            "adder".try_into().unwrap(),
            "multiplier".try_into().unwrap(),
            10u32,
            4u32,
        )
        .unwrap();
    assert_eq!(result, (14u32, 40u32));

    let result = app
        .dynamic_combine_with_network(
            &alice,
            "adder".try_into().unwrap(),
            "aleo".try_into().unwrap(),
            2u32,
            3u32,
        )
        .unwrap();
    assert_eq!(result, 5u32);

    let result = app
        .dynamic_combine_field_target(&alice, "adder".try_into().unwrap(), 10u32, 4u32)
        .unwrap();
    assert_eq!(result, 14u32);

    let adder = MathOps::from(vm.clone(), "adder".try_into().unwrap()).unwrap();
    assert_eq!(adder.combine(&alice, 2u32, 3u32).unwrap(), 5u32);

    let multiplier = MathOps::from(vm.clone(), "multiplier".try_into().unwrap()).unwrap();
    assert_eq!(multiplier.combine(&alice, 2u32, 3u32).unwrap(), 6u32);

    let doubler = Doubler::from(vm, "dyn_example".try_into().unwrap()).unwrap();
    assert_eq!(doubler.double_it(&alice, 1u32).unwrap(), 2u32);
}
