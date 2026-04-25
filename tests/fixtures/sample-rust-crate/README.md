# sample-rust-crate — witness end-to-end fixture

This crate is a real Rust→Wasm program built for a single purpose: drive
witness's integration tests against compiler output, not hand-written WAT.

## What it exercises

| Pattern | Underlying fn | Entry points |
|---|---|---|
| `br_if` (short-circuit `&&`) | `brif_check` | `run_brif_taken`, `run_brif_not_taken` |
| `if/else` | `ifelse_check` | `run_then_arm`, `run_else_arm` |
| `br_table` (match with three arms) | `match_check` | `run_match_arm_0`, `run_match_arm_1`, `run_match_default` |

Each entry point takes no arguments and returns a distinguishable `i32`
(100, 200, or 300) so the integration test can assert on both the counter
deltas and the runner's return value.

## Build

```sh
# from this directory
./build.sh

# Or, manually:
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/witness_sample_fixture.wasm sample.wasm
```

The output is `sample.wasm` in this directory. The integration test
(`tests/integration_e2e.rs`) looks for it at that exact path.

If the wasm32 target isn't installed:

```sh
rustup target add wasm32-unknown-unknown
```

## Why standalone, not a workspace member?

Witness's `Cargo.toml` does not list this fixture as a workspace member.
Treating it as one would force every host-target build of witness (e.g.
`cargo test`) to also build this fixture for `wasm32-unknown-unknown`,
which is the wrong default. CI builds the fixture explicitly via
`build.sh` before running `cargo test --test integration_e2e`.

## Why `no_std`?

`wasm32-unknown-unknown` has no libc, no unwinder, and no built-in panic
infrastructure. Going `no_std` plus a hand-rolled panic handler keeps the
produced module small and the lowering deterministic, with zero
third-party dependencies. The fixture's reproducibility depends only on
rustc itself.

## Reproducibility caveats

- **rustc version sensitivity.** The exact lowering of `&&` and `match`
  to `br_if` / `br_table` depends on rustc + LLVM. The integration test
  asserts on patterns (e.g. "at least one BrIf counter increments on
  this path") rather than on exact branch ids, so minor rustc updates
  shouldn't break it. If a major rustc change collapses one of the
  patterns (e.g. `match` lowered to a chain of `br_if`s instead of a
  `br_table`), update the assertions.
- **Optimization level.** `Cargo.toml` pins `opt-level = 1`, `lto =
  false`, `codegen-units = 1`. Don't change these without re-checking
  what the produced Wasm looks like.
- **No committed `.wasm`.** Built artifacts are in `.gitignore`; the
  source is the single source of truth. CI rebuilds on every run.
