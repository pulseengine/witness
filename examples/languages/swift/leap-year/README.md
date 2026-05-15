# Swift — leap-year fixture (cross-language probe, BLOCKED)

## Status

⚠️ **Blocked on SwiftWasm toolchain compatibility** — fixture
exists but does not build end-to-end yet. See "What blocks
this" below.

## What this would demonstrate

Same predicate as the other fixtures, in Swift. Swift's
`&&`/`||` are short-circuit operators that swiftc → LLVM IR
lowers similarly to clang. Expected wasm shape: `if/else` +
1 `br_if` per source decision — the v0.19 IfThen clustering
target.

## Files

- `leap.swift` — the predicate (UInt32 input via a small
  reference-type box, `@inline(never)` predicate to defeat
  constant-folding).
- `Package.swift` — minimal SwiftPM manifest (SwiftWasm
  cross-compilation requires SwiftPM, not bare swiftc).
- `build.sh` — invokes `swift build --swift-sdk
  6.3-RELEASE-wasm32-unknown-wasip1`.

## What blocks this

Verified 2026-05-14 with:
- Host: Apple Swift 6.3.2 (built into macOS / Xcode)
- SDK: `swift-wasm-6.3-RELEASE-wasm32-unknown-wasip1`
  (latest official SwiftWasm release)

Result: `error: compiled module was created by a different
version of the compiler ''; rebuild 'Swift' and try again`
when SwiftPM tries to import the SDK's Swift stdlib
swiftmodule.

The SwiftWasm release notes name `apple/swift swift-6.3-RELEASE`
as the compatible host. Apple ships **swift-6.3.2** with
current Xcode — and swiftmodule binary format is sensitive to
patch-level differences. The SDK's stdlib was sealed against
6.3-RELEASE's swift-frontend; 6.3.2's frontend refuses to load
it.

## How to unblock

Three paths, in increasing order of cost:

1. **Wait for SwiftWasm 6.3.2 / 6.4 release** — SwiftWasm
   typically catches up within weeks. New SDK release →
   re-run `swift sdk install`.

2. **Install matching apple/swift snapshot toolchain** —
   download `swift-6.3-RELEASE` (~1.5 GB), use it as the
   host via `xcrun -toolchain swift-6.3-RELEASE swift build
   ...`. Heavier setup, fully unblocks.

3. **Build SwiftWasm SDK from source** — full SwiftWasm
   build against the host swift-6.3.2. Multiple hours.

For this fixture's purposes, the gap is purely toolchain
alignment — the fixture source is well-formed and witness's
clustering logic doesn't change. No witness-side fix is
required.

## Cross-language placement

Stays in **Tier C** (should work, not yet probed end-to-end).
The SwiftWasm 6.3-RELEASE SDK install + matching toolchain is
the only thing between here and Tier A.

## What we expect once unblocked

| Metric | Predicted |
|---|---|
| Decisions on leap.swift | 1 (the `\|\|`-arm cluster) |
| `chain_kind` | `or` |
| Inline chains | Some — Swift heavily inlines stdlib calls |
| Source attribution | Subject to the same wasm-ld DWARF gap as C wasi-sdk; expect cross-CU contamination |
