# json_lite — V-model traceability

## Requirement chain

- **REQ-027** Per-decision MC/DC truth-table emission
- **REQ-028** Independent-effect citation per condition
- **REQ-030** Verdict suite as canonical MC/DC evidence — *hand-rolled-parser fixture*

## Design decisions

- **DEC-013** MC/DC instrumentation primitive — linear-memory trace buffer with row markers
- **DEC-014** Short-circuit semantics preserved by instrumentation
- **DEC-016** Verdict suite composition — canonical compound-decision shapes (parser branches)

## Conditions

The parser is split into compound-predicate-rich helper functions, each
of which surfaces several decisions when instrumented:

| Helper | Compound predicates |
|---|---|
| `skip_ws` | byte-class disjunction `b == ' ' \|\| b == '\t' \|\| b == '\n' \|\| b == '\r'` |
| `parse_string` | escape recognition `esc == '"' \|\| esc == '\\' \|\| esc == '/' \|\| esc == 'n' \|\| esc == 'r' \|\| esc == 't'` plus the control-byte guard `b < 0x20` and the unterminated-string fall-through |
| `parse_number` | sign-byte branch, digit-loop bound check, fractional-tail branch, no-digits guard |
| `parse_primitive` | dispatch on first byte (string/keyword/number) — chained `if-else` with multiple compound conditions |
| `parse_array_of_primitives` | empty-array shortcut, comma vs close-bracket dispatch, primitive-call propagation |
| `parse_object` | empty-object shortcut, key-colon-value pattern, comma vs close-brace dispatch |
| `parse` | top-level dispatch (object vs array) and trailing-bytes check |

## Test rows

28 rows in `src/lib.rs` (`run_row_0` through `run_row_27`). Coverage
classes:

- 11 valid objects (rows 0-10) covering primitives, nested object, array
  value, escape strings, leading/trailing whitespace
- 4 valid arrays (rows 11-14) covering empty array, mixed primitives,
  whitespace inside array
- 13 malformed (rows 15-27) covering missing brace, unterminated
  string, bad escape, missing colon, trailing comma, trailing garbage,
  bare keyword, empty buffer, non-string key, doubly-nested object,
  trailing dot in number, raw control byte in string

## Evidence (produced by release pipeline)

| Path | Origin |
|---|---|
| `target/wasm32-unknown-unknown/release/verdict_json_lite.wasm` | `cargo build --release --target wasm32-unknown-unknown` |
| `compliance/verdict-evidence/json_lite/source.wasm` | suite driver |
| `compliance/verdict-evidence/json_lite/instrumented.wasm` | `witness instrument` |
| `compliance/verdict-evidence/json_lite/manifest.json` | `witness instrument` |
| `compliance/verdict-evidence/json_lite/run.json` | `witness run` |
| `compliance/verdict-evidence/json_lite/report.{txt,json}` | `witness report` |
| `compliance/verdict-evidence/json_lite/predicate.json` | `witness predicate` |
| `compliance/verdict-evidence/json_lite/signed.dsse.json` | `witness attest` (DSSE-signed) |

## Why this verdict exists

Hand-rolled byte-level parsers are pervasive across embedded firmware,
config-loaders, network protocol implementations, and language
front-ends. Their idiomatic shape is a tree of `if`/`while` chains with
compound byte-class predicates and explicit short-circuit error
propagation. Verifying that witness reconstructs MC/DC for this shape
without depending on a parser-combinator library closes the loop on
"can you measure structural code coverage on cryptographic / wire-
format parsers?" — which is *the* canonical question DO-178C reviewers
ask of a Wasm-deployed safety system.
