//! #352 step 1 — witness must never over-claim.
//!
//! For a value-producing short-circuit decision (`fn f(..) -> bool { a || b || c }`)
//! rustc emits no `br_if` for the LAST operand, because its value is the
//! function result. Before this guard witness (1) derived each row's outcome
//! from the last observed condition value, which is wrong whenever the chain
//! falls through into that unobserved tail, so conditions were reported
//! `proved` by row pairs whose real decision outcome did not change, and
//! (2) reported `FullMcdc` for a decision with an operand it never saw.
//!
//! The module below has exactly rustc's lowering of the Myers triangle
//! predicate `a + b <= c || a + c <= b || b + c <= a` (verdicts/triangle):
//! two `br_if`s for three operands, the third operand's comparison stored
//! to a local and returned. Real return values of the four rows are
//! `[0, 1, 1, 1]`; rows 1 and 3 both return 1, so they cannot show
//! independent effect of anything.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::as_conversions
)]

use std::path::PathBuf;

use witness::{RunOptions, run_module};
use witness_core::instrument::{
    BranchKind, ChainKind, Decision, Manifest, apply_chain_kinds, apply_unobserved_tails,
    instrument_file,
};
use witness_core::mcdc_report::{ConditionStatus, DecisionStatus, McdcReport};
use witness_core::run_record::RunRecord;

/// rustc's lowering of `a + b <= c || a + c <= b || b + c <= a` (value tail).
const TRIANGLE_WAT: &str = r#"
(module
  (func $f (param i32 i32 i32) (result i32)
    (local i32)
    block
      local.get 1
      local.get 0
      i32.add
      local.get 2
      i32.gt_u
      br_if 0
      i32.const 1
      return
    end
    i32.const 1
    local.set 3
    block
      local.get 2
      local.get 0
      i32.add
      local.get 1
      i32.le_u
      br_if 0
      local.get 2
      local.get 1
      i32.add
      local.get 0
      i32.le_u
      local.set 3
    end
    local.get 3)
  (func (export "run_row_0") (result i32) i32.const 3 i32.const 4 i32.const 5 call $f)
  (func (export "run_row_1") (result i32) i32.const 1 i32.const 2 i32.const 5 call $f)
  (func (export "run_row_2") (result i32) i32.const 5 i32.const 1 i32.const 2 call $f)
  (func (export "run_row_3") (result i32) i32.const 1 i32.const 5 i32.const 2 call $f)
)
"#;

/// `log(a + b <= c || a + c <= b || b + c <= a); 0`: the decision's value goes
/// to a call and the function returns the constant 0 on every row. The
/// function result is NOT the decision's value, so it must never be used as
/// the row outcome (it would read 0 for every row).
const CALLED_TAIL_WAT: &str = r#"
(module
  (func $log (param i32))
  (func $f (param i32 i32 i32) (result i32)
    (local i32)
    block
      local.get 1
      local.get 0
      i32.add
      local.get 2
      i32.gt_u
      br_if 0
      i32.const 1
      call $log
      i32.const 0
      return
    end
    i32.const 1
    local.set 3
    block
      local.get 2
      local.get 0
      i32.add
      local.get 1
      i32.le_u
      br_if 0
      local.get 2
      local.get 1
      i32.add
      local.get 0
      i32.le_u
      local.set 3
    end
    local.get 3
    call $log
    i32.const 0)
  (func (export "run_row_0") (result i32) i32.const 3 i32.const 4 i32.const 5 call $f)
  (func (export "run_row_1") (result i32) i32.const 1 i32.const 2 i32.const 5 call $f)
  (func (export "run_row_2") (result i32) i32.const 5 i32.const 1 i32.const 2 call $f)
  (func (export "run_row_3") (result i32) i32.const 1 i32.const 5 i32.const 2 call $f)
)
"#;

/// `a && b` as a COMPLETE chain: both operands have a `br_if`, the
/// fall-through is a constant. Nothing is unobserved, so the derived
/// (short-circuit) outcome is sound and FullMcdc must stay reachable.
const COMPLETE_AND_WAT: &str = r#"
(module
  (func $g (param i32 i32) (result i32)
    block
      local.get 0
      i32.eqz
      br_if 0
      local.get 1
      i32.eqz
      br_if 0
      i32.const 1
      return
    end
    i32.const 0)
  (func (export "run_row_0") (result i32) i32.const 1 i32.const 1 call $g)
  (func (export "run_row_1") (result i32) i32.const 0 i32.const 1 call $g)
  (func (export "run_row_2") (result i32) i32.const 1 i32.const 0 call $g)
)
"#;

