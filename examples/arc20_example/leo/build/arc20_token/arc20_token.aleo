program arc20_token.aleo;

record Token:
    owner as address.private;
    amount as u128.private;

struct Approval:
    approver as address;
    spender as address;

struct Optional__6A9ZdhLQa8N:
    is_some as boolean;
    val as u128;

mapping balances:
    key as address.public;
    value as u128.public;

mapping allowances:
    key as field.public;
    value as u128.public;

mapping total_supply__:
    key as boolean.public;
    value as u128.public;

function mint_public:
    input r0 as address.public;
    input r1 as u128.public;
    async mint_public r1 r0 into r2;
    output r2 as arc20_token.aleo/mint_public.future;

finalize mint_public:
    input r0 as u128.public;
    input r1 as address.public;
    contains total_supply__[false] into r2;
    get.or_use total_supply__[false] 0u128 into r3;
    ternary r2 r3 0u128 into r4;
    cast r2 r4 into r5 as Optional__6A9ZdhLQa8N;
    ternary r5.is_some r5.val 0u128 into r6;
    add r6 r0 into r7;
    lte r7 1000000000000000u128 into r8;
    assert.eq r8 true;
    set r7 into total_supply__[false];
    get.or_use balances[r1] 0u128 into r9;
    add r9 r0 into r10;
    set r10 into balances[r1];

function mint_private:
    input r0 as address.private;
    input r1 as u128.private;
    cast r0 r1 into r2 as Token.record;
    output r2 as Token.record;

function transfer_public:
    input r0 as address.public;
    input r1 as u128.public;
    async transfer_public self.caller r1 r0 into r2;
    output r2 as arc20_token.aleo/transfer_public.future;

finalize transfer_public:
    input r0 as address.public;
    input r1 as u128.public;
    input r2 as address.public;
    get.or_use balances[r0] 0u128 into r3;
    sub r3 r1 into r4;
    set r4 into balances[r0];
    get.or_use balances[r2] 0u128 into r5;
    add r5 r1 into r6;
    set r6 into balances[r2];

function transfer_private:
    input r0 as Token.record;
    input r1 as address.private;
    input r2 as u128.private;
    sub r0.amount r2 into r3;
    cast r0.owner r3 into r4 as Token.record;
    cast r1 r2 into r5 as Token.record;
    output r4 as Token.record;
    output r5 as Token.record;

function transfer_private_to_public:
    input r0 as Token.record;
    input r1 as address.public;
    input r2 as u128.public;
    sub r0.amount r2 into r3;
    cast r0.owner r3 into r4 as Token.record;
    async transfer_private_to_public r1 r2 into r5;
    output r4 as Token.record;
    output r5 as arc20_token.aleo/transfer_private_to_public.future;

finalize transfer_private_to_public:
    input r0 as address.public;
    input r1 as u128.public;
    get.or_use balances[r0] 0u128 into r2;
    add r2 r1 into r3;
    set r3 into balances[r0];

function transfer_public_to_private:
    input r0 as address.private;
    input r1 as u128.public;
    cast r0 r1 into r2 as Token.record;
    async transfer_public_to_private self.caller r1 into r3;
    output r2 as Token.record;
    output r3 as arc20_token.aleo/transfer_public_to_private.future;

finalize transfer_public_to_private:
    input r0 as address.public;
    input r1 as u128.public;
    get.or_use balances[r0] 0u128 into r2;
    sub r2 r1 into r3;
    set r3 into balances[r0];

function transfer_public_as_signer:
    input r0 as address.public;
    input r1 as u128.public;
    async transfer_public_as_signer self.signer r1 r0 into r2;
    output r2 as arc20_token.aleo/transfer_public_as_signer.future;

finalize transfer_public_as_signer:
    input r0 as address.public;
    input r1 as u128.public;
    input r2 as address.public;
    get.or_use balances[r0] 0u128 into r3;
    sub r3 r1 into r4;
    set r4 into balances[r0];
    get.or_use balances[r2] 0u128 into r5;
    add r5 r1 into r6;
    set r6 into balances[r2];

