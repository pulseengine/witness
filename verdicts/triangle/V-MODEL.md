# triangle — V-model traceability

## Requirement chain

- **REQ-027** Per-decision MC/DC truth-table emission
- **REQ-028** Independent-effect citation per condition
- **REQ-029** MC/DC gap analysis with row-closure recommendations
- **REQ-030** Verdict suite as canonical MC/DC evidence

## Design decisions

- **DEC-013** MC/DC instrumentation primitive — trace buffer with row markers
- **DEC-014** Short-circuit semantics preserved
- **DEC-016** Verdict suite composition

## Conditions

| Id | Source | Wasm |
|---|---|---|
| c1 | `a + b <= c` | first `br_if` of OR chain |
| c2 | `a + c <= b` | second `br_if` (short-circuited when c1=T) |
| c3 | `b + c <= a` | third `br_if` (short-circuited when c1\|\|c2=T) |

## Test rows

4 rows. See `TRUTH-TABLE.md`.

## Evidence

`compliance/verdict-evidence/triangle/` — full instrument-run-report-predicate chain.

## Why this verdict exists

The **canonical literature MC/DC example**. Myers (1979) introduced this
test for the original branch-coverage problem; the DO-178C training
materials use it for MC/DC. If witness's report agrees with Myers, the
report speaks the same language as the safety-critical training corpus.

This verdict is also where short-circuit OR is exercised for the first
time in the suite, complementing leap_year (mixed) and range_overlap
(short-circuit AND).
