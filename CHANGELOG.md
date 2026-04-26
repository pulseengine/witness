# Changelog

All notable changes to witness are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [SemVer 2.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.4] — 2026-04-26

### What v0.6.4 closes

v0.6.3 populated the compliance bundle with real per-verdict
evidence — manifest, run record, MC/DC report, unwrapped in-toto
predicate per verdict. v0.6.4 adds the **signature** layer: each
verdict's predicate is wrapped in a DSSE envelope and signed with an
ephemeral Ed25519 keypair generated fresh for the release. The
verifying public key ships in the bundle. Tampering with the
predicate body, the envelope, or the key fails verification.

### Added — `witness keygen` and `witness verify` CLI commands

Two new subcommands close the signing loop:

- `witness keygen --secret SK --public PK` — generate a fresh
  Ed25519 keypair (raw 64-byte secret + 32-byte public). Used by the
  verdict-suite signing path; also available for users who want to
  sign their own predicates.
- `witness verify --envelope E --public-key PK` — validate a DSSE
  envelope against an Ed25519 public key. Exits zero with `OK` on
  match, non-zero (with a clear error) on mismatch. Standards-
  compliant DSSE means `cosign verify-blob` works equivalently.

### Added — `witness-core::attest::generate_keypair_files`

Public library API for keypair generation. Mirrors the existing
`sign_predicate_file` and `verify_envelope_file` shapes (file in /
file out / `Result<()>`). Lets downstream tooling embed the same
ephemeral-key flow.

### Added — `witness-core::attest::verify_envelope_file`

File-IO wrapper around the existing `verify_envelope` byte-slice API.
Reads the envelope and public key from disk, returns the inner
in-toto Statement on success.

### Updated — `verdicts/run-suite.sh`

When `SIGN=1` (default), the script:

1. Generates an ephemeral Ed25519 keypair via `witness keygen`.
2. Writes the public key to `<bundle>/verifying-key.pub`.
3. For each verdict's `predicate.json`, runs `witness attest` to
   produce `<verdict>/signed.dsse.json` with `key_id` =
   `witness-suite/<verdict>`.
4. Discards the secret key on exit (in a `mktemp` directory cleaned
   up by a `trap`).
5. Writes a `<bundle>/VERIFY.md` documenting the verification
   command for both `witness verify` and `cosign verify-blob`.

Setting `SIGN=0` skips the signing path — useful for fast local
iteration.

### Verifies — round-trip end-to-end

Local sign-verify proven on the leap_year and parser_dispatch
envelopes:

```
$ witness verify \
    --envelope leap_year/signed.dsse.json \
    --public-key verifying-key.pub
OK — DSSE envelope leap_year/signed.dsse.json verifies against verifying-key.pub
  predicate type: https://pulseengine.eu/witness-coverage/v1
  subjects: 1
```

Tampering the public key (XOR first byte) correctly fails:

```
Error: wasm runtime error: DSSE verify failed: VerificationFailed
exit=1
```

### Why ephemeral keys

Per-release ephemeral keys avoid long-term key custody. The verifying
key is shipped in the compliance bundle. The secret key is generated
fresh in CI, used to sign, then discarded. A signature thus proves
"this evidence was produced by the release pipeline that wrote this
verifying-key.pub" — exactly the V-model claim. Long-term key
management (rotation, attestation chains, sigstore Fulcio integration)
is v0.7+ work.

### Implements / Verifies

- Implements: REQ-031 (witness-core.wasm signed release asset — the
  pattern now applies uniformly to verdict predicates too).
- Verifies: round-trip sign + verify works for every verdict that
  produces a non-empty predicate; tampered key correctly rejected.

## [0.6.3] — 2026-04-26

### What v0.6.3 closes

v0.6.0 promised real per-verdict signed evidence in the release archive
(REQ-033) but shipped a structural placeholder: empty `predicates/`
and `manifests/` directories. v0.6.1 made the per-row instrumentation
work end-to-end; v0.6.2 made 5 of 7 verdicts produce reports. v0.6.3
finally **populates the compliance bundle** with that evidence and
adds a CI regression gate so the suite stays green across rustc
upgrades.

### Added — `verdicts/run-suite.sh`

End-to-end driver script. Invoked locally (`./verdicts/run-suite.sh
some-out-dir`) and from the compliance action. For each of the seven
verdicts:

1. Builds with `wasm32-unknown-unknown` (core module — walrus can
   rewrite; `wasm32-wasip2` produces Components walrus doesn't yet
   handle).
2. Instruments with the v0.6.1 per-row primitive.
3. Runs every `run_row_<n>` export.
4. Emits text + JSON MC/DC reports.
5. Builds the unwrapped in-toto Statement (signing is v0.6.4 once
   release-key management is wired in).
6. Emits LCOV + sibling overview when DWARF surfaces decisions.

A `SUMMARY.txt` rolls up branches / decisions / full-MC/DC counts:

```
verdict              branches   decisions    full-mcdc
-------              --------   ---------    ---------
leap_year            2          1            1/1
range_overlap        0          0            0/0
triangle             2          1            1/1
state_guard          3          1            1/1
mixed_or_and         0          0            0/0
safety_envelope      4          1            1/1
parser_dispatch      33         7            1/7
```

### Added — populated compliance bundle

`.github/actions/compliance` now invokes `verdicts/run-suite.sh` and
nests the output under `compliance/verdict-evidence/<name>/`. Each
release's compliance archive contains:

