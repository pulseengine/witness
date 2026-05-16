# Swift — leap-year fixture (cross-language probe)

## Status

✅ **Tier A** — unblocked 2026-05-16 after installing matching
swift-6.3.0 toolchain via swiftly (no sudo required).

## What this demonstrates

Swift's `&&`/`||` short-circuit operators lower (via swiftc →
LLVM IR) similarly to clang. v0.19's IfThen clustering applies.
The fixture's predicate `leapYear(_ y: UInt32) -> Bool` becomes
the Swift-name-mangled `$s4leap0A4YearySbs6UInt32VF` in wasm.

## How to run

Toolchain alignment matters — SwiftWasm 6.3-RELEASE SDK was
built against `apple/swift swift-6.3-RELEASE` (= Swift 6.3.0).
Apple's bundled macOS Swift is 6.3.2, which has a different
swiftmodule binary format. Use swiftly to install the matching
host:

```sh
# One-time setup:
brew install swiftly
swiftly init --assume-yes
. ~/.swiftly/env.sh
swiftly install 6.3.0      # installs into ~/.swiftly, no sudo

# Install SwiftWasm SDK against it:
swift sdk install \
  https://github.com/swiftwasm/swift/releases/download/swift-wasm-6.3-RELEASE/swift-wasm-6.3-RELEASE-wasm32-unknown-wasip1.artifactbundle.zip \
  --checksum 6704d137e532f1ac31eafedd80658f9ee61239f2b6291216a02da32361ea9dcb

# Then:
./build.sh
witness instrument leap.wasm -o inst.wasm
```

## v0.21+walrus0.26 results (verified 2026-05-16)

| Metric | Value |
|---|---|
| Wasm size | ~7 MB (full Swift runtime statically linked) |
| Branches | 66,811 |
| **Decisions** | **4,915** |
| `chain_kind` distribution | or / and / mixed all detected ✅ |
| leap-named branches | 2 (in `$s4leap0A4YearySbs6UInt32VF`) ✅ |
| Inline contexts populated | 0 (Swift's DWARF doesn't reach the inlined-subroutine DIE level in this binary) |

The 4,915 decisions are the biggest single-fixture haul yet —
Swift's standard library adds enormous decision surface
(`Sequence`, `Optional`, `String`, runtime metadata machinery,
etc.). Witness instrumented all of it.

## Caveats — same wasm-ld DWARF gap as wasi-sdk

Every Decision's `source_file` reports `leap.swift:38` — that's
because wasm-ld doesn't relocate DWARF addresses per-CU, so
`lookup_line(byte_offset)` returns whichever line program row
happens to be at the largest `<=` offset, which is our
fixture's last line (leap.swift:38) for all 4,915 clusters.

**Structural finding is real**: 4,915 valid clusters of 2+
conditions sharing a line + chain_kind heuristic firing
correctly (`or`, `and`, `mixed`). The per-decision source
labels need cross-attribution awareness.

## Name mangling signal

Each branch's `function_name` carries the Swift-mangled
signature. Example for our predicate:

```
id=3 kind=br_if fn='$s4leap0A4YearySbs6UInt32VF'
id=4 kind=br_if fn='$s4leap0A4YearySbs6UInt32VF'
```

Decoded: module `leap`, function `leapYear`, takes `Swift.UInt32`,
returns `Swift.Bool`. Witness preserves this mangled name end-
to-end so reviewers can demangle (via `swift demangle`) when
they need the human-readable signature.

## Cross-language placement

**Tier A** — full end-to-end works once swiftly + matching
swift-6.3.0 toolchain are installed. The required setup is
documented above; once toolchain alignment is solved, the
witness side has zero gaps.
