# state_machine — expected coverage notes

This is a real-application fixture (security-protocol state machine).
The full reconstructed decision graph after rustc inlining includes
`valid_pair`'s internal branches, the enum-discriminant comparisons
lowered from `==`/`!=`, and the inlined `Ctx` field accesses. The truth
table here describes the source author's intent, not the post-rustc
decision count.

## Source-author intent — top-level decision

```text
valid_pair(state, next)
    && state != Failed
    && (state != Established || next == Failed)
    && (next != EncryptedExtensions || ctx.have_keys)
    && (next != CertSent          || ctx.cert_loaded)
    && (next != CertVerifySent    || (ctx.cert_loaded && ctx.have_keys))
    && (next != FinishedSent      || ctx.transcript_hash_ok)
    && (next != Established       || (ctx.peer_finished && ctx.transcript_hash_ok))
```

8 top-level conjuncts (`g1..g8`). Inner ORs at g3, g4, g5, g6, g7, g8.
Inner ANDs (within an OR) at g6 and g8.

## Row classes covered

| Class | Rows | Purpose |
|---|---|---|
| happy path forward | 0, 1, 2, 3, 4, 5, 6 | fire each `next != X` guard's true side |
| ctx-missing block | 7, 8, 9, 10, 11, 12, 13 | flip a single inner OR's ctx side |
| graph violation | 14, 15, 16, 17 | exercise `valid_pair` interior |
| terminal-state guard | 18, 19, 23, 24 | g2 / g3 boundary |
| error transition | 20, 21, 22, 23 | `next == Failed` shortcut path |
| race condition | 25, 26 | mixed ctx flags vs graph step |

## Expected reporter behaviour

- `valid_pair` is a separate decision (3-condition mini-decision under
  rustc inlining): the failure-fast-path test, the terminal-state
  cluster, and the increment-by-one check.
- The top-level `can_advance_to` decision is a long AND chain. Witness
  groups it by `(function, source_file, source_line)` since the chain
  spans multiple lines (line-clustering required).
- Inner ORs at g3..g8 each surface as their own short-circuit branch
  pair. Rows 7-13 each prove one of g4..g8's ctx-side dependence by
  pairing with the matching happy-path row.
- g6 and g8 contain inner ANDs (`cert_loaded && have_keys` and
  `peer_finished && transcript_hash_ok`). Rows 9 vs 10 prove
  independent effect of each side of g6's inner AND. Rows 12 vs 13 do
  the same for g8.

## Independent-effect proof sketch (intent)

| Cond | Pair | Type |
|---|---|---|
| g1 (valid_pair) | 0 vs 14 | masking |
| g2 (state != Failed) | 0 vs 18 | masking |
| g3 (Established escape) | 6 vs 19 | masking |
| g4 (have_keys) | 2 vs 7 | unique-cause |
| g5 (cert_loaded) | 3 vs 8 | unique-cause |
| g6.cert_loaded | 4 vs 10 | unique-cause |
| g6.have_keys | 4 vs 9 | unique-cause |
| g7 (transcript_hash_ok / FinishedSent) | 5 vs 11 | unique-cause |
| g8.peer_finished | 6 vs 12 | unique-cause |
| g8.transcript_hash_ok | 6 vs 13 | unique-cause |

## Verdict

**27 rows** designed to give the reporter a clear masking-MC/DC pair
for every inner condition in the 8-conjunct chain. Disagreements
between reporter and intent are bugs only when a row's outcome is
mis-classified or a clearly-source-visible decision is missing from
the reconstructed list.

## Machine-readable section

```json
{
  "verdict": "state_machine",
  "kind": "real-application-security-protocol",
  "row_count": 27,
  "intent_decisions": [
    "valid_pair", "can_advance_to-top-level"
  ],
  "intent_conjuncts": ["g1", "g2", "g3", "g4", "g5", "g6", "g7", "g8"],
  "expected_outcomes": {
    "true": [0, 1, 2, 3, 4, 5, 6, 20, 21, 22, 23],
    "false": [7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 24, 25, 26]
  },
  "intent_pairs": {
    "g1": [0, 14],
    "g2": [0, 18],
    "g3": [6, 19],
    "g4": [2, 7],
    "g5": [3, 8],
    "g6.cert_loaded": [4, 10],
    "g6.have_keys": [4, 9],
    "g7": [5, 11],
    "g8.peer_finished": [6, 12],
    "g8.transcript_hash_ok": [6, 13]
  }
}
```
