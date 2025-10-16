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
It is not needed for the interpreter.

## Generating bindings

### Quick setup with CLI

```bash
leo-bindings update
```

Install and run `leo-bindings update` in the top level of the Leo project.
Run again if `program.json` changes.
Keep this workspace as it was generated, and import it in another package.

The generated bindings are available at `projectname_bindings::projectname::*` in rust.
See how to create accounts and use credits.aleo in the [token example](examples/token/tests/simple_test.rs).
The trait `ProjectnameAleo<N>` is implemented by `network::ProjectnameNetwork<N>` and `interpreter::ProjectnameInterpreter`.
Type aliases `ProjectnameTestnet`, `ProjectnameMainnet`, `ProjectnameCanary`, and `ProjectnameInterpreter` are available.
See how to use the trait in the [dev example](examples/dev/tests/simple_test.rs).

