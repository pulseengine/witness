# json_lite — expected coverage notes

This is a real-application fixture (hand-rolled parser). The full
reconstructed decision graph after rustc inlining is tree-shaped, with
multiple decisions per helper. The table here states the source
author's intent.

## Source-author intent — decision sites

| Site | Predicate / dispatch shape |
|---|---|
| `skip_ws` loop | `b == ' ' \|\| b == '\t' \|\| b == '\n' \|\| b == '\r'` (4-condition OR) |
| `parse_string` opener | `i >= buf.len() \|\| buf[i] != '"'` |
| `parse_string` body | terminator check, escape vs control vs ordinary byte |
| `parse_string` escape codes | 6-way OR over accepted escapes |
| `parse_string` control guard | `b < 0x20` |
| `parse_number` sign | `buf[i] == '-'` |
| `parse_number` digit loop | `b >= '0' && b <= '9'` (compound AND) |
| `parse_number` fractional tail | optional dot + digit-loop |
| `parse_keyword` byte-by-byte compare | per-byte mismatch |
| `parse_primitive` dispatch | `=='"'` / `=='t'` / `=='f'` / `=='n'` / `=='-' \|\| ('0'..'9')` |
| `parse_array_of_primitives` | empty-array shortcut, primitive call, comma-vs-close |
| `parse_object` | empty-object shortcut, key/colon/value/comma-vs-close |
| `parse` top-level | `'{' or '['`; trailing-ws-then-EOF check |

## Row classes covered

| Class | Rows | Notes |
|---|---|---|
| empty object/array | 0, 11 | shortcut paths in parse_object / parse_array |
| primitive values | 1, 2, 3, 12, 13 | exercise parse_primitive dispatch |
| number with fractional | 4, 13 | parse_number fractional tail |
| nested object (1 level) | 5, 6 | allow_nested_inside flips off at level 2 |
| array as value | 7, 8 | parse_array_of_primitives reached via parse_value |
| all escape codes | 9 | escape-OR all-T row |
| leading/trailing ws | 10 | top-level skip_ws on both sides |
| whitespace inside structure | 4, 14 | mid-array/mid-object skip_ws |
| missing close brace | 15 | parse_object falls off end |
| missing open brace | 16 | parse top-level dispatch fails |
| unterminated string | 17 | parse_string runs off end |
| bad escape | 18 | escape-OR all-F |
| missing colon | 19 | object key-colon-value sequence |
| trailing comma | 20 | comma-then-close transition |
| trailing garbage | 21 | parse top-level trailing-bytes check |
| bare keyword | 22 | parse top-level dispatch rejects |
| empty input | 23 | parse top-level i >= len |
| non-string key | 24 | parse_object first-key parse_string fails |
| over-deep nest | 25 | allow_nested_inside guard |
| trailing-dot number | 26 | parse_number fractional empty |
| raw control byte | 27 | parse_string b < 0x20 |

## Expected reporter behaviour

Witness will reconstruct ~40-80 decisions across all helpers (each
helper's `if`/`while` chain plus inlined slice indexing and bounds
checks). MC/DC pairs will be:

- Dense in `skip_ws` and `parse_string`'s escape-OR (rows 9 and 18 are
  paired all-T / all-F for the 6-way escape disjunction).
- Sparse in `parse_keyword` (most malformed rows fail at the same
  byte-mismatch decision).
- Distributed in `parse_object` / `parse_array_of_primitives` as the
  comma-vs-close branch is exercised by both valid (rows 3, 4, 12) and
  malformed (row 20) inputs.

Disagreements between the reporter and the intent table are bugs only
when a row's outcome is mis-classified or when a clearly source-visible
decision is missing from the reconstructed list.

## Verdict

**28 rows.** Like httparse and nom_numbers, full MC/DC across the full
reconstructed graph is *not* expected — the goal is broad reachability
and rich masking-pair structure across the parser's compound
predicates.

## Machine-readable section

```json
{
  "verdict": "json_lite",
  "kind": "real-application-handrolled-parser",
  "row_count": 28,
  "intent_decision_sites": [
    "skip_ws", "parse_string-opener", "parse_string-body",
    "parse_string-escape-or", "parse_string-control-guard",
    "parse_number-sign", "parse_number-digit-loop", "parse_number-fractional",
    "parse_keyword", "parse_primitive-dispatch",
    "parse_array_of_primitives", "parse_object",
    "parse-top-level", "parse-trailing-bytes"
  ],
  "expected_outcomes": {
    "true": [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14],
    "false": [15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27]
  }
}
```
