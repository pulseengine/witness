//! Verdict: 4-condition nested operators.
//!
//! Decision: `(a || b) && (c || d)`.
//!
//! Stresses **operator nesting** — the most common shape of compound
//! Rust boolean expressions in real safety-critical code. Each operand
//! is itself a short-circuit OR; the outer AND short-circuits when the
//! left OR evaluates to false.
//!
//! See `TRUTH-TABLE.md`.

#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[inline(never)]
fn nested(a: bool, b: bool, c: bool, d: bool) -> bool {
    (a || b) && (c || d)
}

// row 0: a=F, b=F → (a||b)=F → AND short-circuits. c,d skipped. F.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_0() -> i32 {
    nested(false, false, false, false) as i32
}

// row 1: a=F, b=T → (a||b)=T. c=F, d=F → (c||d)=F. outcome=F.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_1() -> i32 {
    nested(false, true, false, false) as i32
}

// row 2: a=F, b=T → T. c=F, d=T → T. outcome=T.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_2() -> i32 {
    nested(false, true, false, true) as i32
}

// row 3: a=F, b=T → T. c=T, d skipped. outcome=T.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_3() -> i32 {
    nested(false, true, true, false) as i32
}

// row 4: a=T, b skipped. c=F, d=T → T. outcome=T.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_4() -> i32 {
    nested(true, false, false, true) as i32
}
