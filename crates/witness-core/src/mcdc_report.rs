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
    /// v0.10.0 — declares the polarity convention the truth-table
    /// `c0=T` columns use. v0.9.x reports recorded the **wasm
    /// br_if value** (a Rust `if a { ... }` lowers to
    /// `i32.eqz; br_if`, so the br_if fires when `a` is FALSE; we
    /// recorded that value). v0.10.0 keeps the same on-the-wire
    /// semantics (changing it would silently invert thousands of
    /// existing reports) but adds this field so consumers can detect
    /// the convention. See `docs/concepts.md` §4 for a worked example.
    ///
    /// Always `"wasm-early-exit"` in v0.10.0. v0.10.x may add
    /// `"source-equivalent"` as an opt-in (`witness report
    /// --polarity source`) once the inversion table is fully tested.
    /// Field is `#[serde(default)]` so v0.9.x reports keep loading.
    #[serde(default = "default_polarity")]
    pub interpretation_polarity: String,
}

fn default_polarity() -> String {
    "wasm-early-exit".to_string()
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
    /// v0.11.5 — discriminant-bit independent-effect derivation for
    /// br_table-shape decisions. Present only when (a) the decision
    /// is br_table-shaped, (b) at least one row carried a non-empty
    /// `raw_brvals` map (i.e. the run was produced by v0.11.5+
    /// instrumentation). The per-arm verdict in `conditions` stays
    /// the headline reviewer view; this audit block carries the
    /// textbook MC/DC-over-the-discriminant proof for DO-178C
    /// objective 5.2 work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub br_table_audit: Option<BrTableAudit>,
}

/// v0.11.5 — discriminant-bit MC/DC audit derivation for a br_table
/// decision. Treats the integer discriminant as a vector of bits and
/// tries to find independent-effect witness pairs across rows. Two
/// rows form an independent-effect pair for bit *i* when bit *i*
/// differs and the firing arm differs (because the arm fired *is*
/// the outcome for a switch-shape decision). When such a pair
/// exists for every set bit in the observed range, the decision is
/// "audit-proved" — equivalent to the per-condition MC/DC verdict
/// for boolean decisions.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BrTableAudit {
    /// Number of bits the audit considered. Equal to the highest set
    /// bit position across all observed discriminants, plus one. For a
    /// br_table with arms 0..7 + default, every observed value fits in
    /// 3 bits → `bit_width = 3`. Default-arm rows can lift this if the
    /// observed value runs higher.
    pub bit_width: u32,
    /// Per-bit verdict in ascending bit-position order (bit 0 first).
    pub bits: Vec<BrTableBitVerdict>,
    /// Aggregate audit status. `proved` when every bit position has a
    /// witness pair; `partial` when some bits are proved and some
    /// have gaps; `gap` when any bit is proved-against and missing a
    /// pair (test corpus is too narrow); `not_applicable` when the
    /// decision is reached but no rows carried the required
    /// `raw_brvals` (pre-v0.11.5 instrumentation).
    pub status: BrTableAuditStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum BrTableAuditStatus {
    /// Every bit position has an independent-effect witness pair.
    Proved,
    /// Some bit positions are proved; some have gaps.
    Partial,
    /// At least one bit position was observed across rows but no
    /// independent-effect pair was found for it.
    Gap,
    /// Decision was reached but no rows carried `raw_brvals` data
    /// (pre-v0.11.5 instrumentation).
    NotApplicable,
}

