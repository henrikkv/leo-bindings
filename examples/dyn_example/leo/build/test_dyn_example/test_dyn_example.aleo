import adder.aleo;
import dyn_example.aleo;
import multiplier.aleo;
program test_dyn_example.aleo;

function t_main:
    call dyn_example.aleo/main 2u32 3u32 into r0;
    assert.eq r0 5u32;

function static_import_two_combines:
    call dyn_example.aleo/static_import_two_combines 2u32 3u32 into r0 r1;
    assert.eq r0 5u32;
    assert.eq r1 6u32;

function dynamic_combine_id_routing:
    call dyn_example.aleo/dynamic_combine_id_routing 'adder' 'multiplier' 10u32 4u32 into r0 r1;
    assert.eq r0 14u32;
    assert.eq r1 40u32;

function dynamic_combine_with_network:
    call dyn_example.aleo/dynamic_combine_with_network 'adder' 'aleo' 2u32 3u32 into r0;
    assert.eq r0 5u32;

function dynamic_combine_field_target:
    call dyn_example.aleo/dynamic_combine_field_target 491327349857field 10u32 4u32 into r0;
    assert.eq r0 14u32;

constructor:
    assert.eq edition 0u16;