function transfer_from_public:
    input r0 as address.public;
    input r1 as address.public;
    input r2 as u128.public;
    async transfer_from_public r0 self.caller r2 r1 into r3;
    output r3 as arc20_token.aleo/transfer_from_public.future;

finalize transfer_from_public:
    input r0 as address.public;
    input r1 as address.public;
    input r2 as u128.public;
    input r3 as address.public;
    cast r0 r1 into r4 as Approval;
    hash.bhp256 r4 into r5 as field;
    get.or_use allowances[r5] 0u128 into r6;
    sub r6 r2 into r7;
    set r7 into allowances[r5];
    get.or_use balances[r0] 0u128 into r8;
    sub r8 r2 into r9;
    set r9 into balances[r0];
    get.or_use balances[r3] 0u128 into r10;
    add r10 r2 into r11;
    set r11 into balances[r3];

function transfer_from_public_to_private:
    input r0 as address.public;
    input r1 as address.private;
    input r2 as u128.public;
    cast r1 r2 into r3 as Token.record;
    async transfer_from_public_to_private r0 self.caller r2 into r4;
    output r3 as Token.record;
    output r4 as arc20_token.aleo/transfer_from_public_to_private.future;

finalize transfer_from_public_to_private:
    input r0 as address.public;
    input r1 as address.public;
    input r2 as u128.public;
    cast r0 r1 into r3 as Approval;
    hash.bhp256 r3 into r4 as field;
    get.or_use allowances[r4] 0u128 into r5;
    sub r5 r2 into r6;
    set r6 into allowances[r4];
    get.or_use balances[r0] 0u128 into r7;
    sub r7 r2 into r8;
    set r8 into balances[r0];

function approve_public:
    input r0 as address.public;
    input r1 as u128.public;
    async approve_public self.caller r0 r1 into r2;
    output r2 as arc20_token.aleo/approve_public.future;

finalize approve_public:
    input r0 as address.public;
    input r1 as address.public;
    input r2 as u128.public;
    cast r0 r1 into r3 as Approval;
    hash.bhp256 r3 into r4 as field;
    get.or_use allowances[r4] 0u128 into r5;
    add r5 r2 into r6;
    set r6 into allowances[r4];

function unapprove_public:
    input r0 as address.public;
    input r1 as u128.public;
    async unapprove_public self.caller r0 r1 into r2;
    output r2 as arc20_token.aleo/unapprove_public.future;

finalize unapprove_public:
    input r0 as address.public;
    input r1 as address.public;
    input r2 as u128.public;
    cast r0 r1 into r3 as Approval;
    hash.bhp256 r3 into r4 as field;
    get.or_use allowances[r4] 0u128 into r5;
    sub r5 r2 into r6;
    set r6 into allowances[r4];

function join:
    input r0 as Token.record;
    input r1 as Token.record;
    assert.eq r0.owner r1.owner;
    add r0.amount r1.amount into r2;
    cast r0.owner r2 into r3 as Token.record;
    output r3 as Token.record;

function split:
    input r0 as Token.record;
    input r1 as u128.private;
    sub r0.amount r1 into r2;
    cast r0.owner r2 into r3 as Token.record;
    cast r0.owner r1 into r4 as Token.record;
    output r3 as Token.record;
    output r4 as Token.record;

view balance_of:
    input r0 as address.public;
    get.or_use balances[r0] 0u128 into r1;
    output r1 as u128.public;

view allowance:
    input r0 as address.public;
    input r1 as address.public;
    cast r0 r1 into r2 as Approval;
    hash.bhp256 r2 into r3 as field;
    get.or_use allowances[r3] 0u128 into r4;
    output r4 as u128.public;

view supply:
    contains total_supply__[false] into r0;
    get.or_use total_supply__[false] 0u128 into r1;
    ternary r0 r1 0u128 into r2;
    cast r0 r2 into r3 as Optional__6A9ZdhLQa8N;
    ternary r3.is_some r3.val 0u128 into r4;
    output r4 as u128.public;

view max_supply:
    output 1000000000000000u128 as u128.public;

view decimals:
    output 6u8 as u8.public;

view name:
    output 'ExampleToken' as identifier.public;

view symbol:
    output 'EXT' as identifier.public;

constructor:
    assert.eq edition 0u16;
