use leo_bindings::generate_bindings;

generate_bindings!(
    "examples/dev/outputs/dev.simplified.json",
    snarkvm::console::network::TestnetV0
);
