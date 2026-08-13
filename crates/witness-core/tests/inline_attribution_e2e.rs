//! Real-DWARF regression for gale #179 — inlined-decision attribution.
//!
//! Fixture (`tests/fixtures/inline_chain_v4.{wasm,rs}`, built with
//! `CARGO_PROFILE_RELEASE_DEBUG=2 cargo build --release
//! --target wasm32-unknown-unknown`): a `#[no_mangle] run` that calls an
//! `#[inline(always)] decide` boolean at two sites. The optimizer inlines
//! `decide` into `run`, so the `br_if`s physically live in `run`'s body
//! but DWARF attributes them to `decide` via a `DW_TAG_inlined_subroutine`
//! chain — the same shape as a wit-bindgen export wrapper inlining user
//! logic (the gale hm-thin case).
//!
//! This one fixture pins BOTH bugs that produced the #179
//! `wit_bindgen_cabi_realloc.rs` mis-attribution:
//!   1. `.debug_ranges` (DWARF v4) was never extracted, so `die_ranges`
//!      resolved no address ranges and every inline chain came back empty.
//!   2. walrus `byte_offset` is the instruction's file-absolute position,
//!      but DWARF addresses are relative to the Code section's contents —
//!      the un-rebased offset fell past the last line-table row and
//!      `lookup_line` silently clamped every branch to it.
//!
//! The decision must land on the user's source (`decide`'s body), and the
//! inline chain must name the call sites in the same file — not a stdlib
//! or generated-glue file. The manifest is re-derived by instrumenting the
//! committed wasm each run, so a regression in either fix fails here.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use witness_core::instrument::{Manifest, instrument_file};

#[test]
fn inlined_decision_attributes_to_user_source_not_glue() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let wasm = fixtures.join("inline_chain_v4.wasm");

    let out = std::env::temp_dir().join("witness_inline_chain_v4.instr.wasm");
    instrument_file(&wasm, &out).expect("instrument the fixture wasm");
    let manifest = Manifest::load(&Manifest::path_for(&out)).expect("load the witness manifest");

    // (1) At least one multi-condition decision was reconstructed, and it
    // attributes to the user's source file — the boolean lives in
    // `inline_chain_v4.rs`, NOT in any stdlib/glue file. Pre-fix, the
    // un-rebased offset clamped this to the last line-table row (a
    // library file), exactly the #179 symptom.
    let decision = manifest
        .decisions
        .iter()
        .find(|d| d.conditions.len() >= 2)
        .expect("a multi-condition decision was reconstructed");
    let file = decision
        .source_file
        .as_deref()
        .expect("the decision carries a source file");
    assert!(
        file.ends_with("lib.rs"),
        "decision mis-attributed to `{file}` — expected the user source (lib.rs). \
         A code-section-base or `.debug_ranges` regression reproduces gale #179."
    );

    // (2) The inline chain was recovered (non-empty) and every frame names
    // the user's file. Pre-fix (missing `.debug_ranges`), the DWARF v4
    // ranges resolved to nothing and this map was empty.
    assert!(
        !manifest.branch_inline_chains.is_empty(),
        "branch_inline_chains is empty — DWARF v4 inline frames were dropped (gale #179)"
    );
    for (branch, chain) in &manifest.branch_inline_chains {
        assert!(
            !chain.is_empty(),
            "branch {branch} has an empty inline chain"
        );
        for frame in chain {
            let cf = frame
                .call_file
                .as_deref()
                .expect("inline frame carries a call_file");
            assert!(
                cf.ends_with("lib.rs"),
                "inline frame call_file `{cf}` is not the user source — \
                 the code-section-base rebase or `.debug_ranges` fix regressed"
            );
        }
    }
}
