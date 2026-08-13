#![no_std]
#[panic_handler]
fn ph(_: &core::panic::PanicInfo) -> ! { loop {} }

// A boolean decision the optimizer inlines into `run`, so the br_ifs
// physically live in `run`'s body but DWARF attributes them here via a
// DW_TAG_inlined_subroutine chain (the gale #179 shape). Two call sites
// plus extra structure push LLVM toward *scattered* address ranges
// (DW_AT_ranges into `.debug_ranges`), so this one fixture guards both
// the v4 `.debug_ranges` extraction and the code-section-base rebase.
#[inline(always)]
fn decide(a: i32, b: i32, c: i32, d: i32) -> bool {
    (a < b && b < c) || (c < d && a != d)
}

#[no_mangle]
pub extern "C" fn run(a: i32, b: i32, c: i32, d: i32) -> i32 {
    let mut acc = 0i32;
    if decide(a, b, c, d) { acc += 1; }
    if decide(d, c, b, a) { acc += 2; }
    acc
}
