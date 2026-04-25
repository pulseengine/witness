//! Verdict: 5-condition AND — safety-envelope check.
//!
//! Decision: `temp < T_MAX && press > P_MIN && rpm < RPM_MAX && !fault
//! && mode == MODE_ACTIVE`.
//!
//! Modelled after a real-world automotive safety envelope guard. Five
//! conditions in a single decision exercises **scaling beyond LLVM's
//! 6-condition cap** — Clang and rustc-mcdc both refuse to instrument
//! decisions of more than 6 conditions because their bitmap encoder
//! caps at `2^N` for `N <= 6`. Witness uses a trace buffer (DEC-013)
//! with no encoder constraint and supports decisions of any size.
//!
//! See `TRUTH-TABLE.md`.

#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

struct Telemetry {
    temp: i32,
    pressure: i32,
    rpm: u32,
    fault: bool,
    mode: u8,
}

const T_MAX: i32 = 100;
const P_MIN: i32 = 50;
const RPM_MAX: u32 = 5000;
const MODE_ACTIVE: u8 = 1;

#[inline(never)]
fn within_envelope(t: &Telemetry) -> bool {
    t.temp < T_MAX
        && t.pressure > P_MIN
        && t.rpm < RPM_MAX
        && !t.fault
        && t.mode == MODE_ACTIVE
}

const fn tel(temp: i32, pressure: i32, rpm: u32, fault: bool, mode: u8) -> Telemetry {
    Telemetry { temp, pressure, rpm, fault, mode }
}

// row 0: c1=F (temp too high) → F. c2-c5 short-circuited.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_0() -> i32 {
    within_envelope(&tel(200, 100, 1000, false, MODE_ACTIVE)) as i32
}

// row 1: c1=T, c2=F (pressure too low) → F.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_1() -> i32 {
    within_envelope(&tel(50, 10, 1000, false, MODE_ACTIVE)) as i32
}

// row 2: c1=T, c2=T, c3=F (rpm too high) → F.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_2() -> i32 {
    within_envelope(&tel(50, 100, 10000, false, MODE_ACTIVE)) as i32
}

// row 3: c1=T, c2=T, c3=T, c4=F (fault active) → F.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_3() -> i32 {
    within_envelope(&tel(50, 100, 1000, true, MODE_ACTIVE)) as i32
}

// row 4: c1..c4=T, c5=F (mode != active) → F.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_4() -> i32 {
    within_envelope(&tel(50, 100, 1000, false, 2)) as i32
}

// row 5: all conditions T → T.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_5() -> i32 {
    within_envelope(&tel(50, 100, 1000, false, MODE_ACTIVE)) as i32
}
