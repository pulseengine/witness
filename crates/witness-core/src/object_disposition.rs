//! v0.39 prep (#109, synth #396) — reconcile witness's WASM-level MC/DC
//! truth table against synth's **decision-provenance map** to produce an
//! object-code-traceable disposition per branch.
//!
//! witness measures MC/DC after the *first* lowering (source → WASM).
//! synth performs a *second* lowering (WASM → ARM/RISC-V) that changes the
//! branch set again — folding decisions to branchless predication,
//! eliminating provably-constant arms, or splitting one WASM decision into
//! several object branches. The DO-178C §6.4.4.2 / ISO 26262 source-to-object
//! obligation requires reconciling those. synth emits a provenance map keyed
//! by the WASM **(func_index, instruction_offset)** of the instrumented
//! branch; witness already carries that join key in every [`BranchEntry`]
//! (`function_index` + `byte_offset`). This module joins the two.
//!
//! The reconciler is witness-side by design — witness owns the truth table
//! and `witness-rivet-evidence-v1`. **scry** (sound abstract interpretation,
//! scry #51) supplies the dead-arm justification for the eliminated-constant
//! case; the map carries an opaque evidence reference, not the analysis.
//!
//! This is the consumer half, built and tested against synth-shaped fixtures
//! ahead of synth shipping #396 — the schema here IS the proposed contract.

use crate::instrument::BranchEntry;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Schema tag for the map synth emits (proposed contract).
pub const PROVENANCE_SCHEMA_V1: &str = "synth-provenance-v1";
/// Schema tag for the reconciled report witness emits.
pub const DISPOSITION_SCHEMA_V1: &str = "witness-object-disposition-v1";

/// The join key shared with synth: the WASM location of the instrumented
/// branch. Mirrors [`BranchEntry::function_index`] + [`BranchEntry::byte_offset`].
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct JoinKey {
    pub func_index: u32,
    pub instruction_offset: u32,
}

/// What synth's WASM→object lowering did to one WASM decision.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Disposition {
    /// 1:1 — the branch survives as a single object branch.
    Preserved,
    /// Case 1 — folded to branchless predication (`select`→`movCC`,
    /// compare-feeds-`br_if` fusion, if-conversion). The WASM-level
    /// obligation stays authoritative; nothing untraceable appears.
    FoldedPredication,
    /// Case 2 — a provably-constant arm was eliminated; MC/DC is infeasible
    /// by construction. Needs *justification, not coverage* — `scry_evidence`
    /// references scry's constant-condition / reachability proof.
    EliminatedConstant {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scry_evidence: Option<String>,
    },
    /// Case 3 — split into N object branches (`i64` compare → `cmp;bne;cmp`,
    /// `br_table` → comparison ladder). Synth *introduces* object branches;
    /// each needs object coverage or a faithful-implementation argument.
    SplitIntoObjectBranches { count: u32 },
}

/// synth's decision-provenance map (the proposed input contract).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SynthProvenanceMap {
    pub schema: String,
    pub entries: Vec<ProvenanceEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProvenanceEntry {
    pub func_index: u32,
    pub instruction_offset: u32,
    #[serde(flatten)]
    pub disposition: Disposition,
}

/// The object-traceable verdict for one witness branch.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "verdict", rename_all = "kebab-case")]
pub enum ObjectVerdict {
    /// Preserved or folded — keep the WASM-level MC/DC obligation as the
    /// authoritative one (no new object obligation).
    ObligationStands,
    /// Eliminated-constant — mark justified-infeasible (not a gap) with the
    /// scry evidence reference, if supplied.
    JustifiedInfeasible {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scry_evidence: Option<String>,
    },
    /// Split — flag for object coverage: `object_branches` new branches that
    /// the WASM decision's vectors must cover (or argue faithful).
    NeedsObjectCoverage { object_branches: u32 },
    /// The branch has no entry in synth's map (and no byte_offset to join on,
    /// or synth didn't report it) — provenance unknown, surfaced not hidden.
    NoProvenance,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BranchDisposition {
    pub branch_id: u32,
    pub func_index: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruction_offset: Option<u32>,
    #[serde(flatten)]
    pub verdict: ObjectVerdict,
}

/// The reconciled report: one disposition per witness branch, plus any synth
/// entries that don't join to a witness branch (a divergence — synth reports
/// an object branch witness never instrumented).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ObjectDispositionReport {
    pub schema: String,
    pub branches: Vec<BranchDisposition>,
    pub only_in_synth: Vec<JoinKey>,
}

