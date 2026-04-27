//! Rivet-shape coverage evidence emission.
//!
//! Witness emits coverage evidence in the YAML schema agreed in
//! `docs/research/rivet-evidence-consumer.md` so rivet's `CoverageStore`
//! consumer (landing in the rivet upstream PR coordinated with this
//! v0.3 release) ingests witness output without translation.
//!
//! The on-disk schema mirrors rivet's existing `ResultStore` shape so
//! the consumer code in rivet is a near-drop-in copy of the
//! `ResultStore::load_dir` pattern with `TestResult` swapped for
//! `CoverageEvidence`.
//!
//! # Schema URL
//!
//! [`SCHEMA`] — `https://pulseengine.eu/witness-rivet-evidence/v1`. Top-
//! level wrapper has a `schema:` field for self-identification; rivet
//! rejects unknown schema URLs with a clear error.

use crate::run_record::{BranchHit, RunRecord};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Schema URL embedded at the top of every witness-rivet-evidence file.
pub const SCHEMA: &str = "https://pulseengine.eu/witness-rivet-evidence/v1";
/// Schema version inside the witness-rivet-evidence/v1 namespace. Bump
/// on breaking changes to the *contents* (not the type URL).
pub const VERSION: &str = "1.0";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EvidenceFile {
    pub schema: String,
    pub version: String,
    pub witness_version: String,
    pub run: RunMetadata,
    pub module: ModuleRef,
    pub evidence: Vec<CoverageEvidence>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RunMetadata {
    pub id: String,
    pub timestamp: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModuleRef {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<crate::predicate::Digests>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CoverageEvidence {
    pub artifact: String,
    pub coverage_type: CoverageType,
    pub total: u64,
    pub covered: u64,
    pub percentage: f64,
    pub hits: Vec<u64>,
    pub uncovered_branch_ids: Vec<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CoverageType {
    /// Per-`br_if` / per-arm / per-`br_table`-target — what witness v0.1
    /// + v0.2 emits today.
    Branch,
    /// MC/DC condition decomposition — populated when v0.2.1's DWARF
    /// reconstruction has produced `Decision`s. Reserved.
    Mcdc,
    /// Source-line coverage — out of scope for witness; reserved so the
    /// schema can absorb wasmcov-style projections later.
    Line,
}

/// Mapping of branch ids to rivet artefact ids. Constructed from a
/// user-supplied YAML file, e.g.:
///
/// ```yaml
/// mappings:
///   - branches: [0, 1, 2, 3]
///     artifact: "REQ-001"
///   - branches: [4, 5, 6]
///     artifact: "REQ-002"
/// ```
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RequirementMap {
    pub mappings: Vec<MapEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MapEntry {
    pub branches: Vec<u32>,
    pub artifact: String,
}

impl RequirementMap {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(Error::Io)?;
        // v0.9.4 — was Error::Runtime("wasm runtime error: ..."), which
        // misled tester-reviewers into thinking a wasmtime trap had
        // occurred. Now reports as a config/schema problem.
        serde_yaml::from_str(&text).map_err(|e| Error::RequirementMap {
            path: path.to_path_buf(),
            source: anyhow::Error::new(e),
        })
    }

    /// Build a flat (branch_id → artifact_id) map. Errors if a branch
    /// id appears in more than one mapping (would attribute the same
    /// hit to two requirements).
    pub fn flatten(&self) -> Result<BTreeMap<u32, String>> {
        let mut out: BTreeMap<u32, String> = BTreeMap::new();
        for entry in &self.mappings {
            for &b in &entry.branches {
                if let Some(existing) = out.insert(b, entry.artifact.clone()) {
                    // Schema-level conflict, not a runtime trap. Use a
                    // synthesised PathBuf to flag the offending mapping
                    // since we don't have the source path here.
                    return Err(Error::RequirementMap {
                        path: PathBuf::from("<requirement-map>"),
                        source: anyhow::anyhow!(
                            "branch id {b} mapped to both `{existing}` and `{}`",
                            entry.artifact
                        ),
                    });
                }
            }
        }
        Ok(out)
    }
}

/// Build an `EvidenceFile` from a `RunRecord` and a branch→artefact
/// mapping. Branches not listed in the mapping are dropped silently;
/// branches in the mapping that don't appear in the run produce an
/// error (the mapping refers to a branch witness didn't measure).
pub fn build_evidence(
    record: &RunRecord,
    map: &BTreeMap<u32, String>,
    source_label: &str,
    environment: Option<&str>,
    commit: Option<&str>,
) -> Result<EvidenceFile> {
    let by_id: BTreeMap<u32, &BranchHit> = record.branches.iter().map(|b| (b.id, b)).collect();

    for &id in map.keys() {
        if !by_id.contains_key(&id) {
            return Err(Error::Runtime(anyhow::anyhow!(
                "requirement map references branch id {id} not present in run"
            )));
        }
    }

    // Group branch ids by artifact, preserving the input map's grouping
    // rather than the run record's order.
    let mut groups: BTreeMap<String, Vec<&BranchHit>> = BTreeMap::new();
    for (id, art) in map {
        if let Some(hit) = by_id.get(id) {
            groups.entry(art.clone()).or_default().push(*hit);
        }
    }

    let evidence: Vec<CoverageEvidence> = groups
        .into_iter()
        .map(|(artifact, hits)| {
            let total = u64::try_from(hits.len()).unwrap_or(u64::MAX);
            let covered =
                u64::try_from(hits.iter().filter(|h| h.hits > 0).count()).unwrap_or(u64::MAX);
            // SAFETY-REVIEW: u64 → f64 is lossy only for counters above
            // 2^53; coverage hit counts in real test runs do not approach
            // that. Lossy precision on huge counts is acceptable for a
            // percentage value.
            #[allow(clippy::as_conversions, clippy::cast_precision_loss)]
            let percentage = if total == 0 {
                100.0
            } else {
                let covered_f = covered as f64;
                let total_f = total as f64;
                (covered_f / total_f) * 100.0
            };
            let hit_counts: Vec<u64> = hits.iter().map(|h| h.hits).collect();
            let mut uncovered: Vec<u32> =
                hits.iter().filter(|h| h.hits == 0).map(|h| h.id).collect();
            uncovered.sort_unstable();
            CoverageEvidence {
                artifact,
                coverage_type: CoverageType::Branch,
                total,
                covered,
                percentage,
                hits: hit_counts,
                uncovered_branch_ids: uncovered,
            }
        })
        .collect();

    let timestamp = crate::predicate::now_rfc3339();
    Ok(EvidenceFile {
        schema: SCHEMA.to_string(),
        version: VERSION.to_string(),
        witness_version: env!("CARGO_PKG_VERSION").to_string(),
        run: RunMetadata {
            id: format!("witness-{}", timestamp),
            timestamp,
            source: source_label.to_string(),
            environment: environment.map(str::to_string),
            commit: commit.map(str::to_string),
        },
        module: ModuleRef {
            path: record.module_path.clone(),
            digest: None,
        },
        evidence,
    })
}

/// Save an `EvidenceFile` to disk as YAML.
pub fn save_evidence(file: &EvidenceFile, path: &Path) -> Result<()> {
    let yaml = serde_yaml::to_string(file)
        .map_err(|e| Error::Runtime(anyhow::anyhow!("failed to serialise rivet evidence: {e}")))?;
    std::fs::write(path, yaml).map_err(Error::Io)
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

    fn record_with_hits(hits: &[u64]) -> RunRecord {
        let branches: Vec<BranchHit> = hits
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
            module_path: "app.wasm".to_string(),
            invoked: vec!["f".to_string()],
            branches,
            decisions: vec![],
            trace_health: Default::default(),
        }
    }

    #[test]
    fn build_evidence_groups_by_artifact() {
        let record = record_with_hits(&[1, 0, 3, 0]);
        let map = RequirementMap {
            mappings: vec![
                MapEntry {
                    branches: vec![0, 1],
                    artifact: "REQ-A".to_string(),
                },
                MapEntry {
                    branches: vec![2, 3],
                    artifact: "REQ-B".to_string(),
                },
            ],
        };
        let flat = map.flatten().unwrap();
        let ev = build_evidence(&record, &flat, "test", None, None).unwrap();
        assert_eq!(ev.evidence.len(), 2);
        let a = ev.evidence.iter().find(|e| e.artifact == "REQ-A").unwrap();
        assert_eq!(a.total, 2);
        assert_eq!(a.covered, 1);
        assert_eq!(a.uncovered_branch_ids, vec![1]);
        let b = ev.evidence.iter().find(|e| e.artifact == "REQ-B").unwrap();
        assert_eq!(b.total, 2);
        assert_eq!(b.covered, 1);
    }

    #[test]
    fn build_evidence_rejects_unknown_branch_in_map() {
        let record = record_with_hits(&[1]);
        let map = RequirementMap {
            mappings: vec![MapEntry {
                branches: vec![0, 99],
                artifact: "REQ-X".to_string(),
            }],
        };
        let flat = map.flatten().unwrap();
        let result = build_evidence(&record, &flat, "test", None, None);
        assert!(matches!(result, Err(Error::Runtime(_))));
    }

    #[test]
    fn flatten_rejects_duplicate_branch() {
        let map = RequirementMap {
            mappings: vec![
                MapEntry {
                    branches: vec![0, 1],
                    artifact: "REQ-A".to_string(),
                },
                MapEntry {
                    branches: vec![1, 2],
                    artifact: "REQ-B".to_string(),
                },
            ],
        };
        assert!(map.flatten().is_err());
    }

    #[test]
    fn evidence_file_round_trips_via_yaml() {
        let record = record_with_hits(&[2, 0]);
        let map = RequirementMap {
            mappings: vec![MapEntry {
                branches: vec![0, 1],
                artifact: "REQ-1".to_string(),
            }],
        };
        let flat = map.flatten().unwrap();
        let file = build_evidence(&record, &flat, "test", Some("ci"), Some("abcdef")).unwrap();
        let yaml = serde_yaml::to_string(&file).unwrap();
        let parsed: EvidenceFile = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed.schema, SCHEMA);
        assert_eq!(parsed.evidence.len(), 1);
        assert_eq!(parsed.evidence[0].covered, 1);
        assert_eq!(parsed.run.environment.as_deref(), Some("ci"));
    }
}
