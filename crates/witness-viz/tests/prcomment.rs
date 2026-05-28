//! Integration test for the PR-comment MC/DC delta renderer.

use serde_json::json;
use witness_viz::data::{McdcReport, VerdictBundle};
use witness_viz::prcomment::render_pr_comment;

/// Build a one-verdict bundle from condition statuses. `conds` is a
/// list of (decision_id, condition_index, status) tuples; counts in
/// `overall` are derived so the table math is exercised too.
fn bundle(name: &str, conds: &[(u32, u32, &str)]) -> VerdictBundle {
    use std::collections::BTreeMap;
    let mut decs: BTreeMap<u32, Vec<(u32, &str)>> = BTreeMap::new();
    for (did, ci, st) in conds {
        decs.entry(*did).or_default().push((*ci, *st));
    }
    let proved = conds.iter().filter(|(_, _, s)| *s == "proved").count() as u32;
    let gap = conds.iter().filter(|(_, _, s)| *s == "gap").count() as u32;
    let dead = conds.iter().filter(|(_, _, s)| *s == "dead").count() as u32;
    let decisions: Vec<_> = decs
        .iter()
        .map(|(did, cs)| {
            json!({
                "id": did,
                "source_file": "lib.rs",
                "source_line": 10 + did,
                "status": if cs.iter().all(|(_, s)| *s == "proved") { "full_mcdc" } else { "partial_mcdc" },
                "conditions": cs.iter().map(|(ci, s)| json!({
                    "index": ci, "branch_id": ci, "status": s
                })).collect::<Vec<_>>(),
                "truth_table": [],
            })
        })
        .collect();
    let report: McdcReport = serde_json::from_value(json!({
        "schema": "witness.mcdc.report/v0.5",
        "witness_version": "0.9.0",
        "module": name,
        "overall": {
            "decisions_total": decs.len() as u32,
            "decisions_full_mcdc": decs.values().filter(|cs| cs.iter().all(|(_, s)| *s == "proved")).count() as u32,
            "conditions_total": proved + gap + dead,
            "conditions_proved": proved,
            "conditions_gap": gap,
            "conditions_dead": dead,
        },
        "decisions": decisions,
    }))
    .expect("build report");
    VerdictBundle {
        name: name.to_string(),
        report,
    }
}

#[test]
fn regression_is_surfaced() {
    // base: c0 proved. head: c0 gap. → regression.
    let base = vec![bundle("v", &[(0, 0, "proved")])];
    let head = vec![bundle("v", &[(0, 0, "gap")])];
    let md = render_pr_comment(&base, &head);
    assert!(md.contains("Regressions"), "must have a regressions section: {md}");
    assert!(
        md.contains("`v` decision #0 c0") && md.contains("proved → gap"),
        "must name the regressed condition: {md}"
    );
    // proved count delta 1 → 0 (-1) appears in the table.
    assert!(md.contains("1 → 0 (-1)"), "table must show proved delta: {md}");
}

#[test]
fn improvement_is_surfaced() {
    let base = vec![bundle("v", &[(0, 0, "gap")])];
    let head = vec![bundle("v", &[(0, 0, "proved")])];
    let md = render_pr_comment(&base, &head);
    assert!(md.contains("Improvements"), "must have improvements section: {md}");
    assert!(
        md.contains("gap → proved"),
        "must name the improved condition: {md}"
    );
}

#[test]
fn no_change_says_so() {
    let base = vec![bundle("v", &[(0, 0, "proved"), (0, 1, "gap")])];
    let head = vec![bundle("v", &[(0, 0, "proved"), (0, 1, "gap")])];
    let md = render_pr_comment(&base, &head);
    assert!(
        md.contains("No per-condition status changes"),
        "identical sets report no change: {md}"
    );
    // Unchanged counts render as bare numbers, no arrows.
    assert!(!md.contains("→"), "no transitions ⇒ no arrows in table: {md}");
}

#[test]
fn added_and_removed_verdicts_are_flagged() {
    let base = vec![bundle("old", &[(0, 0, "proved")])];
    let head = vec![bundle("new", &[(0, 0, "proved")])];
    let md = render_pr_comment(&base, &head);
    assert!(md.contains("`new` 🆕"), "added verdict flagged: {md}");
    assert!(md.contains("`old` ❌removed"), "removed verdict flagged: {md}");
}

#[test]
fn gap_to_dead_is_other_not_regression() {
    let base = vec![bundle("v", &[(0, 0, "gap")])];
    let head = vec![bundle("v", &[(0, 0, "dead")])];
    let md = render_pr_comment(&base, &head);
    assert!(
        md.contains("Other transitions"),
        "gap↔dead is neither improvement nor regression: {md}"
    );
    assert!(
        !md.contains("Regressions"),
        "gap → dead must NOT count as a regression (neither was proved): {md}"
    );
}

#[test]
fn empty_both_sides() {
    let md = render_pr_comment(&[], &[]);
    assert!(md.contains("No verdicts on either side"), "{md}");
}
