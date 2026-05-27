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
        source_root: None,
    })
    .expect("run_export");

    // Tree shape.
    assert!(out.join("index.html").is_file(), "index.html");
    assert!(out.join("verdict/foo.html").is_file(), "verdict/foo.html");
    assert!(
        out.join("decision/foo/1.html").is_file(),
        "decision/foo/1.html"
    );
    // v0.24 — proved conditions don't get a drill-down page.
    assert!(
        !out.join("gap/foo/1/0.html").exists(),
        "gap/foo/1/0.html (proved condition) must be skipped"
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
    // index + verdict + decision + 1 gap (proved condition skipped) = 4
    assert_eq!(summary.pages_written, 4, "expected 4 pages, got {summary:?}");
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
    assert!(mf.contains("\"pages_written\": 4"), "manifest pages_written: {mf}");
    assert!(mf.contains("\"verdicts\": 1"), "manifest verdicts: {mf}");

    // Without --source-root, no source snippet is emitted.
    assert!(
        !decision.contains("src-snippet"),
        "decision page should NOT include source snippet when source_root is None"
    );
}

/// v0.24 — when `--source-root` is set and the source file exists,
/// Decision and Gap pages emit an inline `±5 lines` snippet around
/// `source_file:source_line`, with the target line highlighted.
#[test]
fn export_with_source_root_emits_inline_snippets() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let reports = tmp.path().join("reports");
    let out = tmp.path().join("dist");
    let source_root = tmp.path().join("repo");
    std::fs::create_dir_all(&reports).expect("mk reports");
    write_fake_bundle(&reports, "foo");

    // Mirror the fake bundle's source_file path: "src/foo.rs", line 12.
    let src_dir = source_root.join("src");
    std::fs::create_dir_all(&src_dir).expect("mk src dir");
    // 16 lines so the ±5 window around line 12 stays inside the file.
    let src = (1..=16)
        .map(|n| format!("// line {n}: predicate fragment {n}\n"))
        .collect::<String>();
    std::fs::write(src_dir.join("foo.rs"), src).expect("write src/foo.rs");

    let summary = run_export(&ExportOpts {
        reports_dir: reports.clone(),
        out_dir: out.clone(),
        site_title: None,
        source_root: Some(source_root.clone()),
    })
    .expect("run_export");
    assert!(summary.pages_written >= 1);

    let decision = std::fs::read_to_string(out.join("decision/foo/1.html"))
        .expect("read decision page");
    assert!(
        decision.contains("src-snippet"),
        "decision page must include the source-snippet block when source_root is set"
    );
    assert!(
        decision.contains("src-snippet-target"),
        "target line must carry the .src-snippet-target class"
    );
    // Decision is at line 12 in fake bundle → window 7..=16. Verify
    // the target line text appears AND lines outside ±5 don't appear.
    assert!(
        decision.contains("line 12:"),
        "decision snippet must contain the target line content"
    );
    assert!(
        decision.contains("line 7:") && decision.contains("line 16:"),
        "decision snippet must contain the ±5 boundary lines"
    );
    assert!(
        !decision.contains("line 6:"),
        "decision snippet must NOT contain lines outside the ±5 window"
    );

    // Gap page should also carry the snippet (same decision metadata).
    let gap = std::fs::read_to_string(out.join("gap/foo/1/1.html")).expect("read gap page");
    assert!(
        gap.contains("src-snippet-target"),
        "gap page must include the snippet (gap is on the same Decision)"
    );

    // Snippet must HTML-escape its content (no raw `<` etc.). Smoke
    // check: the synthesized source uses // comments — no HTML hazards,
    // but make sure the rendered output doesn't contain unescaped
    // angle brackets from a hypothetical generic in source.
    assert!(
        !decision.contains("<line 12"),
        "no unescaped HTML in snippet"
    );

    // v0.24 — full-file source page exists and is syntax-highlighted.
    let src_page_path = out.join("source/src/foo.rs.html");
    assert!(
        src_page_path.is_file(),
        "full-file source page must be emitted at out/source/src/foo.rs.html"
    );
    let src_page = std::fs::read_to_string(&src_page_path).expect("read source page");
    assert!(
        src_page.contains("class=\"source-full\""),
        "source page must use .source-full class"
    );
    assert!(
        src_page.contains("id=\"L12\""),
        "source page must carry an #L12 anchor for the marked Decision line"
    );
    assert!(
        src_page.contains("class=\"src-line marked\""),
        "the Decision line must be marked"
    );
    // Asset prefix at depth 3 (`source/src/foo.rs.html` → 3 segments
    // deep — `source/` + `src/` + `foo.rs.html`).
    assert!(
        src_page.contains("href=\"../../../_assets/styles.css\""),
        "source page (depth 3) must reference ../../../_assets/styles.css; saw chrome head: {}",
        &src_page[..src_page.len().min(400)]
    );

    // "view full file" link from the snippet section points at the
    // correct relative path with `#L12` anchor.
    assert!(
        decision.contains("source/src/foo.rs.html#L12"),
        "decision snippet must link to the full file at the right anchor"
    );

    // Manifest carries the source_files count.
    let mf2 = std::fs::read_to_string(out.join("summary.json")).expect("read manifest");
    assert!(
        mf2.contains("\"source_files\": 1"),
        "manifest source_files: {mf2}"
    );
}

/// `--source-root` pointing at a tree that's missing the recorded
/// source_file: the snippet is suppressed but the rest of the page
/// renders normally.
#[test]
fn export_with_missing_source_file_degrades_gracefully() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let reports = tmp.path().join("reports");
    let out = tmp.path().join("dist");
    let source_root = tmp.path().join("empty-repo");
    std::fs::create_dir_all(&reports).expect("mk reports");
    std::fs::create_dir_all(&source_root).expect("mk empty repo");
    write_fake_bundle(&reports, "foo");
    // source_root exists but is empty — src/foo.rs is NOT there.

    let summary = run_export(&ExportOpts {
        reports_dir: reports.clone(),
        out_dir: out.clone(),
        site_title: None,
        source_root: Some(source_root.clone()),
    })
    .expect("run_export should succeed even when source files missing");
    assert!(summary.pages_written >= 1);

    let decision = std::fs::read_to_string(out.join("decision/foo/1.html"))
        .expect("decision page rendered even without source");
    assert!(
        !decision.contains("src-snippet"),
        "missing source file must suppress the snippet block (no half-rendered HTML)"
    );
    // Rest of the page is intact.
    assert!(decision.contains("Truth table"), "truth table still present");
}
