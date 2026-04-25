//! Run-record types and pure-data aggregation.
//!
//! Witness produces a `RunRecord` for every coverage run — the result
//! of pairing the module's instrumented counter values with the branch
//! manifest. This module holds the data types plus the cross-run
//! aggregation primitives (`merge_records` / `merge_files`).
//!
//! Wasmtime / walrus-side runners live in the `witness` binary crate,
//! not here. This module compiles to `wasm32-wasip2`.

use crate::Result;
use crate::error::Error;
use crate::instrument::BranchKind;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// Raw-run output: each branch paired with the counter's final value.
///
/// v0.6 adds the `decisions` and `trace_health` fields for MC/DC truth-table
/// reconstruction. Both are `#[serde(default)]` so v0.5 records (schema "2")
/// still deserialise cleanly with empty decisions and a clean trace_health.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RunRecord {
    pub schema_version: String,
    pub witness_version: String,
    pub module_path: String,
    pub invoked: Vec<String>,
    pub branches: Vec<BranchHit>,
    /// v0.6 — per-decision row tables built from per-condition globals
    /// captured between row-marker invocations. Empty when running a
    /// non-instrumented module, when no decisions were reconstructed via
    /// DWARF, or when the harness path didn't surface row data.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<DecisionRecord>,
    /// v0.6 — health of the per-row capture path. Reporters MUST refuse
    /// to emit MC/DC verdicts when `overflow` is true.
    #[serde(default)]
    pub trace_health: TraceHealth,
}

/// One source-level decision's per-row truth table.
///
/// `id` matches `Manifest::Decision::id`. `condition_branch_ids` mirrors
/// the manifest's `Decision::conditions` field (branch ids of the conditions
/// in evaluation order). `rows` carries one entry per row marker the runner
/// emitted, in invocation order.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DecisionRecord {
    pub id: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_line: Option<u32>,
    pub condition_branch_ids: Vec<u32>,
    pub rows: Vec<DecisionRow>,
}

/// One observation of a decision's condition vector + outcome under
/// short-circuit evaluation.
///
/// `evaluated` is sparse: condition indices that were short-circuited and
/// never evaluated are absent. The reporter handles the masking-MC/DC
/// case via this absent-vs-present distinction.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DecisionRow {
    pub row_id: u32,
    /// condition_index (0..N-1) -> evaluated bool. Absent indices indicate
    /// the condition was short-circuited and not evaluated this row.
    pub evaluated: BTreeMap<u32, bool>,
    /// Decision outcome. `None` if the decision was reached but the
    /// outcome was not observed (e.g. function returned mid-chain).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<bool>,
}

/// Health of the per-row capture path. The reporter refuses MC/DC verdicts
/// when `overflow` is true.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TraceHealth {
    /// True if the per-row capture saturated any per-condition counter.
    /// In the per-row-globals scheme, this means a condition was
    /// evaluated more than once between row markers — degenerate for
    /// our verdict suite (each row invokes the predicate once) but
    /// expected for v0.7's loop-bearing programs.
    #[serde(default)]
    pub overflow: bool,
    /// Total rows captured.
    #[serde(default)]
    pub rows: u64,
    /// True if any decision had a row whose outcome could not be
    /// determined (e.g. function panicked or returned mid-chain).
    #[serde(default)]
    pub ambiguous_rows: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BranchHit {
    pub id: u32,
    pub function_index: u32,
    pub function_name: Option<String>,
    pub kind: BranchKind,
    pub instr_index: u32,
    pub hits: u64,
}

impl RunRecord {
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(Error::Serde)?;
        std::fs::write(path, json).map_err(Error::Io)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).map_err(Error::Io)?;
        serde_json::from_slice(&bytes).map_err(|source| Error::RunOutput {
            path: path.to_path_buf(),
            source,
        })
    }
}

/// Counter snapshot the subprocess harness writes; the bridge format
/// between harnesses and witness's run-record assembly.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HarnessSnapshot {
    pub schema: String,
    pub counters: HashMap<String, u64>,
}

