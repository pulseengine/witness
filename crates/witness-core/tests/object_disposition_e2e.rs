//! v0.39 (#109 / REQ-061 / FEAT-044) — end-to-end reconciliation against a REAL
//! synth v0.45.0 `synth-provenance-v1` sidecar (not a hand-written fixture).
//!
//! Fixtures (checked in `tests/fixtures/`, captured 2026-07-16):
//! - `prov396.wat` / `prov396.wasm` — synth's `scripts/repro/provenance_branches_396.wat`.
//! - `prov396.synth-provenance-v1.json` — emitted by synth v0.45.0:
//!   `SYNTH_CMP_SELECT_FUSE=1 synth compile prov396.wat -o out.elf --target cortex-m4
//!    --all-exports --relocatable --emit-provenance`.
//!
//! The witness manifest is **re-derived** here by instrumenting the wasm each run,
//! so the `(func_index, byte_offset)` join is continuously re-verified: if witness's
//! offsets ever stop matching synth's `instruction_offset`s, `no-provenance` becomes
//! non-zero and this test fails. That join-completeness is the empirical settlement
//! of the offset-domain question (VCR-DEC-003) — witness's walrus `InstrLocId` and
//! synth's absolute WASM byte offset are the same domain.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use witness_core::instrument::{Manifest, instrument_file};
use witness_core::object_disposition::{ObjectVerdict, SynthProvenanceMap, reconcile};

#[test]
fn reconciles_real_synth_v045_provenance() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let wasm = fixtures.join("prov396.wasm");
    let sidecar = fixtures.join("prov396.synth-provenance-v1.json");

    // Re-derive witness's manifest from the committed wasm.
    let out = std::env::temp_dir().join("witness_prov396_e2e.instr.wasm");
    instrument_file(&wasm, &out).expect("instrument the fixture wasm");
    let manifest = Manifest::load(&Manifest::path_for(&out)).expect("load the witness manifest");

    // Parse synth's REAL v0.45.0 nested provenance map (schema/module/functions[]).
    let map: SynthProvenanceMap =
        serde_json::from_slice(&std::fs::read(&sidecar).unwrap()).expect("parse the synth sidecar");
    assert_eq!(map.schema, "synth-provenance-v1");

    let report = reconcile(&manifest.branches, &map);

    // (1) THE JOIN IS COMPLETE — every witness branch matched a synth entry.
    // Empirical proof that witness `byte_offset` == synth `instruction_offset`
    // (VCR-DEC-003). Drift here makes this non-zero and fails the build.
    let no_prov = report
        .branches
        .iter()
        .filter(|b| matches!(b.verdict, ObjectVerdict::NoProvenance))
        .count();
    assert_eq!(
        no_prov, 0,
        "offset-domain join broke: {no_prov} witness branch(es) had no synth entry"
    );

    // (2) br_if@47 is `preserved` → obligation-stands (keep the WASM MC/DC obligation).
    let stands = report
        .branches
        .iter()
        .filter(|b| matches!(b.verdict, ObjectVerdict::ObligationStands))
        .count();
    assert_eq!(
        stands, 1,
        "the preserved br_if should keep its WASM obligation"
    );

    // (3) br_table@81 is `split-into-object-branches` → each of its witness branches
    // flags new object coverage (synth introduced object branches).
    assert_eq!(
        report.needs_object_coverage(),
        3,
        "the split br_table's branches should each flag object coverage"
    );

    // (4) synth's folded Select@70 and unconditional Br@95 have no witness branch
    // (witness doesn't instrument them as MC/DC decisions) — SURFACED, not hidden.
    assert_eq!(
        report.only_in_synth.len(),
        2,
        "folded/unconditional synth ops should diverge, visibly"
    );
}
