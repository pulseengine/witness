//! Verdict: base64 (v0.22) decoder driven by 24 rows.
//!
//! Exercises the RFC 4648 decoder through:
//! - Valid standard encoding (padded and unpadded)
//! - URL-safe alphabet
//! - Malformed input (invalid characters, incorrect padding, truncated)
//! - Edge cases (empty, single-char, all-pad)
//!
//! The base64 crate is a small, well-known Rust library with multiple
//! short-circuiting decisions inside its decode loop. Witness reconstructs
//! decisions across the engine + alphabet code paths.

#![no_std]

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// Output buffer the harness writes into. 64 bytes is enough for every
// row's decoded output.
const OUT_CAP: usize = 64;
static mut OUT_BUF: [u8; OUT_CAP] = [0u8; OUT_CAP];

#[inline(never)]
fn try_decode(engine: &impl Engine, input: &[u8]) -> i32 {
    let buf = unsafe { &mut *core::ptr::addr_of_mut!(OUT_BUF) };
    match engine.decode_slice(input, buf) {
        Ok(n) => n as i32,
        Err(_) => -1,
    }
}

// Define each row as: pick an engine, feed input, return decoded byte
// count (or -1 on error).

#[unsafe(no_mangle)] pub extern "C" fn run_row_0() -> i32 { try_decode(&STANDARD, b"") }
#[unsafe(no_mangle)] pub extern "C" fn run_row_1() -> i32 { try_decode(&STANDARD, b"YQ==") }
#[unsafe(no_mangle)] pub extern "C" fn run_row_2() -> i32 { try_decode(&STANDARD, b"YWI=") }
#[unsafe(no_mangle)] pub extern "C" fn run_row_3() -> i32 { try_decode(&STANDARD, b"YWJj") }
#[unsafe(no_mangle)] pub extern "C" fn run_row_4() -> i32 { try_decode(&STANDARD, b"YWJjZA==") }
#[unsafe(no_mangle)] pub extern "C" fn run_row_5() -> i32 { try_decode(&STANDARD, b"SGVsbG8sIFdvcmxkIQ==") }
#[unsafe(no_mangle)] pub extern "C" fn run_row_6() -> i32 { try_decode(&STANDARD, b"YQ") }     // missing padding
#[unsafe(no_mangle)] pub extern "C" fn run_row_7() -> i32 { try_decode(&STANDARD, b"Y@==") }   // invalid char
#[unsafe(no_mangle)] pub extern "C" fn run_row_8() -> i32 { try_decode(&STANDARD, b"=AAA") }   // misplaced padding
#[unsafe(no_mangle)] pub extern "C" fn run_row_9() -> i32 { try_decode(&STANDARD, b"AA=A") }   // padding in middle
#[unsafe(no_mangle)] pub extern "C" fn run_row_10() -> i32 { try_decode(&STANDARD_NO_PAD, b"YQ") }
#[unsafe(no_mangle)] pub extern "C" fn run_row_11() -> i32 { try_decode(&STANDARD_NO_PAD, b"YWI") }
#[unsafe(no_mangle)] pub extern "C" fn run_row_12() -> i32 { try_decode(&STANDARD_NO_PAD, b"YWJj") }
#[unsafe(no_mangle)] pub extern "C" fn run_row_13() -> i32 { try_decode(&STANDARD_NO_PAD, b"YWJjZA") }
#[unsafe(no_mangle)] pub extern "C" fn run_row_14() -> i32 { try_decode(&STANDARD_NO_PAD, b"SGVsbG8") }
#[unsafe(no_mangle)] pub extern "C" fn run_row_15() -> i32 { try_decode(&STANDARD_NO_PAD, b"") }
#[unsafe(no_mangle)] pub extern "C" fn run_row_16() -> i32 { try_decode(&URL_SAFE, b"YWJj") }
#[unsafe(no_mangle)] pub extern "C" fn run_row_17() -> i32 { try_decode(&URL_SAFE, b"YWJjZA==") }
#[unsafe(no_mangle)] pub extern "C" fn run_row_18() -> i32 { try_decode(&URL_SAFE, b"-_8=") }     // url-safe chars
#[unsafe(no_mangle)] pub extern "C" fn run_row_19() -> i32 { try_decode(&URL_SAFE, b"+/8=") }     // standard chars in url-safe (rejected)
#[unsafe(no_mangle)] pub extern "C" fn run_row_20() -> i32 { try_decode(&URL_SAFE_NO_PAD, b"YWJj") }
#[unsafe(no_mangle)] pub extern "C" fn run_row_21() -> i32 { try_decode(&URL_SAFE_NO_PAD, b"-_8") }
#[unsafe(no_mangle)] pub extern "C" fn run_row_22() -> i32 { try_decode(&URL_SAFE_NO_PAD, b"YWJjZA") }
#[unsafe(no_mangle)] pub extern "C" fn run_row_23() -> i32 { try_decode(&URL_SAFE_NO_PAD, b"!@#$") } // garbage