/// Instrument `wat`, reconstruct ONE decision over its `br_if`s the way DWARF
/// reconstruction would (this synthetic module has no DWARF), run every
/// `run_row_N` export, and return the report plus the manifest decision.
fn run(wat: &str, rows: usize) -> (McdcReport, Decision) {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = dir.path().join("source.wasm");
    std::fs::write(&source, wat::parse_str(wat).expect("wat")).expect("write wasm");
    let instrumented = dir.path().join("instrumented.wasm");
    instrument_file(&source, &instrumented).expect("instrument");

    let manifest_path = Manifest::path_for(&instrumented);
    let mut manifest = Manifest::load(&manifest_path).expect("manifest");
    let conditions: Vec<u32> = manifest
        .branches
        .iter()
        .filter(|b| b.kind == BranchKind::BrIf)
        .map(|b| b.id)
        .collect();
    let mut decisions = vec![Decision {
        id: 0,
        conditions,
        source_file: None,
        source_line: None,
        chain_kind: ChainKind::default(),
        inline_context: None,
        unobserved_tail_conditions: 0,
        unobserved_tail_indirect: false,
    }];
    // Same two passes `instrument_file` runs on DWARF-reconstructed decisions.
    // The instrument-time stashes they read are still set on this thread.
    apply_chain_kinds(&mut decisions);
    apply_unobserved_tails(&mut decisions);
    manifest.decisions = decisions.clone();
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).expect("serialize"),
    )
    .expect("write manifest");

    let run_path = dir.path().join("run.json");
    let options = RunOptions {
        module: &instrumented,
        manifest: PathBuf::from(&manifest_path),
        output: &run_path,
        invoke: (0..rows).map(|i| format!("run_row_{i}")).collect(),
        invoke_with_args: vec![],
        call_start: false,
        invoke_all: false,
        harness: None,
        component_backend: false,
        stub_imports: None,
    };
    run_module(&options).expect("run");
    let record = RunRecord::load(&run_path).expect("load run record");
    (
        McdcReport::from_record(&record),
        decisions.into_iter().next().expect("one decision"),
    )
}

#[test]
fn value_tail_chain_never_proves_with_unchanged_outcome_and_is_never_full() {
    let (report, decision) = run(TRIANGLE_WAT, 4);
    let real_outcomes = [false, true, true, true];

    // G2 detection: the last br_if falls into the value tail.
    assert_eq!(
        decision.unobserved_tail_conditions, 1,
        "the unobserved third operand must be detected"
    );

    let d = &report.decisions[0];
    // G2: never FullMcdc, with a machine-readable reason.
    assert!(
        !matches!(d.status, DecisionStatus::FullMcdc),
        "decision with an unobserved trailing operand reported FullMcdc: {:?}",
        d.status
    );
    assert_eq!(
        d.status_reason.as_deref(),
        Some("unobserved_tail_conditions")
    );
    assert_eq!(d.unobserved_tail_conditions, 1);
    assert_eq!(report.overall.decisions_full_mcdc, 0);

    // G1: every row's outcome is the REAL return value ...
    for r in &d.truth_table {
        assert_eq!(
            r.outcome,
            Some(real_outcomes[r.row_id as usize]),
            "row {} outcome is not the actual return value",
            r.row_id
        );
    }
    // ... and no condition is proved by a pair whose real outcome is unchanged.
    for c in &d.conditions {
        if matches!(c.status, ConditionStatus::Proved) {
            let [a, b] = c.pair.expect("proved condition carries its pair");
            assert_ne!(
                real_outcomes[a as usize], real_outcomes[b as usize],
                "c{} proved by rows {a}+{b}, whose real outcomes are equal",
                c.index
            );
        }
    }
}

#[test]
fn complete_chain_keeps_full_mcdc() {
    let (report, decision) = run(COMPLETE_AND_WAT, 3);
    assert_eq!(
        decision.unobserved_tail_conditions, 0,
        "a constant fall-through is not an unobserved operand"
    );
    let d = &report.decisions[0];
    assert!(
        matches!(d.status, DecisionStatus::FullMcdc),
        "complete chain must still reach FullMcdc, got {:?}",
        d.status
    );
    assert!(d.status_reason.is_none());
}

#[test]
fn tail_passed_to_a_call_never_uses_the_return_value() {
    let (report, decision) = run(CALLED_TAIL_WAT, 4);
    assert_eq!(decision.unobserved_tail_conditions, 1);
    assert!(
        decision.unobserved_tail_indirect,
        "a tail whose value is passed to a call is not a direct return"
    );
    let d = &report.decisions[0];
    // Nothing is proved, so the honest status is NoWitness (Partial needs a
    // proved condition); it must never be FullMcdc.
    assert!(
        matches!(
            d.status,
            DecisionStatus::Partial | DecisionStatus::NoWitness
        ),
        "{:?}",
        d.status
    );
    for r in &d.truth_table {
        assert_eq!(
            r.outcome, None,
            "row {} carries the function's constant return as the decision outcome",
            r.row_id
        );
    }
    for c in &d.conditions {
        assert!(
            !matches!(c.status, ConditionStatus::Proved),
            "c{} proved from the function's return value, which is not the decision",
            c.index
        );
    }
    assert!(
        d.conditions
            .iter()
            .any(|c| c.reason.as_deref() == Some("outcome_unverified")),
        "expected outcome_unverified gaps"
    );
}
