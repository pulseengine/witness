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

// ---------------------------------------------------------------------------
// v0.7.5 — expanded test rows for richer MC/DC coverage. Each row
// targets a different code path in httparse / inlined stdlib code.
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn run_row_15() -> i32 {
    // PUT request with body separator.
    parse_request(b"PUT /api/items/1 HTTP/1.1\r\nContent-Length: 0\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_16() -> i32 {
    // DELETE request — exercises method-classification branches.
    parse_request(b"DELETE /api/users/42 HTTP/1.1\r\nHost: x\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_17() -> i32 {
    // Many short headers — exercises the iter loop's bound checks.
    parse_request(
        b"GET / HTTP/1.1\r\n\
        A: 1\r\nB: 2\r\nC: 3\r\nD: 4\r\nE: 5\r\nF: 6\r\nG: 7\r\nH: 8\r\n\r\n",
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_18() -> i32 {
    // Request with body bytes after header terminator (httparse parses
    // up to and including \r\n\r\n; body bytes ignored at this level).
    parse_request(b"POST /upload HTTP/1.1\r\nContent-Length: 5\r\n\r\nhello")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_19() -> i32 {
    // HTTP/1.0 — exercises version comparison branches.
    parse_request(b"GET /old HTTP/1.0\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_20() -> i32 {
    // Bad version number.
    parse_request(b"GET / HTTP/9.9\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_21() -> i32 {
    // Mixed-case method (httparse is case-sensitive — should fail).
    parse_request(b"get / HTTP/1.1\r\nHost: x\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_22() -> i32 {
    // Empty header value.
    parse_request(b"GET / HTTP/1.1\r\nX-Empty:\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_23() -> i32 {
    // Header without space after colon (whitespace-tolerant per RFC).
    parse_request(b"GET / HTTP/1.1\r\nHost:example.com\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_24() -> i32 {
    // Header value containing colons (legal in URIs).
    parse_request(
        b"GET / HTTP/1.1\r\nReferer: https://example.com:8080/path\r\n\r\n",
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_25() -> i32 {
    // Single LF instead of CRLF (some lenient parsers accept).
    parse_request(b"GET / HTTP/1.1\nHost: x\n\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_26() -> i32 {
    // Status with multi-word reason phrase.
    parse_response(b"HTTP/1.1 418 I'm a teapot\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_27() -> i32 {
    // 3xx status (redirect).
    parse_response(
        b"HTTP/1.1 301 Moved Permanently\r\nLocation: /new-path\r\n\r\n",
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_28() -> i32 {
    // 2xx with no headers.
    parse_response(b"HTTP/1.1 204 No Content\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_29() -> i32 {
    // 4xx with detailed reason.
    parse_response(b"HTTP/1.1 422 Unprocessable Entity\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_30() -> i32 {
    // Single byte (truncated request).
    parse_request(b"G")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_31() -> i32 {
    // Just a method, no URI.
    parse_request(b"GET")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_32() -> i32 {
    // Request with one byte beyond final \r\n\r\n (boundary byte).
    parse_request(b"GET / HTTP/1.1\r\n\r\nX")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_33() -> i32 {
    // Maximum reasonable header count near our 16-slot fixture cap.
    parse_request(
        b"GET / HTTP/1.1\r\n\
        H1: a\r\nH2: b\r\nH3: c\r\nH4: d\r\nH5: e\r\nH6: f\r\nH7: g\r\nH8: h\r\n\
        H9: i\r\nHA: j\r\nHB: k\r\nHC: l\r\nHD: m\r\nHE: n\r\nHF: o\r\n\r\n",
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_34() -> i32 {
    // Numeric path with query string.
    parse_request(b"GET /404 HTTP/1.1\r\nAccept: */*\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_35() -> i32 {
    // High-byte (UTF-8 in path; httparse parses bytes).
    parse_request(b"GET /\xe2\x98\x83 HTTP/1.1\r\nHost: x\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_36() -> i32 {
    // Method longer than the canonical 7-char ones.
    parse_request(b"PROPFIND /resource HTTP/1.1\r\nHost: webdav.example.com\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_37() -> i32 {
    // CR alone (malformed line ending — httparse expects CRLF).
    parse_request(b"GET / HTTP/1.1\rHost: x\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_38() -> i32 {
    // Status with no reason phrase + no header.
    parse_response(b"HTTP/1.1 200\r\n\r\n")
}

#[unsafe(no_mangle)]
pub extern "C" fn run_row_39() -> i32 {
    // 1xx informational + custom header.
    parse_response(b"HTTP/1.1 102 Processing\r\nX-Hint: please-wait\r\n\r\n")
}
