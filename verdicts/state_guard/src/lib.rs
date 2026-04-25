//! Verdict: 4-condition AND chain — state-machine transition guard.
//!
//! Modelled after a TLS handshake "can advance to FINISHED" check.
//! Decision: `client_hello_received && server_hello_sent && cert_sent &&
//! key_exchange_received`.
//!
//! Conditions: c1..c4 = each flag.
//!
//! Stresses **deep short-circuit AND chains** — the 4-condition AND is
//! where eager-evaluation primitives (Option A in DEC-013) would force
//! evaluation of conditions that real Rust code skips.
//!
//! See `TRUTH-TABLE.md`.

#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

struct HandshakeState {
    client_hello_received: bool,
    server_hello_sent: bool,
    cert_sent: bool,
    key_exchange_received: bool,
}

#[inline(never)]
fn can_advance_to_finished(s: &HandshakeState) -> bool {
    s.client_hello_received
        && s.server_hello_sent
        && s.cert_sent
        && s.key_exchange_received
}

const fn st(c1: bool, c2: bool, c3: bool, c4: bool) -> HandshakeState {
    HandshakeState {
        client_hello_received: c1,
        server_hello_sent: c2,
        cert_sent: c3,
        key_exchange_received: c4,
    }
}

// row 0: c1=F → F. c2..c4 short-circuited.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_0() -> i32 {
    can_advance_to_finished(&st(false, true, true, true)) as i32
}

// row 1: c1=T, c2=F → F. c3,c4 short-circuited.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_1() -> i32 {
    can_advance_to_finished(&st(true, false, true, true)) as i32
}

// row 2: c1=T, c2=T, c3=F → F. c4 short-circuited.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_2() -> i32 {
    can_advance_to_finished(&st(true, true, false, true)) as i32
}

// row 3: c1..c3=T, c4=F → F.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_3() -> i32 {
    can_advance_to_finished(&st(true, true, true, false)) as i32
}

// row 4: all T → T.
#[unsafe(no_mangle)]
pub extern "C" fn run_row_4() -> i32 {
    can_advance_to_finished(&st(true, true, true, true)) as i32
}