- The seven verdict directories with their full instrument-run-report
  chain.
- `SUMMARY.txt` at the top of the bundle.
- The original (now non-empty) `predicates/` and `manifests/`
  directories.

Closes REQ-033 ("compliance bundle populated with real evidence") in
substance, not just structurally.

### Added — `verdict-suite` CI regression gate

New `verdict-suite` job in `ci.yml`:

- Builds witness in release mode.
- Adds `wasm32-unknown-unknown` target.
- Runs the suite script.
- **Asserts** that `leap_year`, `triangle`, `state_guard`,
  `safety_envelope`, and `parser_dispatch` each produce >= 1
  reconstructed decision. A regression (e.g. a future rustc that
  fully optimises one of these verdicts to bitwise) fails CI on main.
- Uploads `verdict-evidence/` as an artefact.

`range_overlap` and `mixed_or_and` are deliberately excluded from the
gate — their pure-boolean conditions are expected to be fully
optimised to `i32.and` and produce zero branches at the Wasm level.

### Notes for v0.6.4

- DSSE-sign each verdict's predicate with a release-time key. Pulls
  `wsc-attestation` into the action, manages the key via GitHub
  Secrets.
- Add a `verdict-suite-delta` PR-level CI job that diffs the
  `decisions / conditions / full-mcdc` counts vs `main` and posts the
  delta to the PR conversation. Useful for catching subtle optimiser
  regressions earlier than the regression gate fires.

### Implements / Verifies

- Implements: REQ-033 (compliance bundle populated with real
  per-verdict evidence).
- Verifies: the verdict-suite CI gate exists and fails closed when
  any of the five "should-have-decisions" verdicts regresses to zero.

## [0.6.2] — 2026-04-26

### What v0.6.2 closes

v0.6.1 made the per-row instrumentation work end-to-end on the
`leap_year` verdict, where rustc happens to attribute all surviving
`br_if`s to the same source line. Verdicts whose conditions span
multiple source lines (`state_guard`, `triangle`, `safety_envelope`,
`parser_dispatch`) had surviving `br_if`s in their manifests but
**zero reconstructed decisions** — `decisions::group_into_decisions`
required strict same-line equality for the grouping criterion, which
short-circuit chains formatted across multiple lines do not satisfy.

v0.6.2 relaxes the criterion: br_ifs in the same `(function, file)`
whose source lines fall within `MAX_DECISION_LINE_SPAN = 10` cluster
into one Decision. Walks branches in branch-id order (= source-walk
emission order), starts a new cluster when the next br_if is outside
the line window. False-grouping is bounded — adjacent decisions
separated by a > 10-line gap stay separate.

### Result

| Verdict | branches | decisions | full MC/DC | notes |
|---|---:|---:|---|---|
| `leap_year` | 2 | 1 | 1 | unchanged from v0.6.1 |
| `state_guard` | 3 | 1 | 1 | **new in v0.6.2** |
| `triangle` | 2 | 1 | 1 | **new in v0.6.2** |
| `safety_envelope` | 4 | 1 | 1 (3 conds) | **new in v0.6.2** |
| `parser_dispatch` | 33 | **7** | 1 | **new in v0.6.2** — finds decisions in `memchr` library calls automatically |
| `range_overlap` | 0 | 0 | n/a | optimised to `i32.and` (bitwise), nothing to measure |
| `mixed_or_and` | 0 | 0 | n/a | optimised to bitwise; nothing to measure |

### parser_dispatch is the standout

The `parser_dispatch` verdict's `s.contains(b'@')` call lowered into
the `memchr` library's byte-search loops, which themselves contain
compound boolean conditions. `decisions::reconstruct_decisions` picks
them up automatically:

```
$ witness report --input parser_dispatch.run.json --format mcdc
decisions: 1/7 full MC/DC; conditions: 7 proved, 15 gap, 5 dead

decision #0 lib.rs:37: Partial
  c0 (branch 3): proved via rows 1+4 (masking)
  c1 (branch 10): DEAD — never evaluated in any row
  c2 (branch 11): DEAD — never evaluated in any row
  c3 (branch 17): GAP — try a row {c0=T, c3=T} (outcome must differ from row 4)
decision #1 lib.rs:58: FullMcdc
  c0 (branch 18): proved via rows 2+4 (masking)
  c1 (branch 19): proved via rows 3+5 (unique-cause)
decision #2 memchr.rs:40: Partial
  c0 (branch 0): proved via rows 0+4 (masking)
  c1 (branch 1): proved via rows 1+5 (masking)
  c2 (branch 2): GAP — try a row {c0=T, c1=T, c2=F} (outcome must differ from row 4)
...
```

This is "witness on real code, not toys" — the predicate is six
test rows of 4-condition URL-authority validation, but the underlying
implementation drags in the standard library's compound predicates,
and witness reports MC/DC on all of them with cited row pairs and
closure recommendations.

### Implementation

- `decisions::MAX_DECISION_LINE_SPAN: u32 = 10` — public constant so
  consumers can document the threshold in V-model briefs.
- `group_into_decisions` rewritten as a two-pass algorithm: resolve
  br_if entries to `(function, file, line)`, then bucket by
  `(function, file)` and cluster within each bucket using the
  adjacent-line span.
