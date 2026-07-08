program multiplier.aleo;

function combine:
    input r0 as u32.private;
    input r1 as u32.private;
    mul r0 r1 into r2;
    output r2 as u32.private;

constructor:
    assert.eq edition 0u16;
