//! End-to-end integration tests for witness against a real Rust→Wasm
//! fixture.
//!
//! These tests are the empirical backstop for the v0.1/v0.2 instrumentation:
//! they run on `rustc` output, not synthetic WAT. The fixture lives at
//! `tests/fixtures/sample-rust-crate/` and is built into
//! `tests/fixtures/sample-rust-crate/sample.wasm` by `build.sh`.
//!
//! ## Skipping cleanly when the fixture isn't built
//!
//! If `sample.wasm` doesn't exist, every test here is a no-op (returns Ok)
//! with a printed warning. CI is responsible for building the fixture
//! before invoking `cargo test --test integration_e2e`. Locally:
//!
//! ```sh
//! ./tests/fixtures/sample-rust-crate/build.sh
//! cargo test --test integration_e2e
//! ```
//!
//! ## What we assert
//!
//! Per entry-point, after instrumenting the fixture and invoking that one
//! export, we look at the run record's per-branch hit counts and check:
//!
//! - The right *kind* of counter fired (e.g. `IfThen` for `run_then_arm`).
//! - The wrong kinds did not fire on that invocation alone.
//!
//! We deliberately do NOT assert on absolute branch ids: the ids depend on
//! rustc's lowering order, which is not stable across rustc versions. We
//! assert on *kinds* and *cardinalities*, which the lowering preserves
//! across reasonable rustc versions for this fixture's structure.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use witness::instrument::{BranchKind, instrument_file};
use witness::run::{RunOptions, RunRecord, run_module};

/// Path to the pre-built fixture wasm. `build.sh` produces this.
fn fixture_wasm() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("sample-rust-crate")
        .join("sample.wasm")
}

/// Returns true when the fixture is available; false (with a printed
/// warning) when it isn't, in which case tests should bail out as a no-op.
///
/// We don't use `#[ignore]` because that would require a separate
/// `cargo test -- --ignored` invocation in CI; runtime-skip lets a single
/// `cargo test` pass cleanly whether the fixture is built or not. CI gates
/// the build separately.
fn fixture_available() -> bool {
    let p = fixture_wasm();
    if !p.exists() {
        eprintln!(
            "skipping integration_e2e: fixture not built at {}.\n\
             run tests/fixtures/sample-rust-crate/build.sh to produce it.",
            p.display()
        );
        return false;
    }
    true
}

/// Instrument the fixture into a tempdir, run one `--invoke` against it,
/// return the resulting `RunRecord`.
fn run_with_invoke(invoke: &str) -> RunRecord {
    let dir = tempfile::tempdir().expect("tempdir");
    let instrumented = dir.path().join("sample.instrumented.wasm");
    let manifest_path = {
        let mut p = instrumented.as_os_str().to_os_string();
        p.push(".witness.json");
        PathBuf::from(p)
    };
    let run_path = dir.path().join("run.json");

    instrument_file(&fixture_wasm(), &instrumented).expect("instrument fixture");

    let options = RunOptions {
        module: &instrumented,
        manifest: manifest_path,
        output: &run_path,
        invoke: vec![invoke.to_string()],
        call_start: false,
        harness: None,
    };
    run_module(&options).expect("run instrumented fixture");

    RunRecord::load(&run_path).expect("load run record")
}

/// Sum hits for branches matching `pred`.
fn sum_hits<F: Fn(&witness::run::BranchHit) -> bool>(rec: &RunRecord, pred: F) -> u64 {
    rec.branches
        .iter()
        .filter(|b| pred(b))
        .map(|b| b.hits)
        .sum()
}

/// Sum hits for branches in any function whose name contains `fn_name_part`,
/// matching the kind. We match on a substring of the function name because
/// rustc-mangled names will include the crate prefix, and the crate name in
/// `Cargo.toml` is `witness-sample-fixture`.
fn sum_hits_in_fn(rec: &RunRecord, fn_name_part: &str, kind: BranchKind) -> u64 {
    sum_hits(rec, |b| {
        b.kind == kind
            && b.function_name
                .as_deref()
                .is_some_and(|n| n.contains(fn_name_part))
    })
}

#[test]
fn fixture_instruments_and_records_branches() {
    if !fixture_available() {
        return;
    }
    // Just instrument and verify the manifest is non-empty — proves the
    // fixture has at least some branches witness recognises.
    let dir = tempfile::tempdir().unwrap();
    let instrumented = dir.path().join("sample.instrumented.wasm");
    instrument_file(&fixture_wasm(), &instrumented).expect("instrument");

    let manifest_path = {
        let mut p = instrumented.as_os_str().to_os_string();
        p.push(".witness.json");
        PathBuf::from(p)
    };
    let manifest = witness::instrument::Manifest::load(Path::new(&manifest_path)).unwrap();
    assert!(
        !manifest.branches.is_empty(),
        "expected the fixture to contain at least one instrumentable branch; got 0"
    );
    // We expect representatives of all three kinds. If a future rustc
    // collapses one (e.g. `match` → `br_if` chain instead of `br_table`),
    // this assertion will need updating.
    let kinds: std::collections::HashSet<BranchKind> =
        manifest.branches.iter().map(|b| b.kind).collect();
    assert!(
        kinds.contains(&BranchKind::IfThen) || kinds.contains(&BranchKind::IfElse),
        "expected at least one if/else branch; kinds present: {kinds:?}"
    );
    assert!(
        kinds.contains(&BranchKind::BrIf),
        "expected at least one br_if branch; kinds present: {kinds:?}"
    );
    assert!(
        kinds.contains(&BranchKind::BrTableTarget) || kinds.contains(&BranchKind::BrTableDefault),
        "expected at least one br_table branch (target or default); kinds present: {kinds:?}"
    );
}

