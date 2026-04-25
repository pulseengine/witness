//! Branch-set / coverage delta between two manifests or run records.
//!
//! Used by the `witness diff` subcommand and the `witness-delta.yml` PR
//! workflow (see `docs/research/v04-ci-ports.md`). Inputs can be either
//! pair-of-manifests or pair-of-runs; pair-of-manifests reports
//! structural delta only (no coverage percentage), pair-of-runs reports
//! both structural and coverage-percentage delta.

use crate::instrument::{BranchEntry, BranchKind, Manifest};
use crate::run_record::RunRecord;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Schema URL embedded at the top of every delta document.
pub const SCHEMA: &str = "https://pulseengine.eu/witness-delta/v1";

/// Result of comparing two snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    pub schema: String,
    pub witness_version: String,
    pub base: SnapshotMeta,
    pub head: SnapshotMeta,
    pub added_branches: Vec<BranchEntry>,
    pub removed_branches: Vec<BranchEntry>,
    pub changed_branches: Vec<ChangedBranch>,
    /// Populated when both inputs are runs; null for manifest-only inputs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage: Option<CoverageDelta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub source_path: String,
    pub kind: SnapshotKind,
    pub branch_count: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotKind {
    Manifest,
    Run,
}

/// A branch that exists in both inputs but with different fields. We
/// match by `id`; differences in `kind` / `function_index` /
/// `instr_index` typically indicate the underlying module changed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedBranch {
    pub id: u32,
    pub base: BranchSummary,
    pub head: BranchSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchSummary {
    pub function_index: u32,
    pub kind: BranchKind,
    pub instr_index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hits: Option<u64>,
}

/// Coverage-percentage delta. Computed only when both inputs are runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageDelta {
    pub base_total: u32,
    pub base_covered: u32,
    pub base_pct: f64,
    pub head_total: u32,
    pub head_covered: u32,
    pub head_pct: f64,
    pub delta_pct: f64,
}

