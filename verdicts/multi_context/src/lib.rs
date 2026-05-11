//! Witness verdict — multi-call-site fixture for v0.13+ inline-
//! context tagging and v0.14's chain drill-down. Uses stdlib
//! slice helpers (`is_empty`, `contains`, `first`) so rustc
//! reliably emits `DW_TAG_inlined_subroutine` DIEs for the
//! inlined calls; v0.14's chain walker captures them up to ~4
//! levels deep.
//!
//! The predicate `is_valid` is reached through two distinct
//! wrappers `check_first` and `check_second`. The runner
//! exposes 8 invocations (`run_row_0..7`); even rows go via
//! `check_first`, odd rows via `check_second`. See
//! TRUTH-TABLE.md for the v0.13 / v0.14 demo scope.

#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[inline]
fn is_valid(s: &[u8]) -> bool {
    !s.is_empty() && !s.contains(&b' ') && (s.first() == Some(&b'/') || !s.contains(&b':'))
}

#[inline]
fn check_first(s: &[u8]) -> bool {
    is_valid(s)
}

#[inline]
fn check_second(s: &[u8]) -> bool {
    is_valid(s)
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_0() -> i32 {
    check_first(b"") as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_1() -> i32 {
    check_first(b"x y") as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_2() -> i32 {
    check_first(b"/abs") as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_3() -> i32 {
    check_first(b"abs:80") as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_4() -> i32 {
    check_second(b"") as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_5() -> i32 {
    check_second(b"x y") as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_6() -> i32 {
    check_second(b"/abs") as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_7() -> i32 {
    check_second(b"abs:80") as i32
}
