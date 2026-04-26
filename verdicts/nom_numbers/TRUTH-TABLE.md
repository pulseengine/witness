# nom_numbers — expected coverage notes

This is a real-application fixture (like httparse), **not** a
hand-derived synthetic truth table. The full decision graph after rustc
optimisation includes nom's combinator internals, take_while1's
digit-classification loop, `IResult` dispatch, and the inlined
`checked_mul`/`checked_add` carry checks — far more than the
source-author wrote.

## Source-author intent

The hand-written predicate `parse_int` has these decision points:

| Tag | Source predicate |
|---|---|
| t1 | `i.first()` is `Some(&b'-')` or `Some(&b'+')` (sign opt) |
| t2 | `take_while1(is_digit)` — at least one digit consumed |
| t3 | `!rest.is_empty()` — trailing garbage check |
| t4 | `checked_mul(10)` overflowed |
| t5 | `checked_add(d)` overflowed |
| t6 | `acc > u32::MAX as u64` — magnitude range guard |
| t7 | sign branch — `b'-'` vs `b'+'`/None |
| t8 | signed-range guard `acc > (i32::MAX as u64) + 1` |
| t9 | unsigned-range guard `acc > u32::MAX as u64` |

## Row classes covered

| Class | Rows |
|---|---|
| valid u32 magnitudes | 0, 1, 2, 3, 4, 5, 10, 26 |
| valid signed (with sign byte) | 8, 11, 12 |
| signed boundary (i32::MIN) | 8 |
| overflow (u32) | 6, 7 |
| overflow (signed) | 9 |
| leading zeros | 13, 14, 26 |
| empty / sign-only / double-sign | 15, 16, 17 |
| whitespace (leading / trailing / embedded) | 18, 19, 23 |
| prefix-style malformed (0x, 0o) | 20, 21 |
| letters / mixed | 22, 24, 25 |
| trailing null byte | 27 |

## Expected reporter behaviour

Like httparse, witness will surface ~30-100 reconstructed decisions
across:

1. The user-source predicate (sign + digits + range).
2. nom's `take_while1` digit-loop (one decision per byte iteration
   merged by adjacent-line clustering).
3. `IResult` match-arm dispatch (Ok/Err at each `?` propagation site).
4. Inlined `u64::checked_mul` / `checked_add` arithmetic (carry-check
   pairs that produce `Option<u64>` short-circuit branches).
5. Range comparisons (`acc > u32::MAX as u64` and the signed variant).

MC/DC pairs will be sparse on the failure paths (most malformed rows
fall out of nom's combinator chain at the same first-failure decision)
and dense on the success path (every overflow-guard branch is exercised
by the legal-magnitude rows). This is expected and documented as the
"real-application sparse-pair pattern" alongside httparse.

## Verdict

**28 rows.** Full MC/DC is *not* the goal — coverage breadth across
nom's combinator graph is. Disagreements between the reporter and the
intent table above are bugs only when they mis-classify a row's outcome
or fail to reconstruct a decision the source clearly contains.

## Machine-readable section

```json
{
  "verdict": "nom_numbers",
  "kind": "real-application",
  "row_count": 28,
  "intent_decisions": [
    "sign-opt", "digits-take_while1", "trailing-residue",
    "checked_mul-overflow", "checked_add-overflow",
    "u32-range-guard", "sign-branch",
    "signed-range-guard", "unsigned-range-guard"
  ],
  "expected_outcomes": {
    "true": [0, 1, 2, 3, 4, 5, 8, 10, 11, 12, 13, 14, 26],
    "false": [6, 7, 9, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 27]
  }
}
```
