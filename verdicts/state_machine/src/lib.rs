//! Verdict: TLS 1.3 handshake state machine.
//!
//! The transition predicate `can_advance_to(state, next, ctx)` returns
//! `true` iff the state machine may legally move from `state` to `next`
//! given the runtime context flags. Each `run_row_<n>` exports a
//! distinct transition attempt — valid forward steps, blocked attempts
//! (missing prerequisite), malformed transitions (skipping intermediate
//! states), and explicit error transitions to `Failed`.
//!
//! The predicate is intentionally compound so witness can reconstruct
//! genuinely interesting MC/DC decisions:
//!
//! ```text
//!   can_advance_to(state, next, ctx) =
//!       valid_pair(state, next)
//!       && (state != Failed)
//!       && (state != Established || next == Failed)
//!       && (next != EncryptedExtensions || ctx.have_keys)
//!       && (next != CertSent          || ctx.cert_loaded)
//!       && (next != CertVerifySent    || (ctx.cert_loaded && ctx.have_keys))
//!       && (next != FinishedSent      || ctx.transcript_hash_ok)
//!       && (next != Established       || (ctx.peer_finished && ctx.transcript_hash_ok))
//! ```
//!
//! Boolean return so witness records a binary outcome per row.

#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum State {
    Init = 0,
    ClientHelloReceived = 1,
    ServerHelloSent = 2,
    EncryptedExtensions = 3,
    CertSent = 4,
    CertVerifySent = 5,
    FinishedSent = 6,
    Established = 7,
    Failed = 8,
}

#[derive(Copy, Clone)]
pub struct Ctx {
    pub have_keys: bool,
    pub cert_loaded: bool,
    pub transcript_hash_ok: bool,
    pub peer_finished: bool,
}

impl Ctx {
    pub const fn empty() -> Self {
        Ctx {
            have_keys: false,
            cert_loaded: false,
            transcript_hash_ok: false,
            peer_finished: false,
        }
    }
}

/// Returns true iff `state -> next` is a legal forward edge in the TLS 1.3
/// handshake graph. Any transition into `Failed` from a non-terminal state
/// is allowed (error path). All other steps must increment by exactly one
/// stage along the canonical path.
#[inline(never)]
fn valid_pair(state: State, next: State) -> bool {
    if next == State::Failed && state != State::Failed {
        return true;
    }
    let s = state as u8;
    let n = next as u8;
    // Established is terminal except for failure (handled above).
    if state == State::Established {
        return false;
    }
    // Failed is terminal in all directions.
    if state == State::Failed {
        return false;
    }
    // Otherwise must advance by exactly one and stay within 0..=7.
    n == s + 1 && n <= State::Established as u8
}

#[inline(never)]
pub fn can_advance_to(state: State, next: State, ctx: Ctx) -> bool {
    valid_pair(state, next)
        && (state as u8 != State::Failed as u8)
        && (state != State::Established || next == State::Failed)
        && (next != State::EncryptedExtensions || ctx.have_keys)
        && (next != State::CertSent || ctx.cert_loaded)
        && (next != State::CertVerifySent || (ctx.cert_loaded && ctx.have_keys))
        && (next != State::FinishedSent || ctx.transcript_hash_ok)
        && (next != State::Established
            || (ctx.peer_finished && ctx.transcript_hash_ok))
}

#[inline(always)]
fn full_ctx() -> Ctx {
    Ctx {
        have_keys: true,
        cert_loaded: true,
        transcript_hash_ok: true,
        peer_finished: true,
    }
}

// ---------------------------------------------------------------------------
// Test rows — 27 transition attempts.
// ---------------------------------------------------------------------------

// --- happy-path forward edges with a fully populated context. ---

