//! Library surface of the `witness` binary crate.
//!
//! Exposes the wasmtime-using run-side code so integration tests in
//! `tests/` can drive `run_module` without re-implementing it, and the
//! `witness recheck` evidence checker (FEAT-045).
//!
//! Without the `runtime` feature, `run` is a stand-in with the same
//! option types whose `run_module` returns an error, so the CLI builds
//! for wasm32-wasip2 (no wasmtime) and only `witness run` and
//! `witness all` refuse.

pub mod recheck;
#[cfg(feature = "runtime")]
pub mod run;
#[cfg(not(feature = "runtime"))]
#[path = "run_disabled.rs"]
pub mod run;

pub use run::{RunOptions, run_module};
