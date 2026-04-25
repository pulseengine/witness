# Witness Verdict Suite

The verdict suite is witness's canonical MC/DC evidence. Each verdict is a
small, self-contained Rust crate that compiles to `wasm32-wasip2`, exposes
a single decision under MC/DC, and ships:

| File | Purpose |
|---|---|
| `Cargo.toml` | Standalone crate (opts out of the witness workspace). |
| `src/lib.rs` | The predicate plus `run_row_<n>` exports — one no-arg function per test row. |
| `TRUTH-TABLE.md` | The **expected** MC/DC analysis, hand-derived. Acts as the verification oracle for the reporter. |
| `V-MODEL.md` | One-page traceability: requirement → design decision → conditions → rows → signed evidence. |
| `build.sh` | Builds the standalone crate to `wasm32-wasip2` (debug symbols on, no LTO, panic=abort). |

## The seven verdicts

| Verdict | Predicate | Conds | Shape | Rows for full MC/DC |
|---|---|---|---|---|
| `leap_year` | `(y%4==0 && y%100!=0) \|\| y%400==0` | 3 | mixed AND/OR | 4 |
| `range_overlap` | `a.start <= b.end && b.start <= a.end` | 2 | minimal AND | 3 |
| `triangle` | `a+b<=c \|\| a+c<=b \|\| b+c<=a` (Myers paper) | 3 | OR chain | 4 |
| `state_guard` | `a && b && c && d` | 4 | deep AND chain | 5 |
| `mixed_or_and` | `(a\|\|b) && (c\|\|d)` | 4 | nested operators | 5 |
| `safety_envelope` | `temp<t_max && press>p_min && rpm<r_max && !fault && mode==active` | 5 | scaling-stress | 6 |
| `parser_dispatch` | URL authority validator (RFC 3986-shaped) | 4 | real-world anchor | 5 |

The first six are synthetic-but-canonical (so a reviewer can verify the
truth table by eye). `parser_dispatch` is the real-world anchor —
deliberately *not* synthetic, so the suite isn't open to "yes but it
only works on toys" criticism.

## How a verdict folder is read by a human

1. Open `V-MODEL.md` — see which requirement the verdict satisfies, which
   design decision constrains it, and where the signed evidence lives.
2. Open `TRUTH-TABLE.md` — see, by hand, every test row's condition
   vector and outcome, and which two rows prove independent effect for
   each condition.
3. Open `src/lib.rs` — see the predicate and the test rows materialised
   as `run_row_<n>` exports.
4. (At release time) open the verdict's signed `predicate.dsse.json` —
   verify the on-disk evidence matches the hand-derived truth table.

When witness's MC/DC reporter agrees with the hand-derived
`TRUTH-TABLE.md` for every verdict, the report is *correct by
construction* with respect to the suite. Disagreements between the
reporter output and `TRUTH-TABLE.md` are bugs.

## How a verdict folder is read by an AI agent

The same files. The agent reads `TRUTH-TABLE.md`'s machine-readable
section (a structured JSON block), compares to the reporter output, and
flags discrepancies. Agents authoring tests to close MC/DC gaps consume
the gap-closure recommendation directly from the report and emit new
`run_row_<n>` exports that achieve the missing pair. (v0.9 closes this
loop autonomously.)

## Trace identity

Each `run_row_<n>` export corresponds to *exactly one row marker* in the
trace buffer. The wasmtime runner (in `crates/witness/src/run.rs`)
inserts a row marker before invoking each export; everything written to
the trace between row markers is attributed to that row. The test
oracle (`TRUTH-TABLE.md`) names the conditions evaluated per row; the
reporter parses the trace and verifies the `(decision, condition,
value)` records match.

## Build

```sh
./build-all.sh   # builds every verdict to wasm32-wasip2
```

Each verdict has its own `build.sh` for individual-verdict iteration.
The build outputs are not committed; they're regenerated on every CI
run and at release time by `.github/actions/compliance`.