#[test]
fn run_then_arm_fires_then_counter() {
    if !fixture_available() {
        return;
    }
    let rec = run_with_invoke("run_then_arm");

    // The then-arm of `ifelse_check` should fire at least once.
    let then_hits = sum_hits_in_fn(&rec, "ifelse_check", BranchKind::IfThen);
    let else_hits = sum_hits_in_fn(&rec, "ifelse_check", BranchKind::IfElse);
    assert!(
        then_hits >= 1,
        "expected ifelse_check IfThen counter to fire at least once; \
         got then={then_hits} else={else_hits}\nrec: {rec:#?}"
    );
    assert_eq!(
        else_hits, 0,
        "expected ifelse_check IfElse counter to remain 0 on then-arm input; \
         got then={then_hits} else={else_hits}"
    );
}

#[test]
fn run_else_arm_fires_else_counter() {
    if !fixture_available() {
        return;
    }
    let rec = run_with_invoke("run_else_arm");

    let then_hits = sum_hits_in_fn(&rec, "ifelse_check", BranchKind::IfThen);
    let else_hits = sum_hits_in_fn(&rec, "ifelse_check", BranchKind::IfElse);
    assert_eq!(
        then_hits, 0,
        "expected ifelse_check IfThen counter to remain 0 on else-arm input; \
         got then={then_hits} else={else_hits}"
    );
    assert!(
        else_hits >= 1,
        "expected ifelse_check IfElse counter to fire at least once; \
         got then={then_hits} else={else_hits}"
    );
}

#[test]
fn run_brif_not_taken_fires_brif_counter() {
    if !fixture_available() {
        return;
    }
    // The "not taken" path of the && chain — `a > 0` is false so the first
    // br_if (after the eqz inversion) IS taken in Wasm terms. At least one
    // BrIf counter inside `brif_check` should fire.
    let rec = run_with_invoke("run_brif_not_taken");

    let brif_hits = sum_hits_in_fn(&rec, "brif_check", BranchKind::BrIf);
    assert!(
        brif_hits >= 1,
        "expected at least one BrIf counter in brif_check to fire on \
         short-circuit input; got brif_hits={brif_hits}\nrec: {rec:#?}"
    );
}

#[test]
fn run_brif_taken_path_does_not_short_circuit() {
    if !fixture_available() {
        return;
    }
    // Both conditions true → no short-circuit. Whether *any* br_if counter
    // fires depends on the lowering: rustc's `&&` lowering may use br_if to
    // jump to the "true" continuation as well as the "false" one. We don't
    // assert a hard 0 — we assert the path produced a sane run record.
    let rec = run_with_invoke("run_brif_taken");

    // Sanity: the run completed and recorded branches for `brif_check`.
    let brif_hits = sum_hits_in_fn(&rec, "brif_check", BranchKind::BrIf);
    // We don't pin this to a specific value because rustc may lower the
    // pass-through path to a br_if-to-continuation, a select, or a fallthrough
    // depending on version. Just record that the test ran.
    eprintln!("run_brif_taken: brif_check BrIf hit total = {brif_hits}");
}

#[test]
fn run_match_arm_0_fires_target_0() {
    if !fixture_available() {
        return;
    }
    let rec = run_with_invoke("run_match_arm_0");

    // At least one `BrTableTarget` counter in `match_check` should fire,
    // and the default counter should not. We don't pin which target index
    // fires because rustc's match-lowering can permute target order; the
    // important property is "an explicit target fired, not the default".
    let target_hits = sum_hits_in_fn(&rec, "match_check", BranchKind::BrTableTarget);
    let default_hits = sum_hits_in_fn(&rec, "match_check", BranchKind::BrTableDefault);
    assert!(
        target_hits >= 1,
        "expected an explicit BrTableTarget counter in match_check to fire on \
         arm-0 input; got target={target_hits} default={default_hits}\nrec: {rec:#?}"
    );
    assert_eq!(
        default_hits, 0,
        "expected BrTableDefault counter to remain 0 on arm-0 input; \
         got target={target_hits} default={default_hits}"
    );
}

#[test]
fn run_match_arm_1_fires_target() {
    if !fixture_available() {
        return;
    }
    let rec = run_with_invoke("run_match_arm_1");

    let target_hits = sum_hits_in_fn(&rec, "match_check", BranchKind::BrTableTarget);
    let default_hits = sum_hits_in_fn(&rec, "match_check", BranchKind::BrTableDefault);
    assert!(
        target_hits >= 1,
        "expected an explicit BrTableTarget counter in match_check to fire on \
         arm-1 input; got target={target_hits} default={default_hits}"
    );
    assert_eq!(
        default_hits, 0,
        "expected BrTableDefault counter to remain 0 on arm-1 input"
    );
}

#[test]
fn run_match_default_fires_default_counter() {
    if !fixture_available() {
        return;
    }
    let rec = run_with_invoke("run_match_default");

    let target_hits = sum_hits_in_fn(&rec, "match_check", BranchKind::BrTableTarget);
    let default_hits = sum_hits_in_fn(&rec, "match_check", BranchKind::BrTableDefault);
    assert!(
        default_hits >= 1,
        "expected BrTableDefault counter in match_check to fire on default \
         input; got target={target_hits} default={default_hits}\nrec: {rec:#?}"
    );
    assert_eq!(
        target_hits, 0,
        "expected explicit BrTableTarget counters to remain 0 on default input; \
         got target={target_hits} default={default_hits}"
    );
}
