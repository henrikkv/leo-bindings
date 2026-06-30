program delegated_proving_test.aleo;

function divide:
    input r0 as u64.private;
    input r1 as u64.private;
    input r2 as u64.private;
    input r3 as u64.private;
    div r0 r1 into r4;
    div r4 r2 into r5;
    div r5 r3 into r6;
    div r0 r3 into r7;
    div r7 r1 into r8;
    add r6 r8 into r9;
    output r9 as u64.private;

constructor:
    assert.eq edition 0u16;
