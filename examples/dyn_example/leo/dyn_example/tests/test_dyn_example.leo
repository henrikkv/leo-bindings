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
        let add: identifier = 'adder';
        let mul: identifier = 'multiplier';
        assert_eq(dyn_example.aleo::dynamic_combine_id_routing(add, mul, 10u32, 4u32), (14u32, 40u32));
    }

    @test
    fn dynamic_combine_with_network() {
        let prog: identifier = 'adder';
        let net: identifier = 'aleo';
        assert_eq(dyn_example.aleo::dynamic_combine_with_network(prog, net, 2u32, 3u32), 5u32);
    }

    @test
    fn dynamic_combine_field_target() {
        let add_field: field = 'adder' as field;
        assert_eq(dyn_example.aleo::dynamic_combine_field_target(add_field, 10u32, 4u32), 14u32);
    }
}
