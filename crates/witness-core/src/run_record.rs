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
    /// v0.12.0 — DWARF inlined-call-site context this decision was
    /// reconstructed under. Mirrors `Manifest::Decision::inline_context`
    /// (the manifest is the source of truth at instrument time; this
    /// field plumbs it through into the run record so reporters can
    /// attribute decisions to call sites without re-reading the
    /// manifest). Pre-v0.12 records keep deserialising via
    /// `#[serde(default)]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline_context: Option<crate::instrument::InlineContext>,
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
    /// v0.11.5 — raw per-condition `brval` integers, indexed by
    /// condition_index. Populated by the embedded runner when the
    /// instrumented module exports per-row `__witness_brval_<id>`
    /// globals (v0.6.1 instrumentation onwards for br_if; v0.11.5
    /// onwards for br_table arms).
    ///
    /// For boolean conditions (br_if/if-then/if-else) the value is
    /// the wasm-level br_if value (`0` or `1`) and is redundant
    /// with `evaluated`. For br_table arms it carries the **actual
    /// discriminant integer** when the arm fired:
    /// - target arm `i`: `raw_brvals[i] = i` (constant, can be
    ///   skipped by the audit layer).
    /// - default arm: `raw_brvals[default] = actual_discriminant`
    ///   (the load-bearing capture — the audit layer needs this
    ///   for discriminant-bit independent-effect proofs over the
    ///   full bit width, since target-arm rows only pin the value
    ///   to `< N` and default-arm rows could be any `≥ N`).
    ///
    /// `#[serde(default)]` so v0.11.4 and earlier run records
    /// keep deserialising; absent map = audit layer no-ops.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub raw_brvals: BTreeMap<u32, i32>,
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
    /// v0.10.0 — renamed from `ambiguous_rows` (E1 BUG-11 / B5).
    /// Now: True when the trace-buffer parser produced per-iteration
    /// rows (post-v0.7.3 instrumentation with kind=0 + kind=2 records).
    /// Pre-v0.7.3 modules and harness-v1 snapshots leave this `false`
    /// because they only ship counter aggregates. The old name was
    /// misleading — both fully-proved and fully-gap runs would set it
    /// to `true` so long as trace memory had data. Reviewers reading
    /// the field cold thought it indicated *errors*.
    ///
    /// The legacy name `ambiguous_rows` is still accepted on
    /// deserialise (via `#[serde(alias)]`) so v0.9.x run.json files
    /// keep loading; it will be removed in v0.11.
    #[serde(default, alias = "ambiguous_rows")]
    pub trace_parser_active: bool,
    /// v0.9.8 — total trace memory bytes consumed across all rows
    /// (sum of `cursor - TRACE_HEADER_BYTES` per row). Lets reviewers
    /// see how close they got to the trace memory cap. Zero means no
    /// trace records were emitted (pre-v0.7.2 instrumentation, or a
    /// run where every decision short-circuited before reaching a
    /// br_if site).
    #[serde(default)]
    pub bytes_used: u64,
    /// v0.9.8 — pages of trace memory the instrumented module
    /// allocates. Reflects what `WITNESS_TRACE_PAGES` was set to at
    /// `witness instrument` time. Default 16 (= 1 MiB).
    #[serde(default)]
    pub pages_allocated: u32,
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
///
/// Two schema versions are supported:
///
/// - `witness-harness-v1` (v0.6.0+) — counters only. MC/DC reconstruction
///   degrades to branch coverage in this mode.
/// - `witness-harness-v2` (v0.9.5+) — counters plus per-row snapshots
///   carrying brvals / brcnts / trace memory, mirroring exactly what
///   embedded wasmtime mode reads. Subprocess harnesses producing v2
///   data give full MC/DC truth tables.
///
/// The `rows` field is required for v2 and ignored for v1. Existing v1
/// harnesses keep working unchanged.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HarnessSnapshot {
    pub schema: String,
    pub counters: HashMap<String, u64>,

    /// v0.9.5+ — per-row snapshots for full MC/DC reconstruction.
    /// Required when `schema == "witness-harness-v2"`. Each entry is the
    /// captured state immediately after a row's invocation, with
    /// `__witness_trace_reset` + `__witness_row_reset` called before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<Vec<HarnessRow>>,
}

/// One row's worth of captured state in a v2 harness snapshot.
///
/// Maps directly onto what `run_via_embedded` reads after each
/// `--invoke` call — same field semantics, same encoding rules.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HarnessRow {
    /// Name of the export the harness invoked for this row.
    pub name: String,
    /// First i32 result of the export, interpreted as bool. `None` for
    /// void-returning exports.
    #[serde(default)]
    pub outcome: Option<i32>,
    /// `__witness_brval_<id>` global value at end-of-row.
    pub brvals: HashMap<String, u32>,
    /// `__witness_brcnt_<id>` global value at end-of-row.
    pub brcnts: HashMap<String, u32>,
    /// Base64-encoded snapshot of `__witness_trace` memory (with the
    /// 16-byte header). Empty string is allowed and means the harness
    /// chose not to ship the trace (acceptable for chain_kind=And/Or
    /// decisions where condition values alone derive the outcome, but
    /// breaks per-iteration MC/DC for inlined code).
    #[serde(default)]
    pub trace_b64: String,
}

impl HarnessSnapshot {
    /// v0.6.0 schema — counters only.
    pub const SCHEMA_V1: &'static str = "witness-harness-v1";
    /// v0.9.5 schema — counters plus per-row snapshots.
    pub const SCHEMA_V2: &'static str = "witness-harness-v2";

    /// Backwards-compat alias retained because the v0.6.0 docs and
    /// integration test reference `HarnessSnapshot::SCHEMA`. New code
    /// should use `SCHEMA_V1` or `SCHEMA_V2` explicitly.
    pub const SCHEMA: &'static str = Self::SCHEMA_V1;

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

    /// Borrow-friendly counter conversion (used by the v2 path which
    /// also wants to inspect `rows` afterwards).
    pub fn counters_as_id_map(&self) -> Result<HashMap<u32, u64>> {
        let mut out = HashMap::new();
        for (k, v) in &self.counters {
            let id = k.parse::<u32>().map_err(|_| {
                Error::Runtime(anyhow::anyhow!(
                    "harness snapshot contains non-numeric counter id `{k}`"
                ))
            })?;
            out.insert(id, *v);
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
        trace_parser_active: records.iter().any(|r| r.trace_health.trace_parser_active),
        // v0.9.8 — sum bytes-used across merged runs, but take the max
        // pages_allocated since that's a fixed-per-module value (any
        // disagreement here means the inputs were instrumented with
        // different WITNESS_TRACE_PAGES, which is technically a manifest
        // mismatch the merge validator should reject — for now we keep
        // the larger value, which is the safer-side reading).
        bytes_used: records.iter().map(|r| r.trace_health.bytes_used).sum(),
        pages_allocated: records
            .iter()
            .map(|r| r.trace_health.pages_allocated)
            .max()
            .unwrap_or(0),
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
