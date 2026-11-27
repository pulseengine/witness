#![cfg(feature = "runtime")]
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
    BranchKind, ChainKind, Decision, Manifest, TailProbe, apply_chain_kinds,
    apply_unobserved_tails, instrument_file,
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

/// #358 — `TRIANGLE_WAT` with the third operand's comparison routed through
/// `select` (`c3 ? 1 : 0`). Same values, but no instruction leaves the tail
/// value as a recognised boolean, so witness places no probe and the v0.46
/// guard (`unobserved_tail_conditions = 1`) must still apply.
const TRIANGLE_UNPROBED_WAT: &str = r#"
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
      i32.const 1
      i32.const 0
      local.get 2
      local.get 1
      i32.add
      local.get 0
      i32.le_u
      select
      local.set 3
    end
    local.get 3)
  (func (export "run_row_0") (result i32) i32.const 3 i32.const 4 i32.const 5 call $f)
  (func (export "run_row_1") (result i32) i32.const 1 i32.const 2 i32.const 5 call $f)
  (func (export "run_row_2") (result i32) i32.const 5 i32.const 1 i32.const 2 call $f)
  (func (export "run_row_3") (result i32) i32.const 1 i32.const 5 i32.const 2 call $f)
)
"#;

/// #358 — `CALLED_TAIL_WAT` with the same unprobeable `select` tail.
const CALLED_TAIL_UNPROBED_WAT: &str = r#"
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
      i32.const 1
      i32.const 0
      local.get 2
      local.get 1
      i32.add
      local.get 0
      i32.le_u
      select
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
        tail_probe: witness_core::instrument::TailProbe::None,
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
    let (report, decision) = run(TRIANGLE_UNPROBED_WAT, 4);
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
    let (report, decision) = run(CALLED_TAIL_UNPROBED_WAT, 4);
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

// ---- #358: the last operand is OBSERVED through a value-tail probe ----

/// Branch id of the decision's last condition must be a `ValueTail` site.
fn assert_tail_is_probe(wat: &str, decision: &Decision) {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = dir.path().join("source.wasm");
    std::fs::write(&source, wat::parse_str(wat).expect("wat")).expect("write wasm");
    let instrumented = dir.path().join("instrumented.wasm");
    instrument_file(&source, &instrumented).expect("instrument");
    let manifest = Manifest::load(&Manifest::path_for(&instrumented)).expect("manifest");
    let last = *decision.conditions.last().expect("conditions");
    let kind = manifest
        .branches
        .iter()
        .find(|b| b.id == last)
        .map(|b| b.kind);
    assert_eq!(kind, Some(BranchKind::ValueTail));
}

#[test]
fn probed_direct_tail_is_a_real_condition() {
    let (report, decision) = run(TRIANGLE_WAT, 4);
    let real_outcomes = [false, true, true, true];
    assert_eq!(
        decision.conditions.len(),
        3,
        "the third operand is a condition"
    );
    assert_eq!(decision.unobserved_tail_conditions, 0);
    assert!(!decision.unobserved_tail_indirect);
    assert_eq!(decision.tail_probe, TailProbe::Direct);
    assert_tail_is_probe(TRIANGLE_WAT, &decision);

    let d = &report.decisions[0];
    assert_eq!(d.conditions.len(), 3);
    assert_eq!(d.unobserved_tail_conditions, 0);
    assert_ne!(
        d.status_reason.as_deref(),
        Some("unobserved_tail_conditions")
    );
    // Outcomes are still the REAL return values: the probe does not change
    // what the function computes, and the direct tail's outcome source is
    // unchanged.
    for r in &d.truth_table {
        assert_eq!(r.outcome, Some(real_outcomes[r.row_id as usize]));
    }
    // The tail is recorded (source polarity) exactly on the rows that
    // evaluate it: row 0 (3,4,5): 9 <= 3 false; row 2 (5,1,2): 3 <= 5 true.
    for r in &d.truth_table {
        let want = match r.row_id {
            0 => Some(false),
            2 => Some(true),
            _ => None,
        };
        assert_eq!(r.evaluated.get(&2).copied(), want, "row {}", r.row_id);
    }
    let c3 = &d.conditions[2];
    assert!(matches!(c3.status, ConditionStatus::Proved), "{c3:?}");
    let mut pair = c3.pair.expect("pair");
    pair.sort_unstable();
    assert_eq!(pair, [0, 2], "the truth table's c3 pair");
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
fn probed_indirect_tail_outcome_is_the_tail_value_or_nothing() {
    let (report, decision) = run(CALLED_TAIL_WAT, 4);
    assert_eq!(decision.conditions.len(), 3);
    assert_eq!(decision.unobserved_tail_conditions, 0);
    assert_eq!(decision.tail_probe, TailProbe::Indirect);
    assert_tail_is_probe(CALLED_TAIL_WAT, &decision);

    let d = &report.decisions[0];
    // The function returns the constant 0 on every row; that is never an
    // outcome. Rows that evaluate the tail carry its value (the decision's
    // value); rows that stopped earlier carry none.
    for r in &d.truth_table {
        let want = match r.row_id {
            0 => Some(false),
            2 => Some(true),
            _ => None,
        };
        assert_eq!(r.outcome, want, "row {}", r.row_id);
    }
    let c3 = &d.conditions[2];
    assert!(matches!(c3.status, ConditionStatus::Proved), "{c3:?}");
    for c in &d.conditions[..2] {
        assert!(
            !matches!(c.status, ConditionStatus::Proved),
            "c{} proved without an observed outcome on its short-circuit row",
            c.index
        );
    }
    assert!(!matches!(d.status, DecisionStatus::FullMcdc));
}
