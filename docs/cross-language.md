# Witness across languages

**Status:** Living document. Tested entries are verified; "should
work" entries are based on toolchain-emission claims and not
yet end-to-end probed.

## Positioning vs the OSS MC/DC landscape

The open-source MC/DC space is not empty. Existing tools:

| Tool | Languages | Layer | Open source |
|---|---|---|---|
| **GCC `-fcondition-coverage`** | C / C++ / D / Rust | Source-level (frontend) | ✅ since GCC 14 (2024) |
| **[Coveron](https://coveron.github.io/)** | C / C++ | Source-level | ✅ |
| **[linux-mcdc (UIUC, DASC 2025)](https://github.com/xlab-uiuc/linux-mcdc)** | Linux kernel C | Source-level via GCC | ✅ |
| **[GNATcoverage](https://github.com/AdaCore/gnatcoverage)** | Ada (also C via gcov) | Source + object | ✅ (LGPL) |
| **LDRA / VectorCAST / Cantata / Coco** | C/C++/Ada/more | Source-level | ❌ commercial |
| **rustc-mcdc / SCRC 2026 Project Goal** | Rust | Source-level | Work in progress |
| **witness** | any language → wasm | **post-codegen on wasm bytecode** | ✅ |

Witness occupies a different layer: it measures structural
coverage on the wasm artefact, after the frontend has lowered
the source and LLVM has run its optimisation passes. The same
DO-178C "post-preprocessor C" precedent applies — accepted by
certification authorities since 1992: measure what the compiler
emits, not what the engineer typed.

This makes witness **language-agnostic** in principle: any
language that compiles to wasm with DWARF debug info is a
candidate. In practice, witness's decision-reconstruction
heuristics were tuned to rustc; clustering rules may need
per-frontend tweaks to produce useful Decision verdicts on
clang/zig/etc output. Instrumentation + counter capture +
DWARF source-line attribution + inline-chain tracking are
already language-agnostic.

## Language matrix

### Tier A — verified (witness end-to-end works at v0.19)

| Language | Toolchain | Status | Notes |
|---|---|---|---|
| **Rust** | rustc + wasm32-unknown-unknown / wasm32-wasip2 | ✅ shipped | 12 fixtures in `verdicts/`; httparse demonstrates chains up to 8 levels deep |
| **C** (`-O0`) | clang + wasm-ld + DWARF | ✅ probed | Decision clustering works post-v0.19 (IfThen + BrIf clustered). 2 branches → 1 Decision verified on the leap-year fixture at `-O0`. |
| **C** (wasi-sdk `-O0`) | wasi-sdk clang + wasm-ld + `wasm32-wasip1` | ✅ probed | **79 decisions** + 92 inline chains across libc (`vfprintf`, `fwrite`, `memchr`, …). Best demo of witness on real C code. Source attribution partly cross-contaminated by the wasm-ld address-relocation gap. See `examples/languages/c/leap-year-wasi/README.md`. |
| **Zig** | zig 0.16 + wasm32-freestanding `-OReleaseSafe` | ✅ probed | 1 Decision on `leap.zig:17`, `chain_kind = or` detected. Zig lowers `or` to br_if chains (rustc-style), not clang's `if/else`. See `examples/languages/zig/leap-year/README.md`. |
| **Go (TinyGo)** | tinygo 0.41 + wasm-unknown `-opt 1` | ✅ probed | 4 Decisions, 2 in `leap.go:28` (inline copies of `leapYear` into both call sites), `chain_kind = or` + inline chains populated. Cleanest non-Rust DWARF. See `examples/languages/go/leap-year/README.md`. |
| **C++** (wasi-sdk `-O0`) | wasi-sdk clang++ + `wasm32-wasip1 -std=c++20` | ✅ probed | 79 decisions (same shape as C wasi-sdk — libc dominates). C++ specific signal: template monomorphisation visible in `function_name` (`bool leap_year<unsigned int>(unsigned int)`). `chain_kind = or` on the predicate cluster. See `examples/languages/cpp/leap-year/README.md`. |
| **Swift** (SwiftWasm `-Onone`) | swift 6.3.0 (via swiftly) + SwiftWasm 6.3-RELEASE SDK + `wasm32-wasip1` | ✅ probed | **4,915 decisions** — biggest single fixture; Swift runtime dominates. `chain_kind = or / and / mixed` all detected. Predicate visible by mangled name `$s4leap0A4YearySbs6UInt32VF`. Same wasm-ld cross-CU attribution caveat as wasi-sdk. See `examples/languages/swift/leap-year/README.md`. |

### Tier B — clustering works, upstream DWARF gap at `-O1`+

| Language | Toolchain | Status | Notes |
|---|---|---|---|
| **C** (`-O1`+) | clang + wasm-ld + `wasm32-unknown-unknown` | ⚠️ blocked upstream | v0.19 IfThen clustering is correct. `wasm-ld` for this target emits an empty `.debug_line` program (40-byte prologue, zero rows) when inlining or DWARF relocation is involved. Workaround: build at `-O0`, switch to wasi-sdk + `wasm32-wasi`, or use `__attribute__((noinline))` (partial — still wasm-ld-dependent). See `examples/languages/c/leap-year/README.md`. |
| **C** (wasi-sdk `-O1`+) | wasi-sdk clang + `wasm32-wasip1` | ⚠️ blocked upstream | Same wasm-ld DWARF address-relocation gap as `wasm32-unknown-unknown` at `-O1`. wasi-sdk preserves the line program at `-O0` (proven by the 79-Decision result), then collapses it once LTO/inlining kicks in. See `examples/languages/c/leap-year-wasi/README.md`. |

### Tier C — should work, untested or toolchain-blocked

_(Tier C empty after Swift moved to Tier A on 2026-05-16. Setup requirements documented in the Swift fixture README.)_

### Tier D — likely won't work without compiler / tool changes

| Language | Issue |
|---|---|
| **Go (standard `go build`)** | Wasm output has no DWARF. TinyGo is the path. |
| **AssemblyScript** | Source maps only; no DWARF (historically). Recent versions may have improved; needs probing. |
| **Kotlin/Wasm** | Probed 2026-05-14. Two layered blocks: (1) Output targets the wasm-gc proposal — witness's wasm-rewriter (walrus 0.24) rejects it with `gc proposal not supported (at offset 0x10)`. (2) Even with walrus GC support, Kotlin emits source maps (`.wasm.map` V3), not DWARF — witness would need a source-map ingestion path. See `examples/languages/kotlin/leap-year/README.md`. |
| **MoonBit** | Wasm-first language, but DWARF emission status in the 2026 toolchain unverified. Quick probe: `wasm-tools dump out.wasm \| grep .debug_` after compilation. If no `.debug_*` sections, witness has no source attribution. |

## What works language-agnostic at v0.19

These witness features apply to any wasm input with DWARF, regardless of source language:

- **Branch instrumentation** — per-branch counters, brval (per-row condition value), brcnt (per-row reach count) globals. Works on `br_if`, `if/else`, and `br_table` arms.
- **DWARF source-line attribution** — every branch's `byte_offset` resolves to `(file, line)` via the line program.
- **Inline-context tracking (v0.14+)** — `DW_TAG_inlined_subroutine` entries get walked; each branch within an inlined frame carries its full call chain in `branch_inline_chains`. Works on any compiler that emits this DIE.
- **DW_AT_ranges scattered inlines (v0.17+)** — multi-range inlined frames (LTO tail-merge, hot/cold split) get one InlineEntry per range with shared chain.
- **mcdc-v3 envelopes** — schema is structural; doesn't care which language produced the underlying data.

## What needs per-frontend tuning

These witness components were tuned to rustc:

- ~~**`decisions.rs::group_into_decisions`** — clusters by `BrIf` only.~~ **Resolved in v0.19** — `IfThen` now clusters alongside `BrIf` for decision-key purposes (the IfThen arm is the "predicate was true" outcome, semantically equivalent to a BrIf). `IfElse` stays excluded. clang/zig/Swift/TinyGo shapes (one `if/else` + 1 `br_if` per `&&`/`||` source decision) now form Decisions.
- **`instrument.rs::detect_chain_kind`** — looks for rustc's `i32.eqz; br_if` pattern as the `&&` lowering. Other compilers may use different bytecode shapes for short-circuit operators; chain_kind degrades to `Unknown` which means outcome derivation falls back to function-return (still works, just less precise per-iteration).
- **`MAX_DECISION_LINE_SPAN = 10`** — tuned to Rust's source density. C/C++ may want a smaller window; Lean/Coq want larger. Configurable would be a small future change.

## Probe recipe for a new language

If you want to try witness on a language not yet listed:

```sh
# 1. Compile to wasm with DWARF.
$YOUR_COMPILER input.<ext> -o out.wasm  # add debug-info flag

# 2. Confirm DWARF sections present.
wasm-tools dump out.wasm | grep '.debug_'
# Expected: at least .debug_info, .debug_line, .debug_str

# 3. Instrument.
witness instrument out.wasm -o inst.wasm

# 4. Check manifest. Higher numbers are better.
jq '{branches: (.branches | length), decisions: (.decisions | length), inline_contexts: (.branch_inline_contexts | length)}' inst.wasm.witness.json
```

**Interpretation:**

- `branches > 0` and `decisions == 0`: instrumentation works,
  decision-clustering doesn't fit the lowering style. File an
  issue with the source + the manifest's branch kinds.
- `branches > 0` and `decisions > 0`: full witness pipeline
  works. Add an end-to-end fixture under
  `examples/languages/<lang>/`.
- `branches == 0`: either the compiler didn't emit DWARF, or
  it optimised away all the branches (try lower opt-level or
  add a volatile sink to defeat constant folding).

## Contributing a fixture

1. Pick a language from Tier C or D.
2. Run the probe recipe above.
3. Add `examples/languages/<lang>/<fixture-name>/` with:
   - The source file
   - A `build.sh` showing the exact toolchain invocation
   - A `README.md` documenting what witness picked up + what
     didn't (use the C README as a template)
4. Open a PR. Updating the matrix above is part of the change.

This document is the long-running record of witness's
cross-language reach; tier shifts and probe results land here.