impl ObjectDispositionReport {
    /// Count of branches whose object obligation needs separate coverage
    /// (the split case) — the actionable, non-hidden risk surface.
    pub fn needs_object_coverage(&self) -> usize {
        self.branches
            .iter()
            .filter(|b| matches!(b.verdict, ObjectVerdict::NeedsObjectCoverage { .. }))
            .count()
    }

    /// Human-readable summary. Surfaces every split (new object obligation)
    /// and every divergence — a source-to-object report that hides where the
    /// object branch set diverges is worse than useless.
    pub fn to_text(&self) -> String {
        let count =
            |f: fn(&ObjectVerdict) -> bool| self.branches.iter().filter(|b| f(&b.verdict)).count();
        let stands = count(|v| matches!(v, ObjectVerdict::ObligationStands));
        let infeasible = count(|v| matches!(v, ObjectVerdict::JustifiedInfeasible { .. }));
        let no_prov = count(|v| matches!(v, ObjectVerdict::NoProvenance));
        let mut out = format!(
            "object-disposition: {} branches — {stands} obligation-stands, {infeasible} \
             justified-infeasible, {} needs-object-coverage, {no_prov} no-provenance; \
             {} only-in-synth\n",
            self.branches.len(),
            self.needs_object_coverage(),
            self.only_in_synth.len(),
        );
        for b in &self.branches {
            if let ObjectVerdict::NeedsObjectCoverage { object_branches } = b.verdict {
                out.push_str(&format!(
                    "  SPLIT branch {} (func {}, off {:?}): {object_branches} object branches need coverage\n",
                    b.branch_id, b.func_index, b.instruction_offset
                ));
            }
        }
        for k in &self.only_in_synth {
            out.push_str(&format!(
                "  DIVERGENCE only-in-synth: func {} off {} (object branch witness never instrumented)\n",
                k.func_index, k.instruction_offset
            ));
        }
        out
    }
}

