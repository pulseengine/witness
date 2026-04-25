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
| v0.1 (shipped 2026-04-24) | Branch-level coverage. Instrument → run → report. Strict per-instruction counting. | **Resolved:** v0.1 ships counters as exported mutable globals (`__witness_counter_<id>`), not a dump function. Any runtime that can read Wasm globals can extract coverage — no cooperation protocol required. |
| v0.2 | MC/DC condition decomposition via DWARF reconstruction; strict per-`br_if` fallback when DWARF is absent. Per-target `br_table` counting. **No artificial condition-count cap** (witness has no encoder constraint, unlike LLVM/rustc which cap at 6). Subprocess `--harness <cmd>` mode as embedded-runtime escape hatch. Coverage-lifting writeup proving Wasm coverage projects soundly to source-level MC/DC. | Reconstruction-algorithm formalisation; soundness proof relative to source-level MC/DC; DO-178C post-preprocessor precedent sourcing. |
| v0.3 | rivet integration (coverage → requirement) via the `witness-rivet-evidence/v1` schema and the upstreamed `rivet-core::coverage_evidence::CoverageStore` consumer. In-toto coverage predicate emission for sigil bundles (`https://pulseengine.eu/witness-coverage/v1`, opaque to sigil). `witness merge` subcommand for aggregating runs across test binaries. Quality bar tightened: proptest properties on merge / serde / requirement-map; cargo-mutants in CI; miri on the pure-Rust modules; coverage threshold raised to 75%. | Resolved: schema documented in `docs/research/rivet-evidence-consumer.md` and `docs/research/sigil-predicate-format.md`. |
| v0.4 | DWARF-grounded MC/DC reconstruction algorithm body (lifted from v0.2.1; never released as v0.2.1). `witness diff` subcommand for coverage / branch-set delta between two snapshots. CI ports: `witness-delta.yml` PR workflow + `actions/compliance` composite action that bundles release-time evidence (coverage report, in-toto predicates, branch manifests) into a tar.gz attached to the GitHub release. Mythos slop-hunt audit applied (orphan exports + unused deps removed). Compiler-qualification reduction brief documenting where witness's chain substitutes for compiler-qual under ISO 26262-8 §11.4.5 (`docs/research/v04-compiler-qualification-reduction.md`). | Resolved in v0.4 research. |
| v0.5 | **Workspace split**: `witness-core` library compiles to `wasm32-wasip2`; `witness` binary keeps the wasmtime runner. **`witness lcov`** subcommand emits LCOV (DWARF-correlated `BRDA` + sibling overview) for codecov ingestion. **CI dogfood**: `witness measures the fixture` step uploads LCOV to codecov with flag `wasm-bytecode` alongside `rust-source`. **Sigil/wsc integration**: `witness attest` produces DSSE-signed envelopes via `wsc-attestation::dsse::DsseEnvelope::sign_ed25519`. **Wasm artefact**: `witness-core` Wasm build attached to releases. Loom + meld upstream issue drafts in `docs/research/v05-loom-meld-upstream.md`. | Resolved: workspace, LCOV, attest, wasm artefact all shipped. |
| v0.6 | DWARF preservation through loom optimisation + meld fusion (depends on upstream issues landing). Component-model coverage with WIT interface. Post-loom / post-meld measurement points. | When loom / meld land their offset-translation maps. |
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

## v0.2 — MC/DC at Wasm level

This section is the architectural plan for v0.2. The corresponding rivet
artefacts live in `artifacts/requirements.yaml`,
`artifacts/features.yaml`, and `artifacts/design-decisions.yaml` with
`v0.2` tags. The `docs/research/mcdc-bytecode-research.md` brief is the
literature anchor; the constraints listed there flow directly into the
choices below.

### Goal

Group Wasm-level branch instrumentation back into source-level decisions
when DWARF-in-Wasm is present, measure MC/DC over the reconstruction, and
produce evidence that projects soundly to source-level MC/DC claims
(coverage-lifting). When DWARF is absent, fall back to v0.1's strict
per-`br_if` / per-`if-else` counting.

### The decision-granularity problem

Short-circuit evaluation at Rust source (`a && b && c`) compiles to a
sequence of `br_if` instructions. MC/DC requires that each condition
independently affect the decision outcome. Two interpretations:

- **Strict:** each `br_if` is its own decision. Easy to measure; loses
  the source-level "condition" grouping. v0.1 ships this; v0.2 keeps it
  as the fallback.
