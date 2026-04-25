# mixed_or_and — V-model traceability

## Requirement chain

- **REQ-027** Truth-table emission
- **REQ-028** Independent-effect citation
- **REQ-030** Verdict suite

## Design decisions

- **DEC-013** Trace-buffer primitive (sparse `evaluated` map is essential here)
- **DEC-014** Short-circuit preserved
- **DEC-016** Verdict suite composition

## Conditions

4 booleans (c1..c4 = a, b, c, d in source).

## Test rows

5 rows. See `TRUTH-TABLE.md`.

## Evidence

`compliance/verdict-evidence/mixed_or_and/`.

## Why this verdict exists

This is the **most realistic 4-condition shape** in the suite. Real Rust
compound booleans rarely follow the textbook pure-AND or pure-OR
patterns — they mix. mixed_or_and stresses both outer-AND
short-circuit (row 0) and inner-OR short-circuit (rows 3, 4) in the
same decision, which is what makes the trace buffer's sparse
`evaluated` map (DEC-013) load-bearing: the reporter must distinguish
"condition was F" from "condition was masked" to find the right pair
for each independent-effect proof.

The verdict is also the smallest decision shape where a naïve eager-
evaluation primitive (Option A from the v06-instrumentation-primitive
research) would silently produce wrong MC/DC reports — by forcing all
4 conditions to evaluate, it would lose the masking-vs-false
distinction and could "find" pairs that don't exist in the actual run.
