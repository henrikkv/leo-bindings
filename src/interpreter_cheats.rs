use leo_bindings_core::shared_interpreter::with_interpreter_blocking;

pub fn set_block_height(height: u32) {
    with_interpreter_blocking(move |state| {
        let mut interpreter = state.interpreter.borrow_mut();
        interpreter.cursor.block_height = height;
    })
    .expect("Shared interpreter not initialized");
}

pub fn set_block_timestamp(timestamp: i64) {
    with_interpreter_blocking(move |state| {
        let mut interpreter = state.interpreter.borrow_mut();
        interpreter.cursor.block_timestamp = timestamp;
    })
    .expect("Shared interpreter not initialized");
}
