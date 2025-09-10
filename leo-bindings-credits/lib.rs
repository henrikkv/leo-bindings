use leo_bindings::generate_bindings;

generate_bindings!(
    snarkvm::console::network::TestnetV0,
    [],
    ["simplified.json"]
);
