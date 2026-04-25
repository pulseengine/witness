# triangle — expected MC/DC truth table

## Decision

`a + b <= c || a + c <= b || b + c <= a` (Myers-paper "not a triangle")

## Conditions

| Id | Source |
|---|---|
| c1 | `a + b <= c` |
| c2 | `a + c <= b` |
| c3 | `b + c <= a` |

## Test rows (Rust short-circuit OR — stops at first T)

| Row | (a, b, c) | c1 | c2 | c3 | Outcome | Note |
|---|---|---|---|---|---|---|
| 0 | (3, 4, 5) | F | F | F | F | real 3-4-5 triangle |
| 1 | (1, 2, 5) | T | — | — | T | c=5 too long |
| 2 | (5, 1, 2) | F | F | T | T | a=5 too long |
| 3 | (1, 5, 2) | F | T | — | T | b=5 too long |

## Independent-effect proofs (masking MC/DC)

### c1

Pair: row 0 vs row 1.

| | c1 | c2 | c3 | Outcome |
|---|---|---|---|---|
| row 0 | **F** | F | F | F |
| row 1 | **T** | — | — | T |

c1 differs (F vs T). c2: F vs masked. c3: F vs masked. Outcomes differ. **PROVED**.

### c2

Pair: row 0 vs row 3.

| | c1 | c2 | c3 | Outcome |
|---|---|---|---|---|
| row 0 | F | **F** | F | F |
| row 3 | F | **T** | — | T |

c1 same (F), c2 differs, c3: F vs masked. **PROVED**.

### c3

Pair: row 0 vs row 2.

| | c1 | c2 | c3 | Outcome |
|---|---|---|---|---|
| row 0 | F | F | **F** | F |
| row 2 | F | F | **T** | T |

c1 same, c2 same, c3 differs. **PROVED** (unique-cause).

## Verdict

**4 rows, 3 conditions, full MC/DC under masking.**

## Incomplete-rows scenario

Remove row 2: c3 never evaluated to T (rows 1 and 3 short-circuit before
c3, row 0 is F). Gap on c3. Reporter recommends `(c1=F, c2=F, c3=T)` →
expected outcome T.

## Why this verdict matters

This is **the original MC/DC literature example**, dating to Myers (1979)
and re-used across DO-178C training material. If witness reports MC/DC
correctly here, it speaks the same language as the standard reference.

## Machine-readable section

```json
{
  "verdict": "triangle",
  "decision": "a + b <= c || a + c <= b || b + c <= a",
  "conditions": [
    { "id": "c1", "source": "a + b <= c" },
    { "id": "c2", "source": "a + c <= b" },
    { "id": "c3", "source": "b + c <= a" }
  ],
  "passing_rows": [
    { "row": 0, "input": { "a": 3, "b": 4, "c": 5 }, "evaluated": { "c1": false, "c2": false, "c3": false }, "outcome": false },
    { "row": 1, "input": { "a": 1, "b": 2, "c": 5 }, "evaluated": { "c1": true                          }, "outcome": true  },
    { "row": 2, "input": { "a": 5, "b": 1, "c": 2 }, "evaluated": { "c1": false, "c2": false, "c3": true  }, "outcome": true  },
    { "row": 3, "input": { "a": 1, "b": 5, "c": 2 }, "evaluated": { "c1": false, "c2": true              }, "outcome": true  }
  ],
  "independent_effect_proofs": {
    "c1": { "pair": [0, 1], "interpretation": "masking" },
    "c2": { "pair": [0, 3], "interpretation": "masking" },
    "c3": { "pair": [0, 2], "interpretation": "unique-cause" }
  },
  "incomplete_scenario": {
    "removed_rows": [2],
    "expected_gap": {
      "condition": "c3",
      "recommended_row": { "evaluated": { "c1": false, "c2": false, "c3": true }, "expected_outcome": true }
    }
  }
}
```