- **Reconstructed:** group the `br_if` sequence back into the source-level
  decision and measure MC/DC over the reconstruction. Needs
  DWARF-in-Wasm.

### Design choices that the research forced

The MC/DC-on-bytecode brief (`docs/research/mcdc-bytecode-research.md`)
made three choices non-negotiable:

1. **No condition-count cap.** Clang's source-based MC/DC and rustc's
   `-Zcoverage-options=mcdc` both cap decisions at 6 conditions because
   LLVM's coverage bitmap encodes condition combinations as integers in
   `[0, 2^N)`. Witness uses exported globals — no encoder constraint.
   v0.2 must support decisions of any size. This is positioning: witness
   covers what rustc-mcdc cannot.
2. **Per-target `br_table` counting belongs in v0.2, not later.** The
   same DWARF that maps `br_if` chains to source decisions also maps
   `br_table` targets to match arms. Splitting these across versions
   leaves Rust pattern-matching coverage half-done. Ship them together.
3. **Coverage-lifting is an explicit deliverable.** Translation-validation
   literature (Pnueli) has the formal apparatus, but no prior work
   formalises *coverage* lifting. Witness's regulatory defensibility
   depends on the soundness argument: "Wasm-level decision X covers
   source-level decision Y because the lifting from low-level to
   source-level is sound under DWARF-grounded reconstruction."

### Reconstruction algorithm (sketch)

Pseudocode, refined in the v0.2 paper:

```
for each br_if-sequence S in the Wasm CFG:
    line_set = { dwarf_line(instr) : instr in S }
    decision_marker = lexical_decision_id(S, dwarf)  // distinguishes
                                                     // multiple decisions
                                                     // on the same line
    if line_set has a single source line and
       all instrs in S share the same decision_marker:
        group S as one source-level decision D
        emit MC/DC condition table for D
        each br_if in S maps to one condition in D
    else:
        emit each br_if in S as a strict-fallback decision
```

The interesting cases:

- **Macro expansion.** A single source line may compile to multiple
  decisions (e.g., `assert!(a && b)` expands the macro). The
  decision-marker uses the lexical decision id from DWARF, not just the
  line number, to keep these distinct.
- **Inlining.** When a function is inlined, the inlined `br_if`s carry
  their original source line, not the call-site line. The reconstruction
  must group by inlined-source-line, not Wasm-physical-position.
- **CFG fragmentation.** Compiler optimisation may split a logically-
  single decision across multiple basic blocks. The reconstruction must
  follow control-flow merges, not just instruction adjacency.

### `br_table` per-target instrumentation

Replace v0.1's "single counter before `br_table`" with one counter per
target. Implementation candidates:

- **Helper function.** `__witness_brtable_<id>(selector: i32) -> i32`
  increments `counter[id][selector]` (with bounds check) and returns
  `selector`. Cost: one call per `br_table`. Simple. Chosen for v0.2.
- **Inline i32.eq chain.** N `if (selector == k) { incr counter_k }`
  blocks before the `br_table`. Cost: N branches per N-target table —
  too expensive for tables with many targets.

DWARF maps `br_table` targets to source-level match arms; the manifest
gains a `target_index` field and a `match_arm_label` (when DWARF
supplies it).

### Subprocess harness mode (`--harness <cmd>`)

Embedded wasmtime is the default for `witness run`. v0.2 adds a
subprocess escape hatch for modules that need a richer runtime
(`wasm-bindgen-test` on Node/browser, custom WASI capabilities, native
test frameworks):

```
witness run prog.instrumented.wasm --harness "cargo test --target wasm32-wasip1 ..."
```

Protocol (file-based handshake):

1. Witness sets `WITNESS_MODULE=<path>`, `WITNESS_OUTPUT=<run.json>`,
   `WITNESS_MANIFEST=<path>` env vars and spawns the harness command.
2. The harness loads the instrumented module in its native runtime,
   runs tests, then before exiting reads every `__witness_counter_*`
   global and writes a partial run JSON to `$WITNESS_OUTPUT`.
3. Witness joins the harness output with the manifest and emits the
   final run JSON.

A `witness-harness` companion crate (v0.3) would provide the harness-
side helper so test authors don't reinvent the loop.

### Coverage-lifting (the soundness claim)

For each Wasm-level decision D_w produced by the reconstruction
algorithm, define the corresponding source-level decision D_s from the
DWARF mapping. The lifting claim is:

