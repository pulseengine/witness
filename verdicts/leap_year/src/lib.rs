//! Verdict: textbook 3-condition mixed AND/OR MC/DC example.
//!
//! Decision: `(year % 4 == 0 && year % 100 != 0) || year % 400 == 0`
//!
//! Conditions:
//!   c1 = year % 4 == 0
//!   c2 = year % 100 != 0
//!   c3 = year % 400 == 0
//!
//! See `TRUTH-TABLE.md` for the expected MC/DC analysis.

#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[inline(never)]
fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

// row 0: year=2001 — c1=F, c2/c3 short-circuit chain, c3=F → outcome=F
#[unsafe(no_mangle)]
pub extern "C" fn run_row_0() -> i32 {
    is_leap_year(2001) as i32
}

// row 1: year=2004 — c1=T, c2=T, OR short-circuits c3 → outcome=T
#[unsafe(no_mangle)]
pub extern "C" fn run_row_1() -> i32 {
    is_leap_year(2004) as i32
}

// row 2: year=2100 — c1=T, c2=F, c3=F → outcome=F
#[unsafe(no_mangle)]
pub extern "C" fn run_row_2() -> i32 {
    is_leap_year(2100) as i32
}

// row 3: year=2000 — c1=T, c2=F, c3=T → outcome=T
#[unsafe(no_mangle)]
pub extern "C" fn run_row_3() -> i32 {
    is_leap_year(2000) as i32
}
