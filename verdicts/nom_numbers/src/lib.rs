//! Verdict: nom-driven decimal-integer parser (real-application fixture).
//!
//! Each `run_row_<n>` export feeds a different byte slice into a small nom
//! combinator that parses an unsigned decimal integer plus an optional
//! leading sign. Witness instruments the resulting Wasm and captures
//! per-row condition vectors across nom's compound predicates (digit
//! classification, slice bounds, accumulator overflow checks) plus the
//! inlined stdlib helpers nom pulls in (`u32::checked_mul`, byte
//! comparisons, slice indexing).
//!
//! The predicate is intentionally strict — it accepts only:
//!   - an optional single leading `-` or `+`
//!   - one or more ASCII decimal digits `0..=9`
//!   - **no** leading whitespace, `0x`/`0o` prefix, embedded space, or
//!     trailing garbage
//!   - the parsed magnitude must fit in u32; signed inputs must fit i32
//!
//! Boolean return so the witness runner records a binary outcome per row
//! — a prerequisite for MC/DC pair-finding. `1` means the buffer parsed
//! cleanly to end-of-input with the sign-and-range checks satisfied; `0`
//! means any failure (incomplete, malformed, overflow, trailing bytes).

#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

use nom::{
    bytes::complete::take_while1,
    character::is_digit,
    combinator::opt,
    sequence::pair,
    IResult,
};

#[inline(never)]
fn sign(input: &[u8]) -> IResult<&[u8], Option<u8>> {
    opt(sign_inner)(input)
}

fn sign_inner(i: &[u8]) -> IResult<&[u8], u8> {
    match i.first() {
        Some(b) if *b == b'-' || *b == b'+' => Ok((&i[1..], *b)),
        _ => Err(nom::Err::Error(nom::error::Error::new(
            i,
            nom::error::ErrorKind::Char,
        ))),
    }
}

#[inline(never)]
fn digits(input: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while1(is_digit)(input)
}

/// Returns 1 iff input is a complete signed decimal integer in i32 range
/// (or 0..=u32::MAX when unsigned). 0 otherwise.
#[inline(never)]
fn parse_int(input: &[u8]) -> i32 {
    let res: IResult<&[u8], (Option<u8>, &[u8])> = pair(sign, digits)(input);
    let (rest, (sgn, ds)) = match res {
        Ok(v) => v,
        Err(_) => return 0,
    };
    if !rest.is_empty() {
        return 0;
    }
    // Build u64 accumulator with overflow guard.
    let mut acc: u64 = 0;
    for &b in ds {
        let d = (b - b'0') as u64;
        acc = match acc.checked_mul(10) {
            Some(v) => v,
            None => return 0,
        };
        acc = match acc.checked_add(d) {
            Some(v) => v,
            None => return 0,
        };
        if acc > u32::MAX as u64 {
            return 0;
        }
    }
    match sgn {
        Some(b'-') => {
            // signed range: must fit i32
            if acc > (i32::MAX as u64) + 1 {
                0
            } else {
                1
            }
        }
        Some(b'+') | None => {
            if acc > u32::MAX as u64 { 0 } else { 1 }
        }
        Some(_) => 0,
    }
}

// ---------------------------------------------------------------------------
// Test rows — 28 representative byte slices.
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn run_row_0() -> i32 {
    // simple single digit
    parse_int(b"0")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_1() -> i32 {
    parse_int(b"1")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_2() -> i32 {
    parse_int(b"9")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_3() -> i32 {
    parse_int(b"42")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_4() -> i32 {
    parse_int(b"12345")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_5() -> i32 {
    // u32::MAX exact
    parse_int(b"4294967295")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_6() -> i32 {
    // u32::MAX + 1 — overflow
    parse_int(b"4294967296")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_7() -> i32 {
    // way over u32 — overflow during accumulator
    parse_int(b"99999999999999")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_8() -> i32 {
    // i32::MIN absolute value (2147483648) with negative sign — accepted
    parse_int(b"-2147483648")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_9() -> i32 {
    // negative beyond i32::MIN
    parse_int(b"-2147483649")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_10() -> i32 {
    // i32::MAX
    parse_int(b"2147483647")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_11() -> i32 {
    // explicit positive sign
    parse_int(b"+1")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_12() -> i32 {
    // negative single digit
    parse_int(b"-1")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_13() -> i32 {
    // leading zero — accepted (it's a digit)
    parse_int(b"0042")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_14() -> i32 {
    // multiple leading zeros
    parse_int(b"00000")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_15() -> i32 {
    // empty input — fails take_while1
    parse_int(b"")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_16() -> i32 {
    // sign only — fails take_while1
    parse_int(b"-")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_17() -> i32 {
    // sign + sign — second sign is not a digit
    parse_int(b"--1")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_18() -> i32 {
    // leading whitespace — rejected (no whitespace eater)
    parse_int(b" 42")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_19() -> i32 {
    // trailing whitespace — rest non-empty
    parse_int(b"42 ")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_20() -> i32 {
    // hex prefix — '0' is digit, then 'x' is not, leaves rest non-empty
    parse_int(b"0x1f")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_21() -> i32 {
    // octal prefix — same shape as hex, '0o' fails
    parse_int(b"0o17")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_22() -> i32 {
    // letters mixed with digits
    parse_int(b"12a34")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_23() -> i32 {
    // embedded space
    parse_int(b"12 34")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_24() -> i32 {
    // pure letters — digits combinator fails immediately
    parse_int(b"abc")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_25() -> i32 {
    // single non-digit byte
    parse_int(b"x")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_26() -> i32 {
    // very long valid number that fits u32
    parse_int(b"0000004294967295")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_27() -> i32 {
    // null byte after valid digits — trailing garbage
    parse_int(b"42\x00")
}
