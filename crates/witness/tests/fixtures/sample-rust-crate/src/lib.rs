//! End-to-end test fixture for witness.
//!
//! This crate exists for one purpose: produce a real `.wasm` file from real
//! `rustc` so witness's integration tests can verify that instrumentation
//! works against compiler output that is not just hand-written WAT.
//!
//! Each entry-point function is named `run_*`, takes no arguments, and
//! returns a small `i32` whose value is distinguishable per arm (100, 200,
//! 300, ...) so the integration test can also assert on the runner's return
//! value. Returning literal `0` or `1` would be ambiguous against a default
//! "not invoked" reading.
//!
//! Coverage patterns exercised:
//!
//! | Pattern    | Underlying fn          | Entry points                              |
//! |------------|------------------------|-------------------------------------------|
//! | `br_if`    | [`brif_check`]         | `run_brif_taken`, `run_brif_not_taken`    |
//! | `if/else`  | [`ifelse_check`]       | `run_then_arm`, `run_else_arm`            |
//! | `br_table` | [`match_check`]        | `run_match_arm_0`, `run_match_arm_1`,     |
//! |            |                        | `run_match_default`                       |
//!
//! Why `no_std`? `wasm32-unknown-unknown` has no libc, no unwinder, and no
//! built-in panic infrastructure. `no_std` plus a hand-rolled panic handler
//! keeps the produced module small and compiler-deterministic. We also avoid
//! third-party dependencies entirely so this fixture's reproducibility
//! depends only on rustc itself.
//!
//! Why `#[unsafe(no_mangle)]` + `extern "C"`? Predictable export names. The
//! integration test invokes exports by string literal (`run_then_arm`, etc.)
//! and `wasmtime`'s `instance.get_func(&mut store, "run_then_arm")` will not
//! find a Rust-mangled symbol. The 2024 edition spells the attribute
//! `#[unsafe(no_mangle)]` rather than `#[no_mangle]` because the lint moved.

#![no_std]
// The fixture exists to be invoked by witness, not tested as a unit; we
// suppress the "function never used" warning that would fire on the helper
// fns when compiled as a cdylib.
#![allow(dead_code)]

use core::panic::PanicInfo;

/// Required for `no_std` on `wasm32-unknown-unknown`. We never expect to hit
/// this — the entry points use only arithmetic and integer comparisons — so
/// we deliberately drop into an infinite loop rather than emit a `unreachable`
/// trap. `loop {}` with no observable side-effect lowers to a tiny Wasm body.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

// ---------------------------------------------------------------------------
// Underlying functions — these contain the actual branch instructions that
// witness instruments. The `#[inline(never)]` attribute keeps them as
// distinct functions in the produced Wasm so the manifest's
// `function_index` and `function_name` map cleanly.
// ---------------------------------------------------------------------------

/// Short-circuit `&&` — lowers to a `br_if` chain in Wasm. Returns the
/// distinguishable value `100` when both conditions hold, `200` otherwise.
///
/// In Wasm, `a > 0 && b < 10` becomes roughly:
///
/// ```text
///   local.get $a
///   i32.const 0
///   i32.gt_s
///   i32.eqz       ;; invert because br_if jumps OUT on TRUE
///   br_if $false  ;; first br_if — early-exit if a <= 0
///   local.get $b
///   i32.const 10
///   i32.lt_s
///   i32.eqz
///   br_if $false  ;; second br_if — early-exit if b >= 10
///   ...
/// ```
///
/// Rustc may use other patterns (`select`, `if/else`, etc.) depending on
/// optimization level. The fixture pins `opt-level = 1` and `lto = false`
/// in `Cargo.toml` to keep the lowering close to the source structure.
#[inline(never)]
fn brif_check(a: i32, b: i32) -> i32 {
    if a > 0 && b < 10 { 100 } else { 200 }
}

