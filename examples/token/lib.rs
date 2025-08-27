use leo_bindings_macro::generate_bindings;

generate_bindings!(
    "examples/token/outputs/token.initial.json",
    snarkvm::console::network::TestnetV0
);
