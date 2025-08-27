# leo-bindings

Rust bindings and code generation for interacting with Leo programs from Rust. This workspace provides:

- A library crate (`leo-bindings`) with helpers for converting between Leo values and `snarkvm` values, broadcasting transactions, and querying balances.
- A procedural macro crate (`leo-bindings-macro`) that generates a Rust API from the ast snapshot of a Leo program. (dev.initial.json)
- An example crate (`examples/dev`) showing end-to-end usage: generate bindings, deploy the program, and call transitions against a devnet node.

The goal is to make it ergonomic to call Leo transitions from Rust, with type-safe inputs/outputs and minimal boilerplate.

---

## Prerequisites

- Rust toolchain
- `snarkOS` binary available (used by devnet)
- `leo` CLI and toolchain (to build the Leo program and export JSON)

---

## Devnet

You can boot a local devnet suitable for testing program deployment and transition execution. This project was developed against a devnet started with the following command:

```bash
leo devnet --snarkos-features test_network --clear-storage --yes --consensus-heights 0,1,2,3,4,5,6,7,8 --snarkos ~/.cargo/bin/snarkos --tmux
```

---

## Generating bindings

Bindings are generated at compile-time via the `generate_bindings!` macro from the `leo-bindings-macro` crate. The macro consumes the JSON from Leo’s `dev.initial.json` and emits:

- Rust structs for Leo structs and records
- A program client struct with methods for each function
- Conversions between Rust values and `snarkvm` `Value<N>`

In `examples/dev/lib.rs`:

```rust
use leo_bindings_macro::generate_bindings;

generate_bindings!(
    "examples/dev/outputs/dev.initial.json",
    snarkvm::console::network::TestnetV0
);
```

- The first argument is the path to the simplified JSON that contains program name, structs, records, and transitions.
- The second argument is the target network type. Supported values are `TestnetV0`, `MainnetV0`, and `CanaryV0` from `snarkvm::console::network`.

---

## Extracting signatures JSON

The root crate exposes a small CLI to transform Leo’s `dev.initial.json` into the simplified format that the macro uses.
This step is just for debugging. `generate_bindings` does this automatically.

```bash
cargo run --bin get-signatures -- \
  --input examples/dev/outputs/dev.initial.json
```

---

## Types and conversions

The `leo-bindings` crate implements `ToValue` and `FromValue` for:

- Primitive integers: `u8`, `u16`, `u32`, `u64`, `u128`, and signed variants
- `bool`
- `Field<N>` and `Address<N>` from `snarkvm`
- Fixed-size arrays `[T; N]` for supported `T`
- Generated structs/records

This enables seamless conversion between Rust types and the `Value<N>` used by `snarkvm` during execution.

---

## Networking and fees

- The client fetches the program bytecode from the endpoint before execution to ensure the VM has the program loaded.
- It checks your public balance via the `credits.aleo/account` mapping before broadcasting.
