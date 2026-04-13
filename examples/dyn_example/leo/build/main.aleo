import dyn_plugin_add.aleo;
import dyn_plugin_mul.aleo;
import dyn_token_plugin.aleo;
program dyn_example.aleo;

mapping local_counter:
    key as u64.public;
    value as u32.public;

function main:
    input r0 as u32.public;
    input r1 as u32.private;
    add r0 r1 into r2;
    output r2 as u32.private;

function static_import_two_combines:
    input r0 as u32.public;
    input r1 as u32.public;
    call dyn_plugin_add.aleo/combine r0 r1 into r2;
    call dyn_plugin_mul.aleo/combine r0 r1 into r3;
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

function aleo_generator:
    output aleo::GENERATOR as group.private;

function dyn_record_mint_then_double:
    input r0 as u64.public;
    input r1 as identifier.private;
    call dyn_token_plugin.aleo/mint r0 into r2;
    cast r2 into r3 as dynamic.record;
    call.dynamic r1 'aleo' 'double_balance' with r3 (as dynamic.record) into r4 (as dynamic.record);
    get.record.dynamic r4.balance into r5 as u64;
    output r0 as u64.private;
    output r5 as u64.private;

function final_compose_import_and_local:
    input r0 as field.private;
    input r1 as u64.public;
    input r2 as u32.public;
    call.dynamic r0 'aleo' 'increment' with r2 (as u32.private) into r3 r4 (as u32.private dynamic.future);
    async final_compose_import_and_local r4 r1 r2 into r5;
    output r3 as u32.private;
    output r5 as dyn_example.aleo/final_compose_import_and_local.future;

finalize final_compose_import_and_local:
    input r0 as dynamic.future;
    input r1 as u64.public;
    input r2 as u32.public;
    await r0;
    get.or_use local_counter[r1] 0u32 into r3;
    add r3 r2 into r4;
    set r4 into local_counter[r1];

constructor:
    assert.eq edition 0u16;
