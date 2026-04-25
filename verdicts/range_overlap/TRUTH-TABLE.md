# range_overlap — expected MC/DC truth table

## Decision

`a.start <= b.end && b.start <= a.end`

## Conditions

| Id | Source |
|---|---|
| c1 | `a.start <= b.end` |
| c2 | `b.start <= a.end` |

## Test rows (under Rust short-circuit semantics)

| Row | a (start, end) | b (start, end) | c1 | c2 | Outcome |
|---|---|---|---|---|---|
| 0 | (0, 1) | (2, 3) | T | F | F |
| 1 | (2, 3) | (0, 1) | F | — | F |
| 2 | (0, 3) | (1, 2) | T | T | T |

## Independent-effect proofs (masking MC/DC)

### c1

| | c1 | c2 | Outcome |
|---|---|---|---|
| row 1 | **F** | — | F |
| row 2 | **T** | T | T |

c1 differs, c2 in row 1 is masked. Outcomes differ. **PROVED**.

### c2

| | c1 | c2 | Outcome |
|---|---|---|---|
| row 0 | T | **F** | F |
| row 2 | T | **T** | T |

c1 same, c2 differs. Outcomes differ. **PROVED** (unique-cause).

## Verdict

**3 rows, 2 conditions, full MC/DC.**

## Incomplete-rows scenario

Removing row 2: c1 only ever evaluates to F or with c2 masked F outcome.
No row with `c1=T, c2=T, outcome=T` to pair against. c1 has a gap.
Reporter recommends a row with `(c1=T, c2=T)` and expected outcome T.

## Machine-readable section

```json
{
  "verdict": "range_overlap",
  "decision": "a.start <= b.end && b.start <= a.end",
  "conditions": [
    { "id": "c1", "source": "a.start <= b.end" },
    { "id": "c2", "source": "b.start <= a.end" }
  ],
  "passing_rows": [
    { "row": 0, "input": { "a": [0,1], "b": [2,3] }, "evaluated": { "c1": true,  "c2": false }, "outcome": false },
    { "row": 1, "input": { "a": [2,3], "b": [0,1] }, "evaluated": { "c1": false              }, "outcome": false },
    { "row": 2, "input": { "a": [0,3], "b": [1,2] }, "evaluated": { "c1": true,  "c2": true  }, "outcome": true  }
  ],
  "independent_effect_proofs": {
    "c1": { "pair": [1, 2], "interpretation": "masking" },
    "c2": { "pair": [0, 2], "interpretation": "unique-cause" }
  },
  "incomplete_scenario": {
    "removed_rows": [2],
    "expected_gap": {
      "condition": "c1",
      "recommended_row": { "evaluated": { "c1": true, "c2": true }, "expected_outcome": true }
    }
  }
}
```