#[unsafe(no_mangle)]
pub extern "C" fn run_row_0() -> i32 {
    can_advance_to(State::Init, State::ClientHelloReceived, full_ctx()) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_1() -> i32 {
    can_advance_to(
        State::ClientHelloReceived,
        State::ServerHelloSent,
        full_ctx(),
    ) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_2() -> i32 {
    can_advance_to(
        State::ServerHelloSent,
        State::EncryptedExtensions,
        full_ctx(),
    ) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_3() -> i32 {
    can_advance_to(State::EncryptedExtensions, State::CertSent, full_ctx()) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_4() -> i32 {
    can_advance_to(State::CertSent, State::CertVerifySent, full_ctx()) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_5() -> i32 {
    can_advance_to(State::CertVerifySent, State::FinishedSent, full_ctx()) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_6() -> i32 {
    can_advance_to(State::FinishedSent, State::Established, full_ctx()) as i32
}

// --- blocked: prerequisites missing in ctx. ---

#[unsafe(no_mangle)]
pub extern "C" fn run_row_7() -> i32 {
    // EncryptedExtensions without have_keys.
    let mut ctx = full_ctx();
    ctx.have_keys = false;
    can_advance_to(State::ServerHelloSent, State::EncryptedExtensions, ctx) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_8() -> i32 {
    // CertSent without cert_loaded.
    let mut ctx = full_ctx();
    ctx.cert_loaded = false;
    can_advance_to(State::EncryptedExtensions, State::CertSent, ctx) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_9() -> i32 {
    // CertVerifySent with cert_loaded but no have_keys.
    let mut ctx = full_ctx();
    ctx.have_keys = false;
    can_advance_to(State::CertSent, State::CertVerifySent, ctx) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_10() -> i32 {
    // CertVerifySent with have_keys but no cert_loaded.
    let mut ctx = full_ctx();
    ctx.cert_loaded = false;
    can_advance_to(State::CertSent, State::CertVerifySent, ctx) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_11() -> i32 {
    // FinishedSent without transcript_hash_ok.
    let mut ctx = full_ctx();
    ctx.transcript_hash_ok = false;
    can_advance_to(State::CertVerifySent, State::FinishedSent, ctx) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_12() -> i32 {
    // Established without peer_finished.
    let mut ctx = full_ctx();
    ctx.peer_finished = false;
    can_advance_to(State::FinishedSent, State::Established, ctx) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_13() -> i32 {
    // Established without transcript_hash_ok.
    let mut ctx = full_ctx();
    ctx.transcript_hash_ok = false;
    can_advance_to(State::FinishedSent, State::Established, ctx) as i32
}

// --- malformed: state-graph violations. ---

#[unsafe(no_mangle)]
pub extern "C" fn run_row_14() -> i32 {
    // Skip a stage: ServerHelloSent -> CertSent (skips EncryptedExtensions).
    can_advance_to(State::ServerHelloSent, State::CertSent, full_ctx()) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_15() -> i32 {
    // Backward edge: CertSent -> EncryptedExtensions.
    can_advance_to(State::CertSent, State::EncryptedExtensions, full_ctx()) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_16() -> i32 {
    // Self-loop.
    can_advance_to(State::CertSent, State::CertSent, full_ctx()) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_17() -> i32 {
    // Init -> Established (skip everything).
    can_advance_to(State::Init, State::Established, full_ctx()) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_18() -> i32 {
    // After Failed: any forward step rejected.
    can_advance_to(State::Failed, State::Established, full_ctx()) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_19() -> i32 {
    // After Established: forward forbidden (only Failed allowed).
    can_advance_to(State::Established, State::ClientHelloReceived, full_ctx()) as i32
}

// --- error transitions: any non-terminal state may go to Failed. ---

#[unsafe(no_mangle)]
pub extern "C" fn run_row_20() -> i32 {
    can_advance_to(State::Init, State::Failed, Ctx::empty()) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_21() -> i32 {
    can_advance_to(
        State::ClientHelloReceived,
        State::Failed,
        Ctx::empty(),
    ) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_22() -> i32 {
    can_advance_to(State::CertVerifySent, State::Failed, Ctx::empty()) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_23() -> i32 {
    // Established -> Failed: allowed (special case).
    can_advance_to(State::Established, State::Failed, Ctx::empty()) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_24() -> i32 {
    // Failed -> Failed: rejected (terminal).
    can_advance_to(State::Failed, State::Failed, Ctx::empty()) as i32
}

// --- race / late-context conditions. ---

#[unsafe(no_mangle)]
pub extern "C" fn run_row_25() -> i32 {
    // peer_finished arrives but transcript_hash_ok still false (race).
    let ctx = Ctx {
        have_keys: true,
        cert_loaded: true,
        transcript_hash_ok: false,
        peer_finished: true,
    };
    can_advance_to(State::FinishedSent, State::Established, ctx) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_26() -> i32 {
    // Empty ctx, valid graph step — every guard short-circuits to false.
    can_advance_to(
        State::ServerHelloSent,
        State::EncryptedExtensions,
        Ctx::empty(),
    ) as i32
}