/// Plain `if/else` with both arms reachable. Lowers to a Wasm `if A else B
/// end` block, producing two `IfElse` BranchSites in walrus's view (one for
/// `IfThen`, one for `IfElse`). Returns `100` for the then arm, `200` for
/// the else arm.
#[inline(never)]
fn ifelse_check(x: i32) -> i32 {
    if x > 0 { 100 } else { 200 }
}

/// `match` on a small int with several arms — lowers to a Wasm `br_table`
/// when the arms are dense enough (rustc decides). With three arms over
/// `0`, `1`, `_`, current rustc emits a `br_table` with two explicit
/// targets and a default. Returns `100`, `200`, or `300` depending on
/// which arm fires.
///
/// Note: rustc's match-lowering heuristic depends on arm density and
/// rustc version. If a future rustc lowers this to a chain of `br_if`s
/// instead of a `br_table`, the integration test should be updated to
/// match the new lowering — the fixture's purpose is to exercise *the
/// lowering rustc actually produces*, not to artificially force a
/// particular Wasm shape. v0.2.x DWARF reconstruction work will need
/// the rustc-version-pin documented in `rust-toolchain.toml`.
#[inline(never)]
fn match_check(x: i32) -> i32 {
    match x {
        0 => 100,
        1 => 200,
        _ => 300,
    }
}

// ---------------------------------------------------------------------------
// No-argument entry points — these are what witness's `--invoke` calls.
// Each one supplies the input that drives the underlying fn down a
// specific path, so the integration test can assert which counter fired.
// ---------------------------------------------------------------------------

/// Exercises [`ifelse_check`]'s then arm. Expected: `IfThen` counter +1,
/// `IfElse` counter unchanged. Returns 100.
#[unsafe(no_mangle)]
pub extern "C" fn run_then_arm() -> i32 {
    ifelse_check(7)
}

/// Exercises [`ifelse_check`]'s else arm. Expected: `IfElse` counter +1,
/// `IfThen` counter unchanged. Returns 200.
#[unsafe(no_mangle)]
pub extern "C" fn run_else_arm() -> i32 {
    ifelse_check(-3)
}

/// Exercises [`brif_check`] with both conditions true (the "both br_if's
/// fall through" path). Returns 100. Expected: every `BrIf` counter in
/// `brif_check` records 0 (the fall-through case — `br_if` only fires on
/// `taken`). The true cover for "br_if taken" is `run_brif_not_taken`,
/// where the short-circuit kicks in.
#[unsafe(no_mangle)]
pub extern "C" fn run_brif_taken() -> i32 {
    // Both conditions true → no short-circuit → both br_if checks fall
    // through. This is intentionally the "happy path" of the &&; the
    // *taken* counter for `br_if` fires when the inversion-and-jump
    // path is hit, which is what `run_brif_not_taken` does.
    brif_check(5, 5)
}

/// Exercises [`brif_check`] taking the short-circuit path on the FIRST
/// condition (`a > 0` is false → first br_if jumps to the false arm).
/// Returns 200. Expected: at least one `BrIf` counter records >0 hits in
/// `brif_check`.
#[unsafe(no_mangle)]
pub extern "C" fn run_brif_not_taken() -> i32 {
    // a = -1 fails `a > 0`, so the first br_if (after `eqz`) is taken.
    brif_check(-1, 5)
}

/// Exercises [`match_check`] arm 0. Returns 100. Expected: the `BrTableTarget`
/// counter for target index 0 records >0; other target counters and the
/// default counter remain 0.
#[unsafe(no_mangle)]
pub extern "C" fn run_match_arm_0() -> i32 {
    match_check(0)
}

/// Exercises [`match_check`] arm 1. Returns 200. Expected: the `BrTableTarget`
/// counter for target index 1 records >0.
#[unsafe(no_mangle)]
pub extern "C" fn run_match_arm_1() -> i32 {
    match_check(1)
}

/// Exercises [`match_check`]'s default arm. Returns 300. Expected: the
/// `BrTableDefault` counter records >0; explicit-target counters remain 0.
#[unsafe(no_mangle)]
pub extern "C" fn run_match_default() -> i32 {
    match_check(42)
}
