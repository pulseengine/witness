# MC/DC on bytecode / IR / MIR — research brief

This brief surveys how MC/DC is implemented at non-source levels across
the Java, LLVM, Rust, and WebAssembly ecosystems, and identifies the
position witness occupies. Research conducted 2026-04-24.

## Executive summary

Post-source MC/DC has a rich precedent (JaCoCo on JVM bytecode, Clang on
LLVM IR, rustc via HIR→MIR lowering), but every shipped implementation
is **source-aware at lowering time** — it decorates the IR/bytecode with
hints while the AST is still in scope. Witness is **source-blind at
instrumentation time**: we consume a `.wasm` file that has already gone
through rustc + LLVM and emit coverage instrumentation purely from the
Wasm IR. This is a novel measurement point.

The closest existing precedent is **JaCoCo**, which does bytecode-level
branch coverage on JVM class files *without* MC/DC support. JaCoCo ships,
is accepted in regulated contexts, and proves the "instrument
bytecode" pattern works. Witness v0.1 is JaCoCo-equivalent for Wasm;
v0.2 extends it to MC/DC via DWARF reconstruction, which is a research
contribution the literature has not yet made.

## 1. MC/DC on bytecode / IR — prior art

### JaCoCo (JVM bytecode)

- Open-source JVM coverage tool; operates on **compiled class files**
  (bytecode, not Java source).
