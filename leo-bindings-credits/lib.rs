use leo_bindings::generate_network_bindings;

generate_network_bindings!(["testnet", "interpreter"], [], ["simplified.json"]);
