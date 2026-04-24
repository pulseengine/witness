//! Test harness runner — executes an instrumented module and collects counters.
//!
//! # v0.1 strategy
//!
//! 1. Spawn the supplied shell command (`--harness "cargo test ..."`).
//! 2. Ensure the harness can locate the instrumented module (typically via a
//!    `WITNESS_MODULE` environment variable; the harness is expected to load
//!    the module at that path).
//! 3. After the harness exits, the runner expects the harness to have
//!    triggered the `__witness_dump_counters` export at least once and
//!    serialized the result to a file the runner then reads.
//! 4. Combine the counter dump with the manifest written at instrumentation
//!    time and write a run output file (JSON) pairing branch IDs with hit
//!    counts.
//!
//! Alternative host-integration strategy (v0.2+): embed `wasmtime` as a
//! library and run the module directly, giving witness full control over
//! the execution and counter extraction without requiring the harness to
//! cooperate.

use crate::Result;
use std::path::Path;

/// Run the given harness command and collect coverage counters for `module`.
pub fn run_harness(harness: &str, module: &Path, output: &Path) -> Result<()> {
    let _ = (harness, module, output);
    // TODO(v0.1):
    //   1. set WITNESS_MODULE env var to module path
    //   2. spawn harness via std::process::Command
    //   3. on success, read the counter dump the harness wrote
    //   4. merge with the branch manifest (from <module>.witness.json)
    //   5. write run output to `output`
    //
    // Run output schema (v0.1): { "module": str, "branches": [ { "id": u32,
    //   "hits": u64, "function": u32, "instruction_offset": u32,
    //   "kind": "br_if" | "br_table" | "if" } ] }
    todo!("v0.1 — subprocess harness runner; see DESIGN.md")
}
