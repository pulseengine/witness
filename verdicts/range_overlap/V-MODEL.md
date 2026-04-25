# range_overlap — V-model traceability

## Requirement chain

- **REQ-027** Per-decision MC/DC truth-table emission
- **REQ-028** Independent-effect citation per condition
- **REQ-030** Verdict suite as canonical MC/DC evidence

## Design decisions

- **DEC-013** MC/DC instrumentation primitive — linear-memory trace buffer with row markers
- **DEC-014** Short-circuit semantics preserved by instrumentation
- **DEC-016** Verdict suite composition

## Conditions

| Id | Source-level | Wasm-level |
|---|---|---|
| c1 | `a.start <= b.end` | `br_if` from `&&` lowering |
| c2 | `b.start <= a.end` | `br_if` (short-circuited when c1=F) |

## Test rows

3 rows (`run_row_0` through `run_row_2`). See `TRUTH-TABLE.md`.

## Evidence

`compliance/verdict-evidence/range_overlap/` in the release archive contains
the full instrument → run → report → predicate chain.

## Why this verdict exists

range_overlap is the **minimal compound decision** — the smallest possible
compound boolean (2 conditions, single AND). It exists in the suite to:

1. Demonstrate that witness's MC/DC reporting works for compound decisions
   that are too small to require the full machinery.
2. Provide a fast smoke test for the reporter — when the leap_year truth
   table is wrong, range_overlap is the easier failure to debug first.
3. Anchor the lower bound of the suite's complexity range.
