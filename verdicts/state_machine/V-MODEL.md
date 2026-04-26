# state_machine — V-model traceability

## Requirement chain

- **REQ-027** Per-decision MC/DC truth-table emission
- **REQ-028** Independent-effect citation per condition
- **REQ-030** Verdict suite as canonical MC/DC evidence — *security-protocol fixture*

## Design decisions

- **DEC-013** MC/DC instrumentation primitive — linear-memory trace buffer with row markers
- **DEC-014** Short-circuit semantics preserved by instrumentation
- **DEC-016** Verdict suite composition — canonical compound-decision shapes (TLS 1.3 transition guard)

## Conditions

The `can_advance_to` predicate is an 8-conjunct AND chain whose each
conjunct is itself a compound expression:

| Id | Source-level conjunct |
|---|---|
| g1 | `valid_pair(state, next)` |
| g2 | `state != Failed` |
| g3 | `state != Established \|\| next == Failed` |
| g4 | `next != EncryptedExtensions \|\| ctx.have_keys` |
| g5 | `next != CertSent \|\| ctx.cert_loaded` |
| g6 | `next != CertVerifySent \|\| (ctx.cert_loaded && ctx.have_keys)` |
| g7 | `next != FinishedSent \|\| ctx.transcript_hash_ok` |
| g8 | `next != Established \|\| (ctx.peer_finished && ctx.transcript_hash_ok)` |

Inside `valid_pair`, three branches form their own decision (failure
fast-path, terminal-state guards, increment-by-one check). Conjuncts g6
and g8 contain inner ANDs, giving witness genuine compound-MC/DC
material to reconstruct.

## Test rows

27 rows in `src/lib.rs` (`run_row_0` through `run_row_26`):

- 7 rows along the happy path with a fully-satisfied context (rows 0-6)
- 7 rows where exactly one ctx flag is missing (rows 7-13) — exercises
  each `next != X || ctx.flag` guard's independent-effect pair
- 6 rows of malformed graph edges (rows 14-19) — exercises `valid_pair`
- 5 rows of error transitions to `Failed` (rows 20-24) including the
  special-case `Established -> Failed`
- 2 rows of race / partial-ctx scenarios (rows 25-26)

## Evidence (produced by release pipeline)

| Path | Origin |
|---|---|
| `target/wasm32-unknown-unknown/release/verdict_state_machine.wasm` | `cargo build --release --target wasm32-unknown-unknown` |
| `compliance/verdict-evidence/state_machine/source.wasm` | suite driver |
| `compliance/verdict-evidence/state_machine/instrumented.wasm` | `witness instrument` |
| `compliance/verdict-evidence/state_machine/manifest.json` | `witness instrument` |
| `compliance/verdict-evidence/state_machine/run.json` | `witness run` |
| `compliance/verdict-evidence/state_machine/report.{txt,json}` | `witness report` |
| `compliance/verdict-evidence/state_machine/predicate.json` | `witness predicate` |
| `compliance/verdict-evidence/state_machine/signed.dsse.json` | `witness attest` (DSSE-signed) |

## Why this verdict exists

State-machine transition guards are the canonical compound-AND shape
in security-critical code: TLS handshakes, OAuth flows, kernel
syscalls, mTLS authenticators. The predicate has the structure
"only-allow X if (graph-ok AND each-prerequisite-met)" — which
compiles down to a long AND chain with nested context guards. Every
DO-178C and IEC 61508 review on a security stack will look for MC/DC
evidence on exactly this shape. Including it in the witness suite
proves the reporter handles the **deep AND chain with inner ANDs**
case correctly under masking interpretation.
