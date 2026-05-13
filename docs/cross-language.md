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

### Tier B — clustering works, upstream DWARF gap at `-O1`+

| Language | Toolchain | Status | Notes |
|---|---|---|---|
| **C** (`-O1`+) | clang + wasm-ld + `wasm32-unknown-unknown` | ⚠️ blocked upstream | v0.19 IfThen clustering is correct. `wasm-ld` for this target emits an empty `.debug_line` program (40-byte prologue, zero rows) when inlining or DWARF relocation is involved. Workaround: build at `-O0`, switch to wasi-sdk + `wasm32-wasi`, or use `__attribute__((noinline))` (partial — still wasm-ld-dependent). See `examples/languages/c/leap-year/README.md`. |

### Tier C — should work, untested

These languages produce wasm with DWARF when configured
appropriately. End-to-end through witness is unverified but
expected to work — v0.19's IfThen+BrIf clustering covers
LLVM-frontend lowering shapes. The remaining unknown is
whether each toolchain's linker preserves the `.debug_line`
program at the same level wasi-sdk does for C/C++.

| Language | Toolchain | Special features to test |
|---|---|---|
| **C++** | clang + wasm-ld + `-g` | Template monomorphisation produces deep inline chains; the v0.14 chain tracker should expose them. Virtual dispatch is `call_indirect`, not a "decision" in MC/DC sense — would show up in branch counts only. |
| **Zig** | `zig cc` + `-target wasm32-freestanding` + `-g` | `comptime` resolves at compile time — witness measures only runtime branches, which is the correct framing. Error unions (`try` operator) lower to br_if chains; canonical MC/DC case once decision reconstruction handles the shape. |
| **Swift (SwiftWasm)** | swiftc with wasm target + `-g` | Optional pattern matching (`if let`) lowers to br_if chains; should produce textbook MC/DC. Protocol witness tables are runtime dispatch (not MC/DC-applicable). |
| **TinyGo** | `tinygo build -target wasm` + `-debug` | Channels + select{} lower to br_table; v0.11.5 BrTableAudit applies. Interface dispatch is runtime (not MC/DC). |
| **Kotlin/Wasm** | Kotlin 2.0+ with `wasm-js` target + sourcemaps | Sealed-class exhaustive `when` lowers to br_table — textbook br_table audit case. |

### Tier D — likely won't work without compiler changes

| Language | Issue |
|---|---|
| **Go (standard `go build`)** | Wasm output has no DWARF. TinyGo is the path. |
| **AssemblyScript** | Source maps only; no DWARF (historically). Recent versions may have improved; needs probing. |
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
