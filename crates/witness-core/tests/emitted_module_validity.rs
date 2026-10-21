//! #311 — `instrument` must never write a module that fails validation.
//!
//! #207 budgets *globals* arithmetically, but the emitted module can still
//! be rejected on another 1,000,000-capped quantity ("effective type
//! size", then "exports count") while globals stay under their limit.
//! Contract: instrumentation either errors, or the bytes it wrote validate.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects
)]

use std::fmt::Write as _;
use std::path::Path;

use witness_core::instrument::instrument_file;

/// The reporter's shape: `ifs` statements of `(if (local.get 0) (then))`
/// spread over 34 exported functions taking an i32.
fn fixture(ifs: usize) -> Vec<u8> {
    const FUNCS: usize = 34;
    let mut wat = String::from("(module\n");
    for i in 0..FUNCS {
        let n = ifs / FUNCS + usize::from(i < ifs % FUNCS);
        writeln!(wat, "(func (export \"f{i}\") (param i32)").unwrap();
        for _ in 0..n {
            wat.push_str("(if (local.get 0) (then))\n");
        }
        wat.push_str(")\n");
    }
    wat.push(')');
    wat::parse_str(&wat).expect("fixture WAT parses")
}

/// Instrument `ifs` branches; if it succeeded, the written file must
/// validate. Returns whether instrumentation succeeded.
fn instrument_and_check(dir: &Path, ifs: usize) -> bool {
    let input = dir.join("in.wasm");
    let output = dir.join("out.wasm");
    let bytes = fixture(ifs);
    wasmparser::validate(&bytes).expect("fixture is a valid module");
    std::fs::write(&input, &bytes).unwrap();

    match instrument_file(&input, &output) {
        Err(_) => {
            assert!(
                !output.exists(),
                "{ifs} ifs: instrument refused but left an output file behind"
            );
            false
        }
        Ok(()) => {
            let emitted = std::fs::read(&output).unwrap();
            wasmparser::validate(&emitted).unwrap_or_else(|e| {
                panic!("{ifs} ifs: instrument exited ok but wrote an invalid module: {e}")
            });
            true
        }
    }
}

#[test]
fn instrument_output_validates_below_the_global_budget() {
    let dir = tempfile::tempdir().unwrap();
    // 166,649 passes the #207 globals guard (999,894 <= 1,000,000) yet the
    // output exceeds the effective-type-size limit.
    instrument_and_check(dir.path(), 166_649);
}

#[test]
fn instrument_output_validates_at_exports_limit() {
    let dir = tempfile::tempdir().unwrap();
    // 166,666 passes the guard (999,996 globals) yet exports exceed 1,000,000.
    instrument_and_check(dir.path(), 166_666);
}
