//! Integration test for the static-export driver.
//!
//! Builds a fake verdict bundle in a tempdir, runs `run_export`,
//! and asserts the output tree shape + per-page link relativeness.
//! Mirrors the inputs `integration.rs::end_to_end_smoke` uses for
//! the live axum dashboard.

use std::path::Path;

use serde_json::json;

use witness_viz::export::{ExportOpts, run_export};

fn write_fake_bundle(root: &Path, name: &str) {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).expect("create verdict dir");

    let report = json!({
        "schema": "witness.mcdc.report/v0.5",
        "witness_version": "0.9.0",
        "module": "fake_module",
        "overall": {
            "decisions_total": 1,
            "decisions_full_mcdc": 0,
            "conditions_total": 2,
            "conditions_proved": 1,
            "conditions_gap": 1,
            "conditions_dead": 0,
        },
        "decisions": [
            {
                "id": 1,
                "source_file": "src/foo.rs",
                "source_line": 12,
                "status": "partial_mcdc",
                "conditions": [
                    { "index": 0, "branch_id": 100, "status": "proved",
                      "interpretation": "isolates c0", "pair": [0, 1] },
                    { "index": 1, "branch_id": 101, "status": "gap" }
                ],
                "truth_table": [
                    { "row_id": 0, "evaluated": { "0": false, "1": false }, "outcome": false },
                    { "row_id": 1, "evaluated": { "0": true,  "1": false }, "outcome": true }
                ]
            }
        ]
    });
    std::fs::write(
        dir.join("report.json"),
        serde_json::to_vec_pretty(&report).expect("serialize report"),
    )
    .expect("write report.json");

    let manifest = json!({
        "schema": "witness.manifest/v0.5",
        "branches": [
            { "branch_id": 100, "function_name": "foo::eval", "byte_offset": 42 },
            { "branch_id": 101, "function_name": "foo::eval", "byte_offset": 64 },
        ],
    });
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest.json");
}

#[test]
fn export_writes_self_contained_static_site() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let reports = tmp.path().join("reports");
    let out = tmp.path().join("dist");
    std::fs::create_dir_all(&reports).expect("mk reports");
    write_fake_bundle(&reports, "foo");

    let summary = run_export(&ExportOpts {
        reports_dir: reports.clone(),
        out_dir: out.clone(),
        site_title: Some("test".to_string()),
    })
    .expect("run_export");

    // Tree shape.
    assert!(out.join("index.html").is_file(), "index.html");
    assert!(out.join("verdict/foo.html").is_file(), "verdict/foo.html");
    assert!(
        out.join("decision/foo/1.html").is_file(),
        "decision/foo/1.html"
    );
    assert!(
        out.join("gap/foo/1/0.html").is_file(),
        "gap/foo/1/0.html (proved condition)"
    );
    assert!(
        out.join("gap/foo/1/1.html").is_file(),
        "gap/foo/1/1.html (gap condition)"
    );
    assert!(out.join("_assets/styles.css").is_file(), "_assets/styles.css");
    assert!(out.join("summary.json").is_file(), "summary.json");

    // Summary counts add up.
    assert_eq!(summary.verdicts, 1);
    assert_eq!(summary.decisions, 1);
    assert_eq!(summary.conditions, 2);
    // index + verdict + decision + 2 conditions = 5
    assert_eq!(summary.pages_written, 5, "expected 5 pages, got {summary:?}");
    assert!(summary.bytes_written > 1_000, "bytes should be > 1K");

    // Link relativeness checks — the whole point of the export refactor.
    let index = std::fs::read_to_string(out.join("index.html")).expect("read index");
    assert!(
        !index.contains("href=\"/verdict/"),
        "index must not emit absolute /verdict/ links (saw: {index})"
    );
    assert!(
        index.contains("href=\"verdict/foo.html\""),
        "index must link to relative verdict/foo.html"
    );
    // No HTMX in static export.
    assert!(
        !index.contains("htmx.min.js"),
        "static export must omit HTMX (no axum to swap against)"
    );
    // Asset path is relative (depth 0 → no `../` prefix).
    assert!(
        index.contains("href=\"_assets/styles.css\""),
        "index must reference _assets/styles.css relative"
    );

    let verdict = std::fs::read_to_string(out.join("verdict/foo.html")).expect("read verdict");
    // depth 1 → `../` prefix.
    assert!(
        verdict.contains("href=\"../_assets/styles.css\""),
        "verdict page must reference ../_assets/styles.css"
    );
    assert!(
        verdict.contains("href=\"../decision/foo/1.html\""),
        "verdict page must link to ../decision/foo/1.html"
    );
    assert!(
        verdict.contains("href=\"../index.html\""),
        "verdict page back-link must be ../index.html (saw: {})",
        &verdict[..verdict.len().min(800)]
    );

    let decision = std::fs::read_to_string(out.join("decision/foo/1.html")).expect("read decision");
    // depth 2 → `../../` prefix.
    assert!(
        decision.contains("href=\"../../_assets/styles.css\""),
        "decision page must reference ../../_assets/styles.css"
    );
    assert!(
        decision.contains("href=\"../../verdict/foo.html\""),
        "decision back-link must be ../../verdict/foo.html"
    );

    let gap = std::fs::read_to_string(out.join("gap/foo/1/1.html")).expect("read gap");
    // depth 3 → `../../../` prefix.
    assert!(
        gap.contains("href=\"../../../_assets/styles.css\""),
        "gap page must reference ../../../_assets/styles.css"
    );
    assert!(
        gap.contains("href=\"../../../decision/foo/1.html\""),
        "gap back-link must be ../../../decision/foo/1.html"
    );

    // Manifest contains expected counts.
    let mf = std::fs::read_to_string(out.join("summary.json")).expect("read manifest");
    assert!(mf.contains("\"pages_written\": 5"), "manifest pages_written: {mf}");
    assert!(mf.contains("\"verdicts\": 1"), "manifest verdicts: {mf}");
}