- Reports instruction, line, and **branch** coverage per method.
- Explicitly does **not support MC/DC**. From the tool maintainers:
  "JaCoCo's branch coverage is probably not MC/DC… JaCoCo does not
  support MC/DC coverage."
  ([JaCoCo groups thread](https://groups.google.com/g/jacoco/c/b8bAWaWPl6I/m/eMKixUpMCAAJ))
- The JVM itself has no certified implementation for SIL 4 / DO-178C DAL
  A anyway — so the "no MC/DC" limitation has been a non-blocker for the
  tool's adoption in practice.

**Witness parallel.** witness v0.1 occupies exactly this position for
Wasm: bytecode-level branch coverage, no MC/DC decomposition yet.
JaCoCo's existence is the strongest argument that "coverage on bytecode"
is legitimate and useful.

### Clang source-based MC/DC (LLVM IR)

- Landed in Clang / LLVM in January 2024 as part of the source-based
  coverage framework.
  ([LLVM RFC](https://discourse.llvm.org/t/rfc-source-based-mc-dc-code-coverage/59244))
- Instruments at **LLVM IR**, not at the AST directly — but the IR is
  annotated with coverage metadata derived from the source AST at
  lowering time.
- Uses **reduced ordered binary decision diagrams (BDDs)** to represent
  boolean expressions and handle short-circuit evaluation.
- Encodes condition combinations as integers in `[0, 2^N)` and tracks
  hits via a **bitmap**, not masking computation.
- **Hard limit: 6 conditions per decision.** This is a space-optimisation
  constraint baked into the on-disk format.
  ([MaskRay: MC/DC and compiler implementations](https://maskray.me/blog/2024-01-28-mc-dc-and-compiler-implementations))

**Witness implication.** When v0.2 reconstructs Wasm `br_if` sequences
into source-level decisions, we should *not* inherit the 6-condition
limit. Wasm `br_if` chains from short-circuit evaluation are unbounded
in principle, and witness's measurement position does not share LLVM's
bitmap-encoding constraint.

### rustc `-Zcoverage-options=mcdc`

- Shipped as unstable in 2024 via PR #123409, tracked in
  [rust-lang/rust#124144](https://github.com/rust-lang/rust/issues/124144).
- Instrumentation decisions made during **HIR → MIR lowering** via
  `MCDCState` in `BranchInfoBuilder`; MIR emits coverage intrinsics
  that LLVM lowers to the standard bitmap counters.
- Inherits the 6-condition limit from LLVM. Decisions with >6 conditions
  or exactly 1 condition are not instrumented.
- Known issue (as of searches): "Decisions containing constant
  conditions may result [in] incorrect report[s]" — a rustc-side
  constant-folding mismatch with the LLVM coverage encoder.
- Maintenance cost has been described in the issue tracker as "a major
  burden on overall maintenance of coverage instrumentation, and a
  major obstacle to other planned improvements."
- Academic reference: "Towards Modified Condition/Decision Coverage of
  Rust" ([arxiv:2409.08708](https://arxiv.org/abs/2409.08708),
  [AIAA JAIS](https://arc.aiaa.org/doi/10.2514/1.I011558)).

**Witness implication.** rustc's MC/DC has an essentially "source-to-LLVM"
shape: MIR preserves enough structure to drive LLVM's bitmap encoder.
witness measures *after* this pipeline completes, on Wasm bytecode that
LLVM and rustc have both already transformed. The two measurement points
are non-overlapping (rustc's MC/DC does not cover post-LLVM optimisation
effects; witness catches them but loses source structure).

### GCC

- No direct MC/DC support as of January 2024.
- Pending patch by Jørgen Kvalsvik enables `gcc --coverage
  -fcondition-coverage` but remains unmerged at last search.

### Safety-Critical Rust Consortium (SCRC) / Ferrous / DLR

- The SCRC was announced in 2024 and includes Ferrous Systems and DLR
  (Deutsches Zentrum für Luft- und Raumfahrt) among other industrial
  members.
  ([Rust Foundation SCRC page](https://rustfoundation.org/safety-critical-rust-consortium/))
- In 2026 the Consortium is collaborating with the Rust Project on a
  **Rust Project Goal for MC/DC support**, driven by the DO-178C DAL A
  requirement.
  ([Rust blog: What does it take to ship Rust in safety-critical](https://blog.rust-lang.org/2026/01/14/what-does-it-take-to-ship-rust-in-safety-critical/))
- This is the effort AGENTS.md references as the "overdo stance" pair
  for witness. When Rust-level MC/DC ships under this Project Goal,
  witness remains complementary because the two tools measure different
  points in the compilation chain.

## 2. Decision granularity at IR / bytecode level — formalisms

- **Chilenski & Miller (1994)** introduced MC/DC formalisation at source
  level; the paper is the standard reference but is closed-access.
- **Vilkomir & Bowen** extended MC/DC with formal operational semantics
  (2001, 2004).
- **No IR-level formalisation** was surfaced in this research pass.
  The literature on "what counts as a decision after short-circuit
  lowering" is sparse. This is the gap witness v0.2's DWARF-reconstruction
  paper can fill.
- **DO-178C / DO-330 precedent for post-preprocessor C** MC/DC: accepted
  since 1992 (DO-178B), explicitly stated in DO-178C Annex A. Open-access
  primary source not located in this pass; AGENTS.md cites the precedent
  but sourcing the clause verbatim is a separate follow-up.

## 3. Translation validation and coverage lifting

- Translation validation (Pnueli, Siegel & Singerman 1998) is the
  classic source for proving that a compiler preserves program
  semantics. CompCert is the flagship implementation for C.
- The overdo blog post witness aligns with (see
  `overdo-alignment.md`) explicitly positions Z3-backed translation
  validation on the Wasm optimiser as a DO-333 "under-valued asset".
- **No work on "coverage lifting"** (projecting low-level coverage back
  to source-level decisions) was surfaced in this pass. This is another
  candidate topic for witness's v0.2 paper, distinct from the
  decision-granularity paper.

## 4. Existing Wasm coverage tooling

Three tools with partial overlap, none covering witness's v0.1 angle:

### Wasmcov

- Rust library + CLI for coverage analysis of Wasm modules.
  ([wasmcov.io](https://hknio.github.io/wasmcov/))
- Leverages LLVM's coverage instrumentation via `minicov`; the coverage
  signal is **source-level** (generated by LLVM before compilation to
  Wasm), not Wasm-structural.
- Designed for embedded, blockchain, and constrained environments.

**Relationship to witness.** Different measurement point: wasmcov is
source-level-projected-through-Wasm (useful when source is available);
witness is Wasm-structural (useful when source is not available, or when
you want to catch post-LLVM divergences). Complementary, not competing.

### Whamm

- Wasm instrumentation DSL with an optimising compiler that targets
  both bytecode rewriting and an "engine monitoring interface" for
  runtimes that expose one.
  ([arxiv:2504.20192](https://arxiv.org/html/2504.20192),
  [New Stack](https://thenewstack.io/meet-whamm-the-webassembly-instrumentation-framework/))
- General-purpose instrumentation framework; not coverage-specific.
- Published April 2025; witness project predates serious knowledge of
  Whamm in this repo.

**Relationship to witness.** Whamm is a plausible *implementation
backend* for witness in a future refactor — if witness's rewrite phase
grows beyond `walrus`'s ergonomics, Whamm's bytecode-rewriting target
might be a better fit. No immediate action, but worth tracking.

### Wasabi

- Dynamic analysis framework for Wasm with its own parser,
  instrumentation library, and encoder in Rust.
  ([github.com/danleh/wasabi](https://github.com/danleh/wasabi))
- Older (pre-2020); academic provenance.
- General-purpose dynamic analysis rather than coverage-specific.

**Relationship to witness.** Precedent for "Rust-based Wasm
instrumentation framework". No direct overlap in what it measures.

### minicov

- Small LLVM coverage runtime for no-std Rust targets, including Wasm.
- Used by wasmcov under the hood.
- Operates at LLVM IR level via LLVM's standard coverage
  instrumentation.

### wasm-bindgen-test coverage

- Experimental coverage feature in wasm-bindgen-test that relies on
  minicov.
  ([wasm-bindgen guide](https://rustwasm.github.io/docs/wasm-bindgen/wasm-bindgen-test/coverage.html))
- Wraps minicov for the wasm-bindgen testing harness; same LLVM-level
  measurement as wasmcov.

## 5. Wasm-specific instrumentation patterns

- **walrus** (used by witness): Rust AST-level Wasm rewriting. Mature,
  ergonomic, maintained by the Rust and WebAssembly Working Group.
- **wasm-tools**: Lower-level, spec-tracking Wasm manipulation toolkit
  from the Bytecode Alliance. Use when walrus can't express what you
  need.
- **Counter mechanism choices** surveyed:
  - Exported mutable globals (witness v0.1). Trivial to read from any
    runtime that exposes global exports. Standard Wasm API.
  - Host-imported counter function. Requires host cooperation; useful
    when you want centralised accounting but adds call overhead per
    branch.
  - Linear-memory counters + exported dump function (the original
    witness DESIGN.md plan). Works across runtimes that don't expose
    globals, but requires serialisation contract between the
    module-under-test and the harness.
- **Component model implications.** Components wrap one or more core
  modules. Instrumenting a component means either (a) extracting core
  modules, instrumenting each, and re-composing, or (b) operating on
  the component directly (which walrus does not currently support
  well). v0.1 handles core modules only. Component-model support is a
  v0.4 item aligned with the meld fusion work.

## 6. Implications for witness

**Confirmations of the v0.1 design:**
- The JaCoCo precedent validates "bytecode-level branch coverage as a
  shipped tool" — v0.1's scope is defensible.
- The exported-mutable-globals mechanism is strictly simpler than any
  existing post-rustc coverage tool's extraction path, and removes the
  cooperation-protocol overhead wasmcov/wasm-bindgen-test inherit from
  LLVM's bitmap encoder.
- The "complementary-not-competitive" framing against Ferrous/DLR
  Rust-level MC/DC is well-aligned with the SCRC Project Goal's own
  scoping — they measure decisions at the Rust source side; witness
  measures them after rustc+LLVM lowering, which is a different blind
  spot.

**Constraints for v0.2:**
- DWARF reconstruction should *not* inherit LLVM's 6-condition limit.
  Wasm `br_if` chain length is unbounded, and witness's measurement
  position does not share LLVM's bitmap-encoding constraint.
- The "soundness of reconstruction" proof is a genuine contribution —
  no prior work in the surfaced search results formalises
  decision-granularity at IR/bytecode level.
- The reconstruction algorithm's closest neighbour is rustc's
  `MCDCState`, but that runs at HIR→MIR with full source-AST context.
  Witness at Wasm has only DWARF-in-Wasm + control-flow merges to
  work with.

**Research citations that witness's v0.2 paper will need:**
- Chilenski & Miller (1994) — MC/DC original formalism (closed access;
  sourcing a credited secondary open reference is a follow-up).
- Vilkomir & Bowen (2001, 2004) — MC/DC operational semantics.
- Pnueli, Siegel & Singerman (1998) — Translation validation.
- PR #123409 + rust-lang/rust#124144 — rustc MC/DC implementation.
- LLVM source-based coverage RFC — Clang MC/DC implementation.
- arxiv:2409.08708 — "Towards MC/DC of Rust" (most relevant academic
  paper; read in full and cite directly when v0.2 paper is drafted).

## 7. Open questions remaining

- **DO-178C Annex A clause** explicitly naming post-preprocessor C MC/DC
  as acceptable: AGENTS.md asserts the precedent; sourcing the clause
  text verbatim in an open-access document is still TODO.
- **Component-model instrumentation**: walrus does not fully support
  components; need to decide at v0.4 whether to extract-and-reassemble
  core modules or build on `wasm-tools` instead.
- **Per-target `br_table` counting**: v0.2 needs either a selector
  reconstruction pattern or a helper-function call (the latter inflates
  per-branch cost from 4 instructions to a function call + return).
- **Harness-cooperation mode** (v0.2 `--harness <cmd>`): the protocol
  between the subprocess harness and the witness runner is not yet
  specified. Most likely a file-based handshake (harness writes
  `$WITNESS_OUTPUT` after reading globals; runner reads and merges).

---

## Sources

Web search on 2026-04-24. All links verified live at the time of search.

- [Tracking implementation for MC/DC · Issue #124144 · rust-lang/rust](https://github.com/rust-lang/rust/issues/124144)
- [Implement Modified Condition/Decision Coverage by ZhuUx · PR #123409](https://github.com/rust-lang/rust/pull/123409)
- [MC/DC and compiler implementations — MaskRay](https://maskray.me/blog/2024-01-28-mc-dc-and-compiler-implementations)
- [Instrumentation-based Code Coverage — The rustc book](https://doc.rust-lang.org/rustc/instrument-coverage.html)
- [Toward MC/DC of Rust — AIAA JAIS](https://arc.aiaa.org/doi/10.2514/1.I011558) / [arxiv:2409.08708](https://arxiv.org/abs/2409.08708)
- [RFC: Source-based MC/DC Code Coverage — LLVM Discourse](https://discourse.llvm.org/t/rfc-source-based-mc-dc-code-coverage/59244)
- [What does it take to ship Rust in safety-critical? — Rust Blog](https://blog.rust-lang.org/2026/01/14/what-does-it-take-to-ship-rust-in-safety-critical/)
- [Safety-Critical Rust Consortium — Rust Foundation](https://rustfoundation.org/safety-critical-rust-consortium/)
- [JaCoCo Coverage Counters](https://www.eclemma.org/jacoco/trunk/doc/counters.html)
- [JaCoCo groups: what kind of branching coverage](https://groups.google.com/g/jacoco/c/b8bAWaWPl6I/m/eMKixUpMCAAJ)
- [Wasmcov](https://hknio.github.io/wasmcov/)
- [Whamm Wasm instrumentation framework — arxiv:2504.20192](https://arxiv.org/html/2504.20192)
- [Meet Whamm — The New Stack](https://thenewstack.io/meet-whamm-the-webassembly-instrumentation-framework/)
- [Wasabi Wasm dynamic analysis](https://github.com/danleh/wasabi)
- [wasm-bindgen-test coverage guide](https://rustwasm.github.io/docs/wasm-bindgen/wasm-bindgen-test/coverage.html)
