//! Wasm instrumentation — inserts branch counters at every decision point.
//!
//! # v0.1 strategy
//!
//! For each of `br_if`, `br_table`, and `if` instruction in every function:
//!
//! 1. Assign a unique u32 branch ID.
//! 2. Allocate a per-branch i32 global, initialized to zero.
//! 3. Rewrite the instruction so the counter global is incremented on the
//!    taken branch (and for `br_table`, incremented per taken target).
//! 4. Export a synthetic function `__witness_dump_counters` that serializes
//!    all counter globals into linear memory at a well-known offset, returning
//!    `(ptr, len)` to the host.
//! 5. Emit an auxiliary JSON manifest mapping branch IDs to
//!    `(module, function index, instruction offset)` for later report
//!    generation.
//!
//! Semantic preservation invariant: for any input, the instrumented module
//! produces the same observable output as the original, modulo the dump
//! counters export. Verified by round-trip testing against the wasm-tools
//! reference interpreter.
//!
//! # What v0.1 does NOT do
//!
//! - MC/DC condition decomposition (v0.2, DWARF-informed)
//! - Cross-component coverage for meld-fused modules (v0.4)
//! - Variant-aware scope pruning (v0.4)
//!
//! See [`DESIGN.md`](../DESIGN.md) for the full roadmap and the
//! decision-granularity open question.

use crate::Result;
use std::path::Path;

/// Instrument the Wasm module at `input`, writing the instrumented module
/// to `output` and a branch-ID manifest alongside it.
pub fn instrument_file(input: &Path, output: &Path) -> Result<()> {
    let _ = (input, output);
    // TODO(v0.1): implement using `walrus`:
    //   1. read module from `input`
    //   2. walk every function, count branch points, assign IDs
    //   3. rewrite each branch to increment its counter global
    //   4. add exported `__witness_dump_counters` function
    //   5. serialize to `output`
    //   6. write manifest to `output.with_extension("witness.json")`
    //
    // Manifest schema (v0.1): { "branches": [ { "id": u32, "function": u32,
    //   "instruction_offset": u32, "kind": "br_if" | "br_table" | "if" } ] }
    todo!("v0.1 — walrus-based instrumentation; see DESIGN.md")
}
