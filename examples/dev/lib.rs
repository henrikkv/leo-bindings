use leo_bindings::generate_bindings;

#[cfg(feature = "mainnet")]
generate_bindings!(
    "mainnet",
    ["outputs/dev.initial.json"],
    []
);

#[cfg(feature = "testnet")]
generate_bindings!(
    "testnet",
    ["outputs/dev.initial.json"],
    []
);

#[cfg(feature = "canary")]
generate_bindings!(
    "canary",
    ["outputs/dev.initial.json"],
    []
);
