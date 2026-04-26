//! Witness v0.7 demo — httparse driver.
//!
//! Each `run_row_<n>` export feeds a different HTTP/1.x request shape
//! into [`httparse::Request::parse`]. witness instruments the dependent
//! httparse crate's Wasm output and captures per-row condition vectors
//! across all the surviving br_ifs in httparse's parser internals
//! (likely several hundred decisions across method-classification,
//! header parsing, version validation, SIMD byte-search loops in
//! memchr-equivalent code, etc.).
//!
//! The runner invokes each `run_row_<n>` once. The MC/DC report shows
//! which httparse decisions are exercised by which test rows, with
//! gap-closure recommendations for un-witnessed conditions. The
//! traceability matrix in the v0.6.5+ compliance bundle correlates
//! the captured decisions back to the rivet artefact graph.

#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

/// Returns 1 iff the buffer parsed to a complete request, 0 otherwise.
/// Boolean return so the witness runner captures a binary outcome per
/// row — a prerequisite for MC/DC pair-finding.
#[inline(never)]
fn parse_request(buf: &[u8]) -> i32 {
    let mut headers = [httparse::EMPTY_HEADER; 16];
    let mut req = httparse::Request::new(&mut headers);
    matches!(req.parse(buf), Ok(httparse::Status::Complete(_))) as i32
}

#[inline(never)]
fn parse_response(buf: &[u8]) -> i32 {
    let mut headers = [httparse::EMPTY_HEADER; 16];
    let mut resp = httparse::Response::new(&mut headers);
    matches!(resp.parse(buf), Ok(httparse::Status::Complete(_))) as i32
}

// ---------------------------------------------------------------------------
// Request shapes — exercise method dispatch, path parsing, header parsing,
// version classification.
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn run_row_0() -> i32 {
    // Minimal GET with one header.
    parse_request(b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_1() -> i32 {
    // POST with multiple headers including content-length.
    parse_request(
        b"POST /submit HTTP/1.1\r\n\
        Host: api.example.com\r\n\
        Content-Type: application/json\r\n\
        Content-Length: 42\r\n\
        Accept: application/json\r\n\
        \r\n",
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_2() -> i32 {
    // HTTP/1.0 with rare method.
    parse_request(b"OPTIONS * HTTP/1.0\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_3() -> i32 {
    // HEAD with header-folding-style whitespace (legal in some flavours).
    parse_request(
        b"HEAD /resource HTTP/1.1\r\n\
        Host:    api.example.com\r\n\
        User-Agent:\twget/1.21.4\r\n\
        \r\n",
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_4() -> i32 {
    // Connect (proxy CONNECT method).
    parse_request(b"CONNECT proxy.example.com:443 HTTP/1.1\r\nHost: proxy.example.com:443\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_5() -> i32 {
    // Long URI — exercises path-parsing loops.
    parse_request(
        b"GET /v1/api/users/12345/posts?page=2&limit=50&sort=desc&filter=active HTTP/1.1\r\n\
        Host: api.example.com\r\n\
        Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.payload.sig\r\n\
        \r\n",
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_6() -> i32 {
    // Partial — only the first line, no \r\n\r\n. Returns Partial.
    parse_request(b"GET /partial HTTP/1.1\r\nHost: example.com\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_7() -> i32 {
    // Malformed — missing space after method. Should error.
    parse_request(b"GET/ HTTP/1.1\r\nHost: example.com\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_8() -> i32 {
    // Empty buffer. Partial (nothing to parse yet).
    parse_request(b"")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_9() -> i32 {
    // Custom method (PATCH).
    parse_request(b"PATCH /users/1 HTTP/1.1\r\nHost: api.example.com\r\n\r\n")
}

// ---------------------------------------------------------------------------
// Response shapes — exercise status code parsing, reason phrases.
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn run_row_10() -> i32 {
    parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_11() -> i32 {
    parse_response(b"HTTP/1.1 404 Not Found\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_12() -> i32 {
    // Status without reason phrase.
    parse_response(b"HTTP/1.1 200 \r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_13() -> i32 {
    // 1xx status code.
    parse_response(b"HTTP/1.1 100 Continue\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_14() -> i32 {
    // 5xx with custom reason.
    parse_response(b"HTTP/1.1 503 Service Unavailable\r\nRetry-After: 30\r\n\r\n")
}