impl HarnessSnapshot {
    pub const SCHEMA: &'static str = "witness-harness-v1";

    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).map_err(Error::Io)?;
        serde_json::from_slice(&bytes).map_err(|source| Error::RunOutput {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Convert string-keyed counters to `branch_id -> hits` for merging.
    pub fn into_id_map(self) -> Result<HashMap<u32, u64>> {
        let mut out = HashMap::new();
        for (k, v) in self.counters {
            let id = k.parse::<u32>().map_err(|_| {
                Error::Runtime(anyhow::anyhow!(
                    "harness snapshot contains non-numeric counter id `{k}`"
                ))
            })?;
            out.insert(id, v);
        }
        Ok(out)
    }
}

/// Merge multiple `RunRecord`s into one by summing per-branch counters.
///
/// All inputs must share the same `schema_version`, `module_path`, and
/// branch list (same length, same `(id, function_index, instr_index,
/// kind)` per position). `invoked` lists are concatenated in input
/// order; `witness_version` is set to the current crate version (the
/// merge tool advertises itself, not any single input run).
pub fn merge_records(records: &[RunRecord]) -> Result<RunRecord> {
    let head = records
        .first()
        .ok_or_else(|| Error::Runtime(anyhow::anyhow!("merge_records called with no inputs")))?;

    for (i, r) in records.iter().enumerate().skip(1) {
        if r.schema_version != head.schema_version {
            return Err(Error::Runtime(anyhow::anyhow!(
                "schema mismatch at input {i}: expected `{}`, got `{}`",
                head.schema_version,
                r.schema_version
            )));
        }
        if r.module_path != head.module_path {
            return Err(Error::Runtime(anyhow::anyhow!(
                "module_path mismatch at input {i}: expected `{}`, got `{}`",
                head.module_path,
                r.module_path
            )));
        }
        if r.branches.len() != head.branches.len() {
            return Err(Error::Runtime(anyhow::anyhow!(
                "branch count mismatch at input {i}: expected {}, got {}",
                head.branches.len(),
                r.branches.len()
            )));
        }
        for (j, (a, b)) in head.branches.iter().zip(r.branches.iter()).enumerate() {
            let same = a.id == b.id
                && a.function_index == b.function_index
                && a.instr_index == b.instr_index
                && a.kind == b.kind;
            if !same {
                return Err(Error::Runtime(anyhow::anyhow!(
                    "branch entry {j} differs between input 0 and input {i} \
                     (id/function_index/instr_index/kind must match)"
                )));
            }
        }
    }

    let mut merged_branches: Vec<BranchHit> = head.branches.clone();
    for r in records.iter().skip(1) {
        for (acc, b) in merged_branches.iter_mut().zip(r.branches.iter()) {
            acc.hits = acc.hits.saturating_add(b.hits);
        }
    }

    let mut invoked: Vec<String> = Vec::new();
    for r in records {
        invoked.extend(r.invoked.iter().cloned());
    }

    // v0.6: concatenate decision rows in input order, preserving per-decision
    // identity. Cross-record decision ids must agree (they come from the
    // shared manifest); we trust the validation above to catch mismatches.
    let mut merged_decisions: Vec<DecisionRecord> = head.decisions.clone();
    for r in records.iter().skip(1) {
        for (acc, d) in merged_decisions.iter_mut().zip(r.decisions.iter()) {
            acc.rows.extend(d.rows.iter().cloned());
        }
    }
    let merged_health = TraceHealth {
        overflow: records.iter().any(|r| r.trace_health.overflow),
        rows: records.iter().map(|r| r.trace_health.rows).sum(),
        ambiguous_rows: records.iter().any(|r| r.trace_health.ambiguous_rows),
    };

    Ok(RunRecord {
        schema_version: head.schema_version.clone(),
        witness_version: env!("CARGO_PKG_VERSION").to_string(),
        module_path: head.module_path.clone(),
        invoked,
        branches: merged_branches,
        decisions: merged_decisions,
        trace_health: merged_health,
    })
}

/// Read N run JSON files and write the merged result to `output`.
pub fn merge_files(inputs: &[PathBuf], output: &Path) -> Result<()> {
    if inputs.is_empty() {
        return Err(Error::Runtime(anyhow::anyhow!(
            "merge requires at least one input run JSON"
        )));
    }
    let records: Vec<RunRecord> = inputs
        .iter()
        .map(|p| RunRecord::load(p))
        .collect::<Result<Vec<_>>>()?;
    let merged = merge_records(&records)?;
    merged.save(output)
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
    use tempfile::tempdir;

    fn make_record(module_path: &str, hits_seq: &[u64]) -> RunRecord {
        let branches: Vec<BranchHit> = hits_seq
            .iter()
            .enumerate()
            .map(|(i, &h)| BranchHit {
                id: u32::try_from(i).unwrap(),
                function_index: 0,
                function_name: None,
                kind: BranchKind::BrIf,
                instr_index: u32::try_from(i).unwrap(),
                hits: h,
            })
            .collect();
        RunRecord {
            schema_version: "2".to_string(),
            witness_version: "test".to_string(),
            module_path: module_path.to_string(),
            invoked: vec!["fn1".to_string()],
            branches,
            decisions: vec![],
            trace_health: TraceHealth::default(),
        }
    }

    #[test]
    fn merge_sums_counters() {
        let a = make_record("m.wasm", &[1, 0, 3]);
        let b = make_record("m.wasm", &[0, 5, 2]);
        let merged = merge_records(&[a, b]).unwrap();
        assert_eq!(merged.branches.len(), 3);
        assert_eq!(merged.branches[0].hits, 1);
        assert_eq!(merged.branches[1].hits, 5);
        assert_eq!(merged.branches[2].hits, 5);
        assert_eq!(merged.invoked, vec!["fn1".to_string(), "fn1".to_string()]);
    }

    #[test]
    fn merge_rejects_module_mismatch() {
        let a = make_record("m1.wasm", &[1]);
        let b = make_record("m2.wasm", &[1]);
        let result = merge_records(&[a, b]);
        assert!(matches!(result, Err(Error::Runtime(_))));
    }

    #[test]
    fn merge_rejects_branch_count_mismatch() {
        let a = make_record("m.wasm", &[1, 2]);
        let b = make_record("m.wasm", &[1, 2, 3]);
        let result = merge_records(&[a, b]);
        assert!(matches!(result, Err(Error::Runtime(_))));
    }

    #[test]
    fn merge_rejects_empty() {
        let result = merge_records(&[]);
        assert!(matches!(result, Err(Error::Runtime(_))));
    }

    #[test]
    fn merge_files_round_trip() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.json");
        let b = dir.path().join("b.json");
        let out = dir.path().join("merged.json");
        make_record("m.wasm", &[2, 0]).save(&a).unwrap();
        make_record("m.wasm", &[0, 3]).save(&b).unwrap();
        merge_files(&[a, b], &out).unwrap();
        let merged = RunRecord::load(&out).unwrap();
        assert_eq!(merged.branches[0].hits, 2);
        assert_eq!(merged.branches[1].hits, 3);
        assert_eq!(merged.witness_version, env!("CARGO_PKG_VERSION"));
    }
}
