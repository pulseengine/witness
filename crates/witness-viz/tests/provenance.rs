//! Integration test for per-condition provenance (v0.27, DEC-035):
//! function name + branch kind joined from manifest.json by branch_id,
//! plus the inline call chain when one exists.

use std::path::Path;

use serde_json::json;

use witness_viz::data::load_branch_provenance;
use witness_viz::export::{ExportOpts, run_export};

/// A bundle with two conditions: c0 is a `br_table` arm in a local
/// function (no inline chain), c1 is a `br_if` inlined from a core
/// function (has a 2-frame chain). Mirrors the json_lite vs
/// parser_dispatch split seen in the real evidence.
fn write_bundle(root: &Path) {
    let dir = root.join("v");
    std::fs::create_dir_all(&dir).unwrap();

    let report = json!({
        "schema": "witness.mcdc.report/v0.5",
        "witness_version": "0.27.0",
        "module": "v",
        "overall": {
            "decisions_total": 1, "decisions_full_mcdc": 0,
            "conditions_total": 2, "conditions_proved": 1,
            "conditions_gap": 1, "conditions_dead": 0
        },
        "decisions": [{
            "id": 0, "source_file": "lib.rs", "source_line": 40,
            "status": "partial_mcdc",
            "conditions": [
                { "index": 0, "branch_id": 14, "status": "proved",
                  "interpretation": "masking", "pair": [0, 1] },
                { "index": 1, "branch_id": 113, "status": "gap" }
            ],
            "truth_table": [
                { "row_id": 0, "evaluated": {"0": false, "1": false}, "outcome": false },
                { "row_id": 1, "evaluated": {"0": true,  "1": false}, "outcome": true }
            ]
        }]
    });
    std::fs::write(dir.join("report.json"), serde_json::to_vec_pretty(&report).unwrap()).unwrap();

    // Manifest: branches[] keyed by `id`, plus branch_inline_chains
    // keyed by stringified id. Function names are Rust-mangled so the
    // test also exercises demangling.
    let manifest = json!({
        "schema_version": "witness.manifest/v0.6",
        "branches": [
            { "id": 14,  "function_name": "_ZN1v15parse_primitive17habcdef0123456789E",
              "kind": "br_table_target", "byte_offset": 1148 },
            { "id": 113, "function_name": "_ZN4core5slice6memchr14memchr_aligned17h0011223344556677E",
              "kind": "br_if", "byte_offset": 2200 }
        ],
        "branch_inline_chains": {
            "113": [
                { "call_file": "mod.rs",    "call_line": 2447 },
                { "call_file": "memchr.rs", "call_line": 104 }
            ]
        }
    });
    std::fs::write(dir.join("manifest.json"), serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
}

#[test]
fn loader_joins_branches_and_chains_and_demangles() {
    let tmp = tempfile::tempdir().unwrap();
    write_bundle(tmp.path());

    let prov = load_branch_provenance(tmp.path(), "v");

    // br_table arm: demangled function, kind, no chain.
    let c0 = prov.get(&14).expect("branch 14 present");
    assert_eq!(c0.function, "v::parse_primitive", "demangled, hash stripped");
    assert_eq!(c0.kind, "br_table_target");
    assert!(c0.inline_chain.is_empty(), "br_table arm has no inline chain");

    // inlined br_if: function, kind, 2-frame chain.
    let c1 = prov.get(&113).expect("branch 113 present");
    assert_eq!(c1.function, "core::slice::memchr::memchr_aligned");
    assert_eq!(c1.kind, "br_if");
    assert_eq!(c1.inline_chain.len(), 2);
    assert_eq!(c1.inline_chain[0].call_file, "mod.rs");
    assert_eq!(c1.inline_chain[0].call_line, 2447);
    assert_eq!(c1.inline_chain[1].call_file, "memchr.rs");
}

#[test]
fn decision_page_renders_provenance() {
    let tmp = tempfile::tempdir().unwrap();
    let reports = tmp.path().join("reports");
    let out = tmp.path().join("dist");
    std::fs::create_dir_all(&reports).unwrap();
    write_bundle(&reports);

    run_export(&ExportOpts {
        reports_dir: reports.clone(),
        out_dir: out.clone(),
        site_title: None,
        source_root: None,
    })
    .expect("run_export");

    let page = std::fs::read_to_string(out.join("decision/v/0.html")).expect("decision page");

    // br_table arm shows function + kind, no chain.
    assert!(
        page.contains("v::parse_primitive"),
        "decision page must show the demangled function: {page}"
    );
    assert!(
        page.contains(r#"<span class="kind">br_table_target</span>"#),
        "must show the br_table_target kind"
    );
    // inlined branch shows its chain, outermost-first joined with ←.
    assert!(
        page.contains("inlined: mod.rs:2447 ← memchr.rs:104"),
        "must render the inline chain for branch 113: {page}"
    );
    assert!(
        page.contains("core::slice::memchr::memchr_aligned"),
        "must show the demangled core function"
    );
}

#[test]
fn missing_manifest_degrades_to_no_provenance() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("v");
    std::fs::create_dir_all(&dir).unwrap();
    // report.json only — no manifest.json.
    std::fs::write(
        dir.join("report.json"),
        serde_json::to_vec(&json!({
            "schema": "witness.mcdc.report/v0.5", "witness_version": "0.27.0", "module": "v",
            "overall": {"decisions_total":1,"decisions_full_mcdc":1,"conditions_total":1,
                        "conditions_proved":1,"conditions_gap":0,"conditions_dead":0},
            "decisions": [{"id":0,"source_file":"lib.rs","source_line":1,"status":"full_mcdc",
                "conditions":[{"index":0,"branch_id":0,"status":"proved","pair":[0,1]}],
                "truth_table":[]}]
        }))
        .unwrap(),
    )
    .unwrap();

    let prov = load_branch_provenance(tmp.path(), "v");
    assert!(prov.is_empty(), "no manifest ⇒ empty provenance map");
}