/// Reconcile witness's manifest branches against synth's provenance map by
/// the shared `(func_index, instruction_offset)` join key. Branches with no
/// `byte_offset` (synthetic / no source map) or no matching synth entry get
/// `NoProvenance`; synth entries with no matching branch become `only_in_synth`.
pub fn reconcile(branches: &[BranchEntry], map: &SynthProvenanceMap) -> ObjectDispositionReport {
    let by_key: BTreeMap<JoinKey, &Disposition> = map
        .entries
        .iter()
        .map(|e| {
            (
                JoinKey {
                    func_index: e.func_index,
                    instruction_offset: e.instruction_offset,
                },
                &e.disposition,
            )
        })
        .collect();

    let mut matched_keys: std::collections::BTreeSet<JoinKey> = std::collections::BTreeSet::new();
    let mut out = Vec::with_capacity(branches.len());
    for b in branches {
        let key = b.byte_offset.map(|off| JoinKey {
            func_index: b.function_index,
            instruction_offset: off,
        });
        let verdict = match key.and_then(|k| by_key.get(&k).map(|d| (k, *d))) {
            Some((k, disp)) => {
                matched_keys.insert(k);
                match disp {
                    Disposition::Preserved | Disposition::FoldedPredication => {
                        ObjectVerdict::ObligationStands
                    }
                    Disposition::EliminatedConstant { scry_evidence } => {
                        ObjectVerdict::JustifiedInfeasible {
                            scry_evidence: scry_evidence.clone(),
                        }
                    }
                    Disposition::SplitIntoObjectBranches { count } => {
                        ObjectVerdict::NeedsObjectCoverage {
                            object_branches: *count,
                        }
                    }
                }
            }
            None => ObjectVerdict::NoProvenance,
        };
        out.push(BranchDisposition {
            branch_id: b.id,
            func_index: b.function_index,
            instruction_offset: b.byte_offset,
            verdict,
        });
    }

    let only_in_synth: Vec<JoinKey> = by_key
        .keys()
        .filter(|k| !matched_keys.contains(*k))
        .copied()
        .collect();

    ObjectDispositionReport {
        schema: DISPOSITION_SCHEMA_V1.to_string(),
        branches: out,
        only_in_synth,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::instrument::BranchKind;

    fn branch(id: u32, func: u32, off: Option<u32>) -> BranchEntry {
        BranchEntry {
            id,
            function_index: func,
            function_name: None,
            function_display: None,
            kind: BranchKind::BrIf,
            instr_index: id,
            target_index: None,
            byte_offset: off,
            seq_debug: String::new(),
        }
    }

    fn entry(func: u32, off: u32, disposition: Disposition) -> ProvenanceEntry {
        ProvenanceEntry {
            func_index: func,
            instruction_offset: off,
            disposition,
        }
    }

    #[test]
    fn maps_each_disposition_to_its_verdict() {
        let branches = vec![
            branch(0, 0, Some(10)),
            branch(1, 0, Some(20)),
            branch(2, 0, Some(30)),
            branch(3, 0, Some(40)),
        ];
        let map = SynthProvenanceMap {
            schema: PROVENANCE_SCHEMA_V1.to_string(),
            entries: vec![
                entry(0, 10, Disposition::Preserved),
                entry(0, 20, Disposition::FoldedPredication),
                entry(
                    0,
                    30,
                    Disposition::EliminatedConstant {
                        scry_evidence: Some("scry://const/x".to_string()),
                    },
                ),
                entry(0, 40, Disposition::SplitIntoObjectBranches { count: 3 }),
            ],
        };
        let r = reconcile(&branches, &map);
        assert_eq!(r.branches[0].verdict, ObjectVerdict::ObligationStands);
        assert_eq!(r.branches[1].verdict, ObjectVerdict::ObligationStands);
        assert_eq!(
            r.branches[2].verdict,
            ObjectVerdict::JustifiedInfeasible {
                scry_evidence: Some("scry://const/x".to_string())
            }
        );
        assert_eq!(
            r.branches[3].verdict,
            ObjectVerdict::NeedsObjectCoverage { object_branches: 3 }
        );
        assert_eq!(r.needs_object_coverage(), 1);
        assert!(r.only_in_synth.is_empty());
        assert!(r.to_text().contains("SPLIT branch 3"));
    }

    #[test]
    fn unmatched_branch_is_no_provenance_and_synth_extra_diverges() {
        // branch with no byte_offset can't join; a synth entry with no branch.
        let branches = vec![branch(0, 0, None), branch(1, 0, Some(10))];
        let map = SynthProvenanceMap {
            schema: PROVENANCE_SCHEMA_V1.to_string(),
            entries: vec![
                entry(0, 10, Disposition::Preserved),
                entry(0, 99, Disposition::Preserved), // no witness branch here
            ],
        };
        let r = reconcile(&branches, &map);
        assert_eq!(r.branches[0].verdict, ObjectVerdict::NoProvenance);
        assert_eq!(r.branches[1].verdict, ObjectVerdict::ObligationStands);
        assert_eq!(
            r.only_in_synth,
            vec![JoinKey {
                func_index: 0,
                instruction_offset: 99
            }]
        );
        assert!(r.to_text().contains("DIVERGENCE"));
    }

    #[test]
    fn map_round_trips_via_json() {
        let map = SynthProvenanceMap {
            schema: PROVENANCE_SCHEMA_V1.to_string(),
            entries: vec![
                entry(1, 5, Disposition::FoldedPredication),
                entry(1, 9, Disposition::SplitIntoObjectBranches { count: 2 }),
            ],
        };
        let json = serde_json::to_string(&map).unwrap();
        let back: SynthProvenanceMap = serde_json::from_str(&json).unwrap();
        assert_eq!(back.entries.len(), 2);
        assert_eq!(
            back.entries[1].disposition,
            Disposition::SplitIntoObjectBranches { count: 2 }
        );
    }
}
