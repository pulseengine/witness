//! Verdict: Myers-paper triangle "not a triangle" predicate.
//!
//! Decision: `a + b <= c || a + c <= b || b + c <= a`
//!
//! True when the three side lengths cannot form a triangle (any one side
//! is at least as long as the sum of the other two).
//!
//! Conditions:
//!   c1 = a + b <= c
//!   c2 = a + c <= b
//!   c3 = b + c <= a
//!
//! Source: G. J. Myers, *The Art of Software Testing*, 1979 (the original
//! triangle test). Used in MC/DC literature ever since.
//!
//! See `TRUTH-TABLE.md`.

#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[inline(never)]
fn not_a_triangle(a: u32, b: u32, c: u32) -> bool {
    a + b <= c || a + c <= b || b + c <= a
}

// row 0: (3, 4, 5) — real triangle. c1=F, c2=F, c3=F → F
#[unsafe(no_mangle)]
pub extern "C" fn run_row_0() -> i32 {
    not_a_triangle(3, 4, 5) as i32
}

// row 1: (1, 2, 5) — c1=T (3<=5) → T. c2,c3 short-circuited.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_1() -> i32 {
    not_a_triangle(1, 2, 5) as i32
}

// row 2: (5, 1, 2) — c1=F (6<=2), c2=F (7<=1), c3=T (3<=5) → T.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_2() -> i32 {
    not_a_triangle(5, 1, 2) as i32
}

// row 3: (1, 5, 2) — c1=F (6<=2), c2=T (3<=5) → T. c3 short-circuited.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_3() -> i32 {
    not_a_triangle(1, 5, 2) as i32
}
