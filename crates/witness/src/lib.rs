//! Library surface of the `witness` binary crate.
//!
//! Exposes the wasmtime-using run-side code so integration tests in
//! `tests/` can drive `run_module` without re-implementing it.

pub mod run;

pub use run::{RunOptions, run_module};
