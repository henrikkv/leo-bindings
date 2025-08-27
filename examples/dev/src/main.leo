program dev.aleo {
    @noupgrade
    async constructor() {}
    
    transition main(public a: u32, public b: u32) -> public u32 {
        let c: u32 = a + b;
        return c;
    }
    struct A {
        number: u64,
    }
    struct B {
        number: u32,
        a: A,
    }
    transition nested(public b: B) -> public B {
        let resulta: A = A {
            number: 1u64,
        };
        let resultb: B = B {
            number: 2u32,
            a: resulta,
        };
        return resultb;
    }
    transition nested_array(public a: [[u8;5u8];5u8]) -> public [[u8;5u8];5u8] {
        return [[0u8, 0u8, 0u8, 0u8, 0u8],[0u8, 0u8, 0u8, 0u8, 0u8],[0u8, 0u8, 0u8, 0u8, 0u8],[0u8, 0u8, 0u8, 0u8, 0u8],[0u8, 0u8, 0u8, 0u8, 0u8]];
    }
}
