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

## What v0.17 picks up — and what it doesn't

Probe results against witness v0.17 (verified 2026-05-12):

| | Result |
|---|---|
| Wasm produced | ✅ 1264 bytes with `.debug_info` |
| Branches detected | ✅ 12 (4 funcs × 3 each) |
| DWARF byte-offset → source attribution | ✅ resolves correctly |
| **Decisions reconstructed** | ❌ **0** |
| `branch_inline_contexts` populated | ❌ 0 |

### Why decisions count is zero

clang lowers `&& ` chains into a different wasm shape than rustc:

- **rustc** lowers `a && b` to `local.get; i32.eqz; br_if N`
  — a chain of `br_if`s witness clusters into a Decision.
- **clang** lowers `a && b` to a wasm `if/else` block followed
  by one `br_if` — witness's `decisions.rs::group_into_decisions`
  only clusters `BrIf` entries (line 263); `IfThen`/`IfElse`
  entries from clang's `if/else` lowering get counted but never
  form a decision (need ≥ 2 BrIfs in a cluster).

For this fixture, each `run_row_*` has 1 BrIf and 2 If entries,
not enough to form a multi-condition Decision under v0.17's
clustering rule.

## What would fix this

Two design choices, neither implemented yet:

1. **Cluster `IfThen` + `BrIf` entries together** when they
   share a `(function_index, source_file)` key and the source
   lines fall within `MAX_DECISION_LINE_SPAN`. Treat the
   `if/else` lowering's `IfThen` arm as a condition equivalent
   to a `BrIf`. Small code change in `decisions.rs`; risk is
   over-clustering when an `if/else` is a separate source
   statement.

2. **Lowering-aware reconstruction** — detect clang vs rustc
   emission style at instrument time (looking at the
   surrounding bytecode shape) and apply per-style clustering
   rules. Bigger change; gives more precise decisions but
   couples reconstruction to compiler heuristics.

Tracked as **v0.19+** ("decision reconstruction for clang/zig/
other LLVM-frontend wasm shapes"). The instrumentation +
runtime counter / brval / inline-context substrate already
works; only the clustering rule needs the extension.

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
