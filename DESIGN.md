# witness — design

This document captures the architecture, the v0.1→v1.0 roadmap, the known
hard problems, and the invariants that should not regress across versions.
It is intended to be read alongside `README.md` (user-facing) and
`artifacts/requirements.yaml` (traced requirements).

## Goal

Produce MC/DC-quality coverage evidence for WebAssembly components, with
source mapping back to rivet requirements, composable with sigil attestation
bundles, and tractable at AI-velocity authorship scale.

## Non-goals (v0.1)

- Not a full MC/DC tool yet. v0.1 reports strict per-branch coverage
  (`br_if`, `br_table`, `if` each counted independently). Condition-level
  MC/DC decomposition requires DWARF reconstruction and is v0.2.
- Not an orchestration platform. witness does one thing: measure coverage
  on a Wasm module. Composition with rivet / sigil / loom / meld happens
  at the calling layer.
- Not a Wasm runtime. witness depends on an external runtime (wasmtime,
  kiln, or similar) to execute instrumented modules during harness runs.

## Roadmap

| Version | Capability | Blocking question |
|---|---|---|
| v0.1 | Branch-level coverage. Instrument → run → report. Strict per-instruction counting. | **Resolved:** v0.1 ships counters as exported mutable globals (`__witness_counter_<id>`), not a dump function. Any runtime that can read Wasm globals can extract coverage — no cooperation protocol required. |
| v0.2 | MC/DC condition decomposition when DWARF-in-Wasm is present; strict fallback otherwise. | Decision-granularity formal definition — see below. |
| v0.3 | rivet integration (coverage → requirement). In-toto predicate emission for sigil bundles. | rivet schema for coverage predicates; sigil predicate format. |
| v0.4 | Variant-aware scope. Post-cfg, post-meld, post-loom measurement points. | How does witness interact with loom's translation-validation output? |
| v1.0 | Check-It pattern qualification. Emit a checkable coverage attestation that a small qualified checker can validate under DO-330. | What does the minimal trusted checker look like? |

## Architecture (v0.1)

Three stages, each independently testable:

```
   app.wasm ──▶ [instrument] ──▶ app.instrumented.wasm
                                  + app.instrumented.witness.json (manifest)
                                         │
                                         ▼
                                    [run --harness "..."]
                                         │
                                         ▼
                               witness-run.json (raw counters)
                                         │
                                         ▼
                                      [report]
                                         │
                                         ▼
                       coverage-report.{text|json}
```

### instrument

Rewrites the Wasm module with `walrus`:

1. Walk every function, enumerate every `br_if` / `if-else` / `br_table`.
2. For each branch point, allocate a mutable `i32` global initialised to 0
   and export it as `__witness_counter_<id>`.
3. Insert the counter increment on the taken path:
   - **`br_if L`** → `local.tee $tmp; if (inc counter) end; local.get $tmp; br_if L`
     — preserves stack shape; counter fires only when the branch is taken.
   - **`if A else B end`** → prepend counter-increment to each arm sequence
     (two counters per `IfElse`: then-taken and else-taken).
   - **`br_table`** → single "executed" counter inserted immediately before
     the instruction. Per-target counting is v0.2.
4. Emit a side-channel manifest (`<output>.witness.json`) mapping each
   branch id to `(function_index, instr_index, kind, seq_debug)`.

Hosts iterate module exports and read every global whose name starts with
`__witness_counter_`. No module cooperation, no linear-memory serialisation,
no multi-value return.

**Semantic preservation invariant:** the instrumented module produces the
same observable output as the original for every input, modulo the dump
export. Verified by round-trip testing against the wasm-tools reference
interpreter.

### run

v0.1 embeds `wasmtime` and runs the module directly. The runner:

1. Loads the instrumented module and its manifest.
2. Optionally calls `_start` (WASI "command" convention).
3. Invokes zero or more `--invoke <export>` functions in order (no-argument
   exports only in v0.1; parameterised invocations are v0.2).
4. Iterates every export matching `__witness_counter_<id>` and reads each
   global's final value.
5. Pairs hit counts with manifest entries and emits the raw run JSON.

Subprocess-harness execution (v0.2) will add `--harness <cmd>` as the
escape hatch for modules that need a richer runtime than witness embeds.

### report

Aggregates the raw run data into a coverage report:

- Per-function coverage summary.
- Per-branch hit counts.
- Uncovered branches with source location (when manifest provides it).

## Open research question — decision granularity at Wasm level

**The problem.** Short-circuit evaluation at Rust source (`a && b && c`)
compiles to a sequence of `br_if` instructions. MC/DC requires that each
condition independently affect the decision outcome. Two interpretations:

- **Strict:** each `br_if` is its own decision. Easy to measure; loses
  the source-level "condition" grouping. v0.1 uses this.
- **Reconstructed:** group the `br_if` sequence back into the source-level
  decision and measure MC/DC over the reconstruction. Harder; needs
  DWARF-in-Wasm or explicit compiler hints.

**v0.2 plan.** Reconstruction when DWARF is present; strict fallback when
not. The reconstruction algorithm is a local pattern match on `br_if`
sequences that share a common control-flow merge, grouped by source-line
information from DWARF. Exact algorithm TBD; ship the strict-only report
in parallel for any module where the reconstruction fails.

**Deserves a paper.** The decision-granularity definition at Wasm level is
not settled in the literature. A short write-up of the algorithm plus a
proof of its soundness relative to source-level MC/DC would be publishable
and would give witness's v0.2 output regulatory defensibility.

## Dependency choices

**walrus** — Wasm AST rewriting. Mature, ergonomic, maintained by the Rust
and WebAssembly WG. Alternative: `wasm-tools` (lower-level, spec-tracking).
v0.1 uses walrus; reserve wasm-tools for cases walrus cannot express.

**Runtime for harness execution** — v0.1 targets `wasmtime` as the default
because its CLI test-harness integration (`cargo test --target wasm32-wasi`)
is the path of least resistance for the largest existing Rust+Wasm test
ecosystem. kiln integration and other runtime support come later.

**Serialization** — `serde_json` for both run data and reports. Structured
format that rivet can consume directly once the coverage schema ships.

## Testing strategy

- **Unit tests** in each module using `wat` for inline Wasm text.
- **Round-trip tests** — instrument a known module, execute it with and
  without the instrumentation, assert observable equivalence.
- **Golden-output tests** with `insta` for report formatting.
- **Reference comparisons** — for a corpus of small Wasm modules, compare
  witness's branch enumeration against the wasm-tools reference.

## Invariants (don't regress)

1. Instrumented modules must be semantically equivalent to originals on
   every well-formed input, modulo the dump export.
2. Every reported branch must map to a specific `(function_index,
   instruction_offset)` in the original module — not in the instrumented
   module.
3. The manifest format is stable within a major version. Breaking changes
   bump the major version.
4. Reports must be deterministic for a given (module, run-data) pair.