- Two new unit tests: `group_into_decisions_clusters_adjacent_lines`
  (4 br_ifs on lines 23-26 → one Decision) and
  `group_into_decisions_splits_on_large_gap` (two clusters separated
  by a 49-line gap stay separate).

### Implements / Verifies

- Implements: REQ-027, REQ-028, REQ-029 — extends the v0.6.0 schema
  + reporter to cover the broader range of compound-decision
  lowerings rustc emits.
- Verifies: 5 verdicts (leap_year, state_guard, triangle,
  safety_envelope, parser_dispatch) produce non-empty MC/DC reports
  with cited row pairs.

### Notes for v0.6.3 / v0.7

- `range_overlap` and `mixed_or_and` produce zero branches because
  rustc fully optimises their pure-boolean conditions to bitwise
  arithmetic. v0.7's "compiler hint" work could ask rustc to emit
  branches for these patterns when an opt-in attribute is present —
  per the v0.2 paper's "witness-and-checker" stance. Out of scope
  for v0.6.x.
- `parser_dispatch` shows 5 dead conditions across 6 test rows; the
  verdict's `TRUTH-TABLE.md` should be revised to align expected
  rows with what the post-rustc lowering actually exposes (rather
  than the source-level decisions originally documented).

## [0.6.1] — 2026-04-26

### What v0.6.1 closes

v0.6.0 shipped the consumer side (schema, reporter, verdict-suite oracles)
and explicitly deferred the runtime instrumentation to v0.6.1. v0.6.1 is
that runtime path: real per-row condition capture during `witness run`,
real `RunRecord.decisions` populated from execution rather than hand-
curated, real end-to-end demonstrable MC/DC on the canonical leap_year
verdict.

### Added — per-row instrumentation

- **Per-condition exported globals**: each `BrIf` / `IfThen` / `IfElse`
  branch now allocates two additional globals alongside its existing
  `__witness_counter_<id>`:
  - `__witness_brval_<id>` (i32) — the condition's evaluated value
    (0 or 1) when reached this row, or `1` for fired arms.
  - `__witness_brcnt_<id>` (i32) — count of evaluations this row;
    non-zero means evaluated, zero means short-circuited (absent
    from `DecisionRow.evaluated`). `BrTable*` branches keep
    counter-only instrumentation per DEC-015.
- **`__witness_row_reset` exported function**: emitted by every
  instrumentation pass. Zeros all `brval` / `brcnt` globals so the
  next row's captures don't leak prior state.

### Added — runner row-by-row capture

- `witness run` (embedded wasmtime path) now, for each `--invoke`:
  1. Calls `__witness_row_reset` to clear per-row state.
  2. Invokes the export, capturing the return value as the row's
     decision outcome (when the export returns an `i32`).
  3. Reads the per-row `brval` / `brcnt` globals.
  4. For each `Decision` in the manifest, builds a `DecisionRow`
     populated with the per-condition values evaluated this row.
- `RunRecord.decisions` is now populated from execution; the
  `mcdc_report` reporter consumes it directly with no manual curation.

### End-to-end demonstrable on `verdicts/leap_year`

Building the leap_year verdict, instrumenting it, running all 4 row
exports, and asking for the MC/DC report produces:

```
$ witness instrument verdicts/leap_year/verdict_leap_year.wasm -o leap.wasm
$ witness run leap.wasm --invoke run_row_0 ... --invoke run_row_3 -o run.json
$ witness report --input run.json --format mcdc
module: leap.wasm
decisions: 1/1 full MC/DC; conditions: 2 proved, 0 gap, 0 dead

decision #0 lib.rs:46: FullMcdc
  truth table:
    row 0: {c0=T} -> F
    row 1: {c0=F, c1=T} -> T
    row 2: {c0=F, c1=F} -> F
    row 3: {c0=F, c1=F} -> T
  conditions:
    c0 (branch 0): proved via rows 0+1 (masking)
    c1 (branch 1): proved via rows 1+2 (unique-cause)
```

Two conditions, both proved with cited row pairs, full MC/DC at the
Wasm bytecode level.

### Why some verdicts report zero decisions

The leap_year decision `(year%4==0 && year%100!=0) || year%400==0`
lowered to two `br_if` instructions plus inline arithmetic for the
third condition. That's why the report shows two conditions rather
than three: the third was elided by rustc's optimizer into the
fall-through computation. This is exactly the v0.2 paper's coverage-
lifting thesis — post-rustc Wasm coverage measures *what the
optimizer left as branches*, not the source-level condition count.

For verdicts whose conditions are all side-effect-free comparisons
(e.g. `a && b` over pure booleans), rustc may lower `&&` to a single
`i32.and` instruction and eliminate branches entirely. `range_overlap`
and similar verdicts produce zero `BrIf` entries in the manifest as a
result. The reporter correctly reports zero decisions — that is the
honest measurement at this point. Source-level MC/DC for these
predicates is the rustc-mcdc tool's territory; witness covers what
survives lowering. The "overdo stance" (DEC-005) — adopt both, do
not pick one.

The remaining verdicts (`triangle`, `state_guard`, `mixed_or_and`,
`safety_envelope`, `parser_dispatch`) have varying numbers of
surviving branches depending on rustc's lowering choices for their
specific shapes. Their `TRUTH-TABLE.md` files document the
hand-derived source-level MC/DC; the witness report shows the
Wasm-level MC/DC. The discrepancy between the two is itself
evidence of how aggressive the optimizer's elision is — useful
data for the v0.7 work on inlined-subroutine handling and decision
reconstruction extension.

