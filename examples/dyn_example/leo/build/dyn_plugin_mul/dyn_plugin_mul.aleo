program dyn_plugin_mul.aleo;

mapping counts:
    key as address.public;
    value as u32.public;

function combine:
    input r0 as u32.private;
    input r1 as u32.private;
    mul r0 r1 into r2;
    output r2 as u32.private;

function increment:
    input r0 as u32.private;
    async increment self.caller r0 into r1;
    output r0 as u32.private;
    output r1 as dyn_plugin_mul.aleo/increment.future;

finalize increment:
    input r0 as address.public;
    input r1 as u32.public;
    get.or_use counts[r0] 0u32 into r2;
    add r2 r1 into r3;
    set r3 into counts[r0];

constructor:
    assert.eq edition 0u16;
