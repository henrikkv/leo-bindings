program dev.aleo {
    @noupgrade
    async constructor() {}

    mapping balances: u64 => u64;

    mapping bs: u64 => B;
    
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

    async transition store_nested(b: B, id: u64) -> Future {
        return finalize_store_nested(b, id);
    }
    async function finalize_store_nested(b: B, id: u64) {
        Mapping::set(bs, id, b);
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

    async transition asynchronous(balance: u64, id: u64) -> (User, Future) {
        let user: User  = User {
            owner: self.caller,
            id: id,
            balance: balance,
        };
        return (user, finalize_asynchronous(balance, id));
    }
    async function finalize_asynchronous(balance: u64, id: u64) {
        Mapping::set(balances, id, balance);
    }

    transition test_field(a: field) -> field {
        return a;
    }

    transition test_field_tuple(a: field, b: field) -> (field, field) {
        return (a, b);
    }

    transition test_scalar(a: scalar) -> scalar {
        return a;
    }

    transition test_scalar_tuple(a: scalar, b: scalar) -> (scalar, scalar) {
        return (a, b);
    }

    transition test_group(g: group) -> group {
        return g;
    }

    transition test_group_tuple(a: group, b: group) -> (group, group) {
        return (a, b);
    }

    transition test_simple_tuple(a: u32, b: u64) -> (u32, u64) {
        return (a, b);
    }

    transition test_mixed_tuple(a: field, b: bool, c: u32) -> (field, bool, u32) {
        return (a, b, c);
    }

    transition test_field_scalar_tuple(f: field, s: scalar) -> (field, scalar) {
        return (f, s);
    }

    transition test_group_field_tuple(g: group, f: field) -> (group, field) {
        return (g, f);
    }

    transition test_all_types(f: field, s: scalar, g: group, flag: bool) -> (field, scalar, group, bool) {
        return (f, s, g, flag);
    }
}
