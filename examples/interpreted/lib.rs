use leo_bindings::generate_network_bindings;

generate_network_bindings!(["interpreter", "testnet"], ["outputs/dev.initial.json"], []);
