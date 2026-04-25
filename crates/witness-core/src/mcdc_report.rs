//! MC/DC report — truth tables, independent-effect verdicts, gap analysis.
//!
//! Consumes `RunRecord.decisions` (populated by the v0.6 runner from
//! per-condition globals captured between row markers) and emits per-decision
//! truth tables, condition-level independent-effect verdicts under masking
//! MC/DC (DO-178C accepted variant), and row-closure recommendations for
//! conditions that lack a proving pair.
//!
//! Schema URL: `https://pulseengine.eu/witness-mcdc/v1`.
//!
//! Pure-data: this module compiles to `wasm32-wasip2`. Inputs are
//! deserialised RunRecord values; outputs are serialisable McdcReport
//! values plus a human-readable text formatter.

use crate::Result;
use crate::run_record::{DecisionRecord, DecisionRow, RunRecord, TraceHealth};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const MCDC_SCHEMA_URL: &str = "https://pulseengine.eu/witness-mcdc/v1";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McdcReport {
    pub schema: String,
    pub witness_version: String,
    pub module: String,
    pub overall: McdcOverall,
    pub decisions: Vec<DecisionVerdict>,
    pub trace_health: TraceHealth,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct McdcOverall {
    pub decisions_total: u32,
    pub decisions_full_mcdc: u32,
    pub conditions_total: u32,
    pub conditions_proved: u32,
    pub conditions_gap: u32,
    pub conditions_dead: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DecisionVerdict {
    pub id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_line: Option<u32>,
    pub conditions: Vec<ConditionVerdict>,
    /// Read-only view of `DecisionRow` for downstream renderers.
    pub truth_table: Vec<RowView>,
    pub status: DecisionStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    /// Every condition has an independent-effect witness pair.
    FullMcdc,
    /// Some conditions are proved, others have gaps.
    Partial,
    /// Decision was reached but no conditions could be proved
    /// (e.g. only one row exists, or all rows have the same vector).
    NoWitness,
    /// Decision had zero rows in the run.
    Unreached,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConditionVerdict {
    pub index: u32,
    pub branch_id: u32,
    pub status: ConditionStatus,
    /// `unique-cause` when both rows of the proving pair fully evaluated
    /// every condition; `masking` when any other condition in the pair
    /// was short-circuited (DO-178C masking variant); `unique-cause-plus-
    /// masking` for the in-between case. `None` when no pair was found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interpretation: Option<String>,
    /// Row ids of the proving pair. `None` when no pair was found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pair: Option<[u32; 2]>,
    /// When `status` is `Gap`, a recommended condition vector that would
    /// close the missing pair. The expected outcome is "differs from the
    /// existing row this is paired with".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap_closure: Option<GapClosure>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ConditionStatus {
    /// Independent-effect proved by an existing row pair.
    Proved,
    /// Condition was evaluated in at least one row but no proving pair
    /// exists; need an additional test row.
    Gap,
    /// Condition never evaluated across any row in the run.
    Dead,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GapClosure {
    /// Existing row this recommendation is paired against. The
    /// recommended row should have the same evaluated map for all
    /// indices except the target condition, with the target flipped.
    pub paired_with_row: u32,
    /// Recommended condition vector (target condition flipped from
    /// `paired_with_row`'s value).
    pub evaluated: BTreeMap<u32, bool>,
    /// The recommended row's outcome must differ from
    /// `paired_with_row`'s outcome to close the pair under masking
    /// MC/DC; the runner will confirm by executing the predicate.
    pub outcome_must_differ_from: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RowView {
    pub row_id: u32,
    pub evaluated: BTreeMap<u32, bool>,
    pub outcome: Option<bool>,
}

impl McdcReport {
    pub fn from_record(record: &RunRecord) -> Self {
        let mut overall = McdcOverall::default();
        let mut decisions: Vec<DecisionVerdict> = Vec::with_capacity(record.decisions.len());

        for d in &record.decisions {
            let verdict = analyse_decision(d);
            overall.decisions_total = overall.decisions_total.saturating_add(1);
            if matches!(verdict.status, DecisionStatus::FullMcdc) {
                overall.decisions_full_mcdc = overall.decisions_full_mcdc.saturating_add(1);
            }
            for c in &verdict.conditions {
                overall.conditions_total = overall.conditions_total.saturating_add(1);
                match c.status {
                    ConditionStatus::Proved => {
                        overall.conditions_proved = overall.conditions_proved.saturating_add(1);
                    }
                    ConditionStatus::Gap => {
                        overall.conditions_gap = overall.conditions_gap.saturating_add(1);
                    }
                    ConditionStatus::Dead => {
                        overall.conditions_dead = overall.conditions_dead.saturating_add(1);
                    }
                }
            }
            decisions.push(verdict);
        }

        McdcReport {
            schema: MCDC_SCHEMA_URL.to_string(),
            witness_version: env!("CARGO_PKG_VERSION").to_string(),
            module: record.module_path.clone(),
            overall,
            decisions,
            trace_health: record.trace_health.clone(),
        }
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("module: {}\n", self.module));
        out.push_str(&format!(
            "decisions: {}/{} full MC/DC; conditions: {} proved, {} gap, {} dead\n",
            self.overall.decisions_full_mcdc,
            self.overall.decisions_total,
            self.overall.conditions_proved,
            self.overall.conditions_gap,
            self.overall.conditions_dead,
        ));
        if self.trace_health.overflow {
            out.push_str("WARNING: trace overflow detected — verdicts may be incomplete\n");
        }
        for d in &self.decisions {
            out.push_str(&format!(
                "\ndecision #{}{}{}: {:?}\n",
                d.id,
                d.source_file
                    .as_ref()
                    .map(|f| format!(" {f}"))
                    .unwrap_or_default(),
                d.source_line.map(|l| format!(":{l}")).unwrap_or_default(),
                d.status,
            ));
            out.push_str("  truth table:\n");
            for r in &d.truth_table {
                let cells = r
                    .evaluated
                    .iter()
                    .map(|(i, v)| format!("c{i}={}", if *v { "T" } else { "F" }))
                    .collect::<Vec<_>>()
                    .join(", ");
                let outcome = match r.outcome {
                    Some(true) => "T",
                    Some(false) => "F",
                    None => "?",
                };
                out.push_str(&format!(
                    "    row {}: {{{}}} -> {}\n",
                    r.row_id, cells, outcome
                ));
            }
            out.push_str("  conditions:\n");
            for c in &d.conditions {
                match c.status {
                    ConditionStatus::Proved => {
                        let pair = c.pair.unwrap_or([0, 0]);
                        let interp = c.interpretation.as_deref().unwrap_or("unspecified");
                        out.push_str(&format!(
                            "    c{} (branch {}): proved via rows {}+{} ({})\n",
                            c.index, c.branch_id, pair[0], pair[1], interp,
                        ));
                    }
                    ConditionStatus::Gap => {
                        out.push_str(&format!("    c{} (branch {}): GAP", c.index, c.branch_id));
                        if let Some(gap) = &c.gap_closure {
                            let cells = gap
                                .evaluated
                                .iter()
                                .map(|(i, v)| format!("c{i}={}", if *v { "T" } else { "F" }))
                                .collect::<Vec<_>>()
                                .join(", ");
                            out.push_str(&format!(
                                " — try a row {{{}}} (outcome must differ from row {})",
                                cells, gap.paired_with_row,
                            ));
                        }
                        out.push('\n');
                    }
                    ConditionStatus::Dead => {
                        out.push_str(&format!(
                            "    c{} (branch {}): DEAD — never evaluated in any row\n",
                            c.index, c.branch_id
                        ));
                    }
                }
            }
        }
        out
    }
}

pub fn from_run_file(path: &Path) -> Result<McdcReport> {
    let record = RunRecord::load(path)?;
    Ok(McdcReport::from_record(&record))
}

fn analyse_decision(d: &DecisionRecord) -> DecisionVerdict {
    let truth_table: Vec<RowView> = d
        .rows
        .iter()
        .map(|r| RowView {
            row_id: r.row_id,
            evaluated: r.evaluated.clone(),
            outcome: r.outcome,
        })
        .collect();

    if d.rows.is_empty() {
        return DecisionVerdict {
            id: d.id,
            source_file: d.source_file.clone(),
            source_line: d.source_line,
            conditions: d
                .condition_branch_ids
                .iter()
                .enumerate()
                .map(|(i, &bid)| ConditionVerdict {
                    index: u32::try_from(i).unwrap_or(u32::MAX),
                    branch_id: bid,
                    status: ConditionStatus::Dead,
                    interpretation: None,
                    pair: None,
                    gap_closure: None,
                })
                .collect(),
            truth_table,
            status: DecisionStatus::Unreached,
        };
    }

    let mut conditions: Vec<ConditionVerdict> = Vec::with_capacity(d.condition_branch_ids.len());
    let n = d.condition_branch_ids.len();
    for (idx, &branch_id) in d.condition_branch_ids.iter().enumerate() {
        let i = u32::try_from(idx).unwrap_or(u32::MAX);
        let condition_evaluated_anywhere = d.rows.iter().any(|r| r.evaluated.contains_key(&i));
        if !condition_evaluated_anywhere {
            conditions.push(ConditionVerdict {
                index: i,
                branch_id,
                status: ConditionStatus::Dead,
                interpretation: None,
                pair: None,
                gap_closure: None,
            });
            continue;
        }

        match find_independent_effect_pair(&d.rows, i, n) {
            Some((r1, r2, interpretation)) => {
                conditions.push(ConditionVerdict {
                    index: i,
                    branch_id,
                    status: ConditionStatus::Proved,
                    interpretation: Some(interpretation),
                    pair: Some([r1, r2]),
                    gap_closure: None,
                });
            }
            None => {
                let gap = recommend_gap_closure(&d.rows, i);
                conditions.push(ConditionVerdict {
                    index: i,
                    branch_id,
                    status: ConditionStatus::Gap,
                    interpretation: None,
                    pair: None,
                    gap_closure: gap,
                });
            }
        }
    }

    let any_gap = conditions
        .iter()
        .any(|c| matches!(c.status, ConditionStatus::Gap | ConditionStatus::Dead));
    let any_proved = conditions
        .iter()
        .any(|c| matches!(c.status, ConditionStatus::Proved));
    let status = match (any_proved, any_gap) {
        (true, false) => DecisionStatus::FullMcdc,
        (true, true) => DecisionStatus::Partial,
        (false, true) => DecisionStatus::NoWitness,
        (false, false) => DecisionStatus::FullMcdc, // 0 conditions; degenerate
    };

    DecisionVerdict {
        id: d.id,
        source_file: d.source_file.clone(),
        source_line: d.source_line,
        conditions,
        truth_table,
        status,
    }
}

/// Find a pair of rows that prove condition `target_idx` independently
/// affects the decision outcome under masking MC/DC.
///
/// Pair criterion:
/// 1. Both rows have `target_idx` evaluated.
/// 2. The two rows' values for `target_idx` differ.
/// 3. For every other index `i ≠ target_idx`, if `i` is present in BOTH
///    rows' evaluated maps, the values must agree (masking allows
///    indices present in only one row to be compatible by definition).
/// 4. Both rows have `outcome = Some(_)` and outcomes differ.
///
/// Returns `(row_id_1, row_id_2, interpretation)`. Interpretation is
/// `unique-cause` when both rows fully evaluate every condition (no
/// missing indices), `masking` otherwise. Prefers `unique-cause` pairs.
///
// SAFETY-REVIEW: arithmetic on `i + 1` and `i32`/`u32` index bumps is
// bounded by `rows.len()` (Vec length, fits in usize) and
// `total_conditions` (manifest entry count, fits in u32 by construction).
// Wraparound is impossible for any non-degenerate input.
#[allow(clippy::arithmetic_side_effects)]
fn find_independent_effect_pair(
    rows: &[DecisionRow],
    target_idx: u32,
    total_conditions: usize,
) -> Option<(u32, u32, String)> {
    let mut best: Option<(u32, u32, String)> = None;
    for i in 0..rows.len() {
        for j in (i + 1)..rows.len() {
            // SAFETY-REVIEW: `i` and `j` are bounded by `rows.len()`.
            #[allow(clippy::indexing_slicing)]
            let (r1, r2) = (&rows[i], &rows[j]);
            let v1 = match r1.evaluated.get(&target_idx) {
                Some(v) => *v,
                None => continue,
            };
            let v2 = match r2.evaluated.get(&target_idx) {
                Some(v) => *v,
                None => continue,
            };
            if v1 == v2 {
                continue;
            }
            let (o1, o2) = match (r1.outcome, r2.outcome) {
                (Some(a), Some(b)) => (a, b),
                _ => continue,
            };
            if o1 == o2 {
                continue;
            }
            // Check non-target compatibility under masking.
            let mut compatible = true;
            for idx in 0..u32::try_from(total_conditions).unwrap_or(0) {
                if idx == target_idx {
                    continue;
                }
                match (r1.evaluated.get(&idx), r2.evaluated.get(&idx)) {
                    (Some(a), Some(b)) if a != b => {
                        compatible = false;
                        break;
                    }
                    _ => {}
                }
            }
            if !compatible {
                continue;
            }
            // Determine interpretation.
            let r1_full = r1.evaluated.len() == total_conditions;
            let r2_full = r2.evaluated.len() == total_conditions;
            let interp = if r1_full && r2_full {
                "unique-cause"
            } else {
                "masking"
            };
            // Prefer unique-cause when found; remember it and continue
            // searching only if the current best is masking.
            match (&best, interp) {
                (None, _) => {
                    best = Some((r1.row_id, r2.row_id, interp.to_string()));
                }
                (Some((_, _, current)), "unique-cause") if current != "unique-cause" => {
                    best = Some((r1.row_id, r2.row_id, interp.to_string()));
                }
                _ => {}
            }
            if best.as_ref().map(|(_, _, k)| k.as_str()) == Some("unique-cause") {
                return best;
            }
        }
    }
    best
}

/// When no proving pair exists, recommend a row that would close the
/// missing pair by flipping the target condition's value relative to an
/// existing row.
fn recommend_gap_closure(rows: &[DecisionRow], target_idx: u32) -> Option<GapClosure> {
    // Prefer the row with the most evaluated conditions — gives the most
    // specific recommendation (more constraints → more useful for the
    // user/agent constructing the closing test). Ties broken by latest
    // row_id so deterministic regardless of input ordering.
    let anchor = rows
        .iter()
        .filter(|r| r.evaluated.contains_key(&target_idx) && r.outcome.is_some())
        .max_by_key(|r| (r.evaluated.len(), r.row_id))?;
    let anchor_value = *anchor.evaluated.get(&target_idx)?;
    let anchor_outcome = anchor.outcome?;

    // Recommend a row with target flipped, others identical.
    let mut recommended = anchor.evaluated.clone();
    recommended.insert(target_idx, !anchor_value);

    Some(GapClosure {
        paired_with_row: anchor.row_id,
        evaluated: recommended,
        outcome_must_differ_from: Some(anchor_outcome),
    })
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
    use crate::run_record::{DecisionRecord, DecisionRow, RunRecord, TraceHealth};

    fn row(id: u32, evaluated: &[(u32, bool)], outcome: Option<bool>) -> DecisionRow {
        DecisionRow {
            row_id: id,
            evaluated: evaluated.iter().copied().collect(),
            outcome,
        }
    }

    fn record_with_decision(d: DecisionRecord) -> RunRecord {
        RunRecord {
            schema_version: "3".to_string(),
            witness_version: "test".to_string(),
            module_path: "test.wasm".to_string(),
            invoked: vec![],
            branches: vec![],
            decisions: vec![d],
            trace_health: TraceHealth::default(),
        }
    }

    #[test]
    fn leap_year_full_mcdc() {
        // Decision: (c1 && c2) || c3
        // Rows from verdicts/leap_year/TRUTH-TABLE.md (4 rows, full MC/DC).
        let d = DecisionRecord {
            id: 0,
            source_file: Some("leap_year.rs".to_string()),
            source_line: Some(20),
            condition_branch_ids: vec![100, 101, 102],
            rows: vec![
                row(0, &[(0, false), (2, false)], Some(false)),
                row(1, &[(0, true), (1, true)], Some(true)),
                row(2, &[(0, true), (1, false), (2, false)], Some(false)),
                row(3, &[(0, true), (1, false), (2, true)], Some(true)),
            ],
        };
        let report = McdcReport::from_record(&record_with_decision(d));
        assert_eq!(report.overall.decisions_total, 1);
        assert_eq!(report.overall.decisions_full_mcdc, 1);
        assert_eq!(report.overall.conditions_total, 3);
        assert_eq!(report.overall.conditions_proved, 3);
        assert_eq!(report.overall.conditions_gap, 0);
        let v = &report.decisions[0];
        assert!(matches!(v.status, DecisionStatus::FullMcdc));
        for c in &v.conditions {
            assert!(matches!(c.status, ConditionStatus::Proved));
            assert!(c.pair.is_some());
        }
    }

    #[test]
    fn range_overlap_proves_with_two_pairs() {
        // Decision: c1 && c2 (range_overlap)
        let d = DecisionRecord {
            id: 0,
            source_file: None,
            source_line: None,
            condition_branch_ids: vec![10, 11],
            rows: vec![
                row(0, &[(0, true), (1, false)], Some(false)),
                row(1, &[(0, false)], Some(false)),
                row(2, &[(0, true), (1, true)], Some(true)),
            ],
        };
        let report = McdcReport::from_record(&record_with_decision(d));
        assert_eq!(report.overall.conditions_proved, 2);
    }

    #[test]
    fn incomplete_suite_emits_gap() {
        // leap_year minus row 3 (the c1=T, c2=F, c3=T row).
        let d = DecisionRecord {
            id: 0,
            source_file: None,
            source_line: None,
            condition_branch_ids: vec![100, 101, 102],
            rows: vec![
                row(0, &[(0, false), (2, false)], Some(false)),
                row(1, &[(0, true), (1, true)], Some(true)),
                row(2, &[(0, true), (1, false), (2, false)], Some(false)),
            ],
        };
        let report = McdcReport::from_record(&record_with_decision(d));
        assert_eq!(report.overall.conditions_gap, 1);
        let v = &report.decisions[0];
        let c3 = v.conditions.iter().find(|c| c.index == 2).unwrap();
        assert!(matches!(c3.status, ConditionStatus::Gap));
        let gap = c3.gap_closure.as_ref().unwrap();
        // Anchor is row 2 (c3 evaluated, outcome observed). Recommend
        // c1=T, c2=F, c3=T (flipped from row 2).
        assert_eq!(gap.paired_with_row, 2);
        assert_eq!(gap.evaluated.get(&2), Some(&true));
    }

    #[test]
    fn unreached_decision_marked() {
        let d = DecisionRecord {
            id: 0,
            source_file: None,
            source_line: None,
            condition_branch_ids: vec![10, 11],
            rows: vec![],
        };
        let report = McdcReport::from_record(&record_with_decision(d));
        assert!(matches!(
            report.decisions[0].status,
            DecisionStatus::Unreached
        ));
        for c in &report.decisions[0].conditions {
            assert!(matches!(c.status, ConditionStatus::Dead));
        }
    }

    #[test]
    fn dead_condition_when_never_evaluated() {
        // Two rows, both short-circuit before reaching c3.
        let d = DecisionRecord {
            id: 0,
            source_file: None,
            source_line: None,
            condition_branch_ids: vec![100, 101, 102],
            rows: vec![
                row(0, &[(0, false), (2, false)], Some(false)),
                row(1, &[(0, false), (2, true)], Some(true)),
            ],
        };
        let report = McdcReport::from_record(&record_with_decision(d));
        let c1 = report.decisions[0]
            .conditions
            .iter()
            .find(|c| c.index == 1)
            .unwrap();
        assert!(matches!(c1.status, ConditionStatus::Dead));
    }

    #[test]
    fn text_format_smoke() {
        let d = DecisionRecord {
            id: 7,
            source_file: Some("x.rs".to_string()),
            source_line: Some(99),
            condition_branch_ids: vec![1, 2],
            rows: vec![
                row(0, &[(0, true), (1, true)], Some(true)),
                row(1, &[(0, false)], Some(false)),
                row(2, &[(0, true), (1, false)], Some(false)),
            ],
        };
        let report = McdcReport::from_record(&record_with_decision(d));
        let text = report.to_text();
        assert!(text.contains("decision #7"));
        assert!(text.contains("x.rs:99"));
        assert!(text.contains("truth table"));
    }
}
