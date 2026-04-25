//! DWARF-grounded reconstruction of source-level decisions from Wasm
//! `br_if` sequences.
//!
//! # Status
//!
//! v0.2.0 ships **the schema and the fallback path only**. The
//! reconstruction algorithm itself is documented in
//! `docs/paper/v0.2-mcdc-wasm.md` and in the v0.2 section of
//! `DESIGN.md`, but the implementation in this module is currently a
//! stub: [`reconstruct_decisions`] always returns `Ok(vec![])`, which
//! means hosts fall back to the strict per-`br_if` interpretation
//! produced by [`crate::instrument`]. The full reconstruction
//! implementation lands in v0.2.1.
//!
//! Why ship the schema now: the [`crate::instrument::Manifest`] now has
//! a stable `decisions` field, so hosts that consume witness output can
//! be written today without breaking when v0.2.1 starts populating it.
//!
//! # Interface
//!
//! [`reconstruct_decisions`] takes the original (pre-instrumentation)
//! Wasm bytes and the branch manifest, and returns the list of
//! source-level decisions discovered via DWARF correlation.

use crate::Result;
use crate::instrument::{BranchEntry, Decision};

/// Reconstruct source-level decisions from a branch manifest using DWARF.
///
/// v0.2.0 stub: returns an empty vector, signalling that no DWARF-grounded
/// grouping was performed and hosts should treat each `BranchEntry`
/// independently (strict per-`br_if` interpretation).
///
/// The full algorithm — grouping `br_if` sequences by source line plus
/// lexical decision marker, with handling for macro expansion, inlining,
/// and CFG fragmentation — is documented in
/// `docs/paper/v0.2-mcdc-wasm.md` and lands in v0.2.1.
pub fn reconstruct_decisions(wasm_bytes: &[u8], branches: &[BranchEntry]) -> Result<Vec<Decision>> {
    let _ = (wasm_bytes, branches);
    // v0.2.0: fallback only. v0.2.1 will:
    //   1. parse Wasm custom sections .debug_info / .debug_line via gimli
    //   2. build a (function_index, byte_offset) -> (file, line) map
    //   3. group adjacent br_if branches sharing source line + decision id
    //   4. emit one Decision per group
    Ok(Vec::new())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod tests {
    use super::*;
    use crate::instrument::{BranchEntry, BranchKind};

    fn entry(id: u32) -> BranchEntry {
        BranchEntry {
            id,
            function_index: 0,
            function_name: None,
            kind: BranchKind::BrIf,
            instr_index: id,
            target_index: None,
            byte_offset: Some(id),
            seq_debug: format!("Id {{ idx: {id} }}"),
        }
    }

    #[test]
    fn stub_returns_empty_decisions() {
        let entries = vec![entry(0), entry(1), entry(2)];
        let decisions = reconstruct_decisions(b"\x00asm\x01\x00\x00\x00", &entries).unwrap();
        assert!(decisions.is_empty(), "v0.2.0 stub returns no decisions");
    }
}
