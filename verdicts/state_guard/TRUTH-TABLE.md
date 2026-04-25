# state_guard — expected MC/DC truth table

## Decision

`c1 && c2 && c3 && c4` (4-condition AND chain)

## Conditions

| Id | Source | Description |
|---|---|---|
| c1 | `client_hello_received` | TLS state flag |
| c2 | `server_hello_sent` | TLS state flag |
| c3 | `cert_sent` | TLS state flag |
| c4 | `key_exchange_received` | TLS state flag |

## Test rows (Rust short-circuit AND — stops at first F)

| Row | c1 | c2 | c3 | c4 | Outcome |
|---|---|---|---|---|---|
| 0 | F | — | — | — | F |
| 1 | T | F | — | — | F |
| 2 | T | T | F | — | F |
| 3 | T | T | T | F | F |
| 4 | T | T | T | T | T |

## Independent-effect proofs (all pair against row 4)

### c1: row 0 vs row 4

| | c1 | c2 | c3 | c4 | Outcome |
|---|---|---|---|---|---|
| row 0 | **F** | — | — | — | F |
| row 4 | **T** | T | T | T | T |

c1 differs; c2-c4 in row 0 all masked. **PROVED**.

### c2: row 1 vs row 4

| | c1 | c2 | c3 | c4 | Outcome |
|---|---|---|---|---|---|
| row 1 | T | **F** | — | — | F |
| row 4 | T | **T** | T | T | T |

**PROVED**.

### c3: row 2 vs row 4

| | c1 | c2 | c3 | c4 | Outcome |
|---|---|---|---|---|---|
| row 2 | T | T | **F** | — | F |
| row 4 | T | T | **T** | T | T |

**PROVED**.

### c4: row 3 vs row 4

| | c1 | c2 | c3 | c4 | Outcome |
|---|---|---|---|---|---|
| row 3 | T | T | T | **F** | F |
| row 4 | T | T | T | **T** | T |

**PROVED** (unique-cause).

## Verdict

**5 rows, 4 conditions, full MC/DC** — the optimal `N+1` pattern for
N-condition AND chains under masking interpretation.

## Incomplete-rows scenario

Remove row 4: every other row outcome is F. No row with outcome=T.
**All four conditions** have gaps simultaneously. Reporter recommends
the all-T row as the single closure for all four gaps.

## Why this verdict matters

State-machine guards are pervasive in safety-critical Rust (TLS, USB,
CAN bus, automotive E/E). The 4-condition AND chain is the
**lower-bound stress test for eager-evaluation alternatives** — any
instrumentation primitive that forces all 4 conditions to evaluate per
invocation would change behaviour for handshake code, which makes
DEC-014's short-circuit-preservation policy non-negotiable.

## Machine-readable section

```json
{
  "verdict": "state_guard",
  "decision": "c1 && c2 && c3 && c4",
  "conditions": [
    { "id": "c1", "source": "client_hello_received" },
    { "id": "c2", "source": "server_hello_sent" },
    { "id": "c3", "source": "cert_sent" },
    { "id": "c4", "source": "key_exchange_received" }
  ],
  "passing_rows": [
    { "row": 0, "evaluated": { "c1": false                                          }, "outcome": false },
    { "row": 1, "evaluated": { "c1": true,  "c2": false                             }, "outcome": false },
    { "row": 2, "evaluated": { "c1": true,  "c2": true,  "c3": false                }, "outcome": false },
    { "row": 3, "evaluated": { "c1": true,  "c2": true,  "c3": true,  "c4": false   }, "outcome": false },
    { "row": 4, "evaluated": { "c1": true,  "c2": true,  "c3": true,  "c4": true    }, "outcome": true  }
  ],
  "independent_effect_proofs": {
    "c1": { "pair": [0, 4], "interpretation": "masking" },
    "c2": { "pair": [1, 4], "interpretation": "masking" },
    "c3": { "pair": [2, 4], "interpretation": "masking" },
    "c4": { "pair": [3, 4], "interpretation": "unique-cause" }
  },
  "incomplete_scenario": {
    "removed_rows": [4],
    "expected_gap_all": ["c1", "c2", "c3", "c4"],
    "recommended_row": { "evaluated": { "c1": true, "c2": true, "c3": true, "c4": true }, "expected_outcome": true }
  }
}
```
