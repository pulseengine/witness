//! Verdict: hand-rolled subset JSON parser (real-application fixture).
//!
//! Supports:
//!   - Top-level object with string keys; values may be number, string,
//!     bool, null, **one-level-nested object**, or **one-level array of
//!     primitives**.
//!   - Top-level array of primitives.
//!   - Whitespace skipping at structural positions.
//!   - String escape handling: `\"`, `\\`, `\/`, `\n`, `\r`, `\t`.
//!
//! Intentionally *not* full RFC 8259: no scientific-notation numbers, no
//! \uXXXX, no nested arrays of objects, no streaming. The point is to
//! have multiple compound predicates witness can reconstruct, not to
//! ship a real JSON library.
//!
//! Each `run_row_<n>` returns 1 iff `parse(buf)` returns `Some(end)`
//! with `end == buf.len()`. 0 otherwise (malformed, partial, etc.).

#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[inline(never)]
fn skip_ws(buf: &[u8], mut i: usize) -> usize {
    while i < buf.len() {
        let b = buf[i];
        if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
            i += 1;
        } else {
            break;
        }
    }
    i
}

#[inline(never)]
fn parse_string(buf: &[u8], mut i: usize) -> Option<usize> {
    if i >= buf.len() || buf[i] != b'"' {
        return None;
    }
    i += 1;
    while i < buf.len() {
        let b = buf[i];
        if b == b'"' {
            return Some(i + 1);
        }
        if b == b'\\' {
            // escape: must have a following byte and be one of the
            // accepted escape codes.
            if i + 1 >= buf.len() {
                return None;
            }
            let esc = buf[i + 1];
            let ok = esc == b'"'
                || esc == b'\\'
                || esc == b'/'
                || esc == b'n'
                || esc == b'r'
                || esc == b't';
            if !ok {
                return None;
            }
            i += 2;
            continue;
        }
        // control byte rejection.
        if b < 0x20 {
            return None;
        }
        i += 1;
    }
    None // unterminated
}

#[inline(never)]
fn parse_number(buf: &[u8], mut i: usize) -> Option<usize> {
    let start = i;
    if i < buf.len() && buf[i] == b'-' {
        i += 1;
    }
    let digits_start = i;
    while i < buf.len() {
        let b = buf[i];
        if b >= b'0' && b <= b'9' {
            i += 1;
        } else {
            break;
        }
    }
    if i == digits_start {
        return None;
    }
    // single fractional part allowed.
    if i < buf.len() && buf[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < buf.len() {
            let b = buf[i];
            if b >= b'0' && b <= b'9' {
                i += 1;
            } else {
                break;
            }
        }
        if i == frac_start {
            return None;
        }
    }
    if i == start {
        None
    } else {
        Some(i)
    }
}

#[inline(never)]
fn parse_keyword(buf: &[u8], i: usize, kw: &[u8]) -> Option<usize> {
    if i + kw.len() > buf.len() {
        return None;
    }
    let mut j = 0;
    while j < kw.len() {
        if buf[i + j] != kw[j] {
            return None;
        }
        j += 1;
    }
    Some(i + kw.len())
}

#[inline(never)]
fn parse_primitive(buf: &[u8], i: usize) -> Option<usize> {
    if i >= buf.len() {
        return None;
    }
    let b = buf[i];
    if b == b'"' {
        parse_string(buf, i)
    } else if b == b't' {
        parse_keyword(buf, i, b"true")
    } else if b == b'f' {
        parse_keyword(buf, i, b"false")
    } else if b == b'n' {
        parse_keyword(buf, i, b"null")
    } else if b == b'-' || (b >= b'0' && b <= b'9') {
        parse_number(buf, i)
    } else {
        None
    }
}

#[inline(never)]
fn parse_array_of_primitives(buf: &[u8], mut i: usize) -> Option<usize> {
    if i >= buf.len() || buf[i] != b'[' {
        return None;
    }
    i += 1;
    i = skip_ws(buf, i);
    if i < buf.len() && buf[i] == b']' {
        return Some(i + 1);
    }
    loop {
        i = skip_ws(buf, i);
        i = parse_primitive(buf, i)?;
        i = skip_ws(buf, i);
        if i >= buf.len() {
            return None;
        }
        let b = buf[i];
        if b == b',' {
            i += 1;
            continue;
        }
        if b == b']' {
            return Some(i + 1);
        }
        return None;
    }
}

#[inline(never)]
fn parse_value(buf: &[u8], i: usize, allow_nested: bool) -> Option<usize> {
    if i >= buf.len() {
        return None;
    }
    let b = buf[i];
    if b == b'{' && allow_nested {
        parse_object(buf, i, false)
    } else if b == b'[' {
        parse_array_of_primitives(buf, i)
    } else {
        parse_primitive(buf, i)
    }
}

