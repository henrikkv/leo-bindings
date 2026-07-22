import adder.aleo;
import multiplier.aleo;
program dyn_example.aleo;

function main:
    input r0 as u32.public;
    input r1 as u32.private;
    add r0 r1 into r2;
    output r2 as u32.private;

function double_it:
    input r0 as u32.private;
    add r0 r0 into r1;
    output r1 as u32.private;

function static_import_two_combines:
    input r0 as u32.public;
    input r1 as u32.public;
    call adder.aleo/combine r0 r1 into r2;
    call multiplier.aleo/combine r0 r1 into r3;
    output r2 as u32.private;
    output r3 as u32.private;

function dynamic_combine_id_routing:
    input r0 as identifier.private;
    input r1 as identifier.private;
    input r2 as u32.private;
    input r3 as u32.private;
    call.dynamic r0 'aleo' 'combine' with r2 r3 (as u32.private u32.private) into r4 (as u32.private);
    call.dynamic r1 'aleo' 'combine' with r2 r3 (as u32.private u32.private) into r5 (as u32.private);
    output r4 as u32.private;
    output r5 as u32.private;

function dynamic_combine_with_network:
    input r0 as identifier.private;
    input r1 as identifier.private;
    input r2 as u32.private;
    input r3 as u32.private;
    call.dynamic r0 r1 'combine' with r2 r3 (as u32.private u32.private) into r4 (as u32.private);
    output r4 as u32.private;

function dynamic_combine_field_target:
    input r0 as field.private;
    input r1 as u32.private;
    input r2 as u32.private;
    call.dynamic r0 'aleo' 'combine' with r1 r2 (as u32.private u32.private) into r3 (as u32.private);
    output r3 as u32.private;

constructor:
    assert.eq edition 0u16;
