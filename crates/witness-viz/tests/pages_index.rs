//! Integration test for the multi-version `pages-index` landing page.

use witness_viz::pages_index::{VersionRow, load_site_versions, render_pages_index};

fn row(version: &str, decisions: u64, full: u64, proved: u64, gap: u64) -> VersionRow {
    VersionRow {
        version: version.to_string(),
        verdicts: 13,
        decisions,
        decisions_full_mcdc: full,
        conditions_proved: proved,
        conditions_gap: gap,
        conditions_dead: 0,
        source_files: 9,
    }
}

#[test]
fn renders_table_newest_first_with_deltas() {
    // Caller passes newest-first (as load_site_versions returns).
    let rows = vec![
        row("v0.25.0", 176, 142, 600, 80),
        row("v0.24.0", 176, 140, 595, 85),
    ];
    let out = render_pages_index(&rows);
    let b = &out.body;

    // Both versions linked into their dashboards.
    assert!(b.contains(r#"href="v0.25.0/index.html""#), "v0.25 link: {b}");
    assert!(b.contains(r#"href="v0.24.0/index.html""#), "v0.24 link: {b}");

    // Newest (v0.25.0) appears before older (v0.24.0).
    let p25 = b.find("v0.25.0").expect("v0.25 present");
    let p24 = b.find("v0.24.0").expect("v0.24 present");
    assert!(p25 < p24, "newest must come first");

    // v0.25 full-MC/DC 142 vs v0.24 140 → +2 delta annotation on the
    // newest row.
    assert!(b.contains("142 <span class=\"muted\">(+2)</span>"), "full-mcdc +2: {b}");
    // proved 600 vs 595 → +5.
    assert!(b.contains("600 <span class=\"muted\">(+5)</span>"), "proved +5: {b}");
    // gap 80 vs 85 → -5.
    assert!(b.contains("80 <span class=\"muted\">(-5)</span>"), "gap -5: {b}");

    // The oldest row (v0.24.0) has no Δ (nothing older to compare).
    // Its decisions cell is a bare number.
    assert!(b.contains("<td>176</td>"), "oldest row bare cells: {b}");
}

#[test]
fn empty_site_reports_no_versions() {
    let out = render_pages_index(&[]);
    assert!(
        out.body.contains("No versioned dashboards found"),
        "{}",
        out.body
    );
}

#[test]
fn load_scans_versioned_dirs_and_skips_others() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    // Two version dirs + a non-version dir (_assets) that must be skipped.
    for (v, full) in [("v0.24.0", 140u64), ("v0.25.0", 142u64)] {
        let d = root.join(v);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("summary.json"),
            format!(
                r#"{{"tool":"witness-viz","tool_version":"{v}","verdicts":13,"decisions":176,"decisions_full_mcdc":{full},"conditions":685,"conditions_proved":600,"conditions_gap":80,"conditions_dead":5,"source_files":9}}"#
            ),
        )
        .unwrap();
    }
    std::fs::create_dir_all(root.join("_assets")).unwrap();
    // A version-shaped dir WITHOUT summary.json must be skipped, not error.
    std::fs::create_dir_all(root.join("v0.23.0")).unwrap();

    let rows = load_site_versions(root).expect("load");
    assert_eq!(rows.len(), 2, "only the two with summary.json: {rows:?}");
    // Newest first.
    assert_eq!(rows[0].version, "v0.25.0");
    assert_eq!(rows[1].version, "v0.24.0");
    assert_eq!(rows[0].decisions_full_mcdc, 142);

    // Render is non-empty and links both.
    let out = render_pages_index(&rows);
    assert!(out.body.contains("v0.25.0/index.html"));
    assert!(out.body.contains("v0.24.0/index.html"));
}

#[test]
fn pre_v026_summary_without_aggregates_loads_as_zero() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let d = tmp.path().join("v0.23.0");
    std::fs::create_dir_all(&d).unwrap();
    // v0.23-era summary.json had no MC/DC aggregate fields.
    std::fs::write(
        d.join("summary.json"),
        r#"{"tool":"witness-viz","tool_version":"0.23.0","verdicts":13,"decisions":176,"conditions":685,"source_files":0}"#,
    )
    .unwrap();
    let rows = load_site_versions(tmp.path()).expect("load");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].decisions, 176);
    assert_eq!(rows[0].decisions_full_mcdc, 0, "missing field defaults to 0");
    assert_eq!(rows[0].conditions_proved, 0);
}
