# leo-bindings

Rust bindings for interacting with Leo programs.

## Documentation

Documentation is available at: <https://henrikkv.github.io/leo-bindings/leo_bindings>

## Deploying to devnet

Start the devnet with:

```bash
leo devnet --snarkos-features test_network --clear-storage --yes --consensus-heights 0,1,2,3,4,5,6,7,8,9,10,11,12,13 --snarkos ~/.cargo/bin/snarkos --tmux
```

Run tests with:

```bash
cargo test --release -- --nocapture
```

The `--release` flag slows down compile times and speeds up proving times.

## Generating bindings

### Quick setup with CLI

```bash
leo-bindings update
```

Install leo-bindings with `cargo install --path .` and run `leo-bindings update` in the top level of the Leo project.
Run again if `program.json` changes to update the generated files.
Keep this workspace as it was generated, and import it in another Rust package.
Use `--workspace` if the bindings are in a Cargo workspace.

The generated bindings are available at `projectname_bindings::projectname::*` in rust.
See how to create accounts and use credits.aleo in the [token example](examples/token/tests/simple_test.rs).
The struct `ProjectnameAleo<N>` has a constructor that deploys the program if it has not been deployed yet.
It takes a `VMManager<N>` that can be a `NetworkVM` or a `LocalVM`.
LocalVM is faster for testing because it skips some of the proving that is required for the network.
See how to use the struct in the [dev example](examples/dev/tests/simple_test.rs).

`cargo doc --open` can be used to explore the generated code. [credits.aleo documentation](https://henrikkv.github.io/leo-bindings/credits_bindings/credits/trait.CreditsAleo.html)