/// v0.11.5 — per-bit verdict within a br_table audit derivation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BrTableBitVerdict {
    /// Bit position (0 = least-significant).
    pub bit: u32,
    pub status: BrTableBitStatus,
    /// Row pair witnessing independent effect of this bit. Present
    /// only when `status == Proved`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pair: Option<[u32; 2]>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum BrTableBitStatus {
    /// Found a row pair where this bit differs and the firing arm
    /// differs accordingly.
    Proved,
    /// Bit was observed but no proving pair found across the run.
    Gap,
    /// Bit position is dead — every observed discriminant agreed on
    /// this bit's value.
    Dead,
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
            let verdict = analyse_decision(d, &record.branches);
            // v0.9.7 — br_table-shape decisions are arm-coverage, not
            // Boolean MC/DC. Count their conditions in proved/gap/dead
            // (so reviewers see arm-hit truth) but exclude them from
            // the decision MC/DC ratio (numerator + denominator) — that
            // ratio is reserved for Boolean decisions where independent-
            // effect proofs apply. Reliable signal: every condition's
            // branch entry is BrTableTarget / BrTableDefault.
            let kinds: Vec<crate::instrument::BranchKind> = d
                .condition_branch_ids
                .iter()
                .filter_map(|bid| {
                    record
                        .branches
                        .iter()
                        .find(|b| b.id == *bid)
                        .map(|b| b.kind)
                })
                .collect();
            let is_br_table = !kinds.is_empty()
                && kinds.iter().all(|k| {
                    matches!(
                        k,
                        crate::instrument::BranchKind::BrTableTarget
                            | crate::instrument::BranchKind::BrTableDefault
                    )
                });
            if !is_br_table {
                overall.decisions_total = overall.decisions_total.saturating_add(1);
                if matches!(verdict.status, DecisionStatus::FullMcdc) {
                    overall.decisions_full_mcdc = overall.decisions_full_mcdc.saturating_add(1);
                }
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
            interpretation_polarity: default_polarity(),
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

// ---------------------------------------------------------------------------
// v0.7.1 — module-rollup report
// ---------------------------------------------------------------------------

/// Per-file MC/DC summary suitable for httparse-scale outputs where the
/// per-decision detail report (1500+ lines) is unreadable. Groups
/// decisions by `source_file` and reports decision / condition tallies
/// per file plus an overall total.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McdcRollup {
    pub schema: String,
    pub witness_version: String,
    pub module: String,
    pub overall: McdcOverall,
    pub by_file: Vec<FileRollup>,
    pub trace_health: TraceHealth,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileRollup {
    pub source_file: String,
    pub decisions_total: u32,
    pub decisions_full_mcdc: u32,
    pub conditions_total: u32,
    pub conditions_proved: u32,
    pub conditions_gap: u32,
    pub conditions_dead: u32,
}

impl McdcRollup {
    pub fn from_report(r: &McdcReport) -> Self {
        let mut by_file_map: BTreeMap<String, FileRollup> = BTreeMap::new();
        for d in &r.decisions {
            let key = d
                .source_file
                .clone()
                .unwrap_or_else(|| "(no source-file)".to_string());
            let entry = by_file_map.entry(key.clone()).or_insert(FileRollup {
                source_file: key,
                decisions_total: 0,
                decisions_full_mcdc: 0,
                conditions_total: 0,
                conditions_proved: 0,
                conditions_gap: 0,
                conditions_dead: 0,
            });
            entry.decisions_total = entry.decisions_total.saturating_add(1);
            if matches!(d.status, DecisionStatus::FullMcdc) {
                entry.decisions_full_mcdc = entry.decisions_full_mcdc.saturating_add(1);
            }
            for c in &d.conditions {
                entry.conditions_total = entry.conditions_total.saturating_add(1);
                match c.status {
                    ConditionStatus::Proved => {
                        entry.conditions_proved = entry.conditions_proved.saturating_add(1);
                    }
                    ConditionStatus::Gap => {
                        entry.conditions_gap = entry.conditions_gap.saturating_add(1);
                    }
                    ConditionStatus::Dead => {
                        entry.conditions_dead = entry.conditions_dead.saturating_add(1);
                    }
                }
            }
        }
        // Sort by decisions_total descending — the user's most-important
        // file (most decisions) lands at the top.
        let mut by_file: Vec<FileRollup> = by_file_map.into_values().collect();
        // Sort by decisions_total descending — the user's most-important
        // file (most decisions) lands at the top. Negate the key for descending.
        by_file.sort_by_key(|f| std::cmp::Reverse(f.decisions_total));

        McdcRollup {
            schema: format!("{MCDC_SCHEMA_URL}/rollup"),
            witness_version: r.witness_version.clone(),
            module: r.module.clone(),
            overall: r.overall.clone(),
            by_file,
            trace_health: r.trace_health.clone(),
        }
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("module: {}\n", self.module));
        out.push_str(&format!(
            "overall: {}/{} full MC/DC; conditions: {} proved, {} gap, {} dead ({} total)\n",
            self.overall.decisions_full_mcdc,
            self.overall.decisions_total,
            self.overall.conditions_proved,
            self.overall.conditions_gap,
            self.overall.conditions_dead,
            self.overall.conditions_total,
        ));
        if self.trace_health.overflow {
            out.push_str("WARNING: trace overflow detected — verdicts may be incomplete\n");
        }
        out.push('\n');
        out.push_str(&format!(
            "{:<40} {:>10} {:>10} {:>10} {:>10} {:>10}\n",
            "source file", "decisions", "full mcdc", "proved", "gap", "dead",
        ));
        out.push_str(&format!(
            "{:-<40} {:->10} {:->10} {:->10} {:->10} {:->10}\n",
            "", "", "", "", "", "",
        ));
        for f in &self.by_file {
            out.push_str(&format!(
                "{:<40} {:>10} {:>10} {:>10} {:>10} {:>10}\n",
                truncate_left(&f.source_file, 40),
                f.decisions_total,
                f.decisions_full_mcdc,
                f.conditions_proved,
                f.conditions_gap,
                f.conditions_dead,
            ));
        }
        out
    }
}

