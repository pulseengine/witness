# Zig — leap-year fixture (cross-language probe)

## What this demonstrates

First Zig probe for witness. Builds the canonical leap-year
predicate `(y%4==0 and y%100!=0) or (y%400==0)` with
`zig build-exe -target wasm32-freestanding -OReleaseSafe`, then
runs it through witness's pipeline.

## How to run

```sh
./build.sh                                # produces leap.wasm via zig
witness instrument leap.wasm -o inst.wasm
```

## v0.19 results (verified 2026-05-14, Zig 0.16.0)

| Metric | Value |
|---|---|
| Branches | 2 |
| Decisions | **1** ✅ |
| chain_kind | `or` ✅ (witness recognised the `\|\|` short-circuit shape) |
| Source attribution | leap.zig:17 ✅ |
| `.debug_line` rows resolved | ✅ — Zig's DWARF survives wasm linking |

The single Decision groups the two `br_if`s that lower the
predicate's `or` arm. Source-line attribution is exact (line
17 is the `return (...) or (...)` line in leap.zig). The
`chain_kind = or` detection means witness's
`detect_chain_kind` heuristic recognises Zig's lowering as
identical to rustc's `||` pattern.

## What Zig lowers differently from clang

Worth knowing: Zig's frontend emits a **br_if chain** for
`(a and b) or c`, the same shape rustc produces — not the
`if/else` + br_if shape clang produces for the same C
expression. v0.19's IfThen clustering is therefore not
load-bearing for this Zig fixture, but the existing BrIf
clustering catches the pattern. The IfThen clustering still
matters for Zig code that uses explicit `if/else` blocks
(common in Zig style for error-union handling), once we add
that fixture.

## What Zig handles well

- **DWARF survives linking** — Zig's built-in LLD-based wasm
  linker preserves the line program AND populates address
  fields. Unlike `wasm32-unknown-unknown` clang + wasm-ld,
  the addresses are usable for branch attribution.
- **Source paths are accurate** — `source_file` reports
  `leap.zig` rather than zig-stdlib intermediates.
- **`chain_kind` detection works** — same heuristic that
  classifies rustc's `||` correctly fires on Zig's `or`.

## What Zig doesn't yet give us

- **No inline chains** — Zig at `ReleaseSafe` did inline
  `leap_year` (we used `noinline` to defeat that). Even with
  the `noinline` keeping the function separate, Zig's DWARF
  doesn't emit `DW_TAG_inlined_subroutine` entries, so
  cross-function inline chain tracking won't fire.
- **No `if/else` lowering fixture yet** — adding a
  Zig-idiomatic `if` chain (e.g. error union handling) would
  exercise the v0.19 IfThen clustering directly. Open for a
  future fixture.

## Cross-language placement

Promoting Zig from Tier C (untested) → Tier A (verified
end-to-end) for the matrix in `docs/cross-language.md`.
