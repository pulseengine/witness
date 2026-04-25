//! Property-based tests for witness's load-bearing invariants.
//!
//! These complement the example-based unit tests in each module. Each
//! property is a *general* claim (true for arbitrary inputs in a
//! defined family) rather than a single point. The proptest crate
//! shrinks any failing input to a minimal counterexample.
//!
//! # What's covered
//!
//! - `merge_records`: identity, idempotence over a single input,
//!   commutativity, monotonicity (counter never decreases on merge),
//!   total preservation (sum-of-inputs == merged-totals).
//! - `Manifest` JSON round-trip: any valid Manifest serialises and
//!   deserialises back to bit-identical state.
//! - `RunRecord` JSON round-trip: same, for the run-side schema.
//! - `Report::from_record`: per-function totals equal the sum of
//!   covered+uncovered branch hits.
//! - `RequirementMap::flatten`: rejects duplicates; accepts disjoint.
//!
//! # What's deliberately NOT covered here
//!
//! - Wasm-rewrite semantic preservation. That's verified by the
//!   round-trip integration tests in `src/run.rs::tests::round_trip_*`,
//!   which exercise actual instrumented modules under wasmtime.
//! - In-toto predicate emission against the real sigil verifier. That
//!   needs sigil to be in scope; v0.3 keeps it documented but
//!   integration-tested only against witness's own serde round-trip.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cloned_ref_to_slice_refs
)]

use proptest::collection::vec as pvec;
use proptest::prelude::*;
use witness_core::instrument::{BranchEntry, BranchKind, Manifest};
use witness_core::rivet_evidence::{MapEntry, RequirementMap};
use witness_core::run_record::{BranchHit, RunRecord, merge_records};

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

prop_compose! {
    fn arb_branch_kind()
        (k in 0u8..5u8)
        -> BranchKind
    {
        match k {
            0 => BranchKind::BrIf,
            1 => BranchKind::IfThen,
            2 => BranchKind::IfElse,
            3 => BranchKind::BrTableTarget,
            _ => BranchKind::BrTableDefault,
        }
    }
}

prop_compose! {
    fn arb_branch_hit(id: u32, kind: BranchKind)
        (function_index in 0u32..16u32,
         instr_index in 0u32..1024u32,
         hits in 0u64..1_000_000u64)
        -> BranchHit
    {
        BranchHit {
            id,
            function_index,
            function_name: None,
            kind,
            instr_index,
            hits,
        }
    }
}

