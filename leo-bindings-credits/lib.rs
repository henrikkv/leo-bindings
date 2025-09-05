use leo_bindings_macro::generate_bindings_from_simple_json;

generate_bindings_from_simple_json!(
    snarkvm::console::network::TestnetV0,
    ["leo-bindings-credits/simplified.json"]
);
