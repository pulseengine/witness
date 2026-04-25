//! # witness-core
//!
//! Pure-data algorithms for witness, separated from the wasmtime-using
//! CLI binary so this crate compiles to `wasm32-wasip2`.
//!
//! Modules:
//! - [`instrument`] — manifest types + walrus-based instrumentation pass
//! - [`decisions`] — DWARF-grounded MC/DC reconstruction
//! - [`diff`] — branch-set / coverage delta between two snapshots
//! - [`predicate`] — in-toto coverage Statement builder
//! - [`report`] — coverage-report aggregation
//! - [`rivet_evidence`] — rivet-shape evidence emission
//! - [`run_record`] — `RunRecord` types + cross-run merge
//! - [`error`] — `Error` enum and `Result` alias
//!
//! Wasmtime-based execution (the `witness run` CLI path) lives in the
//! `witness` binary crate.

pub mod attest;
pub mod decisions;
pub mod diff;
pub mod error;
pub mod instrument;
pub mod lcov;
pub mod predicate;
pub mod report;
pub mod rivet_evidence;
pub mod run_record;

pub use error::{Error, Result};
