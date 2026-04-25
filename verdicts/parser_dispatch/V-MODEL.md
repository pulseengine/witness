# parser_dispatch — V-model traceability

## Requirement chain

- **REQ-027** Truth-table emission
- **REQ-028** Independent-effect citation
- **REQ-029** Gap-closure recommendation (this verdict's c4/c5 OR is the hardest case)
- **REQ-030** Verdict suite — *real-world anchor*

## Design decisions

- **DEC-013** Trace-buffer primitive
- **DEC-014** Short-circuit preserved (OR sub-expression mid-decision)
- **DEC-016** Verdict suite composition (this verdict's reason for being)

## Conditions

5 boolean expressions on a `&[u8]` URL authority candidate. See
`TRUTH-TABLE.md` for source-level definitions.

## Test rows

6 rows. Each input is a real authority-string shape: empty, with
embedded space, with userinfo, bare-host-with-port, bare-host, IPv6
brackets.

## Evidence

`compliance/verdict-evidence/parser_dispatch/`.

## Why this verdict exists

parser_dispatch is the suite's **non-synthetic anchor**. The other six
verdicts are textbook MC/DC examples — useful for verification of the
reporter's correctness, but open to the criticism that they only
demonstrate witness on toys.

This verdict provides a **real-world predicate shape** from a real-
world domain (URL parsing). The decision pattern `c1 && c2 && c3 &&
(c4 || c5)` is pervasive in input validation across the Rust
ecosystem; the 5-condition mix-of-AND-and-OR is exactly the shape
where naïve eager-evaluation MC/DC instrumentation produces wrong
reports. This verdict is therefore *both* the suite's real-world
credibility anchor *and* the trickiest reporter test (because the
inner OR's pair-finding doesn't reduce to "every cond vs the all-T
row", which works for state_guard and safety_envelope).

## What this verdict does NOT cover

Pattern matching dispatched via `br_table` — that's deferred to v0.7
per DEC-015. parser_dispatch deliberately uses byte-slice methods
(`s.contains`, `s.first`) that lower to `br_if`/`if-else`, not
`match` arms. A v0.7 verdict using a real `match` expression on
parser tokens will close that scope.
