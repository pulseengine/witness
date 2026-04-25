//! Verdict: minimal 2-condition AND.
//!
//! Decision: `a.start <= b.end && b.start <= a.end`
//!
//! Conditions:
//!   c1 = a.start <= b.end
//!   c2 = b.start <= a.end
//!
//! See `TRUTH-TABLE.md`.

#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

struct Range {
    start: i32,
    end: i32,
}

#[inline(never)]
fn ranges_overlap(a: Range, b: Range) -> bool {
    a.start <= b.end && b.start <= a.end
}

// row 0: a=(0,1), b=(2,3) — c1=T, c2=F → F
#[unsafe(no_mangle)]
pub extern "C" fn run_row_0() -> i32 {
    ranges_overlap(Range { start: 0, end: 1 }, Range { start: 2, end: 3 }) as i32
}

// row 1: a=(2,3), b=(0,1) — c1=F (c2 short-circuited) → F
#[unsafe(no_mangle)]
pub extern "C" fn run_row_1() -> i32 {
    ranges_overlap(Range { start: 2, end: 3 }, Range { start: 0, end: 1 }) as i32
}

// row 2: a=(0,3), b=(1,2) — c1=T, c2=T → T
#[unsafe(no_mangle)]
pub extern "C" fn run_row_2() -> i32 {
    ranges_overlap(Range { start: 0, end: 3 }, Range { start: 1, end: 2 }) as i32
}
