// C++ exception handling — br_table audit probe.
//
// `try { ... } catch (T1) catch (T2) catch (T3)` lowers in wasm
// (via -fwasm-exceptions or libcxxabi unwind) to a `br_table` over
// type-index arms in the landing pad. v0.9.7+ witness clusters
// br_table arms into one Decision per (function, file, line) with
// chain_kind = Unknown — the truth-table view shows each catch arm
// must be exercised.
//
// We use distinct exception classes to keep the table dense and
// the failure modes obvious.

#include <cstdint>
#include <cstdio>

#ifndef PARSER_NO_EH
#include <stdexcept>

struct ParseError : std::runtime_error { using std::runtime_error::runtime_error; };
struct RangeError : std::runtime_error { using std::runtime_error::runtime_error; };
struct EOFError   : std::runtime_error { using std::runtime_error::runtime_error; };
#endif

volatile int input_kind;

#ifndef PARSER_NO_EH
[[gnu::noinline]]
static int parse_token(int kind) {
    switch (kind) {
        case 0: throw ParseError("bad token");
        case 1: throw RangeError("out of bounds");
        case 2: throw EOFError("unexpected end");
        default: return kind;
    }
}

// dispatch() catches each type — its landing pad becomes a br_table.
[[gnu::noinline]]
static int dispatch(int kind) {
    try {
        return parse_token(kind);
    } catch (const ParseError&) {
        return -1;
    } catch (const RangeError&) {
        return -2;
    } catch (const EOFError&) {
        return -3;
    } catch (...) {
        return -99;
    }
}
#else
// Fallback: same control-flow shape as the EH variant but via
// integer error codes, so the wasm `br_table` over `kind` still
// exercises witness's v0.9.7 br_table audit pass.
[[gnu::noinline]]
static int parse_token(int kind) {
    switch (kind) {
        case 0: return -1;  // bad token
        case 1: return -2;  // out of bounds
        case 2: return -3;  // unexpected end
        default: return kind;
    }
}

[[gnu::noinline]]
static int dispatch(int kind) { return parse_token(kind); }
#endif

__attribute__((export_name("run_parse")))    int run_parse()    { input_kind = 0; return dispatch(input_kind); }
__attribute__((export_name("run_range")))    int run_range()    { input_kind = 1; return dispatch(input_kind); }
__attribute__((export_name("run_eof")))      int run_eof()      { input_kind = 2; return dispatch(input_kind); }
__attribute__((export_name("run_ok")))       int run_ok()       { input_kind = 42; return dispatch(input_kind); }

int main() {
    std::printf("%d %d %d %d\n", run_parse(), run_range(), run_eof(), run_ok());
    return 0;
}
