# nom_numbers — V-model traceability

## Requirement chain

- **REQ-027** Per-decision MC/DC truth-table emission
- **REQ-028** Independent-effect citation per condition
- **REQ-030** Verdict suite as canonical MC/DC evidence — *real-application fixture*

## Design decisions

- **DEC-013** MC/DC instrumentation primitive — linear-memory trace buffer with row markers
- **DEC-014** Short-circuit semantics preserved by instrumentation
- **DEC-016** Verdict suite composition — real-application coverage anchors

## Conditions

The user-written predicate `parse_int` contains a small set of compound
checks (sign byte classification, take_while1 success, accumulator
overflow guards, signed-range vs unsigned-range branch, residue test).
Once nom's combinators inline, the post-rustc Wasm exposes substantially
more decisions: `take_while1`'s digit-classification loop, `IResult`
match-arm dispatch, `checked_mul`/`checked_add` carry checks, and the
`acc > u32::MAX as u64` range guard. Like httparse, this is a
real-application fixture — the truth-table file describes intent, not
the full reconstructed decision graph.

## Test rows

28 rows in `src/lib.rs` (`run_row_0` through `run_row_27`). Coverage
classes:

- valid magnitudes (single digit, multi-digit, u32::MAX exact)
- signed inputs (positive sign, negative sign, i32::MIN boundary)
- leading zeros (single, multiple, padded-to-u32::MAX)
- overflow (u32::MAX + 1, far overflow, signed beyond i32::MIN)
- malformed (sign-only, double-sign, hex/oct prefix, embedded space,
  letters, trailing whitespace, trailing null, leading whitespace)
- empty input

## Evidence (produced by release pipeline)

| Path | Origin |
|---|---|
| `target/wasm32-unknown-unknown/release/verdict_nom_numbers.wasm` | `cargo build --release --target wasm32-unknown-unknown` |
| `compliance/verdict-evidence/nom_numbers/source.wasm` | suite driver |
| `compliance/verdict-evidence/nom_numbers/instrumented.wasm` | `witness instrument` |
| `compliance/verdict-evidence/nom_numbers/manifest.json` | `witness instrument` |
| `compliance/verdict-evidence/nom_numbers/run.json` | `witness run` |
| `compliance/verdict-evidence/nom_numbers/report.{txt,json}` | `witness report --format mcdc[-json]` |
| `compliance/verdict-evidence/nom_numbers/predicate.json` | `witness predicate` |
| `compliance/verdict-evidence/nom_numbers/signed.dsse.json` | `witness attest` (DSSE-signed) |

## Why this verdict exists

nom is one of the most widely used parser-combinator libraries in the
Rust ecosystem. A real witness deployment will encounter nom-derived
decisions across *every* binary protocol parser, every DSL frontend,
and most config-file readers shipped under no_std. Verifying that
witness reconstructs nom's decision graph cleanly — including the
combinators that hide compound predicates behind macro-generated
`IResult` plumbing — is the test-of-fitness for the v0.7+ MC/DC
pipeline against parser-combinator code.
