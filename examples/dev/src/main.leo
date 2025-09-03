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
    transition nested_array(a: [[u8;5u8];5u8]) -> [[u8;5u8];5u8] {
        return [[0u8, 0u8, 0u8, 0u8, 0u8],[0u8, 0u8, 0u8, 0u8, 0u8],[0u8, 0u8, 0u8, 0u8, 0u8],[0u8, 0u8, 0u8, 0u8, 0u8],[0u8, 0u8, 0u8, 0u8, 0u8]];
    }
    
    record User {
        owner: address,
        id: u64,
        public balance: u64,
    }
    
    transition create_user(owner: address, id: u64, balance: u64) -> User {
        let user: User = User {
            owner: owner,
            id: id,
            balance: balance,
        };
        return user;
    }
    
    transition consume_user(user: User) -> u64 {
        return user.balance;
    }
}
