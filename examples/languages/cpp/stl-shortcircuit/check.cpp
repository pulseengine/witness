// C++ STL short-circuit probe — inline-chain depth via std::any_of /
// std::all_of with lambdas.
//
// Goal: demonstrate that v0.14's inline-chain tracker captures
// multi-level STL inlines. `std::any_of` and `std::all_of` both
// short-circuit on the lambda predicate — at -O0, the predicate
// is inlined into the iterator loop, which is itself inlined
// into the caller. We expect a chain of length 2+ in
// branch_inline_chains for the predicate's br_ifs.

#include <algorithm>
#include <array>
#include <cstdio>

volatile int input_kind;

[[gnu::noinline]]
static bool any_negative(const int* arr, int n) {
    return std::any_of(arr, arr + n, [](int x) { return x < 0; });
}

[[gnu::noinline]]
static bool all_positive(const int* arr, int n) {
    return std::all_of(arr, arr + n, [](int x) { return x > 0; });
}

__attribute__((export_name("run_row_0")))
bool run_row_0() {
    input_kind = 0;
    int data[] = {1, 2, 3, 4};
    return any_negative(data, 4);
}

__attribute__((export_name("run_row_1")))
bool run_row_1() {
    input_kind = 1;
    int data[] = {1, -2, 3, 4};
    return any_negative(data, 4);
}

__attribute__((export_name("run_row_2")))
bool run_row_2() {
    input_kind = 2;
    int data[] = {1, 2, 3, 4};
    return all_positive(data, 4);
}

__attribute__((export_name("run_row_3")))
bool run_row_3() {
    input_kind = 3;
    int data[] = {1, 2, 0, 4};
    return all_positive(data, 4);
}

int main() {
    std::printf("%d %d %d %d\n", run_row_0(), run_row_1(), run_row_2(), run_row_3());
    return 0;
}
