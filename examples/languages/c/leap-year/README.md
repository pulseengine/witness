# C — leap-year fixture (cross-language probe)

## What this demonstrates

Witness was originally tuned to rustc-emitted wasm. This is the
first C-language probe. The fixture compiles
`(y%4==0 && y%100!=0) || (y%400==0)` with `clang --target=wasm32`
and runs it through witness's pipeline to see what works.

## How to run

```sh
./build.sh                                # produces leap.wasm
witness instrument leap.wasm -o inst.wasm # writes manifest
witness run inst.wasm \
    --invoke run_row_0 --invoke run_row_1 \
    --invoke run_row_2 --invoke run_row_3 \
    -o run.json
witness report --input run.json --format mcdc
```

## What v0.19 picks up — and the upstream blocker

Probe results against witness v0.19 (verified 2026-05-13):

| | Result |
|---|---|
| Wasm produced | ✅ 1264 bytes with `.debug_info` |
| Branches detected | ✅ 12 (4 funcs × 3 each: IfThen + IfElse + BrIf) |
| DWARF line-program rows present | ❌ **empty at `-O1`** |
| Decisions reconstructed at `-O1` | ❌ 0 (no line rows → can't resolve file/line) |
| Decisions reconstructed at `-O0` | ✅ 1 (post-v0.19 IfThen clustering) |

### Two layered issues — one fixed in v0.19, one upstream

**Issue 1 — clustering rule** (fixed in v0.19): clang lowers
`a && b` to a wasm `if/else` block followed by one `br_if` —
historically witness's clustering only paired `BrIf` entries, so
clang shapes got 1 BrIf + 1 IfThen + 1 IfElse per source decision
and never reached the `cluster.len() >= 2` gate.

v0.19 extends `decisions.rs::group_into_decisions` to cluster
`IfThen` alongside `BrIf` for decision-key purposes (the IfThen
arm is the "predicate was true" outcome, semantically equivalent
to a BrIf). `IfElse` stays excluded — it's the negation of the
same site, counting it would inflate condition counts.

**Issue 2 — empty DWARF line program at `-O1`** (upstream, not
witness): when clang force-inlines `leap_year` (it has the
`DW_AT_inline = DW_INL_inlined` flag), wasm-ld for the
`wasm32-unknown-unknown` target drops the line program rows for
the inlined function. `llvm-dwarfdump` confirms `.debug_line`
contains only the prologue (40 bytes, zero rows). Without line
rows, no branch offset resolves to a `(file, line)` tuple, and
the decision-clustering pass never runs.

### Workarounds for the upstream DWARF gap

| Workaround | Effect |
|---|---|
| Build at `-O0` | Restores line program; decision count climbs (see leap-o0 example) |
| Switch to wasi-sdk + `wasm32-wasi` | wasi-sdk's linker preserves line rows at `-O2` |
| Use `__attribute__((noinline))` on `leap_year` | Keeps the inline site, line program intact at `-O1` |
| Wait for an upstream wasm-ld fix | Tracked LLVM bug, no ETA |

The v0.19 change is load-bearing for clang/zig/TinyGo/Swift/
Kotlin probes once their DWARF makes it to the clustering pass —
those need to clear issue 2 separately (most don't, since
wasm32-wasi or wasi-sdk is the standard build path).

## Why witness still beats source-level C tools at this layer

Even though decision reconstruction is incomplete, witness
already captures **post-LLVM transformations** that source-level
tools (GCC `-fcondition-coverage`, Coveron) cannot:

- Inlining — `static int leap_year(...)` is inlined into each
  `run_row_*`. Source-level tools measure the predicate once;
  witness sees each inlined copy.
- LLVM optimisation — when `-O2` / `-O3` reshapes the predicate
  (e.g. constant folding partial conditions, hoisting a check
  out of a loop), source-level tools report decisions that may
  not exist in the emitted code. Witness reports what actually
  shipped.
- Cross-compiler agreement — the same source compiled by
  different clang/gcc/rustc versions can produce different
  wasm; witness measures the actual artefact.

The cross-language story is: **GCC `-fcondition-coverage` and
Coveron measure C/C++ source-level MC/DC; witness measures
post-codegen MC/DC on wasm regardless of source language**.
Different chain layers, additive evidence for any compliance
dossier that asks for structural-coverage proof on the
emitted artefact (DO-178C, ISO 26262 with the post-preprocessor
precedent).
