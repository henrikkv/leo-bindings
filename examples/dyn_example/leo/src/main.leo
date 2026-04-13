import dyn_plugin_add.aleo;
import dyn_plugin_mul.aleo;
import dyn_token_plugin.aleo;

final fn increment_local_counter(key: u64, delta: u32) {
    let cur: u32 = Mapping::get_or_use(local_counter, key, 0u32);
    Mapping::set(local_counter, key, cur + delta);
}

program dyn_example.aleo {
    mapping local_counter: u64 => u32;

    @noupgrade
    constructor() {}

    fn main(public a: u32, b: u32) -> u32 {
        return a + b;
    }

    fn static_import_two_combines(public a: u32, public b: u32) -> (u32, u32) {
        return (dyn_plugin_add.aleo::combine(a, b), dyn_plugin_mul.aleo::combine(a, b));
    }

    fn dynamic_combine_id_routing(add_prog: identifier, mul_prog: identifier, a: u32, b: u32) -> (u32, u32) {
        let from_add: u32 = dyn_plugin_add.aleo::MathOps@(add_prog)::combine(a, b);
        let from_mul: u32 = dyn_plugin_mul.aleo::MathOps@(mul_prog)::combine(a, b);
        return (from_add, from_mul);
    }

    fn dynamic_combine_with_network(prog: identifier, net: identifier, a: u32, b: u32) -> u32 {
        return dyn_plugin_add.aleo::MathOps@(prog, net)::combine(a, b);
    }

    fn dynamic_combine_field_target(program_name_as_field: field, a: u32, b: u32) -> u32 {
        return dyn_plugin_add.aleo::MathOps@(program_name_as_field)::combine(a, b);
    }

    fn aleo_generator() -> group {
        return Aleo::generator();
    }

    fn dyn_record_mint_then_double(
        public amount: u64,
        token_program: identifier,
    ) -> (u64, u64) {
        let concrete_mint = dyn_token_plugin.aleo::mint(amount);
        let minted_as_dyn: dyn record = concrete_mint as dyn record;
        let doubled_as_dyn: dyn record =
            dyn_token_plugin.aleo::TokenOps@(token_program)::double_balance(minted_as_dyn);
        let doubled_balance: u64 = doubled_as_dyn.balance;
        return (amount, doubled_balance);
    }

    fn final_compose_import_and_local(target: field, public key: u64, public delta: u32) -> (u32, Final) {
        let (v, f): (u32, Final) = dyn_plugin_add.aleo::Counter@(target)::increment(delta);
        return (
            v,
            final {
                f.run();
                increment_local_counter(key, delta);
            },
        );
    }
}
