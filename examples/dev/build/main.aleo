program dev.aleo;

struct A:
    number as u64;

struct B:
    number as u32;
    a as A;

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

constructor:
    assert.eq edition 0u16;
