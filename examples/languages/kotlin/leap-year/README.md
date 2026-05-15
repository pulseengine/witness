# Kotlin/Wasm — leap-year fixture (cross-language probe, BLOCKED)

## Status

❌ **Blocked on wasm-gc + DWARF support** — fixture builds
cleanly via Kotlin Multiplatform's `wasmJs()` target, but
witness's wasm-rewriter (walrus 0.24) cannot parse the output
because it uses the wasm-gc proposal. Independently, Kotlin
emits source maps (`.wasm.map`) rather than DWARF, so even if
parsing succeeded, source-line attribution wouldn't work.

## What this would demonstrate

Same predicate as the other fixtures, in Kotlin idiomatic
form. Kotlin/Wasm 2.2+ uses the wasm-gc target (GC reference
types) which is post-MVP. Sealed-class exhaustive `when`
lowers to br_table — would be a textbook br_table audit case
once the tooling caught up.

## Files

- `src/wasmJsMain/kotlin/leap.kt` — the predicate
  (`@JsExport` on each function to keep them in the export
  table; Kotlin would otherwise tree-shake them)
- `build.gradle.kts` — Kotlin Multiplatform plugin with
  `wasmJs() { nodejs() }`
- `settings.gradle.kts` — Gradle project name
- `build.sh` — invokes `gradle compileProductionExecutableKotlinWasmJsOptimize`

## What blocks this

Verified 2026-05-14:

```
$ witness instrument leap.wasm -o leap.instr.wasm
Error: failed to parse Wasm module at leap.wasm
Caused by:
    gc proposal not supported (at offset 0x10)
```

The error fires at the TYPE section (offset 0x10) because
Kotlin/Wasm emits GC-typed types. Two layered blockers:

1. **walrus 0.24 doesn't support wasm-gc** — the wasm
   rewriter witness uses to insert counters can't parse
   the module. This is a witness-side blocker that would
   need a walrus upgrade with GC support (in flight at
   walrus repo but not yet released in 0.x).

2. **No DWARF, only source maps** — Kotlin/Wasm emits
   `.wasm.map` source maps (V3 format), not DWARF. Even
   with walrus support for wasm-gc, witness would need a
   source-map ingestion path to attribute branches to
   `.kt` source lines.

The fixture output `leap.wasm` (3.5 KB) confirms both: header
shows `TAG` (exception handling), `DATACOUNT`, and
`sourceMappingURL` sections — no `.debug_*` sections at all.

## What unblocking would look like

| Change | Effect |
|---|---|
| walrus 0.24+ → wasm-gc support | Witness can instrument the module; decisions on br_ifs/IfThens would land but with no source attribution |
| witness gains `--source-map` flag accepting V3 .wasm.map | Source attribution works; v0.19's IfThen clustering applies as normal |

Both are non-trivial. Kotlin/Wasm sits in Tier D until both
land.

## Cross-language placement

Moves from **Tier C** (should work, untested) → **Tier D**
(blocked on upstream tool changes). The blocker isn't a wasm-
ld DWARF gap (that's Tier B); it's a more fundamental wasm-gc
parsing block + a separate source-map ingestion design.

## Comparison with prior probes

| Probe | Block | Tier |
|---|---|---|
| Rust | None | A |
| C wasi-sdk -O0 | None | A |
| Zig | None | A |
| TinyGo | None | A |
| C++ wasi-sdk -O0 | None | A |
| Swift | Toolchain version alignment (apple 6.3.2 vs SwiftWasm-built-against 6.3.0) | C |
| **Kotlin/Wasm** | **walrus wasm-gc + DWARF missing** | **D** |
