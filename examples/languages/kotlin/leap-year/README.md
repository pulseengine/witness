# Kotlin/Wasm — leap-year fixture (cross-language probe, partial)

## Status

⚠️ **Branch-level only.** As of walrus 0.26 + the legacy-`try`/`catch`
fix (pinned via witness PR #37 from the `pulseengine/walrus` fork;
upstream PR `wasm-bindgen/walrus#316`), `witness instrument`
**succeeds** on Kotlin/Wasm output — 54 branches captured on this
fixture. **Decisions stay at 0** because Kotlin emits source maps
(`.wasm.map` V3), not DWARF. Witness's decision reconstruction
(`decisions.rs`) needs `.debug_line` rows to attribute branches to
source and cluster them. The next step to unlock decisions is a
source-map ingestion path in witness-core.

Previous status (pre walrus 0.26 fix): hard panic at parse time —
walrus could not even read the wasm-gc TYPE section. That blocker
is gone.

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

## Current results (verified 2026-05-23, witness #37 + walrus fork)

| Metric | Value |
|---|---|
| `witness instrument` | ✅ succeeds (was: hard panic) |
| Branches captured | **54** (27 if_then + 27 if_else) |
| Decisions reconstructed | **0** |
| Source attribution | ❌ no DWARF; Kotlin emits `.wasm.map` |

The fixture output `leap.wasm` (3.5 KB) shows the wasm sections:
no `.debug_*` at all, but a `sourceMappingURL` custom section
pointing at `.wasm.map` (V3 source maps). Witness reads DWARF;
ingesting V3 source maps is the remaining gap.

## What's left to unlock decisions

A single witness-side change: parse V3 source maps and build a
`LineMap`-equivalent that feeds `decisions.rs` when DWARF is
absent. Scoped as the next feature; design note tracked
separately. Once it lands, the 54 branches above should cluster
into decisions per `v0.19`'s existing IfThen / BrIf rules.

Note that source maps are structurally weaker than DWARF:
- No `DW_TAG_inlined_subroutine` → no inline-chain tracking
- No address-range info → no per-range coverage
- Only `(file, line, column, name)` tuples

So Kotlin's MC/DC report will be flatter than (e.g.) the Rust
verdicts — but it'll be real coverage data, not zero.

## Cross-language placement

Was Tier D (couldn't parse). Now **Tier B-ish** — instruments
cleanly, branch-level coverage works, decisions blocked on
source-map ingestion (same tier as `C -O1` is blocked on the
wasm-ld DWARF gap, but witness-side rather than upstream).

## Comparison with prior probes

| Probe | Block | Tier |
|---|---|---|
| Rust | None | A |
| C wasi-sdk -O0 | None | A |
| Zig | None | A |
| TinyGo | None | A |
| C++ wasi-sdk -O0 | None | A |
| Swift | Toolchain version alignment (apple 6.3.2 vs SwiftWasm-built-against 6.3.0) | A |
| **Kotlin/Wasm** | **Source maps not DWARF (decisions); walrus wasm-gc fixed via #37** | **B (partial)** |
