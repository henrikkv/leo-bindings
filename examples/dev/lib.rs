use leo_bindings_macro::generate_bindings;

generate_bindings!(
    snarkvm::console::network::TestnetV0,
    ["outputs/dev.initial.json"]
);
