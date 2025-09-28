program dev.aleo;

record User:
    owner as address.private;
    id as u64.private;
    balance as u64.public;

struct A:
    number as u64;

struct B:
    number as u32;
    a as A;

record Container:
    owner as address.private;
    data as B.private;

mapping balances:
    key as u64.public;
    value as u64.public;

mapping bs:
    key as u64.public;
    value as B.public;

function main:
    input r0 as u32.public;
    input r1 as u32.public;
    add r0 r1 into r2;
    output r2 as u32.public;

function nested:
    input r0 as B.public;
    cast 1u64 into r1 as A;
    cast 2u32 r1 into r2 as B;
    output r2 as B.public;

function store_nested:
    input r0 as B.private;
    input r1 as u64.private;
    async store_nested r0 r1 into r2;
    output r2 as dev.aleo/store_nested.future;

finalize store_nested:
    input r0 as B.public;
    input r1 as u64.public;
    set r0 into bs[r1];

function nested_array:
    input r0 as [[u8; 5u32]; 5u32].private;
    cast 0u8 0u8 0u8 0u8 0u8 into r1 as [u8; 5u32];
    cast 0u8 0u8 0u8 0u8 0u8 into r2 as [u8; 5u32];
    cast 0u8 0u8 0u8 0u8 0u8 into r3 as [u8; 5u32];
    cast 0u8 0u8 0u8 0u8 0u8 into r4 as [u8; 5u32];
    cast 0u8 0u8 0u8 0u8 0u8 into r5 as [u8; 5u32];
    cast r1 r2 r3 r4 r5 into r6 as [[u8; 5u32]; 5u32];
    output r6 as [[u8; 5u32]; 5u32].private;

function create_user:
    input r0 as address.private;
    input r1 as u64.private;
    input r2 as u64.private;
    cast r0 r1 r2 into r3 as User.record;
    output r3 as User.record;

function consume_user:
    input r0 as User.record;
    output r0.balance as u64.private;

function create_container:
    input r0 as address.private;
    input r1 as B.private;
    cast r0 r1 into r2 as Container.record;
    output r2 as Container.record;

function consume_container:
    input r0 as Container.record;
    output r0.data as B.private;

function asynchronous:
    input r0 as u64.private;
    input r1 as u64.private;
    cast self.caller r1 r0 into r2 as User.record;
    async asynchronous r0 r1 into r3;
    output r2 as User.record;
    output r3 as dev.aleo/asynchronous.future;

finalize asynchronous:
    input r0 as u64.public;
    input r1 as u64.public;
    set r0 into balances[r1];

function test_all_types:
    input r0 as field.private;
    input r1 as scalar.private;
    input r2 as group.private;
    input r3 as boolean.private;
    output r0 as field.private;
    output r1 as scalar.private;
    output r2 as group.private;
    output r3 as boolean.private;

constructor:
    assert.eq edition 0u16;