> If MC/DC is satisfied for D_w (every condition demonstrated to
> independently affect the decision outcome), then MC/DC is satisfied
> for D_s under the assumption that the compiler preserves the
> independence-of-condition relation when lowering the source decision.

This assumption is exactly what translation validation (Pnueli et al.)
proves about correctness-preserving compilation. For DAL A use, the
lifting argument needs either:

- **Direct proof:** demonstrate that rustc + LLVM at a specific
  optimisation level preserves the independence relation. Hard.
- **Witness-and-checker:** have rustc emit the decision-marker DWARF
  itself, and trust the marker. Practical for v0.2; the small qualified
  checker (v1.0) verifies the marker matches the structure.

v0.2 ships the witness-and-checker variant and documents the
soundness-relative-to-DWARF-correctness assumption in the paper.

### Differentiation from related work

(Mirrors the README "Related work" section; see also
`docs/research/mcdc-bytecode-research.md`.)

| Tool | Measurement point | Relationship to witness |
|---|---|---|
| **JaCoCo** | JVM bytecode | Direct precedent. Branch coverage at bytecode is a shipped, accepted pattern. JaCoCo doesn't have MC/DC; v0.2 closes that gap for Wasm. |
| **Clang source-based MC/DC** | LLVM IR | 6-condition cap; needs source AST decoration; doesn't survive Wasm lowering. |
| **rustc `-Zcoverage-options=mcdc`** | HIR → MIR | 6-condition cap; pre-LLVM; complementary to witness (different blind spots). |
| **wasmcov / minicov** | LLVM source-level projected through Wasm | Different measurement point. Source-level coverage *via* Wasm execution. Complementary; not competing. |
| **Whamm** | Wasm bytecode rewriting / engine monitoring | General-purpose instrumentation DSL; possible future implementation backend for witness's rewrite phase. |
| **Wasabi** | Dynamic Wasm analysis | Precedent for Rust-based Wasm instrumentation; not coverage-specific. |

The Ferrous/DLR Rust-MC/DC effort under the SCRC 2026 Project Goal sits
at the rustc layer. Witness sits at the post-rustc Wasm layer. Both are
adopted because the measurement points have different blind spots — the
"overdo stance" from `docs/research/overdo-alignment.md`.

### v0.2 honest-assessment table

(Per the overdo-alignment C7 constraint — what v0.2 *will* clear vs.
*won't*.)

- ✅ Subprocess `--harness` escape hatch for non-wasmtime runtimes
- ✅ Per-target `br_table` counting (helper-function-call pattern)
- ✅ No artificial condition-count cap (positioning win vs LLVM/rustc-mcdc)
- ✅ Manifest schema for Decisions (`decisions: Vec<Decision>` field;
   `byte_offset: Option<u32>` on `BranchEntry`)
- ✅ Strict per-`br_if` fallback when DWARF is absent (the manifest's
   `decisions` field is empty)
- ✅ Coverage-lifting writeup with stated soundness assumption
   (`docs/paper/v0.2-mcdc-wasm.md`, 8.2k words)
- ◐ DWARF-grounded reconstruction algorithm — **v0.2.0 ships the stub**
   (`src/decisions.rs::reconstruct_decisions` returns `Ok(vec![])`); the
   full algorithm body lands in v0.2.1. The schema is locked so v0.2.0
   manifests forward-compat with v0.2.1 reconstructed output.
- ◐ Soundness *proof* of lifting — depends on rustc+LLVM optimisation
   preservation; v0.2 ships the assumption-stated variant (DEC-010)
- ❌ Per-target `br_table` DWARF labelling (the helper counts targets
   correctly but `target_index → match_arm_label` mapping needs the
   reconstruction algorithm; lands with v0.2.1)
- ❌ Component-model coverage (still v0.4)
- ❌ rivet evidence-format integration (still v0.3)
- ❌ sigil in-toto predicate emission (still v0.3)
- ❌ Check-It qualification artefact (still v1.0)

### Paper output

`docs/paper/v0.2-mcdc-wasm.md`. Target: 15-25 pages. Sections: motivation
(C-macro precedent + JaCoCo lateral), formal definition of MC/DC at Wasm,
reconstruction algorithm, coverage-lifting claim and its soundness
argument, comparison with rustc-mcdc and Clang, regulatory framing. Cite
arxiv:2409.08708 ("Towards MC/DC of Rust") directly. Source the DO-178C
post-preprocessor MC/DC clause from a credited secondary if the standard
itself stays paywalled.

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
