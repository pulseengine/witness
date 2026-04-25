//! Verdict: real-world anchor — URL authority validator (RFC 3986 shape).
//!
//! Decision: `!s.is_empty() && !s.contains(b' ') && !s.contains(b'@')
//! && (s.first() == Some(&b'[') || !s.contains(b':'))`
//!
//! The check rejects empty / space-bearing / userinfo-bearing authority
//! strings, then permits a colon **only when the host begins with `[`**
//! (the `[IPv6]:port` syntax) — otherwise a bare colon means a port,
//! which a strict authority parser rejects.
//!
//! This is the suite's **non-synthetic anchor**. Every other verdict is
//! a textbook example; this one is a real predicate from a real domain
//! (URL parsing). When witness reports MC/DC correctly here, the
//! "we work on real code" claim survives review.
//!
//! Conditions:
//!   c1 = !s.is_empty()
//!   c2 = !s.contains(b' ')
//!   c3 = !s.contains(b'@')
//!   c4 = s.first() == Some(&b'[')
//!   c5 = !s.contains(b':')
//!
//! Note: c4 || c5 is the inner OR. When c4=T, c5 short-circuits.
//!
//! See `TRUTH-TABLE.md`.

#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[inline(never)]
fn is_valid_authority(s: &[u8]) -> bool {
    !s.is_empty()
        && !s.contains(&b' ')
        && !s.contains(&b'@')
        && (s.first() == Some(&b'[') || !s.contains(&b':'))
}

// row 0: empty — c1=F → F.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_0() -> i32 {
    is_valid_authority(b"") as i32
}

// row 1: "x y" — c1=T, c2=F (has space) → F.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_1() -> i32 {
    is_valid_authority(b"x y") as i32
}

// row 2: "u@h" — c1=T, c2=T, c3=F (has '@') → F.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_2() -> i32 {
    is_valid_authority(b"u@h") as i32
}

// row 3: "h:80" — c1..c3=T, c4=F (no '['), c5=F (has ':') → F.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_3() -> i32 {
    is_valid_authority(b"h:80") as i32
}

// row 4: "h" — c1..c3=T, c4=F, c5=T → T.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_4() -> i32 {
    is_valid_authority(b"h") as i32
}

// row 5: "[fe80::]" — c1..c3=T, c4=T (starts with '['). c5 short-circuited. → T.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_5() -> i32 {
    is_valid_authority(b"[fe80::]") as i32
}
