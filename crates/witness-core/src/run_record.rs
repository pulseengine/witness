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
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Raw-run output: each branch paired with the counter's final value.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RunRecord {
    pub schema_version: String,
    pub witness_version: String,
    pub module_path: String,
    pub invoked: Vec<String>,
    pub branches: Vec<BranchHit>,
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

    Ok(RunRecord {
        schema_version: head.schema_version.clone(),
        witness_version: env!("CARGO_PKG_VERSION").to_string(),
        module_path: head.module_path.clone(),
        invoked,
        branches: merged_branches,
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