fn truncate_left(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        // Keep the rightmost max-1 chars and prepend an ellipsis.
        let suffix: String = s.chars().rev().take(max.saturating_sub(1)).collect();
        let suffix: String = suffix.chars().rev().collect();
        format!("…{suffix}")
    }
}

pub fn rollup_from_run_file(path: &Path) -> Result<McdcRollup> {
    let report = from_run_file(path)?;
    Ok(McdcRollup::from_report(&report))
}

fn analyse_decision(
    d: &DecisionRecord,
    branches: &[crate::run_record::BranchHit],
) -> DecisionVerdict {
    let truth_table: Vec<RowView> = d
        .rows
        .iter()
        .map(|r| RowView {
            row_id: r.row_id,
            evaluated: r.evaluated.clone(),
            outcome: r.outcome,
        })
        .collect();

    // v0.9.7 — br_table-shape decision. All conditions are
    // BrTableTarget / BrTableDefault entries that share a
    // (function, file, line) triple. There's no Boolean MC/DC to
    // prove (arms are mutually exclusive); per-arm coverage is the
    // honest measurement. Use BranchHit.hits to mark each condition
    // Proved (counter > 0) or Dead (counter = 0).
    let condition_kinds: Vec<crate::instrument::BranchKind> = d
        .condition_branch_ids
        .iter()
        .filter_map(|bid| branches.iter().find(|b| b.id == *bid).map(|b| b.kind))
        .collect();
    let is_br_table_decision = !condition_kinds.is_empty()
        && condition_kinds.iter().all(|k| {
            matches!(
                k,
                crate::instrument::BranchKind::BrTableTarget
                    | crate::instrument::BranchKind::BrTableDefault
            )
        });

    if is_br_table_decision {
        let conditions: Vec<ConditionVerdict> = d
            .condition_branch_ids
            .iter()
            .enumerate()
            .map(|(i, &bid)| {
                let hits = branches
                    .iter()
                    .find(|b| b.id == bid)
                    .map(|b| b.hits)
                    .unwrap_or(0);
                let (status, interpretation) = if hits > 0 {
                    (ConditionStatus::Proved, Some("br-table-arm".to_string()))
                } else {
                    (ConditionStatus::Dead, None)
                };
                ConditionVerdict {
                    index: u32::try_from(i).unwrap_or(u32::MAX),
                    branch_id: bid,
                    status,
                    interpretation,
                    pair: None,
                    gap_closure: None,
                }
            })
            .collect();
        let any_proved = conditions
            .iter()
            .any(|c| matches!(c.status, ConditionStatus::Proved));
        let any_dead = conditions
            .iter()
            .any(|c| matches!(c.status, ConditionStatus::Dead));
        let status = match (any_proved, any_dead) {
            (true, false) => DecisionStatus::FullMcdc,
            (true, true) => DecisionStatus::Partial,
            (false, true) => DecisionStatus::Unreached,
            (false, false) => DecisionStatus::Unreached,
        };
        // v0.11.5 — derive the discriminant-bit audit verdict from
        // the captured `raw_brvals` per row. Skipped silently when
        // pre-v0.11.5 instrumentation produced no raw values.
        let br_table_audit = derive_br_table_audit(d);
        return DecisionVerdict {
            id: d.id,
            source_file: d.source_file.clone(),
            source_line: d.source_line,
            conditions,
            truth_table,
            status,
            br_table_audit,
        };
    }

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
            br_table_audit: None,
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
        // Boolean (non-br_table) decisions don't carry the audit
        // block — the per-condition MC/DC verdict above is the
        // textbook proof. The audit layer is reserved for
        // switch-shape decisions where the source has no boolean
        // chain to find pairs over.
        br_table_audit: None,
    }
}