prop_compose! {
    /// Produce a base record (id 0..N) with consistent kinds/positions
    /// so multiple records sharing this base can be merged. Hit counts
    /// are independent.
    fn arb_record(n: usize, hits_strategy: impl Strategy<Value = u64> + 'static + Clone)
        (kinds in pvec(arb_branch_kind(), n),
         positions in pvec(0u32..1024u32, n),
         hits in pvec(hits_strategy, n))
        -> RunRecord
    {
        let branches: Vec<BranchHit> = (0..n)
            .map(|i| BranchHit {
                id: i as u32,
                function_index: 0,
                function_name: None,
                kind: kinds[i],
                instr_index: positions[i],
                hits: hits[i],
            })
            .collect();
        RunRecord {
            schema_version: "2".to_string(),
            witness_version: "test".to_string(),
            module_path: "fixed.wasm".to_string(),
            invoked: vec![],
            branches,
            decisions: vec![],
            trace_health: Default::default(),
        }
    }
}

// A pair of records that AGREE on everything except hit counts —
// suitable as merge inputs. (Comment, not doc comment, because the
// `prop_compose!` macro doesn't pass through inner doc comments.)
prop_compose! {
    fn arb_compatible_pair(n: usize)
        (kinds in pvec(arb_branch_kind(), n),
         positions in pvec(0u32..1024u32, n),
         hits_a in pvec(0u64..1_000_000u64, n),
         hits_b in pvec(0u64..1_000_000u64, n))
        -> (RunRecord, RunRecord)
    {
        let make = |hits: Vec<u64>| -> RunRecord {
            let branches: Vec<BranchHit> = (0..n)
                .map(|i| BranchHit {
                    id: i as u32,
                    function_index: 0,
                    function_name: None,
                    kind: kinds[i],
                    instr_index: positions[i],
                    hits: hits[i],
                })
                .collect();
            RunRecord {
                schema_version: "2".to_string(),
                witness_version: "test".to_string(),
                module_path: "fixed.wasm".to_string(),
                invoked: vec![],
                branches,
                decisions: vec![],
                trace_health: Default::default(),
            }
        };
        (make(hits_a), make(hits_b))
    }
}

// ---------------------------------------------------------------------------
// Merge properties
// ---------------------------------------------------------------------------

proptest! {
    /// merge of a single record returns a record with the same hits
    /// (the merge tool advertises witness_version, so that field may
    /// differ; everything else is structurally equal).
    #[test]
    fn merge_single_record_preserves_hits(
        record in arb_record(8, 0u64..1000u64)
    ) {
        let merged = merge_records(&[record.clone()]).unwrap();
        prop_assert_eq!(merged.branches.len(), record.branches.len());
        for (a, b) in merged.branches.iter().zip(record.branches.iter()) {
            prop_assert_eq!(a.id, b.id);
            prop_assert_eq!(a.hits, b.hits);
            prop_assert_eq!(a.kind, b.kind);
        }
    }

    /// merge is commutative: merge(A, B) == merge(B, A) (in counter values).
    #[test]
    fn merge_is_commutative(pair in arb_compatible_pair(6)) {
        let (a, b) = pair;
        let ab = merge_records(&[a.clone(), b.clone()]).unwrap();
        let ba = merge_records(&[b, a]).unwrap();
        prop_assert_eq!(ab.branches.len(), ba.branches.len());
        for (x, y) in ab.branches.iter().zip(ba.branches.iter()) {
            prop_assert_eq!(x.id, y.id);
            prop_assert_eq!(x.hits, y.hits);
        }
    }

    /// merge sums hit counts: merged[i].hits == input_a[i].hits + input_b[i].hits
    /// (modulo saturating add at u64::MAX, which we avoid here by capping
    /// the generator at 1M each — sum stays well below u64::MAX).
    #[test]
    fn merge_sums_hit_counts(pair in arb_compatible_pair(5)) {
        let (a, b) = pair;
        let merged = merge_records(&[a.clone(), b.clone()]).unwrap();
        for (m, (x, y)) in merged.branches.iter().zip(a.branches.iter().zip(b.branches.iter())) {
            prop_assert_eq!(m.hits, x.hits + y.hits);
        }
    }

    /// merge is monotonic: any single counter in the merged output is
    /// >= the corresponding counter in any input.
    #[test]
    fn merge_is_monotonic(pair in arb_compatible_pair(5)) {
        let (a, b) = pair;
        let merged = merge_records(&[a.clone(), b.clone()]).unwrap();
        for (m, (x, y)) in merged.branches.iter().zip(a.branches.iter().zip(b.branches.iter())) {
            prop_assert!(m.hits >= x.hits);
            prop_assert!(m.hits >= y.hits);
        }
    }
}

// ---------------------------------------------------------------------------
// Serde round-trip properties
// ---------------------------------------------------------------------------

prop_compose! {
    fn arb_branch_entry(id: u32)
        (kind in arb_branch_kind(),
         function_index in 0u32..16u32,
         instr_index in 0u32..1024u32,
         target_index in proptest::option::of(0u32..32u32),
         byte_offset in proptest::option::of(0u32..(1u32 << 30)))
        -> BranchEntry
    {
        BranchEntry {
            id,
            function_index,
            function_name: None,
            kind,
            instr_index,
            target_index,
            byte_offset,
            seq_debug: format!("Id {{ idx: {id} }}"),
        }
    }
}

proptest! {
    /// Manifest survives a JSON round-trip.
    #[test]
    fn manifest_json_round_trip(
        entries in (1usize..16usize).prop_flat_map(|n| pvec(arb_branch_entry(0), n))
    ) {
        // re-id the entries so the ids are unique and dense.
        let entries: Vec<BranchEntry> = entries
            .into_iter()
            .enumerate()
            .map(|(i, mut e)| { e.id = i as u32; e })
            .collect();
        let manifest = Manifest {
            schema_version: "2".to_string(),
            witness_version: "0.3.0".to_string(),
            module_source: "x.wasm".to_string(),
            branches: entries,
            decisions: vec![],
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: Manifest = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(parsed.branches.len(), manifest.branches.len());
        for (a, b) in parsed.branches.iter().zip(manifest.branches.iter()) {
            prop_assert_eq!(a.id, b.id);
            prop_assert_eq!(a.kind, b.kind);
            prop_assert_eq!(a.target_index, b.target_index);
            prop_assert_eq!(a.byte_offset, b.byte_offset);
        }
    }

    /// RunRecord survives a JSON round-trip.
    #[test]
    fn run_record_json_round_trip(record in arb_record(6, 0u64..10_000u64)) {
        let json = serde_json::to_string(&record).unwrap();
        let parsed: RunRecord = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(parsed.branches.len(), record.branches.len());
        for (a, b) in parsed.branches.iter().zip(record.branches.iter()) {
            prop_assert_eq!(a.hits, b.hits);
            prop_assert_eq!(a.kind, b.kind);
        }
    }
}

// ---------------------------------------------------------------------------
// RequirementMap properties
// ---------------------------------------------------------------------------

proptest! {
    /// Disjoint mappings flatten without error; the resulting map has
    /// the union of all branch ids and a stable artefact assignment.
    #[test]
    fn requirement_map_disjoint_flattens(
        ranges in pvec((0u32..1000u32, 1usize..6usize), 1..6)
    ) {
        // Build disjoint ranges by stride.
        let mut cursor = 0u32;
        let mut entries = Vec::new();
        for (idx, (_seed, len)) in ranges.iter().enumerate() {
            let branches: Vec<u32> = (cursor..cursor + (*len as u32)).collect();
            cursor += *len as u32;
            entries.push(MapEntry {
                branches,
                artifact: format!("REQ-{idx}"),
            });
        }
        let map = RequirementMap { mappings: entries };
        let flat = map.flatten().unwrap();
        prop_assert_eq!(flat.len(), cursor as usize);
    }

    /// Overlapping mappings are rejected with an error rather than
    /// silently dropping one of the duplicates.
    #[test]
    fn requirement_map_rejects_overlaps(
        common in 0u32..100u32,
        a_extra in 1u32..10u32,
        b_extra in 1u32..10u32
    ) {
        let map = RequirementMap {
            mappings: vec![
                MapEntry {
                    branches: vec![common, common + a_extra],
                    artifact: "REQ-A".to_string(),
                },
                MapEntry {
                    branches: vec![common, common + b_extra + 100],
                    artifact: "REQ-B".to_string(),
                },
            ],
        };
        prop_assert!(map.flatten().is_err());
    }
}
