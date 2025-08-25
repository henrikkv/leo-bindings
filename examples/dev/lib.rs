use leo_bindings_macro::generate_bindings;

generate_bindings!(
    "examples/dev/outputs/dev.initial.json",
    snarkvm::console::network::TestnetV0
);
