interface MathOps {
    fn combine(a: u32, b: u32) -> u32;
}

interface Counter {
    fn increment(delta: u32) -> (u32, Final);
}

program dyn_plugin_add.aleo : MathOps + Counter {
    mapping counts: address => u32;

    fn combine(a: u32, b: u32) -> u32 {
        return a + b;
    }

    fn increment(delta: u32) -> (u32, Final) {
        let who: address = self.caller;
        return (
            delta,
            final {
                let cur: u32 = Mapping::get_or_use(counts, who, 0u32);
                Mapping::set(counts, who, cur + delta);
            },
        );
    }

    @noupgrade
    constructor() {}
}
