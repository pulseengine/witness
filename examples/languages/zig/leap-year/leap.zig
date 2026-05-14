// Zig — leap-year MC/DC probe.
//
// Same predicate as the C/Rust fixtures: `(y%4==0 and y%100!=0)
// or (y%400==0)`. Zig's `and`/`or` short-circuit operators lower
// to LLVM IR essentially identical to clang's, so the wasm
// emission should be `if/else` + 1 `br_if` per source decision —
// the shape v0.19's IfThen clustering targets.
//
// We use a `noinline` predicate + an exported `year_input` global
// the runner mutates between calls — that defeats Zig's constant
// folding without dragging in atomics (whose memory-ordering
// machinery dominated the branch counts in earlier iterations).

export var year_input: u32 = 0;

noinline fn leap_year(y: u32) callconv(.c) bool {
    return (y % 4 == 0 and y % 100 != 0) or (y % 400 == 0);
}

// Single entry that the runner invokes after setting `year_input`.
// Returning the predicate result keeps the call from being
// optimised to a constant.
export fn run() bool {
    return leap_year(year_input);
}
