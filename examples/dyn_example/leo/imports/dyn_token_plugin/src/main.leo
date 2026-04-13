interface TokenOps {
    record Token;
    fn double_balance(t: Token) -> Token;
}

program dyn_token_plugin.aleo : TokenOps {
    record Token {
        owner: address,
        balance: u64,
    }

    fn double_balance(t: Token) -> Token {
        return Token { owner: t.owner, balance: t.balance * 2u64 };
    }

    fn mint(public balance: u64) -> Token {
        return Token { owner: self.signer, balance: balance };
    }

    @noupgrade
    constructor() {}
}