#[inline(never)]
fn parse_object(buf: &[u8], mut i: usize, allow_nested_inside: bool) -> Option<usize> {
    if i >= buf.len() || buf[i] != b'{' {
        return None;
    }
    i += 1;
    i = skip_ws(buf, i);
    if i < buf.len() && buf[i] == b'}' {
        return Some(i + 1);
    }
    loop {
        i = skip_ws(buf, i);
        i = parse_string(buf, i)?;
        i = skip_ws(buf, i);
        if i >= buf.len() || buf[i] != b':' {
            return None;
        }
        i += 1;
        i = skip_ws(buf, i);
        i = parse_value(buf, i, allow_nested_inside)?;
        i = skip_ws(buf, i);
        if i >= buf.len() {
            return None;
        }
        let b = buf[i];
        if b == b',' {
            i += 1;
            continue;
        }
        if b == b'}' {
            return Some(i + 1);
        }
        return None;
    }
}

#[inline(never)]
fn parse(buf: &[u8]) -> Option<usize> {
    let mut i = skip_ws(buf, 0);
    if i >= buf.len() {
        return None;
    }
    let b = buf[i];
    let end = if b == b'{' {
        parse_object(buf, i, true)?
    } else if b == b'[' {
        parse_array_of_primitives(buf, i)?
    } else {
        return None;
    };
    let i = skip_ws(buf, end);
    if i == buf.len() { Some(i) } else { None }
}

#[inline(never)]
fn ok(buf: &[u8]) -> i32 {
    matches!(parse(buf), Some(_)) as i32
}

// ---------------------------------------------------------------------------
// Test rows — 28 representative JSON-ish byte slices.
// ---------------------------------------------------------------------------

// --- valid objects ---

#[unsafe(no_mangle)]
pub extern "C" fn run_row_0() -> i32 {
    ok(b"{}")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_1() -> i32 {
    ok(b"{\"k\":1}")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_2() -> i32 {
    ok(b"{\"k\":\"v\"}")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_3() -> i32 {
    ok(b"{\"a\":true,\"b\":false,\"c\":null}")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_4() -> i32 {
    // multiple keys, mixed primitives + whitespace.
    ok(b"{ \"a\" : 1 , \"b\" : -2.5 , \"c\" : \"hi\" }")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_5() -> i32 {
    // one-level nested object.
    ok(b"{\"outer\":{\"inner\":42}}")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_6() -> i32 {
    // nested object with multiple keys.
    ok(b"{\"o\":{\"a\":1,\"b\":2}}")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_7() -> i32 {
    // value is array of primitives.
    ok(b"{\"arr\":[1,2,3]}")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_8() -> i32 {
    // empty array as value.
    ok(b"{\"arr\":[]}")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_9() -> i32 {
    // string with all accepted escapes.
    ok(b"{\"s\":\"a\\\"b\\\\c\\/d\\ne\\rf\\tg\"}")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_10() -> i32 {
    // leading + trailing whitespace at top level.
    ok(b"   \n\t {\"k\":1}\r\n   ")
}

// --- valid arrays ---

#[unsafe(no_mangle)]
pub extern "C" fn run_row_11() -> i32 {
    ok(b"[]")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_12() -> i32 {
    ok(b"[1,2,3,4,5]")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_13() -> i32 {
    ok(b"[\"a\",\"b\",null,true,false,-1.25]")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_14() -> i32 {
    // whitespace inside array.
    ok(b"[ 1 , 2 , 3 ]")
}

// --- malformed ---

#[unsafe(no_mangle)]
pub extern "C" fn run_row_15() -> i32 {
    // missing closing brace.
    ok(b"{\"k\":1")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_16() -> i32 {
    // missing opening brace.
    ok(b"\"k\":1}")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_17() -> i32 {
    // unterminated string.
    ok(b"{\"k\":\"oops}")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_18() -> i32 {
    // invalid escape \q.
    ok(b"{\"k\":\"a\\qb\"}")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_19() -> i32 {
    // missing colon between key and value.
    ok(b"{\"k\" 1}")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_20() -> i32 {
    // trailing comma.
    ok(b"{\"k\":1,}")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_21() -> i32 {
    // garbage after valid object.
    ok(b"{\"k\":1}garbage")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_22() -> i32 {
    // bare keyword at top level — not object/array.
    ok(b"true")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_23() -> i32 {
    // empty buffer.
    ok(b"")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_24() -> i32 {
    // object with non-string key.
    ok(b"{1:2}")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_25() -> i32 {
    // doubly-nested object (we forbid: array_of_primitives only, and
    // parse_object with allow_nested_inside=false at level 2).
    ok(b"{\"a\":{\"b\":{\"c\":1}}}")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_26() -> i32 {
    // number with trailing dot.
    ok(b"{\"k\":1.}")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_27() -> i32 {
    // control byte in string body (raw \n, not escaped).
    ok(b"{\"k\":\"a\nb\"}")
}