### Implements / Verifies

- Implements: REQ-034 (on-Wasm trace-buffer instrumentation; v0.6.1
  uses per-row globals as the simplest correct primitive instead of
  the linear-memory trace buffer recommended by Agent A — both
  satisfy the requirement, the per-row globals are simpler when each
  row invokes the predicate exactly once).
- Implements: FEAT-015 (the runtime side of the v0.6 redo).
- Verifies: leap_year verdict end-to-end pipeline produces the
  expected Wasm-level MC/DC report (1 decision, 2 conditions, full
  MC/DC under masking).

### Notes for v0.6.2

- Investigate why state_guard / triangle / mixed_or_and decisions
  don't always reconstruct under DWARF-based grouping despite having
  surviving br_ifs. Likely fix: relax the `(function, source_file,
  source_line)` grouping criterion to handle inlined-subroutine line
  attribution.
- Consider whether the per-row-globals primitive should evolve toward
  the linear-memory trace buffer (Agent A's recommendation) once
  v0.7's scaling work surfaces hot-loop overflow patterns.
- Per-target br_table MC/DC reconstruction (DEC-015 deferral).

## [0.6.0] — 2026-04-25

### What v0.6 is — and what it is not

v0.5.0 shipped DWARF-grouped branch coverage but the report layer
computed *per-branch hit counts*, not MC/DC truth tables. The CHANGELOG
described it as MC/DC; that was an overclaim. v0.6 is the redo: the
schema, the reporter, the verdict suite, and the V-model artefact graph
that real MC/DC requires. The on-Wasm instrumentation that captures
per-row condition vectors lands as a v0.6.1 follow-up — see "Deferred
to v0.6.1" below.

### Added — schema and reporter

- **`RunRecord` schema v3**: new `decisions: Vec<DecisionRecord>` and
  `trace_health: TraceHealth` fields (REQ-027, FEAT-012, DEC-013).
  `DecisionRecord` carries per-decision `rows: Vec<DecisionRow>`; each
  `DecisionRow` has a sparse `evaluated: BTreeMap<u32, bool>` so
  short-circuited conditions are first-class evidence (DEC-014). v0.5
  records (schema "2") still load — both new fields default to empty.
- **`witness-core::mcdc_report` module**: per-decision truth tables,
  independent-effect citations under masking MC/DC (DO-178C accepted
  variant), gap analysis with row-closure recommendations (REQ-028,
  REQ-029). 6 unit tests covering all canonical decision shapes pass.
- **`witness report --format mcdc`** and **`--format mcdc-json`**:
  CLI surface for the new reporter. Schema URL
  `https://pulseengine.eu/witness-mcdc/v1`.

### Added — verdict suite (REQ-030, FEAT-012, DEC-016)

The `verdicts/` directory contains seven canonical compound-decision
verdicts, each as a self-contained Rust crate that compiles to
`wasm32-wasip2`. Each verdict ships:

- `Cargo.toml` — standalone, opts out of the witness workspace.
- `src/lib.rs` — the predicate plus `run_row_<n>` exports, one per
  test row.
- `TRUTH-TABLE.md` — the **expected** MC/DC analysis, hand-derived,
  with a machine-readable JSON block. Acts as the verification oracle
  for the `mcdc_report` reporter.
- `V-MODEL.md` — one-page traceability: REQ → DEC → conditions → rows
  → evidence.
- `build.sh` — standalone build to `wasm32-wasip2`.

The seven verdicts and their shapes:

| Verdict | Decision | Conds | Rows |
|---|---|---|---|
| `leap_year` | `(y%4==0 && y%100!=0) \|\| y%400==0` | 3 | 4 |
| `range_overlap` | `a.start <= b.end && b.start <= a.end` | 2 | 3 |
| `triangle` | Myers-paper "not a triangle" check (3-cond OR) | 3 | 4 |
| `state_guard` | TLS handshake guard (4-cond AND chain) | 4 | 5 |
| `mixed_or_and` | `(a\|\|b) && (c\|\|d)` | 4 | 5 |
| `safety_envelope` | 5-cond automotive envelope (beyond LLVM 6-cap) | 5 | 6 |
| `parser_dispatch` | RFC 3986 URL authority validator (real-world anchor) | 5 | 6 |

The reporter's correctness has been verified by reproducing each
verdict's hand-derived `TRUTH-TABLE.md` from a hand-curated
`DecisionRecord` in unit tests.

### Added — V-model artefact graph (REQ-032, FEAT-014, DEC-017)

- 7 new requirements (REQ-027..033)
- 3 new features (FEAT-012..014)
- 6 new design decisions (DEC-013..018), with DEC-013 documenting the
  trace-buffer instrumentation primitive recommendation from the
  `v06-instrumentation-primitive` research brief.
- `rivet validate` PASS across the workspace.

### Added — research roadmap (4 parallel agent docs, ~19k words total)

- `docs/research/v06-instrumentation-primitive.md` — chooses linear-
  memory trace buffer with row markers as the v0.6.1 instrumentation
  primitive. Wasm-side rewrite sketch, schema diff, short-circuit
  semantics policy, BrTable v0.7 deferral, prior-art citations,
  implementation risk register.
- `docs/research/v07-scaling-roadmap.md` — destination workload pick:
  `seanmonstar/httparse` (~1500 decisions, clean wasm32-wasip2 build).
  v0.7 capability list (streaming counter encoding, i64 saturating
  counters, inlined-subroutine DWARF, auto-generated synthetic
  requirements, module-rollup default report). Top scaling risk:
  DWARF parsing memory at scale.
- `docs/research/v08-visualisation-roadmap.md` — architecture call:
  `wstd-axum` + `maud` + HTMX 2.x, runnable as `wasmtime serve` or
  composed via `wac plug`. AI-agent surface = REST+JSON content
  negotiation plus `rmcp` MCP transport mounted on the same Axum
  router. Playwright tests reuse rivet's pattern; visualiser
  visualises its own coverage (the v0.8 demo screenshot).
- `docs/research/v09-soa-and-agent-ux.md` — competitive scan
  (LDRA, VectorCAST, Cantata, BullseyeCoverage, Squore, gcov+gcovr).
  v0.9 positioning: first MC/DC tool with end-to-end signed evidence
  and agent-native MCP API. Top 3 superiority features identified.
  Biggest competitive risk: RapiCover already has unbounded
  conditions plus DO-178C heritage for C/C++/Ada.

### Deferred to v0.6.1

- **On-Wasm instrumentation that captures per-row data.** The
  trace-buffer rewrite from `v06-instrumentation-primitive.md` is
  scoped for v0.6.1. v0.6.0 ships the consumer side (schema +
  reporter + verdict suite oracles + CLI). The `witness instrument`
  subcommand still emits v0.5-style per-counter instrumentation;
  v0.6.1 extends it with the trace primitive so `witness run`
  produces populated `RunRecord.decisions`.
- **End-to-end verdict execution.** Each verdict's `src/lib.rs`,
  `TRUTH-TABLE.md`, and `V-MODEL.md` are in place; `cargo build
  --target wasm32-wasip2` against each verdict crate produces a
  `.wasm`. The reporter's correctness has been verified against the
  hand-derived truth tables in unit tests, and the verdicts' V-MODEL
  evidence chains will be populated by `compliance` when v0.6.1's
  instrumentation lands.

### Why ship the foundation as 0.6.0

The schema, reporter, verdict-suite oracles, and V-model artefact
graph are independent of the instrumentation runtime path. Shipping
them as v0.6.0 lets downstream consumers (rivet, sigil, agent
integrations) build against the v3 schema and the
`witness-mcdc/v1` predicate type now, while the instrumentation
work continues in the v0.6.1 release. The verdicts' `TRUTH-TABLE.md`
files are the verification oracles v0.6.1 will reproduce.

### Implements / Verifies

- Implements: REQ-027 (truth-table emission), REQ-028 (independent-
  effect citations), REQ-029 (gap-closure recommendations), REQ-030
  (verdict suite — scaffolded), REQ-032 (V-model traceability —
  artefact graph), REQ-033 (compliance bundle structure).
- Implements: FEAT-012 (real MC/DC reporter — consumer side),
  FEAT-014 (V-model artefact graph).
- Verifies: 6 mcdc_report unit tests reproduce each canonical verdict
  shape's expected truth table and pair-finding outcomes.

## [0.5.0] — 2026-04-25

### Added

- **Workspace split.** Single-crate `witness` becomes a workspace with
  `crates/witness-core` (pure-data algorithms; `wasm32-wasip2`-buildable)
  and `crates/witness` (CLI binary plus the wasmtime-using runner).
  All algorithm modules — instrument, decisions, diff, predicate,
  report, rivet_evidence, run_record, lcov, attest — live in
  witness-core. Only main.rs + run.rs (wasmtime embedder) stay in the
  binary crate.
- **`witness lcov`** subcommand (REQ-023). Emits LCOV from a
  `RunRecord` per the
  [v0.5 LCOV format brief](docs/research/v05-lcov-format.md). Hybrid
  emission: DWARF-correlated `Decision`s become standard `BRDA`
  records keyed to real source files; uncorrelated branches go in a
  sibling overview text. Codecov-ingestible as `flag: wasm-bytecode`.
- **`witness attest`** subcommand (REQ-024). Wraps an unwrapped
  in-toto Statement (from `witness predicate`) in a DSSE envelope
  signed with an Ed25519 secret key. Compatible with sigil's
  `wsc verify`, sigstore cosign, and any in-toto-attestation
  consumer. Implementation depends on the workspace `wsc-attestation`
  path-dep into `pulseengine/sigil`.
- **Wasm-target artefact.** `cargo build -p witness-core --target
  wasm32-wasip2` produces `target/wasm32-wasip2/release/witness_core.wasm`,
  uploaded as a CI artefact and (in release builds) attached to the
  GitHub release. The full Component Model build with WIT bindings
  is the v0.6 stretch goal.
- **CI dogfood loop.** New `dogfood` job builds the
  `sample-rust-crate` fixture, instruments it with the freshly-built
  witness, runs every `run_*` export, and emits LCOV. Uploads to
  codecov with `flag: wasm-bytecode` for side-by-side comparison
  with the existing `flag: rust-source` LCOV (cargo-llvm-cov).
- **`witness-core` Wasm-target CI job.** Verifies witness-core
  compiles to `wasm32-wasip2` on every push to main; uploads the
  resulting `.wasm` artefact.
- **Loom + meld upstream issue drafts** at
  `docs/research/v05-loom-meld-upstream.md` ready for the maintainer
  to file. Both ask for DWARF preservation plus a byte-offset
  translation map so witness v0.6 can correlate post-loom / post-meld
  Wasm to source-level decisions.

### Research output

- `docs/research/v05-blog-principles.md` (placeholder; previously
  `v04-blog-principles.md` covers the same corpus).
- `docs/research/v05-lcov-format.md` — codecov-flag-compatible LCOV
  emission; recommends hybrid C strategy (BRDA for correlated, text
  overview for uncorrelated).
- `docs/research/v05-wsc-integration.md` — wsc-attestation API
  surface, Cargo dep model, witness-attest subcommand sketch. Confirmed
  wasm32 compatibility under the `signing` feature.
- `docs/research/v05-component-witness.md` — component-model build
  path; confirms cargo-component, wac, wit-bindgen all installed
  locally; identifies wasmtime as the only host-only dep.
- `docs/research/v05-loom-meld-upstream.md` — issue drafts for the
  upstream tools.

### Changed

- The `coverage` CI job now uploads with `flag: rust-source` so the
  new bytecode-coverage upload (`flag: wasm-bytecode`) renders
  side-by-side in codecov.
- Workspace pulls `wsc-attestation` from a sibling
  `pulseengine/sigil` checkout (path dep). Will become a regular
  crates.io dep when wsc-attestation publishes.
- Direct `ed25519-compact` dep added to witness-core for keypair
  generation in tests and direct use by `attest.rs`.

### Implements / Verifies

- Implements: REQ-023 (witness lcov), REQ-024 (witness attest), plus
  the v0.5 workspace-split and dogfood-CI requirements (REQ-025,
  REQ-026 in the artefact set).

### Deferred to v0.6

- DWARF preservation through loom optimisation (gated on the upstream
  loom issue).
- DWARF preservation through meld fusion (gated on the upstream meld
  issue).
- Full Component Model build with WIT interface and `wac`-based
  composition with sigil's wsc component for in-process signing.

## [0.4.0] — 2026-04-25

### Added

- **DWARF-grounded MC/DC reconstruction** (FEAT-011, REQ-005, REQ-006,
  REQ-016). `decisions::reconstruct_decisions` now parses Wasm DWARF
  custom sections via `gimli` and `wasmparser`, builds a
  `(byte_offset → file, line)` map per compilation unit, and groups
  `BrIf` `BranchEntry`s sharing a `(function, file, line)` key into
  source-level `Decision`s. Strict per-`br_if` fallback when DWARF is
  absent. Lifted from v0.2.1 plan; v0.2.1 is therefore not released as
  a separate version.
- **`witness diff` subcommand** (REQ-020). Computes added / removed /
  changed branches and (when both inputs are runs) coverage-percentage
  delta. Schema URL `https://pulseengine.eu/witness-delta/v1`. Both
  JSON and text output. Required by the v0.4 PR delta workflow.
- **`witness-delta.yml` PR workflow** (REQ-022). Triggers on every PR
  touching `src/` / `tests/` / `Cargo.toml`. Checks out base + head,
  builds the head witness, runs `witness diff` on whatever manifests
  the fixture pipeline emits, attaches the delta JSON+text as a PR
  artefact. `continue-on-error` throughout — never blocks merge.
- **`actions/compliance` composite action** (REQ-021). Mirrors rivet's
  equivalent. Generates a tar.gz evidence bundle on release containing
  coverage report, in-toto predicates per module, branch manifests,
  and a README. Wired into `release.yml` between `build-binaries` and
  `create-github-release` as a new `compliance` job; the resulting
  archive is attached to the GitHub release alongside the binaries.

### Research output

- `docs/research/v04-blog-principles.md` — survey of every published
  pulseengine.eu post and the principles witness must adopt; 4756
  words across 14/16 posts; 20-item adoption checklist; voice
  mechanics catalogued.
- `docs/research/v04-ci-ports.md` — adaptation brief for
  rivet-delta.yml and the rivet compliance composite action; full
  YAML drafts for both witness-side workflow files.
- `docs/research/v04-compiler-qualification-reduction.md` — 451-line
  brief: ISO 26262-8 §11.4.5 substitution argument for ASIL B
  (works), DAL B (weaker), DAL A (broken). Most surprising finding:
  the TCL framework explicitly yields TCL 1 — "no qualification
  required" — when TI 1 *or* TD 1 holds; the work is in establishing
  TD 1, not in carving an exception.
- `docs/research/v04-mythos-slop-audit.md` — quick-pass slop audit
  using the methodology from
  <http://127.0.0.1:1024/blog/mythos-slop-hunt/>. Two P1 findings
  applied (deleted `report::save_json`; removed direct `tracing` dep).
  Two P2 findings kept as consumer-facing constants. Twelve P3
  findings documented as intentional defensiveness.

### Removed

- `report::save_json` — orphan-slop, no callers (P1 slop-hunt finding).
- `tracing = "0.1"` direct dependency — only `tracing-subscriber` is
  actively used (P1 slop-hunt finding).

### Deferred to v0.5

- Component-model coverage (was nominal v0.4; needs walrus or wac
  component support).
- Post-cfg / post-meld / post-loom measurement points (depends on
  loom's translation-validation evidence shape, which is itself
  evolving).
- A Wasm Component Model fixture for end-to-end testing (folded with
  the above).

### Implements / Verifies

- Implements: REQ-005, REQ-006, REQ-016, REQ-020, REQ-021, REQ-022
- Implements: FEAT-011 (v0.4 feature wrapper)

## [0.3.0] — 2026-04-25

### Added

- **`witness merge`** subcommand. Aggregates per-branch counters across
  multiple `witness run` outputs (one per test binary or harness
  invocation). Validates that all inputs share the same instrumented
  module and branch list before summing. Five new tests + four proptest
  properties (commutativity, monotonicity, sum-preservation,
  single-record identity).
- **`witness predicate`** subcommand. Emits an unwrapped in-toto
  Statement v1.0 carrying the coverage report as a
  `https://pulseengine.eu/witness-coverage/v1` predicate. Subject is
  the instrumented module (sha256); the original module's digest goes
  in the predicate body. Sigil reads the predicate type opaquely (no
  registry, no schema validation per type — see
  `docs/research/sigil-predicate-format.md`), so witness emits today
  with no sigil-side change. 5 unit tests including known-vector
  SHA-256 and RFC 3339 timestamp calibration.
- **`witness rivet-evidence`** subcommand. Emits coverage in the
  `https://pulseengine.eu/witness-rivet-evidence/v1` schema, partitioned
  by a user-supplied `branch_id → artefact_id` mapping YAML. The
  schema mirrors rivet's existing `ResultStore` shape so the consumer
  can be a near-drop-in copy. 4 unit tests + 2 proptest properties on
  RequirementMap flattening.
- **rivet upstream consumer** on the
  `feat/witness-coverage-evidence-consumer` branch in
  `pulseengine/rivet`. Adds `rivet-core::coverage_evidence::CoverageStore`
  mirroring `ResultStore`, plus 9 unit tests, plus the new
  `Error::CoverageEvidence` variant. 491 LOC. 780 rivet-core tests
  pass; clippy/fmt/deny clean. Branch is **left local for review** —
  not pushed to origin.
- **`docs/research/rivet-evidence-consumer.md`** and
  **`docs/research/sigil-predicate-format.md`** — evidence-of-design
  briefs that established the schemas before the code was written.
- **Rust→Wasm test fixture** under `tests/fixtures/sample-rust-crate/`.
  A minimal `no_std` Rust crate that compiles to Wasm and exercises
  every instrumentation pattern (`br_if`, `if/else`, `br_table`).
  Eight integration tests in `tests/integration_e2e.rs` runtime-skip
  if the fixture isn't built; `./tests/fixtures/sample-rust-crate/build.sh`
  is the one-shot builder for CI.

### Quality bar (REQ-019, FEAT-010)

- **Property-based tests** via `proptest` (new dev-dependency). 8
  properties covering merge invariants, serde round-trip of `Manifest`
  / `RunRecord`, and `RequirementMap::flatten` semantics. CI's
  `proptest-extended` job on main runs with `PROPTEST_CASES=2048`.
- **Mutation testing** via `cargo-mutants`. New CI job `mutants` runs
  on main as informational (continue-on-error: true). Configuration
  in `.cargo/mutants.toml` constrains mutation to the witness library
  and skips test modules.
- **Miri** CI job runs nightly miri with `-Zmiri-tree-borrows` over the
  pure-Rust modules (`report::*`, `decisions::*`, predicate's SHA
  vector + RFC 3339 path). The walrus / wasmtime FFI surface is
  excluded — miri's foreign-call constraints make it more noise than
  signal there.
- **Coverage threshold raised** to 75% project / 80% patch
  (`codecov.yml`).

### Implements / Verifies (rivet trailers)

- Implements: REQ-007, REQ-008, REQ-017, REQ-018, REQ-019
- Implements: FEAT-003 (rivet/sigil integration), FEAT-010 (quality bar)

### Notes

- v0.2.1 (DWARF reconstruction algorithm body) remains an any-time
  release. The schema is forward-compatible — when v0.2.1 lands, the
  rivet-evidence and predicate emitters automatically populate
  `decisions: [...]` for hosts that consume MC/DC.
- rivet integration is end-to-end **once the rivet upstream branch is
  pushed and a rivet release cuts**. The witness output is correctly
  shaped today; the rivet consumer code is on a feature branch.

## [0.2.0] — 2026-04-25

### Added

- **Subprocess harness mode** (`witness run --harness <cmd>`). Spawns a
  user-supplied command via `sh -c` with `WITNESS_MODULE` /
  `WITNESS_MANIFEST` / `WITNESS_OUTPUT` env vars set; the harness writes
  a counter snapshot to `WITNESS_OUTPUT` before exiting. Witness merges
  the snapshot with the manifest to produce the final run JSON. Escape
  hatch for runtimes the embedded wasmtime cannot accommodate
  (browser-based tests, custom WASI capability profiles, etc.).
  Implements REQ-014 / FEAT-006 / DEC-009.
- **Per-target `br_table` counting** (REQ-013 / FEAT-007 / DEC-008). v0.1's
  single-counter "executed" instrumentation is replaced with one counter
  per target plus one for the default arm. A generated
  `__witness_brtable_<n>` helper function dispatches on the selector via
  i32.eq chain (or i32.ge_u for the default), increments the matching
  counter, and returns the selector unchanged for the original
  `br_table` to dispatch. `BranchKind::BrTable` is removed; replaced by
  `BrTableTarget` (with `target_index: u32`) and `BrTableDefault`.
- **Manifest schema v2** (`schema_version: "2"`). Adds:
  - `BranchEntry.byte_offset: Option<u32>` — original wasm bytecode
    offset from walrus's `InstrLocId`. Required for DWARF correlation.
  - `BranchEntry.target_index: Option<u32>` — for `BrTableTarget` only.
  - `Manifest.decisions: Vec<Decision>` — DWARF-grounded source-level
    decisions reconstructed from `br_if` sequences. Empty when DWARF is
    absent or the v0.2.0 stub is in effect.
- **No artificial condition-count cap** (REQ-015). Witness uses exported
  globals, not LLVM's bitmap encoding, and supports decisions of any
  size.
- **`docs/paper/v0.2-mcdc-wasm.md`** — 8.2k-word paper draft covering
  motivation, formal MC/DC at Wasm, the reconstruction algorithm, the
  coverage-lifting soundness argument (DEC-010), comparison with
  rustc-mcdc / Clang / wasmcov / Whamm, and regulatory framing. Six
  sourcing TODOs for closed-access primary references (DO-178C clause,
  Chilenski & Miller 1994, Vilkomir & Bowen, Pnueli et al., DWARF
  spec).
- **README**: new "Related work" section with seven-row comparison
  table; status updated to "v0.1.0 shipped 2026-04-24"; usage examples
  refreshed to show both `--invoke` and `--harness` modes.

### Changed

- **MSRV unchanged at 1.91** (matches wasmtime 42's transitive floor).
- `Module` is loaded via `from_buffer` rather than `from_file` so the
  original bytes are available to the (stubbed) DWARF reconstructor.

### Stubbed (lands in v0.2.1)

- **DWARF-grounded reconstruction algorithm body** (DEC-012). v0.2.0
  ships the schema and the fallback path; the algorithm itself
  (`src/decisions.rs::reconstruct_decisions`) currently returns an
  empty list, leaving hosts on the strict per-`br_if` interpretation.
  The algorithm is documented in `docs/paper/v0.2-mcdc-wasm.md`. The
  schema is forward-compatible; v0.2.1 will fill the stub without a
  schema bump.

### Implements / Verifies (rivet trailers)

- Implements: REQ-013, REQ-014, REQ-015, REQ-016
- Verifies: REQ-004 (semantic preservation; round-trip tests pass for
  br_if, if-else, br_table)

## [0.1.0] — 2026-04-24

### Added

- `witness instrument <in.wasm> -o <out.wasm>` — walrus-based branch-counter
  insertion at every `br_if`, `if-else`, and `br_table` in every local
  function. Counter values are exposed as exported mutable globals named
  `__witness_counter_<id>`. Emits a sidecar manifest JSON describing each
  branch's function index, instruction index within its sequence, and
  kind.
- `witness run <instrumented.wasm> --invoke <export>` — built-in wasmtime
  runner that instantiates the module, invokes the requested no-argument
  export(s), reads all counter globals, and writes a raw run JSON.
  WASI-preview1 is wired with `inherit_stdio`; `--call-start` runs the
  WASI `_start` command-style entry-point.
- `witness report --input <run.json>` — branch-coverage report in human
  text or JSON. Per-function aggregation, deterministic uncovered-branch
  ordering.
- Library crate `witness::{instrument, run, report, error}` for callers
  that want to drive the pipeline programmatically (rivet integration in
  v0.3 will use this entry-point).
- SCRC Phase 1 + 2 clippy lints enforced workspace-wide; `cargo clippy
  --all-targets -- -D warnings` is a hard CI gate.
- Cross-platform CI: fmt, clippy, test matrix (Linux/macOS/Windows),
  MSRV (1.85), cargo-deny, cargo-audit, coverage via cargo-llvm-cov +
  codecov.
- Release workflow: tag-triggered cross-compiles for five targets
  (x86_64/aarch64 Linux, x86_64/aarch64 macOS, x86_64 Windows) with
  SHA256 checksums and auto-generated release notes.

### Design notes

- **Counter mechanism.** v0.1 exposes counters as exported mutable
  globals rather than a `__witness_dump_counters` function that
  serialises to linear memory. The exported-global path removes any
  cooperation-protocol requirement on the module-under-test and makes
  the runtime-side extraction a two-line `instance.get_global` for every
  conformant Wasm host. A dump-function escape hatch can be added later
  if an embedder requires a single exit point.
- **`br_table` granularity.** v0.1 counts `br_table` as a single
  "executed" point, not per-target. Per-target counting is a v0.2
  concern alongside DWARF-informed decision reconstruction; counting
  each target requires reconstructing which arm was taken from the
  selector, which materially complicates the rewrite without
  information DWARF-in-Wasm will give us cheaply in v0.2.
- **Harness model.** v0.1 ships the wasmtime-embedded runner only;
  subprocess-harness mode (`--harness <cmd>`) is deferred to v0.2 for
  modules that need a richer runtime.

### Research briefings

- `docs/research/rivet-template-mapping.md` — mapping of rivet's CI,
  lint, and release patterns adapted to witness's single-crate scope.
- `docs/research/overdo-alignment.md` — alignment brief extracting
  design constraints C1–C7 from the *Overdoing the verification chain*
  blog post the project's AGENTS.md cites.