/// Load either a `Manifest` or a `RunRecord` based on schema sniffing —
/// callers don't need to commit upfront which input shape they have.
fn load_snapshot(path: &Path) -> Result<Snapshot> {
    let bytes = std::fs::read(path).map_err(Error::Io)?;
    // Try RunRecord first (it's narrower) then fall back to Manifest.
    if let Ok(record) = serde_json::from_slice::<RunRecord>(&bytes) {
        // RunRecord has `branches: Vec<BranchHit>` — distinguish from
        // Manifest's `branches: Vec<BranchEntry>` by checking for the
        // `hits` field implicitly via successful `RunRecord` parse.
        return Ok(Snapshot::Run {
            path: path.to_string_lossy().into_owned(),
            record,
        });
    }
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|source| Error::Manifest {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(Snapshot::Manifest {
        path: path.to_string_lossy().into_owned(),
        manifest,
    })
}

#[derive(Debug)]
enum Snapshot {
    Manifest { path: String, manifest: Manifest },
    Run { path: String, record: RunRecord },
}

impl Snapshot {
    fn meta(&self) -> SnapshotMeta {
        match self {
            Self::Manifest { path, manifest } => SnapshotMeta {
                source_path: path.clone(),
                kind: SnapshotKind::Manifest,
                branch_count: u32::try_from(manifest.branches.len()).unwrap_or(u32::MAX),
            },
            Self::Run { path, record } => SnapshotMeta {
                source_path: path.clone(),
                kind: SnapshotKind::Run,
                branch_count: u32::try_from(record.branches.len()).unwrap_or(u32::MAX),
            },
        }
    }

    fn entries(&self) -> Vec<BranchEntry> {
        match self {
            Self::Manifest { manifest, .. } => manifest.branches.clone(),
            Self::Run { record, .. } => record
                .branches
                .iter()
                .map(|b| BranchEntry {
                    id: b.id,
                    function_index: b.function_index,
                    function_name: b.function_name.clone(),
                    kind: b.kind,
                    instr_index: b.instr_index,
                    target_index: None,
                    byte_offset: None,
                    seq_debug: String::new(),
                })
                .collect(),
        }
    }

    fn hits_for(&self, id: u32) -> Option<u64> {
        match self {
            Self::Manifest { .. } => None,
            Self::Run { record, .. } => record.branches.iter().find(|b| b.id == id).map(|b| b.hits),
        }
    }

    fn coverage(&self) -> Option<(u32, u32)> {
        match self {
            Self::Manifest { .. } => None,
            Self::Run { record, .. } => {
                let total = u32::try_from(record.branches.len()).unwrap_or(u32::MAX);
                let covered = u32::try_from(record.branches.iter().filter(|b| b.hits > 0).count())
                    .unwrap_or(u32::MAX);
                Some((total, covered))
            }
        }
    }
}

/// Compare two snapshots and produce a `Delta`.
pub fn diff(base: &Path, head: &Path) -> Result<Delta> {
    let base_snap = load_snapshot(base)?;
    let head_snap = load_snapshot(head)?;

    let base_entries = base_snap.entries();
    let head_entries = head_snap.entries();

    let base_by_id: BTreeMap<u32, &BranchEntry> = base_entries.iter().map(|e| (e.id, e)).collect();
    let head_by_id: BTreeMap<u32, &BranchEntry> = head_entries.iter().map(|e| (e.id, e)).collect();

    let mut added_branches: Vec<BranchEntry> = head_entries
        .iter()
        .filter(|e| !base_by_id.contains_key(&e.id))
        .cloned()
        .collect();
    added_branches.sort_by_key(|e| e.id);

    let mut removed_branches: Vec<BranchEntry> = base_entries
        .iter()
        .filter(|e| !head_by_id.contains_key(&e.id))
        .cloned()
        .collect();
    removed_branches.sort_by_key(|e| e.id);

    let mut changed_branches: Vec<ChangedBranch> = Vec::new();
    for (&id, &base_e) in &base_by_id {
        let Some(&head_e) = head_by_id.get(&id) else {
            continue;
        };
        let same_shape = base_e.function_index == head_e.function_index
            && base_e.kind == head_e.kind
            && base_e.instr_index == head_e.instr_index;
        let base_hits = base_snap.hits_for(id);
        let head_hits = head_snap.hits_for(id);
        let hits_changed = base_hits != head_hits;
        if !same_shape || hits_changed {
            changed_branches.push(ChangedBranch {
                id,
                base: BranchSummary {
                    function_index: base_e.function_index,
                    kind: base_e.kind,
                    instr_index: base_e.instr_index,
                    hits: base_hits,
                },
                head: BranchSummary {
                    function_index: head_e.function_index,
                    kind: head_e.kind,
                    instr_index: head_e.instr_index,
                    hits: head_hits,
                },
            });
        }
    }
    changed_branches.sort_by_key(|c| c.id);

    let coverage = match (base_snap.coverage(), head_snap.coverage()) {
        (Some((bt, bc)), Some((ht, hc))) => {
            // SAFETY-REVIEW: u32 → f64 is exact for any value below 2^24,
            // and lossy only above 2^53. Real branch counts stay well
            // below 2^24.
            #[allow(clippy::as_conversions, clippy::cast_precision_loss)]
            let pct = |c: u32, t: u32| -> f64 {
                if t == 0 {
                    100.0
                } else {
                    (c as f64 / t as f64) * 100.0
                }
            };
            let bp = pct(bc, bt);
            let hp = pct(hc, ht);
            Some(CoverageDelta {
                base_total: bt,
                base_covered: bc,
                base_pct: bp,
                head_total: ht,
                head_covered: hc,
                head_pct: hp,
                delta_pct: hp - bp,
            })
        }
        _ => None,
    };

    Ok(Delta {
        schema: SCHEMA.to_string(),
        witness_version: env!("CARGO_PKG_VERSION").to_string(),
        base: base_snap.meta(),
        head: head_snap.meta(),
        added_branches,
        removed_branches,
        changed_branches,
        coverage,
    })
}

/// Convenience: diff and write to disk as JSON.
pub fn diff_to_file(base: &Path, head: &Path, output: &Path) -> Result<()> {
    let delta = diff(base, head)?;
    let json = serde_json::to_string_pretty(&delta).map_err(Error::Serde)?;
    std::fs::write(output, json).map_err(Error::Io)
}

/// Render the Delta as a human-readable text summary (used by
/// `witness diff --format text` and the PR-comment body).
pub fn delta_to_text(delta: &Delta) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "witness delta {}\n  base: {} ({}, {} branches)\n  head: {} ({}, {} branches)\n",
        delta.witness_version,
        delta.base.source_path,
        match delta.base.kind {
            SnapshotKind::Manifest => "manifest",
            SnapshotKind::Run => "run",
        },
        delta.base.branch_count,
        delta.head.source_path,
        match delta.head.kind {
            SnapshotKind::Manifest => "manifest",
            SnapshotKind::Run => "run",
        },
        delta.head.branch_count,
    ));
    if let Some(cov) = &delta.coverage {
        out.push_str(&format!(
            "\ncoverage: {:.1}% → {:.1}% ({:+.1} pp)\n",
            cov.base_pct, cov.head_pct, cov.delta_pct
        ));
    }
    out.push_str(&format!(
        "\nadded:   {} branch{}\nremoved: {} branch{}\nchanged: {} branch{}\n",
        delta.added_branches.len(),
        if delta.added_branches.len() == 1 {
            ""
        } else {
            "es"
        },
        delta.removed_branches.len(),
        if delta.removed_branches.len() == 1 {
            ""
        } else {
            "es"
        },
        delta.changed_branches.len(),
        if delta.changed_branches.len() == 1 {
            ""
        } else {
            "es"
        },
    ));
    out
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
    use crate::instrument::{BranchEntry, BranchKind, Manifest};
    use crate::run_record::{BranchHit, RunRecord};
    use tempfile::tempdir;

    fn make_branch(id: u32, kind: BranchKind, instr_index: u32) -> BranchEntry {
        BranchEntry {
            id,
            function_index: 0,
            function_name: None,
            kind,
            instr_index,
            target_index: None,
            byte_offset: None,
            seq_debug: format!("Id {{ idx: {id} }}"),
        }
    }

    fn write_manifest(path: &Path, branches: Vec<BranchEntry>) {
        let m = Manifest {
            schema_version: "2".to_string(),
            witness_version: "test".to_string(),
            module_source: "x.wasm".to_string(),
            branches,
            decisions: vec![],
        };
        std::fs::write(path, serde_json::to_string(&m).unwrap()).unwrap();
    }

    fn write_run(path: &Path, hits: &[(u32, u64)]) {
        let branches: Vec<BranchHit> = hits
            .iter()
            .map(|&(id, h)| BranchHit {
                id,
                function_index: 0,
                function_name: None,
                kind: BranchKind::BrIf,
                instr_index: id,
                hits: h,
            })
            .collect();
        let r = RunRecord {
            schema_version: "2".to_string(),
            witness_version: "test".to_string(),
            module_path: "x.wasm".to_string(),
            invoked: vec![],
            branches,
            decisions: vec![],
            trace_health: Default::default(),
        };
        std::fs::write(path, serde_json::to_string(&r).unwrap()).unwrap();
    }

    #[test]
    fn manifest_diff_added_removed_changed() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base.json");
        let head = dir.path().join("head.json");
        write_manifest(
            &base,
            vec![
                make_branch(0, BranchKind::BrIf, 5),
                make_branch(1, BranchKind::IfThen, 10),
                make_branch(2, BranchKind::BrIf, 15),
            ],
        );
        write_manifest(
            &head,
            vec![
                make_branch(0, BranchKind::BrIf, 5),
                make_branch(1, BranchKind::IfThen, 11), // changed instr_index
                make_branch(3, BranchKind::BrTableTarget, 20), // added
            ],
        );
        let d = diff(&base, &head).unwrap();
        assert_eq!(d.added_branches.len(), 1);
        assert_eq!(d.added_branches[0].id, 3);
        assert_eq!(d.removed_branches.len(), 1);
        assert_eq!(d.removed_branches[0].id, 2);
        assert_eq!(d.changed_branches.len(), 1);
        assert_eq!(d.changed_branches[0].id, 1);
        assert!(
            d.coverage.is_none(),
            "manifest-only inputs → no coverage delta"
        );
    }

    #[test]
    fn run_diff_produces_coverage_delta() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base.json");
        let head = dir.path().join("head.json");
        write_run(&base, &[(0, 1), (1, 0), (2, 1)]); // 2/3 covered
        write_run(&head, &[(0, 1), (1, 1), (2, 1)]); // 3/3 covered
        let d = diff(&base, &head).unwrap();
        let cov = d.coverage.expect("two runs → coverage delta");
        assert_eq!(cov.base_total, 3);
        assert_eq!(cov.base_covered, 2);
        assert_eq!(cov.head_covered, 3);
        assert!((cov.delta_pct - (100.0 - 200.0 / 3.0)).abs() < 1e-9);
        // Branch 1's hit count went from 0 to 1 — that's a change.
        assert!(d.changed_branches.iter().any(|c| c.id == 1));
    }

    #[test]
    fn delta_to_text_renders_summary() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("base.json");
        let head = dir.path().join("head.json");
        write_run(&base, &[(0, 1), (1, 0)]);
        write_run(&head, &[(0, 1), (1, 1)]);
        let d = diff(&base, &head).unwrap();
        let s = delta_to_text(&d);
        assert!(s.contains("witness delta"));
        assert!(s.contains("coverage:"));
        assert!(s.contains("added:"));
    }
}
