# v0.7 scaling roadmap — from curated verdicts to real-application MC/DC

Research brief, 2026-04-25. Inputs: `DESIGN.md`, `AGENTS.md`, `CHANGELOG.md`,
`artifacts/{requirements,features,design-decisions}.yaml`,
`docs/research/mcdc-bytecode-research.md`,
`docs/research/v05-component-witness.md`,
`docs/research/v05-loom-meld-upstream.md`,
`docs/research/rivet-evidence-consumer.md`,
`docs/research/overdo-alignment.md`, the rivet `coverage_evidence` module,
and a web pass on candidate workloads, MC/DC scaling literature, and
visualiser support.

The brief proposes the v0.9 destination workload, the v0.7 capability list,
the scaling break-points witness will hit between 7-decision toy
verdicts (v0.6) and a 5,000-decision real application (v0.9), the V-model
artefact graph that makes a 5,000-decision claim auditable by a human, and
the provisional rivet artefacts ready to land in `artifacts/` as
`status: proposed`.

## Constraints inherited from earlier versions

- **No artificial condition-count cap** (REQ-015). Witness must measure
  decisions of any size; v0.7 cannot regress this.
- **Semantic preservation under instrumentation** (REQ-004). Round-trip
  testing remains the gate. v0.7 picks workloads that *can* be
  round-tripped under wasmtime in CI within a sane budget.
- **Manifest schema stable within a major version**. v0.7 may extend the
  schema (with `#[serde(default, skip_serializing_if = ...)]`) but cannot
  break v0.5 readers.
- **Reports deterministic for `(module, run-data)`**. Any aggregation,
  bucketing, or rollup added in v0.7 must hash the same input to the same
  bytes.
- **Component-model coverage is v0.6/v0.5 work**, not v0.7's job.
  v0.7 measures core Wasm modules with sufficient decision density to
  stress the report path.
- **Overdo stance** — witness composes with rustc-mcdc; v0.7's scaling
  story does not need to displace anything.

## 1. Recommended v0.9 destination workload

### Primary pick — **`httparse`** (HTTP/1.x push parser)

- Source: <https://github.com/seanmonstar/httparse>
- License: MIT/Apache-2.0 (compatible).
- Compile target: trivial `wasm32-wasip2`. `no_std`-clean optionally;
  builds without `ring` / `mio` / native deps. The crate's only build
  oddity is a SIMD acceleration path under `build.rs` — gated by a cfg
  that we disable for the wasm target.
- Decision-count estimate: **1,200–1,800 decisions** post-rustc/LLVM. The
  parser is hand-rolled with character-class fall-throughs (`is_token`,
  `is_header_value_token`, `is_uri_token`, `is_vchar` etc.), each of
  which lowers to a `br_table` or a `br_if` chain. Header continuation,
  CR/LF / LF handling, version parsing, and chunked-body framing each
  contribute ~50–200 conditions in concentrated decision blocks.
