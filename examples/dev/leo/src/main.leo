program dev.aleo {
    @noupgrade
    constructor() {}

    mapping balances: u64 => u64;

    mapping bs: u64 => B;

    mapping block_heights: u64 => u32;
    mapping block_timestamps: u64 => i64;

    fn main(public a: u32, public b: u32) -> public u32 {
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
    fn nested(public b: B) -> public B {
        let resulta: A = A {
            number: 1u64,
        };
        let resultb: B = B {
            number: 2u32,
            a: resulta,
        };
        return resultb;
    }

    fn store_nested(b: B, id: u64) -> Final {
        return final { Mapping::set(bs, id, b); };
    }

    fn nested_array(a: [[u8;5u8];5u8]) -> [[u8;5u8];5u8] {
        return [[0u8, 0u8, 0u8, 0u8, 0u8],[0u8, 0u8, 0u8, 0u8, 0u8],[0u8, 0u8, 0u8, 0u8, 0u8],[0u8, 0u8, 0u8, 0u8, 0u8],[0u8, 0u8, 0u8, 0u8, 0u8]];
    }

    record User {
        owner: address,
        id: u64,
        public balance: u64,
    }

    record Container {
        owner: address,
        data: B,
    }

    fn create_user(owner: address, id: u64, balance: u64) -> User {
        let user: User = User {
            owner: owner,
            id: id,
            balance: balance,
        };
        return user;
    }

    fn consume_user(user: User) -> u64 {
        return user.balance;
    }

    fn create_container(owner: address, data: B) -> Container {
        let container: Container = Container {
            owner: owner,
            data: data,
        };
        return container;
    }

    fn consume_container(container: Container) -> B {
        return container.data;
    }

    fn asynchronous(balance: u64, id: u64) -> (User, Final) {
        let user: User  = User {
            owner: self.caller,
            id: id,
            balance: balance,
        };
        return (user, final { Mapping::set(balances, id, balance); });
    }

    fn test_all_types(f: field, s: scalar, g: group, flag: bool) -> (field, scalar, group, bool) {
        return (f, s, g, flag);
    }

    fn store_block_info(id: u64) -> Final {
        return final {
            Mapping::set(block_heights, id, block.height);
            Mapping::set(block_timestamps, id, block.timestamp);
        };
    }
}
