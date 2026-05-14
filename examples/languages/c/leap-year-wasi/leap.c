// Companion to ../leap-year/leap.c. Same predicate, but built with
// wasi-sdk targeting wasm32-wasi (with preview1 runtime symbols
// available). wasi-sdk's clang+wasm-ld pair preserves the DWARF
// line program through linking, where wasm32-unknown-unknown drops
// it — see ../leap-year/README.md for the upstream gap.
//
// `_start` is the wasi entry point; we invoke the four predicate
// rows from it so witness can run the module under `wasmtime`-style
// runtimes via `witness run --invoke run_row_N`.

#include <stdio.h>

volatile unsigned year_input;

static int leap_year(unsigned y) {
    return (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
}

__attribute__((export_name("run_row_0")))
int run_row_0(void) { year_input = 2001; return leap_year(year_input); }

__attribute__((export_name("run_row_1")))
int run_row_1(void) { year_input = 2004; return leap_year(year_input); }

__attribute__((export_name("run_row_2")))
int run_row_2(void) { year_input = 2100; return leap_year(year_input); }

__attribute__((export_name("run_row_3")))
int run_row_3(void) { year_input = 2000; return leap_year(year_input); }

int main(void) {
    int r0 = run_row_0();
    int r1 = run_row_1();
    int r2 = run_row_2();
    int r3 = run_row_3();
    printf("rows: %d %d %d %d\n", r0, r1, r2, r3);
    return 0;
}
