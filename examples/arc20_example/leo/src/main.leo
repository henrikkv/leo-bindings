export const MAX_SUPPLY: u128 = 1_000_000_000_000_000u128;

struct Approval {
    approver: address,
    spender: address,
}

program arc20_token.aleo : arc20::IARC20 {
    @noupgrade
    constructor() {}

    mapping balances: address => u128;
    mapping allowances: field => u128;

    storage total_supply: u128;

    record Token {
        owner: address,
        amount: u128,
    }



    fn mint_public(public recipient: address, public amount: u128) -> Final {
        return final {
            let minted: u128 = total_supply.unwrap_or(0u128);
            let new_supply: u128 = minted + amount;
            assert(new_supply <= MAX_SUPPLY);
            total_supply = new_supply;

            let balance: u128 = balances.get_or_use(recipient, 0u128);
            balances.set(recipient, balance + amount);
        };
    }

    fn mint_private(private recipient: address, private amount: u128) -> Token {
        return Token {
            owner: recipient,
            amount: amount,
        };
    }



    fn transfer_public(public recipient: address, public amount: u128) -> Final {
        let sender: address = std::ctx::caller();
        return final {
            let sender_balance: u128 = balances.get_or_use(sender, 0u128);
            balances.set(sender, sender_balance - amount);
            let recipient_balance: u128 = balances.get_or_use(recipient, 0u128);
            balances.set(recipient, recipient_balance + amount);
        };
    }

    fn transfer_private(input: Token, private recipient: address, private amount: u128) -> (Token, Token) {
        let change: Token = Token {
            owner: input.owner,
            amount: input.amount - amount,
        };
        let transferred: Token = Token {
            owner: recipient,
            amount: amount,
        };
        return (change, transferred);
    }

    fn transfer_private_to_public(input: Token, public recipient: address, public amount: u128) -> (Token, Final) {
        let change: Token = Token {
            owner: input.owner,
            amount: input.amount - amount,
        };
        return (change, final {
            let recipient_balance: u128 = balances.get_or_use(recipient, 0u128);
            balances.set(recipient, recipient_balance + amount);
        });
    }

    fn transfer_public_to_private(private recipient: address, public amount: u128) -> (Token, Final) {
        let sender: address = std::ctx::caller();
        let transferred: Token = Token {
            owner: recipient,
            amount: amount,
        };
        return (transferred, final {
            let sender_balance: u128 = balances.get_or_use(sender, 0u128);
            balances.set(sender, sender_balance - amount);
        });
    }

    fn transfer_public_as_signer(public recipient: address, public amount: u128) -> Final {
        let sender: address = std::ctx::signer();
        return final {
            let sender_balance: u128 = balances.get_or_use(sender, 0u128);
            balances.set(sender, sender_balance - amount);
            let recipient_balance: u128 = balances.get_or_use(recipient, 0u128);
            balances.set(recipient, recipient_balance + amount);
        };
    }

    fn transfer_from_public(public owner: address, public recipient: address, public amount: u128) -> Final {
        let spender: address = std::ctx::caller();
        return final {
            let key: field = BHP256::hash_to_field(Approval { approver: owner, spender: spender });
            let allowed: u128 = allowances.get_or_use(key, 0u128);
            allowances.set(key, allowed - amount);

            let owner_balance: u128 = balances.get_or_use(owner, 0u128);
            balances.set(owner, owner_balance - amount);
            let recipient_balance: u128 = balances.get_or_use(recipient, 0u128);
            balances.set(recipient, recipient_balance + amount);
        };
    }

    fn transfer_from_public_to_private(
        public owner: address,
        private recipient: address,
        public amount: u128,
    ) -> (Token, Final) {
        let spender: address = std::ctx::caller();
        let transferred: Token = Token {
            owner: recipient,
            amount: amount,
        };
        return (transferred, final {
            let key: field = BHP256::hash_to_field(Approval { approver: owner, spender: spender });
            let allowed: u128 = allowances.get_or_use(key, 0u128);
            allowances.set(key, allowed - amount);

            let owner_balance: u128 = balances.get_or_use(owner, 0u128);
            balances.set(owner, owner_balance - amount);
        });
    }



    fn approve_public(public spender: address, public amount: u128) -> Final {
        let approver: address = std::ctx::caller();
        return final {
            let key: field = BHP256::hash_to_field(Approval { approver: approver, spender: spender });
            let current: u128 = allowances.get_or_use(key, 0u128);
            allowances.set(key, current + amount);
        };
    }

    fn unapprove_public(public spender: address, public amount: u128) -> Final {
        let approver: address = std::ctx::caller();
        return final {
            let key: field = BHP256::hash_to_field(Approval { approver: approver, spender: spender });
            let current: u128 = allowances.get_or_use(key, 0u128);
            allowances.set(key, current - amount);
        };
    }



    fn join(input_1: Token, input_2: Token) -> Token {
        assert_eq(input_1.owner, input_2.owner);
        return Token {
            owner: input_1.owner,
            amount: input_1.amount + input_2.amount,
        };
    }

    fn split(input: Token, private amount: u128) -> (Token, Token) {
        let remainder: Token = Token {
            owner: input.owner,
            amount: input.amount - amount,
        };
        let split_off: Token = Token {
            owner: input.owner,
            amount: amount,
        };
        return (remainder, split_off);
    }



    view fn balance_of(account: address) -> u128 {
        return balances.get_or_use(account, 0u128);
    }

    view fn allowance(owner: address, spender: address) -> u128 {
        let key: field = BHP256::hash_to_field(Approval { approver: owner, spender: spender });
        return allowances.get_or_use(key, 0u128);
    }

    view fn supply() -> u128 {
        return total_supply.unwrap_or(0u128);
    }

    view fn max_supply() -> u128 {
        return MAX_SUPPLY;
    }

    view fn decimals() -> u8 {
        return 6u8;
    }

    view fn name() -> identifier {
        return 'ExampleToken';
    }

    view fn symbol() -> identifier {
        return 'EXT';
    }
}
