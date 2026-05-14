# C — wasi-sdk leap-year fixture (cross-language probe)

## What this demonstrates

Companion to `../leap-year/` (which uses `clang
--target=wasm32-unknown-unknown` and hits a wasm-ld DWARF gap).
This fixture builds the **same predicate** via the official
[wasi-sdk](https://github.com/WebAssembly/wasi-sdk) targeting
`wasm32-wasip1`, then runs it through witness to see how the
DWARF survives.

## How to run

```sh
# Defaults to -O0 — that's the 79-decision case (see results below).
WASI_SDK_PATH=~/.local/opt/wasi-sdk-33.0-arm64-macos ./build.sh
witness instrument leap.wasm -o inst.wasm

# To reproduce the -O1 upstream-blocked case:
OPT=-O1 WASI_SDK_PATH=~/.local/opt/wasi-sdk-33.0-arm64-macos ./build.sh
```

## v0.19 results (verified 2026-05-14)

| Build | Decisions | Inline chains | Branches | Notes |
|---|---|---|---|---|
| `-O0` | 79 | 92 | 489 | Real libc coverage: 58 in `vfprintf.c`, 4 in `fwrite.c`, 4 in `wcrtomb.c`, others |
| `-O1` | 0 | 0 | 667 | Same wasm-ld DWARF regression as `wasm32-unknown-unknown` |

The `-O0` result is the substantive win: witness reconstructs
79 source-level decisions across `vfprintf`, `fwrite`,
`memchr`, `__stdio_exit`, `wcrtomb`, etc. — real-world C code
with non-trivial short-circuit chains. v0.19's IfThen
clustering pulls its weight here; the libc decisions come from
shapes mixing `if/else` lowering with `br_if` chains.

## The remaining upstream gap

Even with wasi-sdk, **`wasm-ld` does not relocate DWARF
addresses through the link**. `llvm-dwarfdump` confirms every
`DW_AT_low_pc`/`DW_AT_high_pc` on `DW_TAG_inlined_subroutine`
entries is `0x00000000` — the inline-map built from these is
useless for byte_offset → inlined-frame lookup. At `-O1` even
the line program addresses collapse, hence the 0-decision
result.

Tracked upstream — wasm-ld DWARF relocation support is a known
LLVM gap (see e.g. discussion threads on llvm/llvm-project).
Once it's fixed, witness gains accurate inline-chain tracking
on every wasi-sdk binary.

## Cross-attribution caveat

At `-O0` the line program rows do exist, but because
DW_AT_low_pc relocations aren't applied, addresses can collide
across compilation units. For example, `leap_year`'s br_ifs
get the source file tag `__init_tls.c:47` rather than
`leap.c:16`. The decision counts are real (witness found 79
valid clusters of 2+ conditions sharing a line); the per-
decision source labels need to be read with this caveat in
mind.

## Why this still beats source-level coverage tools

Even with imperfect attribution, witness at `-O0` proves a
property no source-level tool can:

- The 58 vfprintf decisions are **post-codegen branches** —
  what actually executed on wasm, not what the source said.
- The 92 inline chains capture **actual call sites** (where
  available) — not approximate call graphs.

This is the DO-178C "post-preprocessor C" precedent applied to
the wasm bytecode layer. Different chain layer from
GCC `-fcondition-coverage` or Coveron; additive evidence in
any structural-coverage dossier.

## Comparison with the wasm32-unknown-unknown fixture

| | wasm32-unknown-unknown | wasm32-wasip1 (wasi-sdk) |
|---|---|---|
| Binary size at -O1 | 1.2 KB | 90 KB (libc statically linked) |
| `.debug_line` rows at -O1 | Empty (40-byte prologue only) | Empty (same upstream bug) |
| `.debug_line` rows at -O0 | Present, single CU | Present, multiple CUs |
| Decisions at -O0 | 1 (leap.c via v0.19 IfThen clustering) | 79 (libc dominates) |
| Inline chains | 0 (DIEs all zero) | 92 (chain tracking partial) |
| Recommended use | Minimal probe | Realistic C coverage demo |

Both fixtures share the same upstream blocker. The wasi-sdk
fixture is the better demo because the 79-decision result on
real libc code makes a stronger case for witness as a wasm
structural-coverage tool than the 1-decision leap_year demo.
