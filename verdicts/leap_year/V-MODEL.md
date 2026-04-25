# leap_year — V-model traceability

## Requirement chain

- **REQ-027** Per-decision MC/DC truth-table emission
- **REQ-028** Independent-effect citation per condition
- **REQ-029** MC/DC gap analysis with row-closure recommendations
- **REQ-030** Verdict suite as canonical MC/DC evidence

## Design decisions

- **DEC-013** MC/DC instrumentation primitive — linear-memory trace buffer with row markers
- **DEC-014** Short-circuit semantics preserved by instrumentation
- **DEC-016** Verdict suite composition — canonical compound-decision shapes

## Conditions

| Id | Source-level expression | Wasm-level branch (post-instrumentation) |
|---|---|---|
| c1 | `year % 4 == 0` | `br_if` derived from short-circuit lowering of `&&` |
| c2 | `year % 100 != 0` | `br_if` (short-circuited when c1=F) |
| c3 | `year % 400 == 0` | `br_if` evaluated when `c1 && c2` is F (short-circuited when `c1 && c2` is T) |

Conditions are grouped into a single `Decision` by `decisions::reconstruct_decisions` based on shared `(function, source_file, source_line)`.

## Test rows

4 rows in `src/lib.rs` (`run_row_0` through `run_row_3`). Each export
corresponds to one row marker in the trace buffer. See `TRUTH-TABLE.md`
for the hand-derived expected condition vectors and outcomes.

## Evidence (produced by release pipeline)

| Path | Origin |
|---|---|
| `target/wasm32-wasip2/release/verdict_leap_year.wasm` | `cargo build --release --target wasm32-wasip2` |
| `verdict_leap_year.instrumented.wasm` | `witness instrument` |
| `verdict_leap_year.witness.json` | `witness instrument` (manifest with conditions + decisions) |
| `verdict_leap_year.run.json` | `witness run` (per-row trace records) |
| `verdict_leap_year.report.json` | `witness report --format mcdc` (truth table + verdicts) |
| `verdict_leap_year.lcov` | `witness lcov` |
| `verdict_leap_year.predicate.json` | `witness predicate` (unwrapped in-toto Statement) |
| `verdict_leap_year.dsse.json` | `witness attest` (DSSE-signed envelope) |
| `verdict_leap_year.evidence.yaml` | `witness rivet-evidence` |

All artefacts bundled under `compliance/verdict-evidence/leap_year/` in
the release archive.

## Verification

The reporter is **correct on this verdict** when its emitted truth
table and independent-effect proofs match `TRUTH-TABLE.md` exactly.
The CI integration test at
`crates/witness/tests/integration_verdicts.rs` runs the verdict suite
and asserts agreement.

## Why this verdict exists in the suite

leap_year is the canonical 3-condition mixed AND/OR MC/DC example, used
in DO-178C training material and academic MC/DC literature for decades.
Including it makes witness's MC/DC capability **trivially verifiable
against the standard reference example**: anyone familiar with MC/DC
education can read `TRUTH-TABLE.md` and confirm correctness without
running code.
