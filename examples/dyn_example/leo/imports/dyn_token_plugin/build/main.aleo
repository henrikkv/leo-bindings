program dyn_token_plugin.aleo;

record Token:
    owner as address.private;
    balance as u64.private;

function double_balance:
    input r0 as Token.record;
    mul r0.balance 2u64 into r1;
    cast r0.owner r1 into r2 as Token.record;
    output r2 as Token.record;

function mint:
    input r0 as u64.public;
    cast self.signer r0 into r1 as Token.record;
    output r1 as Token.record;

constructor:
    assert.eq edition 0u16;
