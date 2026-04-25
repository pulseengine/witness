# leap_year — expected MC/DC truth table

## Decision

`(year % 4 == 0 && year % 100 != 0) || year % 400 == 0`

## Conditions

| Id | Source | Description |
|---|---|---|
| c1 | `year % 4 == 0` | divisible by 4 |
| c2 | `year % 100 != 0` | not divisible by 100 |
| c3 | `year % 400 == 0` | divisible by 400 |

## Test rows (under Rust short-circuit semantics)

| Row | year | c1 | c2 | c3 | Outcome | Conditions evaluated |
|---|---|---|---|---|---|---|
| 0 | 2001 | F | — | F | F | c1, c3 (c2 short-circuited by c1=F → c1&&c2=F → fall through to c3) |
| 1 | 2004 | T | T | — | T | c1, c2 (OR short-circuits c3 because c1&&c2=T) |
| 2 | 2100 | T | F | F | F | c1, c2, c3 |
| 3 | 2000 | T | F | T | T | c1, c2, c3 |

`—` = condition not evaluated under short-circuit semantics. The
`evaluated` map in the trace buffer is sparse — `—` means "the condition
slot is absent from this row's record".

## Independent-effect proofs (masking MC/DC, DO-178C accepted variant)

### c1 (`year % 4 == 0`)

Pair: row 0 vs row 1.

| | c1 | c2 | c3 | Outcome |
|---|---|---|---|---|
| row 0 | **F** | — | F | F |
| row 1 | **T** | T | — | T |

c1 differs. c2 in row 0 is masked (short-circuited by c1=F); c2 in row 1
is T. c3 in row 0 is F; c3 in row 1 is masked. Under masking MC/DC, the
masked positions are compatible with any value; outcomes differ →
**c1 has independent effect: PROVED**.

### c2 (`year % 100 != 0`)

Pair: row 1 vs row 2.

| | c1 | c2 | c3 | Outcome |
|---|---|---|---|---|
| row 1 | T | **T** | — | T |
| row 2 | T | **F** | F | F |

c1 same (T), c2 differs (T vs F), c3 in row 1 is masked (compatible),
c3 in row 2 is F. Outcomes differ → **c2 has independent effect: PROVED**.

### c3 (`year % 400 == 0`)

Pair: row 2 vs row 3.

| | c1 | c2 | c3 | Outcome |
|---|---|---|---|---|
| row 2 | T | F | **F** | F |
| row 3 | T | F | **T** | T |

c1 same, c2 same, c3 differs. Outcomes differ → **c3 has independent
effect: PROVED**.

## Verdict

**4 rows, 3 conditions, full MC/DC achieved under masking interpretation.**

## Incomplete-rows scenario (deliberate gap)

If row 3 is removed from the test suite, the row set becomes {0, 1, 2}.
c3 only ever evaluates to F (in rows 0 and 2). No row with c3=T exists,
so no pair can prove c3's independent effect. The reporter should:

- Flag c3 as an MC/DC gap.
- Recommend a row with `(c1=T, c2=F, c3=T)` and expected outcome `T`.

## Machine-readable section (for AI-agent consumers)

```json
{
  "verdict": "leap_year",
  "decision": "(year % 4 == 0 && year % 100 != 0) || year % 400 == 0",
  "conditions": [
    { "id": "c1", "source": "year % 4 == 0" },
    { "id": "c2", "source": "year % 100 != 0" },
    { "id": "c3", "source": "year % 400 == 0" }
  ],
  "passing_rows": [
    { "row": 0, "input": { "year": 2001 }, "evaluated": { "c1": false, "c3": false }, "outcome": false },
    { "row": 1, "input": { "year": 2004 }, "evaluated": { "c1": true,  "c2": true  }, "outcome": true  },
    { "row": 2, "input": { "year": 2100 }, "evaluated": { "c1": true,  "c2": false, "c3": false }, "outcome": false },
    { "row": 3, "input": { "year": 2000 }, "evaluated": { "c1": true,  "c2": false, "c3": true  }, "outcome": true  }
  ],
  "independent_effect_proofs": {
    "c1": { "pair": [0, 1], "interpretation": "masking" },
    "c2": { "pair": [1, 2], "interpretation": "masking" },
    "c3": { "pair": [2, 3], "interpretation": "unique-cause" }
  },
  "incomplete_scenario": {
    "removed_rows": [3],
    "expected_gap": {
      "condition": "c3",
      "recommended_row": { "evaluated": { "c1": true, "c2": false, "c3": true }, "expected_outcome": true }
    }
  }
}
```
