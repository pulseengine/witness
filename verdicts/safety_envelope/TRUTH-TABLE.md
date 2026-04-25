# safety_envelope — expected MC/DC truth table

## Decision

`temp < T_MAX && press > P_MIN && rpm < RPM_MAX && !fault && mode == MODE_ACTIVE`

(5 conditions chained with `&&`)

## Conditions

| Id | Source | Constants |
|---|---|---|
| c1 | `temp < T_MAX` | T_MAX = 100 |
| c2 | `pressure > P_MIN` | P_MIN = 50 |
| c3 | `rpm < RPM_MAX` | RPM_MAX = 5000 |
| c4 | `!fault` | |
| c5 | `mode == MODE_ACTIVE` | MODE_ACTIVE = 1 |

## Test rows (5-cond AND, optimal N+1 = 6)

| Row | c1 | c2 | c3 | c4 | c5 | Outcome |
|---|---|---|---|---|---|---|
| 0 | F | — | — | — | — | F |
| 1 | T | F | — | — | — | F |
| 2 | T | T | F | — | — | F |
| 3 | T | T | T | F | — | F |
| 4 | T | T | T | T | F | F |
| 5 | T | T | T | T | T | T |

## Independent-effect proofs (each ci paired against row 5)

### c1: row 0 vs row 5

c1 differs F → T; c2-c5 in row 0 all masked. **PROVED**.

### c2: row 1 vs row 5

c1 same (T), c2 differs F → T, c3-c5 in row 1 masked. **PROVED**.

### c3: row 2 vs row 5

c1, c2 same; c3 differs; c4, c5 in row 2 masked. **PROVED**.

### c4: row 3 vs row 5

c1-c3 same; c4 differs; c5 in row 3 masked. **PROVED**.

### c5: row 4 vs row 5

c1-c4 all T in both; c5 differs F → T. **PROVED** (unique-cause).

## Verdict

**6 rows, 5 conditions, full MC/DC** — the optimal `N+1` pattern,
matching the state_guard analysis at one higher condition count.

## Why this verdict matters

This is the **scaling-stress** verdict. Three things it proves:

1. **No artificial 6-condition cap.** Clang and rustc-mcdc cap at 6
   conditions per decision because their LLVM bitmap encoder needs
   `2^N` table entries. Witness's trace buffer has no encoder
   constraint. Five conditions is below the cap, but the suite is
   designed to be extended — once safety_envelope works, a 7-cond
   variant trivially follows.
2. **Real-world shape.** Safety envelopes are pervasive in
   automotive (E/E architectures), aerospace (flight envelope
   protection), industrial control (interlock systems). The
   "all clear" check is almost always a long AND chain.
3. **Optimal N+1 row pattern under masking.** The verdict's row
   pattern is the textbook minimal MC/DC test set for an N-cond
   AND chain: one row per condition becoming the first F, plus one
   all-T row. The reporter must reproduce this minimal pattern for
   the verdict's gap-closure recommendations.

## Machine-readable section

```json
{
  "verdict": "safety_envelope",
  "decision": "c1 && c2 && c3 && c4 && c5",
  "conditions": [
    { "id": "c1", "source": "temp < T_MAX" },
    { "id": "c2", "source": "pressure > P_MIN" },
    { "id": "c3", "source": "rpm < RPM_MAX" },
    { "id": "c4", "source": "!fault" },
    { "id": "c5", "source": "mode == MODE_ACTIVE" }
  ],
  "passing_rows": [
    { "row": 0, "evaluated": { "c1": false                                                              }, "outcome": false },
    { "row": 1, "evaluated": { "c1": true,  "c2": false                                                 }, "outcome": false },
    { "row": 2, "evaluated": { "c1": true,  "c2": true,  "c3": false                                    }, "outcome": false },
    { "row": 3, "evaluated": { "c1": true,  "c2": true,  "c3": true,  "c4": false                       }, "outcome": false },
    { "row": 4, "evaluated": { "c1": true,  "c2": true,  "c3": true,  "c4": true,  "c5": false          }, "outcome": false },
    { "row": 5, "evaluated": { "c1": true,  "c2": true,  "c3": true,  "c4": true,  "c5": true           }, "outcome": true  }
  ],
  "independent_effect_proofs": {
    "c1": { "pair": [0, 5], "interpretation": "masking" },
    "c2": { "pair": [1, 5], "interpretation": "masking" },
    "c3": { "pair": [2, 5], "interpretation": "masking" },
    "c4": { "pair": [3, 5], "interpretation": "masking" },
    "c5": { "pair": [4, 5], "interpretation": "unique-cause" }
  }
}
```
