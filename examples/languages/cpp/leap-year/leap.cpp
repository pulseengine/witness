// C++ — leap-year MC/DC probe.
//
// Same predicate as the C/Rust/Zig/Go fixtures. C++ via wasi-sdk
// gives us two angles the C fixture can't:
//   1. Template monomorphisation produces inline chains that
//      v0.14's chain tracker should expose (`isLeap<T>()`).
//   2. `constexpr` evaluation can fold the predicate at compile
//      time; using `volatile` defeats that and forces runtime
//      branches.
//
// Lowering expectation: clang++ lowers `&&`/`||` to `if/else` +
// 1 `br_if` per source decision (same as clang for C), so v0.19's
// IfThen clustering is load-bearing here.

#include <cstdint>
#include <cstdio>

volatile std::uint32_t year_input;

// Template + noinline keeps the function present so we can verify
// instantiation and inline-chain tracking on its call sites.
template <typename T>
[[gnu::noinline]] static bool leap_year(T y) {
    return (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
}

extern "C" {

__attribute__((export_name("run_row_0")))
bool run_row_0() { year_input = 2001; return leap_year<std::uint32_t>(year_input); }

__attribute__((export_name("run_row_1")))
bool run_row_1() { year_input = 2004; return leap_year<std::uint32_t>(year_input); }

__attribute__((export_name("run_row_2")))
bool run_row_2() { year_input = 2100; return leap_year<std::uint32_t>(year_input); }

__attribute__((export_name("run_row_3")))
bool run_row_3() { year_input = 2000; return leap_year<std::uint32_t>(year_input); }

}  // extern "C"

int main() {
    std::printf("rows: %d %d %d %d\n", run_row_0(), run_row_1(), run_row_2(), run_row_3());
    return 0;
}
