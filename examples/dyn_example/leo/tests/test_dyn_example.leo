import dyn_example.aleo;

program test_dyn_example.aleo {
    @noupgrade
    constructor() {}

    @test
    fn t_main() {
        assert_eq(dyn_example.aleo::main(2u32, 3u32), 5u32);
    }

    @test
    fn static_import_two_combines() {
        assert_eq(dyn_example.aleo::static_import_two_combines(2u32, 3u32), (5u32, 6u32));
    }

    @test
    fn dynamic_combine_id_routing() {
        let add: identifier = 'dyn_plugin_add';
        let mul: identifier = 'dyn_plugin_mul';
        assert_eq(dyn_example.aleo::dynamic_combine_id_routing(add, mul, 10u32, 4u32), (14u32, 40u32));
    }

    @test
    fn dynamic_combine_with_network() {
        let prog: identifier = 'dyn_plugin_add';
        let net: identifier = 'aleo';
        assert_eq(dyn_example.aleo::dynamic_combine_with_network(prog, net, 2u32, 3u32), 5u32);
    }

    @test
    fn dynamic_combine_field_target() {
        let add_field: field = 'dyn_plugin_add' as field;
        assert_eq(dyn_example.aleo::dynamic_combine_field_target(add_field, 10u32, 4u32), 14u32);
    }

    @test
    fn aleo_generator() {
        let g: group = dyn_example.aleo::aleo_generator();
        assert_eq(g, g);
    }

    @test
    fn dyn_record_mint_then_double() {
        let token_program: identifier = 'dyn_token_plugin';
        assert_eq(dyn_example.aleo::dyn_record_mint_then_double(7u64, token_program), (7u64, 14u64));
    }

    @test
    fn final_compose_import_and_local() {
        let target: field = 'dyn_plugin_add' as field;
        let (v, _f) = dyn_example.aleo::final_compose_import_and_local(target, 1u64, 12u32);
        assert_eq(v, 12u32);
    }
}
