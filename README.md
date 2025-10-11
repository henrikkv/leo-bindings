# leo-bindings

Rust bindings for interacting with Leo programs.


## Deploying to devnet

Start the devnet with:
```bash
leo devnet --snarkos-features test_network --clear-storage --yes --consensus-heights 0,1,2,3,4,5,6,7,8,9 --snarkos ~/.cargo/bin/snarkos --tmux
```

Run tests with:
```bash
RUSTFLAGS="-Zmacro-backtrace" RUST_BACKTRACE=full cargo test --release -- --nocapture
```
The `--release` flag slows down compile times and speeds up proving times.


## Generating bindings

In your leo project, run 
```bash
cargo init
```

Add this to `Cargo.toml`:
```toml
[lib]
name = "projectname_bindings"
path = "lib.rs"

[features]
default = ["interpreter"]
mainnet = []
testnet = []
canary = []
interpreter = []

[dependencies]
leo-bindings = { git = "https://github.com/henrikkv/leo-bindings" }
leo-bindings-credits = { git = "https://github.com/henrikkv/leo-bindings" }
rand = "0.8"
snarkvm = "4.2.1"
```

Generate the ast snapshot with 
```bash
leo build --enable-initial-ast-snapshot
```

Create `lib.rs`:
```rust
use leo_bindings::generate_bindings;
generate_bindings!(["outputs/projectname.initial.json"]);
```
The generated bindings are available at `projectname_bindings::projectname::*` in rust.
See how to create accounts and use credits.aleo in the [token example](examples/token/tests/simple_test.rs).


The trait `ProjectnameAleo<N>` is implemented by `network::ProjectnameNetwork<N>` and `interpreter::ProjectnameInterpreter`.

Type aliases `ProjectnameTestnet`, `ProjectnameMainnet`, `ProjectnameCanary`, and `ProjectnameInterpreter` are available.

See how to use the trait in the [dev example](examples/dev/tests/simple_test.rs).

Add this to `.gitignore` if you want to publish the bindings:
```gitignore
outputs/*
!outputs/
!outputs/*.initial.json
```
