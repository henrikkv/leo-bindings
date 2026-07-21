import adder.aleo;
import multiplier.aleo;

program dyn_example.aleo {
    @noupgrade
    constructor() {}

    fn main(public a: u32, b: u32) -> u32 {
        return a + b;
    }

    fn static_import_two_combines(public a: u32, public b: u32) -> (u32, u32) {
        return (adder.aleo::combine(a, b), multiplier.aleo::combine(a, b));
    }

    fn dynamic_combine_id_routing(add_prog: identifier, mul_prog: identifier, a: u32, b: u32) -> (u32, u32) {
        let from_add: u32 = math_ops_lib::MathOps@(add_prog)::combine(a, b);
        let from_mul: u32 = math_ops_lib::MathOps@(mul_prog)::combine(a, b);
        return (from_add, from_mul);
    }

    fn dynamic_combine_with_network(prog: identifier, net: identifier, a: u32, b: u32) -> u32 {
        return math_ops_lib::MathOps@(prog, net)::combine(a, b);
    }

    fn dynamic_combine_field_target(program_name_as_field: field, a: u32, b: u32) -> u32 {
        return math_ops_lib::MathOps@(program_name_as_field)::combine(a, b);
    }
}
