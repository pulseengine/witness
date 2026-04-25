//! # witness
//!
//! MC/DC-style branch coverage for WebAssembly components.
//!
//! ## Architecture (v0.1)
//!
//! Three mechanical stages, each composable and independently testable:
//!
//! 1. [`instrument`] — rewrite a `.wasm` module to count branch hits at every
//!    `br_if` / `br_table` / `if` instruction. The instrumented module is
//!    semantically equivalent on any well-formed input, with counters as the
//!    only side effect.
//! 2. [`run`] — execute a supplied test-harness command against the
//!    instrumented module, then extract the counter values.
//! 3. [`report`] — turn the raw counters into a coverage report keyed to
//!    `(module, function, offset)`. JSON for machines (rivet, CI), text for
//!    humans.
//!
//! ## Scope discipline
//!
//! v0.1 is strict per-`br_if` / per-`br_table` counting. It is **not**
//! MC/DC in the condition-decomposition sense yet — that is v0.2 and depends
//! on DWARF-in-Wasm reconstruction. See [`DESIGN.md`](../DESIGN.md) for the
//! decision-granularity open question and the incremental roadmap.
//!
//! ## Ecosystem
//!
//! - [rivet](https://github.com/pulseengine/rivet) consumes witness reports
//!   as requirement-to-test coverage evidence (v0.3).
//! - [sigil](https://github.com/pulseengine/sigil) carries witness reports
//!   as in-toto coverage predicates in signed attestation bundles (v0.3).
//! - [loom](https://github.com/pulseengine/loom) emits the post-optimization
//!   Wasm that witness measures; loom's translation validation is what makes
//!   "coverage on optimized Wasm" a valid stand-in for "coverage on
//!   pre-optimization Wasm" (v0.4).

pub mod decisions;
pub mod error;
pub mod instrument;
pub mod report;
pub mod run;

pub use error::{Error, Result};
