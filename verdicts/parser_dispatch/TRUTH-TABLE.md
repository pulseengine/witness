# parser_dispatch — expected MC/DC truth table

## Decision

```
!s.is_empty()
    && !s.contains(b' ')
    && !s.contains(b'@')
    && (s.first() == Some(&b'[') || !s.contains(b':'))
```

(RFC 3986–shaped URL authority validator, simplified.)

## Conditions

| Id | Source |
|---|---|
| c1 | `!s.is_empty()` |
| c2 | `!s.contains(b' ')` |
| c3 | `!s.contains(b'@')` |
| c4 | `s.first() == Some(&b'[')` |
| c5 | `!s.contains(b':')` |

The structure is `c1 && c2 && c3 && (c4 || c5)`. The inner `c4 || c5`
is short-circuit OR.

## Test rows

| Row | input | c1 | c2 | c3 | c4 | c5 | Outcome | Notes |
|---|---|---|---|---|---|---|---|---|
| 0 | `""` | F | — | — | — | — | F | empty |
| 1 | `"x y"` | T | F | — | — | — | F | space rejected |
| 2 | `"u@h"` | T | T | F | — | — | F | userinfo rejected |
| 3 | `"h:80"` | T | T | T | F | F | F | bare host with port |
| 4 | `"h"` | T | T | T | F | T | T | bare host, no port |
| 5 | `"[fe80::]"` | T | T | T | T | — | T | IPv6 brackets |

## Independent-effect proofs (masking MC/DC)

### c1 (`!is_empty`)

Pair: row 0 vs row 4.

| | c1 | c2 | c3 | c4 | c5 | Outcome |
|---|---|---|---|---|---|---|
| row 0 | **F** | — | — | — | — | F |
| row 4 | **T** | T | T | F | T | T |

c1 differs; c2-c5 in row 0 all masked. **PROVED**.

### c2 (`!contains-space`)

Pair: row 1 vs row 4.

c1 same (T), c2 differs F → T, c3-c5 in row 1 masked. **PROVED**.

### c3 (`!contains-@`)

Pair: row 2 vs row 4.

c1, c2 same; c3 differs; c4, c5 in row 2 masked. **PROVED**.

### c4 (`starts-with-[`)

Pair: row 3 vs row 5.

| | c1 | c2 | c3 | c4 | c5 | Outcome |
|---|---|---|---|---|---|---|
| row 3 | T | T | T | **F** | F | F |
| row 5 | T | T | T | **T** | — | T |

c1-c3 same; c4 differs; c5 in row 5 masked, F in row 3 (compatible). **PROVED**.

### c5 (`!contains-:`)

Pair: row 3 vs row 4.

| | c1 | c2 | c3 | c4 | c5 | Outcome |
|---|---|---|---|---|---|---|
| row 3 | T | T | T | F | **F** | F |
| row 4 | T | T | T | F | **T** | T |

All other conditions identical. c5 differs. **PROVED** (unique-cause).

## Verdict

**6 rows, 5 conditions, full MC/DC** under masking. Note that this is
a 5-cond decision with mixed operators — the row count matches
safety_envelope (also 6 rows for 5 conds) but the proof structure is
non-trivial: c4 and c5 form an OR sub-expression, so the standard
"every cond against the all-T row" pattern doesn't apply.

## Why this verdict matters

parser_dispatch is **the suite's defence against the "but it only works
on toys" criticism.** Every other verdict in the suite is a synthetic
textbook example. This one is a real-world predicate shape from
a real-world domain (URL parsing).

It's also the suite's mixed-operator-with-real-semantics test: the
inner OR `(c4 || c5)` short-circuits in two distinct ways depending
on input, and the reporter must correctly derive the row pairings
without confusing the OR's short-circuit behaviour with the outer
AND's.

## Machine-readable section

```json
{
  "verdict": "parser_dispatch",
  "decision": "!s.is_empty() && !s.contains(b' ') && !s.contains(b'@') && (s.first() == Some(&b'[') || !s.contains(b':'))",
  "conditions": [
    { "id": "c1", "source": "!s.is_empty()" },
    { "id": "c2", "source": "!s.contains(b' ')" },
    { "id": "c3", "source": "!s.contains(b'@')" },
    { "id": "c4", "source": "s.first() == Some(&b'[')" },
    { "id": "c5", "source": "!s.contains(b':')" }
  ],
  "passing_rows": [
    { "row": 0, "input": "",         "evaluated": { "c1": false                                                                  }, "outcome": false },
    { "row": 1, "input": "x y",      "evaluated": { "c1": true,  "c2": false                                                     }, "outcome": false },
    { "row": 2, "input": "u@h",      "evaluated": { "c1": true,  "c2": true,  "c3": false                                        }, "outcome": false },
    { "row": 3, "input": "h:80",     "evaluated": { "c1": true,  "c2": true,  "c3": true,  "c4": false, "c5": false              }, "outcome": false },
    { "row": 4, "input": "h",        "evaluated": { "c1": true,  "c2": true,  "c3": true,  "c4": false, "c5": true               }, "outcome": true  },
    { "row": 5, "input": "[fe80::]", "evaluated": { "c1": true,  "c2": true,  "c3": true,  "c4": true                            }, "outcome": true  }
  ],
  "independent_effect_proofs": {
    "c1": { "pair": [0, 4], "interpretation": "masking" },
    "c2": { "pair": [1, 4], "interpretation": "masking" },
    "c3": { "pair": [2, 4], "interpretation": "masking" },
    "c4": { "pair": [3, 5], "interpretation": "masking" },
    "c5": { "pair": [3, 4], "interpretation": "unique-cause" }
  }
}
```