/// v0.11.5 — derive the discriminant-bit audit verdict for a
/// br_table-shape decision. Walks the captured `raw_brvals` per row
/// and tries to find independent-effect witness pairs across each
/// bit position. A pair (row A, row B) proves bit *i* when:
/// - A and B differ in bit *i* of their discriminant, AND
/// - the firing arm differs between A and B (because the firing
///   arm is the outcome for a switch-shape decision).
///
/// The firing arm per row is recovered by finding the unique
/// condition index in `evaluated` that the row marked as present —
/// the runner sets that on every arm fire (br_table arms get one
/// hit per row at most).
///
/// Returns `None` when the decision is reached but no row carried
/// `raw_brvals` (pre-v0.11.5 instrumentation), so consumers can
/// distinguish "audit skipped because no data" from "audit ran and
/// produced a verdict."
fn derive_br_table_audit(d: &DecisionRecord) -> Option<BrTableAudit> {
    // Collect (row_id, discriminant, firing_arm_idx) tuples for
    // every row that recorded a discriminant. Empty raw_brvals
    // means pre-v0.11.5 instrumentation; bail out so the outer
    // verdict carries `None` rather than a misleading
    // `not_applicable` derivation.
    let mut samples: Vec<(u32, u32, u32)> = Vec::new();
    for r in &d.rows {
        // The runner records one entry in raw_brvals per condition
        // that fired this row; for br_table decisions there's at
        // most one. The condition_index is the firing arm; the
        // value is the discriminant (= arm index for target arms,
        // actual integer for default arm).
        if let Some((&arm_idx, &val)) = r.raw_brvals.iter().next() {
            // Reinterpret as unsigned for bit-decomposition. The
            // i32 → u32 reinterpret is bit-equivalent and sound
            // for any wasm-emitted i32.
            let val_u32 = u32::from_ne_bytes(val.to_ne_bytes());
            samples.push((r.row_id, val_u32, arm_idx));
        }
    }
    if samples.is_empty() {
        return None;
    }

    // Bit width = position of highest set bit + 1 across all
    // observed discriminants, clamped to ≥ 1 so the verdict has at
    // least one bit to report on. 32-bit ceiling is the wasm
    // discriminant width; anything wider would be a different
    // wasm op.
    let max_val = samples.iter().map(|(_, v, _)| *v).max().unwrap_or(0);
    let bit_width: u32 = if max_val == 0 {
        1
    } else {
        let leading = max_val.leading_zeros();
        32u32.saturating_sub(leading)
    };

    let mut bits: Vec<BrTableBitVerdict> =
        Vec::with_capacity(usize::try_from(bit_width).unwrap_or(usize::MAX));
    for bit in 0..bit_width {
        let mask: u32 = 1u32 << bit;
        // Partition rows by this bit's value.
        let mut bit_set: Vec<&(u32, u32, u32)> = Vec::new();
        let mut bit_clear: Vec<&(u32, u32, u32)> = Vec::new();
        for sample in &samples {
            if sample.1 & mask != 0 {
                bit_set.push(sample);
            } else {
                bit_clear.push(sample);
            }
        }
        if bit_set.is_empty() || bit_clear.is_empty() {
            // Bit value never varied across the observed rows —
            // no proving pair is possible, but it's not a "gap"
            // in the test corpus either; this is a Dead bit.
            bits.push(BrTableBitVerdict {
                bit,
                status: BrTableBitStatus::Dead,
                pair: None,
            });
            continue;
        }
        // Find a pair (s, c) where the firing arm differs. That
        // proves bit `bit` independently affected the routing.
        let mut found_pair: Option<[u32; 2]> = None;
        'outer: for s in &bit_set {
            for c in &bit_clear {
                if s.2 != c.2 {
                    found_pair = Some([s.0, c.0]);
                    break 'outer;
                }
            }
        }
        let (status, pair) = match found_pair {
            Some(p) => (BrTableBitStatus::Proved, Some(p)),
            None => (BrTableBitStatus::Gap, None),
        };
        bits.push(BrTableBitVerdict { bit, status, pair });
    }

    // Roll up the per-bit verdicts to an aggregate audit status.
    let any_proved = bits
        .iter()
        .any(|b| matches!(b.status, BrTableBitStatus::Proved));
    let any_gap = bits
        .iter()
        .any(|b| matches!(b.status, BrTableBitStatus::Gap));
    let status = match (any_proved, any_gap) {
        (true, false) => BrTableAuditStatus::Proved,
        (true, true) => BrTableAuditStatus::Partial,
        (false, true) => BrTableAuditStatus::Gap,
        // No proved-or-gap bits means every bit is Dead — the
        // discriminant value never actually varied. Treat as gap
        // (the test corpus is too narrow to reason about
        // independent effect at all).
        (false, false) => BrTableAuditStatus::Gap,
    };
    Some(BrTableAudit {
        bit_width,
        bits,
        status,
    })
}

