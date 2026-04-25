# mixed_or_and — expected MC/DC truth table

## Decision

`(a || b) && (c || d)`

## Conditions

| Id | Source |
|---|---|
| c1 | `a` |
| c2 | `b` |
| c3 | `c` |
| c4 | `d` |

## Test rows (Rust short-circuit semantics)

| Row | a | b | c | d | Outcome | Notes |
|---|---|---|---|---|---|---|
| 0 | F | F | — | — | F | `a\|\|b`=F → AND short-circuits |
| 1 | F | T | F | F | F | `a\|\|b`=T, `c\|\|d`=F |
| 2 | F | T | F | T | T | |
| 3 | F | T | T | — | T | `c\|\|d` short-circuits at c=T |
| 4 | T | — | F | T | T | `a\|\|b` short-circuits at a=T |

## Independent-effect proofs (masking MC/DC)

### a (c1)

Pair: row 0 vs row 4.

| | a | b | c | d | Outcome |
|---|---|---|---|---|---|
| row 0 | **F** | F | — | — | F |
| row 4 | **T** | — | F | T | T |

a differs (F vs T). b: F vs masked (compat). c: masked vs F (compat). d: masked vs T (compat). **PROVED**.

### b (c2)

Pair: row 0 vs row 2.

| | a | b | c | d | Outcome |
|---|---|---|---|---|---|
| row 0 | F | **F** | — | — | F |
| row 2 | F | **T** | F | T | T |

a same (F), b differs, c: masked vs F, d: masked vs T. **PROVED**.

### c (c3)

Pair: row 1 vs row 3.

| | a | b | c | d | Outcome |
|---|---|---|---|---|---|
| row 1 | F | T | **F** | F | F |
| row 3 | F | T | **T** | — | T |

a same, b same, c differs, d: F vs masked (compat). **PROVED**.

### d (c4)

Pair: row 1 vs row 2.

| | a | b | c | d | Outcome |
|---|---|---|---|---|---|
| row 1 | F | T | F | **F** | F |
| row 2 | F | T | F | **T** | T |

a, b, c all same. d differs. **PROVED** (unique-cause).

## Verdict

**5 rows, 4 conditions, full MC/DC.** Demonstrates correct handling of
**both** outer-AND short-circuit (row 0) and inner-OR short-circuit (rows 3, 4).

## Why this verdict matters

This is the most realistic 4-condition shape — most compound booleans
in real Rust code aren't pure-AND or pure-OR but mixed. The reporter
must:

1. Correctly identify that row 0's `(c, d)` are masked (not evaluated).
2. Find the right pair-by-row for each condition under masking MC/DC.
3. Distinguish "condition is F" from "condition is masked" (the
   `evaluated` map's sparse-key semantics from DEC-013).

## Machine-readable section

```json
{
  "verdict": "mixed_or_and",
  "decision": "(a || b) && (c || d)",
  "conditions": [
    { "id": "c1", "source": "a" },
    { "id": "c2", "source": "b" },
    { "id": "c3", "source": "c" },
    { "id": "c4", "source": "d" }
  ],
  "passing_rows": [
    { "row": 0, "evaluated": { "c1": false, "c2": false                          }, "outcome": false },
    { "row": 1, "evaluated": { "c1": false, "c2": true,  "c3": false, "c4": false }, "outcome": false },
    { "row": 2, "evaluated": { "c1": false, "c2": true,  "c3": false, "c4": true  }, "outcome": true  },
    { "row": 3, "evaluated": { "c1": false, "c2": true,  "c3": true               }, "outcome": true  },
    { "row": 4, "evaluated": { "c1": true,                "c3": false, "c4": true  }, "outcome": true  }
  ],
  "independent_effect_proofs": {
    "c1": { "pair": [0, 4], "interpretation": "masking" },
    "c2": { "pair": [0, 2], "interpretation": "masking" },
    "c3": { "pair": [1, 3], "interpretation": "masking" },
    "c4": { "pair": [1, 2], "interpretation": "unique-cause" }
  }
}
```
