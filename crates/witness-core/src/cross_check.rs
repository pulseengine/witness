//! v0.36 (REQ-058 / DEC-044) — differential cross-runtime coverage check.
//!
//! The same instrumented artifact, run under two backends (the embedded
//! runtime, a kiln `--harness`, a wasmtime-coredump path) with the same
//! invocation, MUST produce identical per-branch hit counts. Any
//! divergence is a defect signal — in a backend, or the instrumentation
//! — and is surfaced explicitly, never a silent pass. With wasmtime as
//! the mature oracle, this cross-validates kiln as it matures
//! (witness#110). This module is the pure-data comparison; orchestration
//! (run under each backend → run.json) lives in the CLI / CI.

use crate::run_record::{BranchHit, RunRecord};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One branch present in both runs whose hit counts disagree.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CounterDivergence {
    pub branch_id: u32,
    pub a_hits: u64,
    pub b_hits: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
}

/// The result of cross-checking two run records' per-branch counters.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CrossCheckReport {
    pub label_a: String,
    pub label_b: String,
    /// Branch ids present in both runs whose counts agree.
    pub agreeing: usize,
    /// Branch ids present in both runs whose counts disagree.
    pub divergences: Vec<CounterDivergence>,
    /// Branch ids only the A run reported (a run-set mismatch — the two
    /// runs measured different branch sets, itself a divergence).
    pub only_in_a: Vec<u32>,
    pub only_in_b: Vec<u32>,
}

impl CrossCheckReport {
    /// The runs fully agree: same branch set, same counts everywhere.
    pub fn agree(&self) -> bool {
        self.divergences.is_empty() && self.only_in_a.is_empty() && self.only_in_b.is_empty()
    }

    /// Human-readable summary. Lists every divergence — a coverage tool
    /// that hides where two runtimes disagree is worse than useless.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let verdict = if self.agree() { "AGREE" } else { "DIVERGE" };
        out.push_str(&format!(
            "cross-check {} vs {}: {verdict}\n  {} branches agree, {} diverge, {} only-in-{}, {} only-in-{}\n",
            self.label_a,
            self.label_b,
            self.agreeing,
            self.divergences.len(),
            self.only_in_a.len(),
            self.label_a,
            self.only_in_b.len(),
            self.label_b,
        ));
        for d in &self.divergences {
            let f = d.function.as_deref().unwrap_or("(anon)");
            out.push_str(&format!(
                "  branch {} ({f}): {}={} vs {}={}\n",
                d.branch_id, self.label_a, d.a_hits, self.label_b, d.b_hits
            ));
        }
        if !self.only_in_a.is_empty() {
            out.push_str(&format!(
                "  only in {}: {:?}\n",
                self.label_a, self.only_in_a
            ));
        }
        if !self.only_in_b.is_empty() {
            out.push_str(&format!(
                "  only in {}: {:?}\n",
                self.label_b, self.only_in_b
            ));
        }
        out
    }
}

/// Cross-check two run records' per-branch hit counts.
pub fn cross_check(a: &RunRecord, b: &RunRecord, label_a: &str, label_b: &str) -> CrossCheckReport {
    let map_a: BTreeMap<u32, &BranchHit> = a.branches.iter().map(|h| (h.id, h)).collect();
    let map_b: BTreeMap<u32, &BranchHit> = b.branches.iter().map(|h| (h.id, h)).collect();

    let mut agreeing = 0usize;
    let mut divergences = Vec::new();
    for (id, ha) in &map_a {
        if let Some(hb) = map_b.get(id) {
            if ha.hits == hb.hits {
                agreeing = agreeing.saturating_add(1);
            } else {
                divergences.push(CounterDivergence {
                    branch_id: *id,
                    a_hits: ha.hits,
                    b_hits: hb.hits,
                    function: ha.display_name().map(str::to_string),
                });
            }
        }
    }
    let only_in_a: Vec<u32> = map_a
        .keys()
        .filter(|id| !map_b.contains_key(id))
        .copied()
        .collect();
    let only_in_b: Vec<u32> = map_b
        .keys()
        .filter(|id| !map_a.contains_key(id))
        .copied()
        .collect();

    CrossCheckReport {
        label_a: label_a.to_string(),
        label_b: label_b.to_string(),
        agreeing,
        divergences,
        only_in_a,
        only_in_b,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::instrument::BranchKind;
    use crate::run_record::{RunRecord, TraceHealth};

    fn rec(hits: &[(u32, u64)]) -> RunRecord {
        RunRecord {
            schema_version: "3".to_string(),
            witness_version: "test".to_string(),
            module_path: "m.wasm".to_string(),
            invoked: vec![],
            branches: hits
                .iter()
                .map(|&(id, h)| BranchHit {
                    id,
                    function_index: 0,
                    function_name: None,
                    function_display: None,
                    kind: BranchKind::BrIf,
                    instr_index: id,
                    hits: h,
                })
                .collect(),
            decisions: vec![],
            trace_health: TraceHealth::default(),
        }
    }

    #[test]
    fn identical_runs_agree() {
        let a = rec(&[(0, 3), (1, 0), (2, 7)]);
        let b = rec(&[(0, 3), (1, 0), (2, 7)]);
        let r = cross_check(&a, &b, "embedded", "kiln");
        assert!(r.agree(), "{}", r.to_text());
        assert_eq!(r.agreeing, 3);
    }

    #[test]
    fn a_counter_divergence_is_flagged() {
        let a = rec(&[(0, 3), (1, 0)]);
        let b = rec(&[(0, 3), (1, 5)]); // branch 1 diverges
        let r = cross_check(&a, &b, "embedded", "kiln");
        assert!(!r.agree());
        assert_eq!(r.divergences.len(), 1);
        assert_eq!(r.divergences[0].branch_id, 1);
        assert_eq!((r.divergences[0].a_hits, r.divergences[0].b_hits), (0, 5));
        assert!(r.to_text().contains("DIVERGE"));
    }

    #[test]
    fn branch_set_mismatch_is_a_divergence() {
        let a = rec(&[(0, 1), (1, 1)]);
        let b = rec(&[(0, 1)]); // branch 1 missing from b
        let r = cross_check(&a, &b, "embedded", "kiln");
        assert!(!r.agree());
        assert_eq!(r.only_in_a, vec![1]);
    }
}