/// Find a pair of rows that prove condition `target_idx` independently
/// affects the decision outcome under masking MC/DC.
///
/// v0.10.0 — the search algorithm itself lives in the
/// [`witness_mcdc_checker`] crate so safety-critical adopters can audit
/// the qualifiable kernel (~70 LoC, no I/O, no DWARF, no walrus) in
/// isolation. This wrapper bridges from `DecisionRow` (the run-record
/// shape) to `witness_mcdc_checker::Row` (the kernel shape).
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
fn find_independent_effect_pair(
    rows: &[DecisionRow],
    target_idx: u32,
    total_conditions: usize,
) -> Option<(u32, u32, String)> {
    let kernel_rows: Vec<witness_mcdc_checker::Row> = rows
        .iter()
        .map(|r| witness_mcdc_checker::Row {
            row_id: r.row_id,
            evaluated: r.evaluated.clone(),
            outcome: r.outcome,
        })
        .collect();
    witness_mcdc_checker::find_independent_effect_pair(&kernel_rows, target_idx, total_conditions)
        .map(|(a, b, interp)| (a, b, interp.to_string()))
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
    use crate::instrument::BranchKind;
    use crate::run_record::{BranchHit, DecisionRecord, DecisionRow, RunRecord, TraceHealth};

    fn row(id: u32, evaluated: &[(u32, bool)], outcome: Option<bool>) -> DecisionRow {
        DecisionRow {
            row_id: id,
            evaluated: evaluated.iter().copied().collect(),
            outcome,
            raw_brvals: BTreeMap::new(),
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
    fn br_table_audit_proves_each_observed_bit() {
        // v0.11.5 — drive a 4-arm br_table (3 explicit targets + 1
        // default) with discriminants 0, 1, 2, 7. The audit layer
        // should treat the discriminant as 3 bits wide (because 7
        // sets bit 2) and find independent-effect pairs for every
        // bit position:
        //   bit 0: rows {1, 7} (set) vs {0, 2} (clear). Pair: (1, 0)
        //          → arm 1 vs arm 0; differs.
        //   bit 1: rows {2, 7} (set) vs {0, 1} (clear). Pair: (2, 0)
        //          → arm 2 vs arm 0; differs.
        //   bit 2: rows {7} (set, default-arm) vs {0,1,2} (clear,
        //          target arms). Pair: (7-row, 0-row) — arm
        //          differs.
        // To exercise this, build a DecisionRecord by hand with
        // raw_brvals populated.
        let mut rows: Vec<DecisionRow> = Vec::new();
        for (row_id, discriminant) in [(0u32, 0i32), (1, 1), (2, 2), (3, 7)] {
            // condition_index for the firing arm: target arms map
            // 1:1 (arm 0 → cond 0, arm 1 → cond 1, arm 2 → cond 2);
            // default-arm rows hit cond 3 (the default arm's
            // position in condition_branch_ids).
            let cond_idx: u32 = if discriminant < 3 {
                u32::try_from(discriminant).unwrap()
            } else {
                3
            };
            let mut evaluated = BTreeMap::new();
            evaluated.insert(cond_idx, true);
            let mut raw_brvals = BTreeMap::new();
            raw_brvals.insert(cond_idx, discriminant);
            rows.push(DecisionRow {
                row_id,
                evaluated,
                outcome: None,
                raw_brvals,
            });
        }
        let d = DecisionRecord {
            id: 0,
            source_file: Some("switch.rs".to_string()),
            source_line: Some(10),
            condition_branch_ids: vec![10, 11, 12, 13],
            rows,
        };
        // Manifest needs br_table-shape branch entries so the
        // analysis takes the br_table path.
        let mut record = record_with_decision(d);
        record.branches = (0..3)
            .map(|i| BranchHit {
                id: 10 + i,
                function_index: 0,
                function_name: None,
                kind: BranchKind::BrTableTarget,
                instr_index: 0,
                hits: 1,
            })
            .chain(std::iter::once(BranchHit {
                id: 13,
                function_index: 0,
                function_name: None,
                kind: BranchKind::BrTableDefault,
                instr_index: 0,
                hits: 1,
            }))
            .collect();
        let report = McdcReport::from_record(&record);
        let dec = report.decisions.first().expect("decision");
        let audit = dec.br_table_audit.as_ref().expect("br_table_audit present");
        assert_eq!(audit.bit_width, 3, "discriminant 7 sets bit 2");
        assert!(
            matches!(audit.status, BrTableAuditStatus::Proved),
            "every bit should be proved with discriminants {{0,1,2,7}}: {audit:?}"
        );
        for bit in &audit.bits {
            assert!(
                matches!(bit.status, BrTableBitStatus::Proved),
                "bit {} should be proved: {bit:?}",
                bit.bit
            );
            assert!(bit.pair.is_some(), "bit {} missing pair", bit.bit);
        }
    }

    #[test]
    fn br_table_audit_absent_when_pre_v0_11_5_run() {
        // Pre-v0.11.5 instrumentation produces rows with empty
        // raw_brvals; the audit layer should yield None (not a
        // misleading not_applicable verdict at the bit level).
        let mut rows: Vec<DecisionRow> = Vec::new();
        for row_id in 0u32..3 {
            let mut evaluated = BTreeMap::new();
            evaluated.insert(row_id, true);
            rows.push(DecisionRow {
                row_id,
                evaluated,
                outcome: None,
                raw_brvals: BTreeMap::new(),
            });
        }
        let d = DecisionRecord {
            id: 0,
            source_file: None,
            source_line: None,
            condition_branch_ids: vec![10, 11, 12],
            rows,
        };
        let mut record = record_with_decision(d);
        record.branches = (0..3)
            .map(|i| BranchHit {
                id: 10 + i,
                function_index: 0,
                function_name: None,
                kind: if i < 2 {
                    BranchKind::BrTableTarget
                } else {
                    BranchKind::BrTableDefault
                },
                instr_index: 0,
                hits: 1,
            })
            .collect();
        let report = McdcReport::from_record(&record);
        let dec = report.decisions.first().expect("decision");
        assert!(
            dec.br_table_audit.is_none(),
            "no raw_brvals → audit absent (got {:?})",
            dec.br_table_audit
        );
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
