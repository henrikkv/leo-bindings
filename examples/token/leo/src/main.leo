program token.aleo {
    @noupgrade
    constructor() {}

    mapping account: address => u64;

    record Token {
        owner: address,
        amount: u64,
    }

    fn mint_public(public receiver: address, public amount: u64) -> Final {
        return final {
            let current_amount: u64 = Mapping::get_or_use(account, receiver, 0u64);
            Mapping::set(account, receiver, current_amount + amount);
        };
    }

    fn mint_private(receiver: address, amount: u64) -> Token {
        return Token {
            owner: receiver,
            amount: amount,
        };
    }

    fn transfer_public(public receiver: address, public amount: u64) -> Final {
        let sender: address = self.caller;
        return final {
            let sender_amount: u64 = Mapping::get_or_use(account, sender, 0u64);
            Mapping::set(account, sender, sender_amount - amount);
            let receiver_amount: u64 = Mapping::get_or_use(account, receiver, 0u64);
            Mapping::set(account, receiver, receiver_amount + amount);
        };
    }

    fn transfer_private(sender: Token, receiver: address, amount: u64) -> (Token, Token) {
        let difference: u64 = sender.amount - amount;

        let remaining: Token = Token {
            owner: sender.owner,
            amount: difference,
        };

        let transferred: Token = Token {
            owner: receiver,
            amount: amount,
        };

        return (remaining, transferred);
    }

    fn transfer_private_to_public(sender: Token, public receiver: address, public amount: u64) -> (Token, Final) {
        let difference: u64 = sender.amount - amount;

        let remaining: Token = Token {
            owner: sender.owner,
            amount: difference,
        };

        return (remaining, final {
            let current_amount: u64 = Mapping::get_or_use(account, receiver, 0u64);
            Mapping::set(account, receiver, current_amount + amount);
        });
    }

    fn transfer_public_to_private(public receiver: address, public amount: u64) -> (Token, Final) {
        let sender: address = self.caller;
        let transferred: Token = Token {
            owner: receiver,
            amount: amount,
        };

        return (transferred, final {
            let current_amount: u64 = Mapping::get_or_use(account, sender, 0u64);
            Mapping::set(account, sender, current_amount - amount);
        });
    }

}
