# state_guard — V-model traceability

## Requirement chain

- **REQ-027** Per-decision MC/DC truth-table emission
- **REQ-028** Independent-effect citation per condition
- **REQ-030** Verdict suite as canonical MC/DC evidence

## Design decisions

- **DEC-013** Trace-buffer instrumentation primitive
- **DEC-014** Short-circuit semantics preserved (this verdict is the canonical stress test)
- **DEC-016** Verdict suite composition

## Conditions

4 boolean flags on a `HandshakeState` struct (c1..c4 in source).

## Test rows

5 rows in `src/lib.rs`. See `TRUTH-TABLE.md` for the 4-cond-AND truth table.

## Evidence

`compliance/verdict-evidence/state_guard/`.

## Why this verdict exists

state_guard is the **deep-AND-chain stress test**. The 5-row optimal
pattern (one row per condition transition + one all-T row) is the
textbook lower bound for N-condition AND under masking MC/DC, so it
serves as a unit test for the reporter's pair-finding algorithm.

It also justifies DEC-014's short-circuit-preservation policy: any
primitive that forced all 4 conditions to evaluate per row would
materially change observable behaviour for stateful code (each flag
read could trigger memory loads with cache effects in a real handshake
implementation). Witness must not change observable behaviour to
measure coverage; this verdict makes the constraint visible.