- Test suite: ~70 unit tests in `src/lib.rs` plus the project's
  fuzz corpus (folded into a corpus runner in v0.7's harness brief).
  Tests run in <1 second under wasmtime.
- Why it is a good MC/DC subject: HTTP/1.x parsing is full of compound
  boolean conditions on bytes (e.g. `b == b' ' || b == b'\t'`,
  `is_token_char(b) && b != b':'`). Each compound condition is exactly
  the construct MC/DC was designed to cover. The crate already runs
  fuzz/coverage in upstream CI, so the test suite is real.
- V-model story: the crate satisfies a single high-level requirement —
  *"parse RFC 7230 HTTP/1.x messages without panicking on malformed
  input"* — that decomposes naturally into ~40–60 sub-requirements
  (one per RFC clause: request-line shape, header field, CRLF rules,
  obs-fold, chunked-encoding framing, etc.). Each sub-requirement maps
  to a contiguous block of decisions, which is the rollup unit witness
  needs. The crate is **shipped in tokio's `hyper`** and so the
  V-model story compounds: witness covers the parser; downstream
  consumers inherit the coverage claim via supply-chain evidence
  (sigil bundle).

### Backup #1 — **`wasmparser`** (event-driven Wasm binary parser)

- Source: <https://github.com/bytecodealliance/wasm-tools/tree/main/crates/wasmparser>
- License: Apache-2.0 with LLVM exception.
- Compile target: `wasm32-wasip2` clean. Already used by witness itself
  (DWARF section extraction in `decisions.rs`).
- Decision-count estimate: **3,500–5,000 decisions** — the crate is one
  giant validator with per-instruction validation arms, so it will be
  the densest MC/DC subject in the candidate set.
- Test suite: bundled `tests/` directory plus the wasm-tools spec-test
  driver. Runs in 1–2 seconds with the bundled fixtures.
- Why it is a good MC/DC subject: parser/validator state machines have
  deeply nested compound conditions on `(version, section_kind,
  feature_flag)` tuples. Whamm and witness both consume wasmparser, so
  dogfooding it gives the project leverage — coverage gaps are bugs we
  notice ourselves.
- V-model story: requirements decompose by Wasm spec section
  (preamble, type section, function section, code section, data section,
  custom section, …). Each section-validator is a small, contained
  decision graph; rollup at section level matches how the spec is
  written. **Drawback:** decision count exceeds httparse, which makes
  it a stress test rather than a "real-application baseline" — keep it
  as a stretch target after v0.7 stabilises.

### Backup #2 — **`witness-core` itself** (dogfood)

- Source: this repo's `crates/witness-core/`.
- Compile target: already verified `wasm32-wasip2` clean (v0.5 ships
  the artefact).
- Decision-count estimate: **600–900 decisions** today, growing to
  ~1,500 after v0.7 lands. Too small to be the headline v0.9 destination
  but the **only candidate where the V-model and the source code share
  a repo**, which makes it the lowest-friction integration test for
  the rivet artefact graph.
- Why it is a good MC/DC subject: `instrument.rs` (926 lines), `decisions.rs`
  (DWARF parser, 422 lines), `diff.rs` (set algebra over manifests, 441
  lines) all carry the kind of compound conditions MC/DC differentiates
  on. Mutation testing already runs over these (REQ-019); MC/DC pairs
  with mutation results to confirm the test suite catches not only
  mutated branches but mutated *condition independence*.
- V-model story: every decision links back to an existing REQ-/FEAT-/DEC-
  artefact in this repo. This is the candidate that proves the rivet
  artefact graph mechanics end-to-end before we point witness at an
  external crate that does not own its own rivet store.

### Candidates rejected and why

- **`tokio` core** — too much native (mio, parking_lot raw syscalls,
  io_uring); the parts that *do* compile (`tokio-util`, `bytes`) are
  too small to test scaling.
- **`rustls`** — would be ideal as a state-machine subject, but ring
  has only partial wasm32 support today (web-sys for randomness,
  vanilla-C fallback in 0.17), and the crate's CI does not run a
  full handshake corpus on wasm. Defer to v1.0+.
- **`serde_json`** core — compiles cleanly; decision-count is
  moderate (~800) but the test suite is dominated by serde derive
  fixtures that exercise the same parser arms repeatedly. Witness would
  report high coverage trivially without stressing scale.
- **`url`** / **`nom`** / **`pest`** — all clean wasm targets but each
  individual crate is decision-light (200–600). Could be combined into
  a "small parser bundle" workload as a v0.7 staging step.
- **`walrus`** — pulls `id-arena`, fine for wasm; decision count
  ~1,500. Reasonable backup but witness depends on walrus, so a
  failure here cascades into witness's own correctness story.
- **MoonBit-compiled programs** — interesting because MoonBit emits
  Wasm directly without rustc/LLVM. v0.7 should add **one** MoonBit
  fixture as a "compiler-portfolio" data point, not as the v0.9
  destination. The DWARF reconstruction depends on rustc DWARF shape;
  MoonBit's DWARF (if any) is a separate research item.
- **`kiln`/`spar`/`loom`/`meld`** sibling repos — checked but not
  accessible in this thread; flagged for the maintainer to inspect.
  Likely too small individually; spar in particular would be valuable
  if it has the MBSE state machine the user mentions.

**Pick**: `httparse` as the v0.9 destination, `wasmparser` as the
density stress-test, `witness-core` as the dogfood-and-rivet integration
test. v0.7 builds the capability stack on the dogfood subject and
validates against `httparse`.

## 2. v0.7 capability list (V-model-ordered)

Ordered by V-model criticality — verification-side first (without these,
the destination measurement is not measurable), then evidence-side, then
ergonomics-and-CI.

### V&V-side capabilities (must land first)

1. **FEAT-201 — Streaming counter encoding** (`witness instrument
   --counters=stream` mode). Today every branch gets a Wasm `mut i32`
   global, which a 5,000-decision module exports as 5,000 globals; some
   wasmtime configurations and most browser hosts cap exports at 2,048
   or 4,096. Add a packed-counter alternative: one `i64`-element memory
   region exported via a single `__witness_counters` memory + a
   `__witness_counter_count` global. Manifest schema gains
   `counter_layout: "globals" | "memory"`. Default stays `globals` for
   v0.7; `memory` is opt-in via `--counters=memory`. Round-trip tested
   on httparse. Derives from REQ-001, refines DEC-003.

2. **FEAT-202 — Counter saturation policy**. `u32` overflows under fuzz
   loops trivially. v0.6 still uses `i32` add-with-wraparound. v0.7
   bumps the global type to `i64` and documents the saturation policy:
   counters saturate at `i64::MAX` (rather than wrap), and the
   instrumentation includes a single saturating-add helper in the
   `--counters=memory` path. Pure schema bump in `globals` mode (the
   instrumented add becomes `i64.add` with the same observable
   semantics for hit-counts < 2^63).

3. **FEAT-203 — Decision-density validation**. After reconstruction,
   v0.7 emits a "decision-density check" line (decisions per function,
   conditions per decision, max-condition decision). Above configurable
   thresholds (default: 32 conditions / decision) the report flags the
   decision for human review — not because the cap is real, but
   because dense decisions are the place where the reconstruction
   algorithm is most likely to be wrong. Rooted in DEC-006's "honest
   interpretation" stance.

4. **FEAT-204 — Inlined-subroutine handling for DWARF reconstruction**.
   v0.5's reconstruction uses the innermost source line, which collapses
   inlined decisions over-eagerly (`decisions.rs:25-29` already flags
   this). v0.7 walks `DW_TAG_inlined_subroutine` and treats inlined
   ranges as their own lexical scope so a single-line `match` macro
   that expands into N decisions is reported as N decisions, not one.
   Required for httparse: `is_token` etc. are inlined hot paths.

5. **FEAT-205 — Per-target `br_table` decision rollup**. v0.5 ships
   per-target counters (FEAT-007) but does not group them into
   source-level `match`-arm decisions. v0.7 extends `Decision` so a
   `Decision { conditions: Vec<u32>, kind: DecisionKind }` can carry
   either `BrIfChain` (today) or `MatchArms` (new). Required for
   wasmparser and httparse — pattern matching on bytes is half their
   decision surface.

### Evidence-side capabilities (without these, scale evidence is not auditable)

6. **FEAT-206 — Module rollup view in reports**. Today reports list
   every uncovered branch. At 5,000 decisions that is unreadable. v0.7
   adds a `Report::module_rollup()` that aggregates by
   `(source_file, function)` with covered / total / percentage and
   emits the per-decision detail behind a `--detailed` flag. Default
   text output becomes a triage tree.

7. **FEAT-207 — Per-module rivet evidence with auto-generated
   sub-requirements**. v0.5's `rivet-evidence` requires a hand-written
   `branch_id → artefact_id` map. At 5,000 decisions hand-mapping is
   infeasible. v0.7 auto-generates a synthetic
   `REQ-AUTO-<file>-<function>` requirement per `(source_file, function)`
   pair when no explicit mapping exists, so coverage flows into rivet
   without per-branch annotation. The synthetic requirements carry
   `status: derived` and `tags: [auto-generated, witness]`. Human
   reviewers add narrative requirements over time, replacing synthetic
   ones; the schema is forward-compatible.

8. **FEAT-208 — Per-PR coverage delta budget**. Today's
   `witness-delta.yml` workflow uploads delta as an artefact but does
   not gate. v0.7 adds an opt-in **per-decision** budget to
   `witness diff`: PR may not lose MC/DC coverage on any decision
   already covered on `main`. Configurable thresholds for new code.
   Implements the rivet "evidence ratchet" pattern.

9. **FEAT-209 — Persistent decision IDs across builds**. Today decision
   ids are dense indices into the manifest — they shift if a function
   is added before another. v0.7 adds a content-addressed decision id:
   `sha256(function_name || source_file || source_line || condition_index)`
   truncated to a stable 16-hex-char id. The integer id remains in the
   manifest for fast lookups; the content id is the one rivet stores
   in evidence. Enables the v0.6 "this decision was uncovered last
   release" archaeology that the curated verdict suite cannot test.

### CI / ergonomics (without these, scaling stops in the maintainer's terminal)

10. **FEAT-210 — Incremental coverage** (`witness incremental --since
    <ref>`). Compute coverage only on the decisions whose source
    `(file, line)` changed since `ref`. Internally still runs the full
    binary (no compiler integration), but the report only emits the
    delta — same load on the runtime, much smaller report surface.
    Useful to keep PR feedback time bounded.

11. **FEAT-211 — Codecov / SonarQube exporter for MC/DC**. LCOV's `BRDA`
    is branch-coverage only; v0.5 already emits it. v0.7 adds a
    SonarQube generic-format XML exporter (`witness sonar`) that does
    carry condition-level coverage and a Cobertura emitter for tools
    that read condition-coverage from Cobertura. Codecov accepts the
    LCOV path today; v0.7 reaches enterprise dashboards that LCOV
    cannot cover.

12. **FEAT-212 — Compressed run-record format** (`run.json` →
    `run.cbor` with optional zstd). For a 5,000-decision module with
    1,000 invocations the JSON run-record is ~50 MiB. v0.7 supports
    CBOR + zstd (gated behind a feature flag) which empirically
    compresses ~10:1 on coverage data; downstream consumers (rivet,
    sigil) already accept CBOR via existing serde.

13. **FEAT-213 — Module-size regression check**. The instrumentation
    multiplier on httparse will be measured at v0.7 release time
    (target: ≤ 1.8x; hard ceiling: 3x). CI fails if a release exceeds
    the ceiling. The check is informational on PRs.

14. **FEAT-214 — Memory-budget runtime caps**. v0.7's wasmtime runner
    sets a default `Config::max_wasm_stack(1MB)` and a
    `StoreLimits::memory_size(256 MB)` so a 5,000-decision module
    cannot OOM the CI runner without an explicit `--memory-limit`
    override. Reuses existing wasmtime APIs.

15. **FEAT-215 — Harness-mode counter streaming**. v0.5's harness
    protocol writes the full counter snapshot at process exit. For
    a fuzz-driven harness with 100k iterations this means the harness
    holds the entire counter array in memory the whole time. v0.7
    extends the protocol so the harness *may* write incremental
    snapshots that witness merges; backwards-compatible with v0.5
    harnesses (the new "snapshot-stream" key in the env-var contract
    is opt-in).

## 3. Scaling break-points

| Surface | v0.6 (curated, ~50 decisions) | v0.7 target (~5,000 decisions) | What breaks first | Mitigation |
|---|---|---|---|---|
| Counter globals | 50 exports | 5,000 exports | wasmtime `Config::max_wasm_globals` defaults to 4,096 in some configurations; some browser hosts cap at 2,048; module-validation linear scan over exports becomes the dominant load cost | FEAT-201 packed-memory layout; default stays globals until 1,024 decisions, auto-switches above (with a log line) |
| Counter integer width | `i32`, no overflow concern at hand-curated test counts | Fuzz loops hit `i32::MAX` in seconds | Wraparound makes "covered" / "uncovered" indistinguishable | FEAT-202 widen to `i64` with saturation |
| Manifest size | 5 KB JSON | 2–5 MiB JSON | `serde_json::from_reader` on 5 MiB allocates the whole tree; load time dominates `witness report` cold start | Stream-parse via `serde_json::StreamDeserializer` for the `branches` array; release CBOR encoding (FEAT-212) |
| Run-record size | 8 KB JSON | 50 MiB+ JSON | Disk + git-LFS pressure on CI artefacts; codecov upload limits | FEAT-212 CBOR + zstd (~10:1 ratio empirically) |
| DWARF parsing | <1 MB `.debug_line` per module | 10–30 MB across `.debug_line` / `.debug_info` / `.debug_str` | gimli's `EndianSlice<LittleEndian>` reader is fast but `build_line_map` allocates one `BTreeMap` entry per row; 30 MB → ~1M rows → ~200 MB heap | Bound the line map to functions referenced by manifest entries; stream rather than fully buffer; v0.7 measures actual high-water under httparse and decides whether the gimli `read::Dwarf<EndianSlice>` zero-copy form suffices |
| Reconstruction grouping | O(N) over branches | O(N²) in the worst case if grouped by line and lines repeat | At 5,000 conditions this is 25M comparisons — still milliseconds, but the over-grouping issue (one source line, many decisions) gets worse | FEAT-204 inlined-subroutine handling cuts the equivalence classes; sort branches by `(function, file, line)` first to make grouping linear |
| Report render time | <50 ms text | seconds for full per-branch dump | Default text mode prints one line per uncovered branch; 5,000 lines is a wall of green | FEAT-206 module rollup default; `--detailed` for the wall |
| Wasm module size | original 50 KB → instrumented 80 KB (1.6x) | original 200 KB → instrumented 400-600 KB (2-3x) | Acceptable; one global per branch is ~10 bytes of import/export header per branch; the helper functions for `br_table` per-target are O(1) per call site | FEAT-213 regression check: hard cap 3x, informational at 1.8x |
| Per-row trace storage | not implemented | required for fuzz / property test harnesses | A "trace" row per invocation × decision is `O(invocations × decisions)` — not feasible to store in full | v0.7 does **not** ship per-row traces; counters only. v0.8 considers reservoir-sampled traces for failed-test diagnosis |
| LCOV ingestion | small, parses fine | codecov rejects > 16 MB LCOV uploads; SonarQube reports degrade | LCOV is text-based, very sparse encoding | FEAT-211 SonarQube generic-XML and Cobertura paths; LCOV stays for codecov but emits the rolled-up `Decision`s only, not the strict-fallback per-`br_if` flood |
| In-toto predicate size | small statement | a 50 MiB run-record inside an in-toto Statement is unwieldy | DSSE envelope can sign anything but consumers often reject > 10 MiB | Predicate carries the *report* (rolled-up percentages and uncovered-decision list), not the raw counters. Raw counters live in the rivet evidence file at `coverage_evidence/`. Already true today; v0.7 documents the split. |
| CI runtime | <1 minute | target <5 minutes including build, instrument, run, report, upload | wasmtime cold start, gimli parsing, codecov upload | FEAT-210 incremental mode for PRs; FEAT-208 delta budget runs only on changed decisions |
| Memory budget per `witness run` | <50 MB | should stay <512 MB even on httparse fuzz corpus | wasmtime store + counters + manifest in memory simultaneously | FEAT-214 default `StoreLimits` |

**Biggest single risk**: DWARF parsing memory at scale. Gimli's
in-memory line program reconstruction is fast but allocates per row,
and Rust's standard libcore DWARF in `wasm32-wasip2` includes
substantial inlined-subroutine metadata that compounds. v0.7 should
measure first (FEAT-203's density check is the diagnostic) and keep a
fallback to "skip reconstruction, fall back to strict per-`br_if`" if
DWARF processing exceeds a configurable budget.

## 4. V-model traceability at scale

The v0.6 curated verdict suite has 7 decisions, each hand-mapped to one
REQ- artefact and one V-MODEL.md narrative. That model does not
generalise — at 5,000 decisions a human cannot read 5,000 V-MODEL.md
files, and at AI-velocity authorship, no human will write them either.

### The artefact graph at scale

The rivet artefact graph composes by *aggregation*, not by per-decision
manual link. Three layers:

**Layer 1 — Module / file rollup (auto-generated).** For each
`(crate, source_file)` pair, witness emits one synthetic
`REQ-AUTO-<crate>-<file>` requirement at `status: derived` (FEAT-207).
Decisions in that file link to that synthetic requirement via
`witness-rivet-evidence/v1`'s artefact map. The synthetic requirement
carries enough metadata (file path, function-name set, condition count,
last-measurement coverage %) for a reviewer to ask "what does this file
do?" without reading individual decisions. **One artefact per file** —
`httparse` has 4 source files, so 4 synthetic requirements; wasmparser
has ~50, so 50.

**Layer 2 — Crate-level claim.** A hand-written `REQ-<crate>-<n>` per
RFC clause / spec section / state-machine state, linked to the
underlying synthetic requirements via rivet's `refines` link. A
reviewer reads ~40 of these for httparse rather than 1,500 individual
decisions. Coverage rolls up: a `REQ-<crate>` is "covered" iff every
synthetic requirement it refines is at the configured threshold (e.g.
85% MC/DC). Rivet's existing coverage rule machinery (rivet's
`coverage.rs`) computes the rollup with no witness-side change.

**Layer 3 — Domain claim.** Hand-written `FEAT-<domain>-<n>` ("the HTTP
parser does not panic on malformed input") link to crate-level
requirements via `satisfies`. This is the layer assessors see. The
chain `FEAT → REQ-<crate> → REQ-AUTO-<crate>-<file> → BranchEntry` is
four hops; rivet's `impact` command walks it.

### Generated requirements vs. hand-written requirements

Synthetic requirements carry metadata that distinguishes them from
human-written ones:

```yaml
- id: REQ-AUTO-httparse-src-iter
  type: requirement
  title: "Coverage rollup for httparse::src::iter"
  status: derived
  description: >
    Auto-generated by witness 0.7 to anchor coverage evidence for
    file httparse/src/iter.rs (functions: as_slice, peek, next,
    advance, len). Replace this requirement with a hand-written one
    that captures the function-set's intent when the V-model story
    matures.
  tags: [auto-generated, witness, v0.7]
  fields:
    priority: should
    category: functional
    auto_generated: true
    last_coverage_pct: 92.4
    decision_count: 47
```

Rivet's validator treats `status: derived` as a non-blocking warning,
not an error, so the artefact graph stays valid as humans replace
synthetic requirements over time.

### How a human reviewer audits 5,000 decisions

The audit workflow is:

1. **Start at FEAT-level.** "Does the parser satisfy RFC 7230?" Read
   ~5 FEAT artefacts.
2. **Drill to crate-REQ.** For each FEAT, look at the linked crate-level
   requirements (~40). Each carries a coverage % and a list of
   uncovered decisions.
3. **Spot-check uncovered decisions.** For each crate-REQ below
   threshold, the coverage % links to the witness report's
   per-decision detail. Reviewer reads the source line + condition
   table for that one decision. ~20 decisions reviewed per audit.
4. **Trust auto-generated layer.** The reviewer never reads
   `REQ-AUTO-*` directly unless investigating a specific gap.

This compresses a 5,000-decision audit to ~50 documents read. The
math: 5 FEAT + 40 crate-REQ + 5 narrative DEC + ~20 spot-check
decisions = ~70 reads. Comparable to the audit budget for a curated
v0.6 verdict (where the reviewer reads every V-MODEL.md).

### What makes this defensible to assessors

DO-178C and ISO 26262 do **not** require per-decision narrative
requirements; they require traceable evidence. The rivet artefact
graph + witness coverage report + sigil-signed bundle satisfies
"traceable" — the assessor can drill from claim to bytes. The
synthetic-requirement layer is an implementation detail of the
traceability, not a regulatory artefact in itself. Cantata, LDRA, and
VectorCAST all generate function-level coverage entries the same way —
witness's contribution is making the layer explicit and machine-readable.

### Rivet's `coverage.rs` already supports this

The rivet `coverage.rs` consumer (in
`/Users/r/git/pulseengine/rivet/rivet-core/src/coverage.rs`) computes
*traceability-rule* coverage today — "what fraction of REQ artefacts
have a satisfies link to a FEAT". Adding a coverage-evidence rollup
("what fraction of decisions linked to REQ-X are covered above
threshold") is a small extension. The witness `rivet-evidence` schema
already carries enough data; rivet's open `feat/witness-coverage-evidence-consumer`
branch already lays the groundwork. v0.7's FEAT-207 ships the witness
side of the auto-generation; the rivet side is a follow-up issue.

## 5. Provisional rivet artefacts

Drafts ready to land in `artifacts/` with `status: proposed`. IDs follow
the existing project convention (REQ-, FEAT-, DEC-) and continue from
the v0.5 numbering (REQ-026 was the last v0.5 reference; REQ-100 series
reserved for v0.7 to avoid collisions with v0.6's verdict-suite work).

### Requirements

```yaml
- id: REQ-101
  type: requirement
  title: Streaming counter encoding for high-decision modules
  status: proposed
  description: >
    The system shall provide a `--counters=memory` mode for
    `witness instrument` that places counters in a single exported
    Wasm memory rather than as N exported globals. Required for
    modules with > 2,048 decisions where some Wasm hosts impose
    export-count limits. The default mode remains exported globals
    (DEC-003) to preserve v0.5 behavioural compatibility.
  tags: [v0.7, scale, instrumentation]
  fields:
    priority: must
    category: functional

- id: REQ-102
  type: requirement
  title: i64 saturating counters
  status: proposed
  description: >
    Counters shall be 64-bit and shall saturate at i64::MAX rather
    than wrap. Required to keep "covered / uncovered" distinguishable
    on fuzz-driven harnesses that exceed 2^31 hits per branch.
    Schema-compatible with v0.5; the manifest gains a
    `counter_width: "i64"` field defaulted to "i64" on v0.7+ and
    "i32" on read of older manifests.
  tags: [v0.7, scale, instrumentation]
  fields:
    priority: must
    category: non-functional

- id: REQ-103
  type: requirement
  title: Auto-generated synthetic requirements for coverage rollup
  status: proposed
  description: >
    When `witness rivet-evidence` is invoked without a
    `branch_id → artefact_id` map, the system shall auto-generate one
    synthetic `REQ-AUTO-<crate>-<file>` artefact per `(crate, source_file)`
    pair represented in the manifest's `decisions` field. Each
    synthetic requirement carries `status: derived`, decision count,
    last-measurement coverage percentage, and a placeholder description
    pointing the reader at the source-file path. Required to make
    coverage on real-application-scale (5,000-decision) modules tractable
    without per-branch hand annotation.
  tags: [v0.7, rivet, scale, evidence]
  fields:
    priority: must
    category: functional

- id: REQ-104
  type: requirement
  title: Module-rollup default report mode
  status: proposed
  description: >
    The default `witness report` text output for a module with > 100
    decisions shall be a `(file, function) -> covered/total/percent`
    rollup. Per-branch detail moves behind a `--detailed` flag.
    Required so a 5,000-decision report fits on one screen.
  tags: [v0.7, reporting, scale]
  fields:
    priority: must
    category: functional

- id: REQ-105
  type: requirement
  title: Inlined-subroutine handling in DWARF reconstruction
  status: proposed
  description: >
    `decisions::reconstruct_decisions` shall walk
    `DW_TAG_inlined_subroutine` entries and treat each inlined range
    as its own lexical scope. Required because real-application
    parsers (httparse, wasmparser) inline character-class predicates
    aggressively; the v0.5 implementation collapses these into one
    over-grouped decision per call site.
  tags: [v0.7, mcdc, dwarf]
  fields:
    priority: must
    category: functional

- id: REQ-106
  type: requirement
  title: Per-target br_table decision rollup
  status: proposed
  description: >
    `Decision` shall carry a `kind: DecisionKind` enum with variants
    `BrIfChain` (existing) and `MatchArms` (new). For `match` lowering
    via `br_table`, the per-target counters (FEAT-007) shall be
    grouped into a `MatchArms` decision when DWARF maps targets to a
    common source-line match expression. Required for accurate MC/DC
    accounting on Rust pattern-matching code.
  tags: [v0.7, mcdc, instrumentation]
  fields:
    priority: must
    category: functional

- id: REQ-107
  type: requirement
  title: Compressed run-record format (CBOR + zstd)
  status: proposed
  description: >
    The system shall accept and emit run records in CBOR with optional
    zstd compression behind a `compression` cargo feature. JSON
    remains the default. Required for 50 MiB+ run records that JSON
    encoding makes infeasible to upload to codecov / sigil bundles.
  tags: [v0.7, scale, schema]
  fields:
    priority: should
    category: non-functional

- id: REQ-108
  type: requirement
  title: SonarQube and Cobertura emitters for MC/DC
  status: proposed
  description: >
    The system shall emit coverage in SonarQube generic-format XML
    (with condition-coverage attributes) and Cobertura XML, in
    addition to the existing LCOV path (REQ-023). Required because
    LCOV's BRDA carries branch but not MC/DC condition data; enterprise
    visualisers (SonarQube, Cobertura consumers) require these formats.
  tags: [v0.7, reporting, ecosystem]
  fields:
    priority: should
    category: functional

- id: REQ-109
  type: requirement
  title: Persistent content-addressed decision IDs
  status: proposed
  description: >
    Each decision shall carry a content-addressed id derived from
    `sha256(function_name || source_file || source_line ||
    condition_index)` truncated to 16 hex characters. The integer id
    remains for fast lookup; the content id is what `rivet-evidence`
    persists. Required for cross-build comparison: a decision retains
    its identity across an unrelated change to the function order.
  tags: [v0.7, schema, evidence]
  fields:
    priority: must
    category: functional

- id: REQ-110
  type: requirement
  title: Per-PR coverage delta budget enforcement
  status: proposed
  description: >
    The `witness diff` subcommand shall accept a `--budget` flag that
    fails the workflow when any decision covered on the base ref is
    uncovered on the head ref. Required to operationalise the rivet
    "evidence ratchet" pattern at PR scale.
  tags: [v0.7, ci, evidence]
  fields:
    priority: should
    category: functional

- id: REQ-111
  type: requirement
  title: Module-size regression check
  status: proposed
  description: >
    The release CI shall measure instrumented-module size relative to
    the original module on the project fixture suite (sample-rust-crate
    plus the v0.7 destination workload). The release fails if the
    multiplier exceeds 3x; PRs are informational at 1.8x. Required to
    keep instrumentation overhead bounded as the project gains features.
  tags: [v0.7, ci, scale]
  fields:
    priority: should
    category: non-functional
```

### Features

```yaml
- id: FEAT-201
  type: feature
  title: v0.7 — scaling capability stack
  status: proposed
  description: >
    Streaming counter encoding (REQ-101), i64 saturating counters
    (REQ-102), inlined-subroutine handling (REQ-105), per-target
    br_table decision rollup (REQ-106), persistent decision IDs
    (REQ-109), CBOR+zstd run-records (REQ-107). Together these enable
    measurement on real-application-scale Wasm modules (httparse class:
    1,000-2,000 decisions; wasmparser class: 5,000+) without
    regressing v0.5's curated-verdict workflow.
  tags: [v0.7, scale]
  fields:
    phase: phase-7
  links:
    - type: satisfies
      target: REQ-101
    - type: satisfies
      target: REQ-102
    - type: satisfies
      target: REQ-105
    - type: satisfies
      target: REQ-106
    - type: satisfies
      target: REQ-107
    - type: satisfies
      target: REQ-109

- id: FEAT-202
  type: feature
  title: v0.7 — evidence rollup at module scale
  status: proposed
  description: >
    Auto-generated synthetic REQ-AUTO-<crate>-<file> artefacts
    (REQ-103), module-rollup default report (REQ-104), per-PR delta
    budget (REQ-110). Together these make a 5,000-decision claim
    reviewable by a human in <100 reads via the three-layer artefact
    graph (FEAT → crate-REQ → synthetic-REQ → BranchEntry).
  tags: [v0.7, rivet, evidence]
  fields:
    phase: phase-7
  links:
    - type: satisfies
      target: REQ-103
    - type: satisfies
      target: REQ-104
    - type: satisfies
      target: REQ-110
    - type: refines
      target: FEAT-003

- id: FEAT-203
  type: feature
  title: v0.7 — enterprise visualiser emitters
  status: proposed
  description: >
    SonarQube generic-format XML and Cobertura XML emitters (REQ-108).
    Module-size regression CI check (REQ-111). Together these reach
    the enterprise dashboard surface that the LCOV-only path
    (FEAT-001 / REQ-023) cannot cover.
  tags: [v0.7, reporting, ecosystem]
  fields:
    phase: phase-7
  links:
    - type: satisfies
      target: REQ-108
    - type: satisfies
      target: REQ-111

- id: FEAT-204
  type: feature
  title: v0.7 — destination-workload validation
  status: proposed
  description: >
    Land the v0.7 capability stack against three fixture workloads:
    witness-core itself (dogfood + rivet integration test),
    seanmonstar/httparse (1,500-decision real-application baseline),
    bytecodealliance/wasm-tools/wasmparser (5,000-decision density
    stress test). Each fixture lives under
    crates/witness/tests/fixtures/ with a build.sh and a CI job
    measuring coverage and module-size growth. The fixtures form the
    v0.9 destination evidence baseline.
  tags: [v0.7, fixtures, validation]
  fields:
    phase: phase-7
  links:
    - type: satisfies
      target: REQ-104
    - type: satisfies
      target: REQ-111
    - type: depends-on
      target: FEAT-201
    - type: depends-on
      target: FEAT-202
```

### Design decisions

```yaml
- id: DEC-101
  type: design-decision
  title: v0.7 default counter mode stays exported globals; memory mode is opt-in
  status: proposed
  description: >
    `witness instrument` continues to export one `__witness_counter_<id>`
    global per branch by default in v0.7. The packed-memory mode
    (REQ-101) is opt-in via `--counters=memory`. Auto-switching above
    a configurable threshold (default 1,024 decisions) emits a log
    line and uses memory mode automatically.
  rationale: >
    Defaulting to memory would break v0.5 hosts that read counter
    globals via the `__witness_counter_*` export iteration pattern
    (DEC-003 contract). Opt-in keeps behavioural compatibility while
    adding a high-decision-count escape hatch. Auto-switching at
    threshold preserves the v0.5 path for the curated-verdict suite
    (always under 1,024 decisions) and only changes behaviour when
    the user is already in v0.7-scale territory.
  alternatives: >
    Default to memory for all v0.7 modules — rejected because it
    breaks v0.5 host integrations without warning. Hard-fail above
    the global cap with a "use --counters=memory" hint — rejected
    because real workflows hit this in CI where adding a flag is
    high-friction.
  tags: [v0.7, instrumentation, scale]
  links:
    - type: satisfies
      target: REQ-101
    - type: refines
      target: DEC-003

- id: DEC-102
  type: design-decision
  title: Synthetic requirements carry status:derived, never status:approved
  status: proposed
  description: >
    Auto-generated `REQ-AUTO-<crate>-<file>` artefacts (REQ-103)
    always carry `status: derived` so the rivet validator can
    distinguish them from human-written requirements. Replacement
    by a human-written requirement is the expected lifecycle; the
    artefact graph remains valid in either state because rivet
    treats `derived` as a non-blocking informational status.
  rationale: >
    The synthetic layer's job is to give coverage evidence somewhere
    to attach when no human-curated requirement exists yet. Allowing
    them to carry approved status would let teams "claim" coverage
    against artefacts no human ever reviewed — false-positive
    traceability. The derived status flags them as machine-generated
    placeholders.
  alternatives: >
    Use a separate artefact type `requirement-auto` — rejected
    because it doubles the schema surface and rivet's link types
    apply uniformly. Mark in a freeform tag only — rejected because
    tags are not validation-aware.
  tags: [v0.7, rivet, scale, evidence]
  links:
    - type: satisfies
      target: REQ-103

- id: DEC-103
  type: design-decision
  title: Decision content-addressing uses sha256 truncated to 16 hex chars
  status: proposed
  description: >
    Persistent decision IDs (REQ-109) are computed as
    `sha256(function_name || "\0" || source_file || "\0" ||
    source_line.to_string() || "\0" || condition_index.to_string())`
    truncated to the leading 16 hex characters. Collisions within a
    single module are checked at instrumentation time; the integer
    id remains the manifest's primary key.
  rationale: >
    16 hex chars (64 bits) gives a birthday-bound collision probability
    of ~10^-9 at 1M decisions, well above the largest plausible
    workload. SHA-256 is already a witness-core dependency (predicate
    emission). The null-byte separator avoids ambiguity between
    components.
  alternatives: >
    Full SHA-256 (64 hex) — rejected as visually unwieldy for log
    output and rivet artefact ids. xxhash64 — rejected because adding
    a non-cryptographic hash dep just for IDs increases the dependency
    footprint when sha2 is already present.
  tags: [v0.7, schema, evidence]
  links:
    - type: satisfies
      target: REQ-109

- id: DEC-104
  type: design-decision
  title: v0.7 destination is httparse, not rustls
  status: proposed
  description: >
    The v0.9 destination workload is `seanmonstar/httparse` with
    `bytecodealliance/wasm-tools/wasmparser` as the density stress
    test and `witness-core` itself as the dogfood subject. rustls
    is rejected as a destination because ring's wasm32 support is
    partial as of 2026-04 (web-sys for randomness, vanilla-C
    fallback in 0.17 not exercised in upstream CI for the full
    handshake corpus).
  rationale: >
    httparse is a real RFC 7230 parser shipped via hyper into a
    significant fraction of Rust's web stack — coverage on httparse
    has direct industrial leverage. Decision count is in the
    1,000-2,000 range that exercises FEAT-201/202/204/205/206
    without overshooting into wasmparser-class density. The crate's
    test suite runs in <1 second under wasmtime, fitting CI budgets.
    rustls would be the dream subject but ring on wasm32 is not
    ready for the full handshake test corpus, so the destination
    would be inconclusive.
  alternatives: >
    rustls — deferred to v1.0+ once ring's wasm32 support stabilises.
    serde_json core — rejected as not decision-dense enough. tokio
    core — rejected for native deps.
  tags: [v0.7, fixtures, scope]
  links:
    - type: satisfies
      target: REQ-104

- id: DEC-105
  type: design-decision
  title: v0.7 does not ship per-row trace storage
  status: proposed
  description: >
    v0.7 stores hit counts only, not per-invocation traces. The
    trace use case (which input drove which decisions) is recognised
    but deferred to v0.8 with reservoir sampling.
  rationale: >
    A trace row is `O(invocations × decisions)` and at the v0.7
    target scale (5,000 decisions, fuzz-driven) a full trace store
    is gigabytes. Reservoir sampling makes this tractable but adds
    a new schema surface, deserves its own design pass, and is not
    on the critical path to the v0.9 destination claim. Counters
    alone satisfy MC/DC's coverage requirement; traces are a
    diagnostic luxury on top.
  alternatives: >
    Ship full traces with on-disk indexing — rejected as scope creep
    and a major schema bump. Ship traces only on test-failure paths
    — interesting; deferred to the v0.8 design pass.
  tags: [v0.7, scope, traces]
```

## Sources consulted

Primary internal:

- `/Users/r/git/pulseengine/witness/DESIGN.md`
- `/Users/r/git/pulseengine/witness/AGENTS.md`
- `/Users/r/git/pulseengine/witness/CHANGELOG.md`
- `/Users/r/git/pulseengine/witness/artifacts/{requirements,features,design-decisions}.yaml`
- `/Users/r/git/pulseengine/witness/crates/witness-core/src/{instrument,decisions,run_record,rivet_evidence,lcov}.rs`
- `/Users/r/git/pulseengine/witness/docs/research/mcdc-bytecode-research.md`
- `/Users/r/git/pulseengine/witness/docs/research/v05-component-witness.md`
- `/Users/r/git/pulseengine/witness/docs/research/v05-loom-meld-upstream.md`
- `/Users/r/git/pulseengine/witness/docs/research/rivet-evidence-consumer.md`
- `/Users/r/git/pulseengine/witness/docs/research/overdo-alignment.md`
- `/Users/r/git/pulseengine/rivet/rivet-core/src/coverage.rs`

External (web pass):

- <https://github.com/seanmonstar/httparse> — destination candidate
- <https://github.com/bytecodealliance/wasm-tools> — wasmparser
  decision-density stress test
- <https://docs.rs/wasmparser/latest/wasmparser/> — wasmparser API
  surface
- <https://doc.rust-lang.org/nightly/rustc/platform-support/wasm32-wasip2.html>
  — target capabilities (full std support)
- <https://github.com/rust-lang/regex/issues/422> — regex wasm
  compilation context
- <https://github.com/rustls/rustls/issues/808> — rustls wasm32
  support state (negative result, hence backup-not-pick)
- <https://www.rapitasystems.com/mcdc-coverage> — RapiCover supports
  1,000 conditions/decision; precedent that the unbounded-MC/DC
  position witness takes (DEC-007) is industry-realistic
- <https://www.synopsys.com/blogs/chip-design/mc-dc-struggle-reaching-100-percent.html>
  — scaling pain points in real-world MC/DC campaigns
- <https://deepwiki.com/linux-test-project/lcov/5.2-mcdc-coverage>
  — LCOV's MC/DC support shape; informs FEAT-211 / REQ-108
- <https://maskray.me/blog/2024-01-28-mc-dc-and-compiler-implementations>
  — referenced via the existing mcdc-bytecode-research brief

Sibling pulseengine repos `kiln`, `spar`, `loom`, `meld` were noted as
candidate workloads but access was denied in this thread; the
maintainer should inspect them locally and consider adding `spar`'s
MBSE state machine to the v0.7 fixture suite if it has the decision
density and a runnable test harness.
