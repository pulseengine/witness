# witness — concepts

A single page that names every term you'll meet in a witness report
and walks one fixture row by row. Read this before the quickstart
if the truth-table block on first run looks like jargon, or after
the quickstart if you understand `./run.sh` but not its output.

The register is "Go/Python developer who has never read DO-178C."
No prior MC/DC theory required; aerospace context arrives in
section 7 only.

## 1. The 60-second pitch

`witness` is a coverage tool. You give it a WebAssembly module; it
runs the module, watches every Boolean branch the runtime takes, and
emits a per-decision **truth table** plus a verdict for each
condition: was its independent effect on the decision *proved* by
the rows you ran?

What's different from `gcov` / Clang `-fprofile-instr-generate`:
witness measures **post-compiler** Wasm. The compiler has already
short-circuited your `&&`s, fused `% 400 == 0` into a `br_if` chain
with `% 4 == 0`, and inlined helpers. Witness reports coverage of
*what runs*, not what you typed. DO-178C accepted the same shape of
argument for post-preprocessor C in 1992 (section 7).

A run produces three artefacts: a JSON report, an LCOV file for
codecov, and a sigstore-compatible DSSE envelope. The envelope is
what you hand to a regulator or a release pipeline.

## 2. Vocabulary

Every term is defined here, with the version it landed and a
one-line example. Cross-reference back here whenever a report
field looks like jargon.

| Term | Plain-English definition | Example | Landed |
|------|-------------------------|---------|--------|
| **decision** | A complete Boolean expression the program evaluates to one outcome — typically the controlling expression of an `if`, `while`, or `match` arm guard. | `(year % 4 == 0 && year % 100 != 0) \|\| year % 400 == 0` is one decision with three conditions. | v0.6.0 |
| **condition** | One Boolean operand inside a decision. Counted per surviving operand *after rustc optimisation* — that may be fewer than the source has. | In the leap-year predicate the report shows two conditions; rustc fuses the `% 400 == 0` check into the same `br_if` chain. | v0.6.0 |
| **branch** | A single Wasm `br_if` (or `if/else` arm, or `br_table` arm) the instrumenter inserted a counter at. One branch entry per surviving short-circuit jump. Numbered globally in the manifest as `branch 0`, `branch 1`, … | A two-condition `&&` chain compiles to two `br_if`s → two branches. | v0.1.0 |
| **row / truth table** | One execution of the decision recorded as a `(condition values, outcome)` tuple. The truth table is the union of rows seen across the run. Row ids start at 0. | `row 0: {c0=F} -> F` — one execution that short-circuited at the first condition. | v0.6.0 (schema), v0.6.1 (real captures) |
| **outcome** | The Boolean value the whole decision evaluated to on that row. `T`, `F`, or `?` (decision started but the function returned mid-chain). | `row 1: {c0=T, c1=T} -> T`. | v0.6.0 |
| **interpretation** | How a condition's independent effect was proved. One of `unique-cause`, `masking`, `unique-cause-plus-masking`, or `br-table-arm`. See dedicated rows below. | `c0 (branch 0): proved via rows 0+1 (masking)`. | v0.6.0 |
| **unique-cause MC/DC** | The strict variant: both rows of the proving pair fully evaluated *every* condition (no short-circuits). Easiest case to read; rare in real code because Rust short-circuits. | Two rows where neither short-circuited — both reached `c0` and `c1`. | v0.6.0 |
| **masking MC/DC** | The DO-178C-accepted variant: the proving pair may have short-circuited *other* conditions, as long as the target condition's independent effect on the outcome is logically isolable. This is what witness reports for almost every Rust decision. | `c0` proved via row 0 (`{c0=F} -> F`, short-circuited) + row 1 (`{c0=T, c1=T} -> T`, fully evaluated). | v0.6.0 |
| **unique-cause-plus-masking** | Mixed pair: one row fully evaluated, the other short-circuited. Sits between the two above. | A run where one row hits all conditions and its pair short-circuits c1. | v0.6.0 |
| **br-table-arm interpretation** | Special case: the decision is a Wasm `br_table` (jump table) where each "condition" is an arm. Independent-effect doesn't apply; witness reports per-arm hit/miss instead. | `match` over an integer compiles to `br_table` → arms tracked, not Boolean MC/DC. | v0.9.7 |
| **short-circuit** | Rust's `&&` and `\|\|` stop evaluating after the outcome is determined. The remaining conditions never run. Witness records this as a `*` (don't-care) in the row's evaluated map — the row only carries values for conditions that actually executed. | `false && expensive()` — `expensive()` never runs; only `c0=F` is recorded. | v0.6.0 |
| **don't-care (`*`)** | A condition that did not execute on this row, displayed as `*` or simply omitted from the row's `evaluated` map. NOT "we don't know" — "the runtime never reached it." | `row 0: {c0=F} -> F` — c1 short-circuited, no `*` printed because the table only lists evaluated cells. | v0.6.0 |
| **proved** | Status: this condition has a witness pair under masking MC/DC. | `c0 (branch 0): proved via rows 0+1 (masking)`. | v0.6.0 |
| **gap** | Status: condition was evaluated in at least one row, but no proving pair exists yet. The reporter prints a closure recommendation: a row vector that would close the gap. | `c1 (branch 1): GAP — try a row {c0=T, c1=T} (outcome must differ from row 4)`. | v0.6.0 |
| **dead** | Status: condition was never evaluated in any row. Either the runtime never reached it, or it's permanently short-circuited by upstream conditions in the rows you ran. | `c2 (branch 11): DEAD — never evaluated in any row`. | v0.6.0 |
| **chain_kind** | Wasm-level classification of a decision's `br_if` chain. `And` = every br_if branches on FALSE (the standard `&&` lowering); `Or` = every br_if branches on TRUE (the `\|\|` lowering); `Mixed` = both patterns; `Unknown` = couldn't classify. The runner uses this to derive per-row outcomes from condition values. | A four-condition `&&` chain → `chain_kind: And`. | v0.8.0 |
| **BrTableTarget / BrTableDefault** | Branch kinds for `br_table` (Wasm jump tables, used by `match` on integers). `BrTableTarget` covers one arm of the table; `BrTableDefault` covers the fall-through. Per-arm counters live separately from `BrIf` counters. | `match x { 0 => …, 1 => …, _ => … }` → 2 `BrTableTarget` + 1 `BrTableDefault`. | v0.9.7 |
| **trace_health.ambiguous_rows** | Header field on every report. `true` means the trace parser engaged — i.e. real rows were captured from the trace memory and reconciled into a truth table. **Not** "the rows are ambiguous." Misnamed; v0.10.0 will rename to `trace_parser_active`. | `"trace_health": {"ambiguous_rows": true, "rows": 5, ...}` is the success state. | v0.6.0 (field), v0.10.0 (rename pending) |
| **trace_health.overflow** | True if the per-row counter saturated (a condition fired more than 255 times between row markers). For the verdict suite this means a degenerate test; for real loops, bump `WITNESS_TRACE_PAGES`. | `"overflow": false` on every clean run. | v0.6.0 |
| **WITNESS_TRACE_PAGES** | Env var honoured at `witness instrument` time. Each Wasm page is 64 KiB; default 16 = 1 MiB of trace memory. Bump for fuzz harnesses. | `WITNESS_TRACE_PAGES=64 witness instrument …` | v0.9.8 |
| **DSSE envelope** | Dead Simple Signing Envelope — sigstore-format JSON wrapping a signed payload. Witness emits one envelope per run; payload is the in-toto Statement. | `envelope.json`. Verifies under `cosign verify-blob` from v0.10.0 onward. | v0.6.4 |
| **in-toto Statement** | The unsigned JSON inside the DSSE envelope. Standard supply-chain provenance shape: a `subject` (the wasm file's sha256), a `predicateType` (the schema URL), a `predicate` body. | `predicate type: https://pulseengine.eu/witness-coverage/v1`. | v0.6.3 (unsigned), v0.6.4 (signed) |
| **predicateType `witness-coverage/v1`** | The branch-coverage Statement: total branches, covered branches, per-function tallies, uncovered list. Suitable for codecov-style consumers. Truth table is **not** signed — only the branch summary is. | The default `witness predicate` output through v0.9.x. | v0.6.4 |
| **predicateType `witness-mcdc/v1`** | The MC/DC Statement: carries the full per-decision truth table, condition pairs with interpretation, and a sha256 of the canonical JSON. Signs what a regulator actually needs. v0.10.0 adds the kind via `witness predicate --kind mcdc`. | Pending v0.10.0; the schema URL is referenced today, the predicate body is not yet emitted. | v0.10.0 (planned) |
| **ambiguous_rows** | See trace_health.ambiguous_rows above. Same field, two callsites. Rename to `trace_parser_active` is in v0.10.0 scope. | (see above) | v0.6.0 |
| **interpretation_polarity** | New v0.10.0 report-header field. Documents whether condition values are "wasm-level early-exit-fired" (today's behaviour) or "source-level Boolean value" (the post-v0.10 default if normalisation lands). Lets a downstream consumer detect the inversion. | `"interpretation_polarity": "wasm-early-exit"`. | v0.10.0 (planned) |

## 3. The leap-year fixture, walked row by row

`witness new my-fixture` scaffolds a Rust crate exporting a single
`is_leap` function. The predicate is the textbook leap-year rule:

```rust
#[inline(never)]
fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn is_leap(year: i32) -> i32 {
    is_leap_year(year as u32) as i32
}
```

Source has three conditions: `year % 4 == 0`, `year % 100 != 0`,
and `year % 400 == 0`. Rustc fuses the third into the same `br_if`
chain as the first two during optimisation, so the compiled module
has two surviving conditions. Witness reports two — that's expected
and called out at the top of `lib.rs`.

The scaffolded `run.sh` invokes five years:

| Year | Source-level evaluation | What rustc-compiled wasm sees |
|------|-------------------------|--------------------------------|
| 2001 | `2001 % 4 == 0` is **F** → short-circuits at the first `&&` arm; whole decision is F | one branch fires: c0 (the early-exit) |
| 2004 | `2004 % 4 == 0` is T; `2004 % 100 != 0` is T; outcome T (the `\|\|` doesn't need to look at `% 400`) | two branches fire: c0 didn't early-exit; c1 didn't early-exit; outcome T |
| 2100 | `2100 % 4 == 0` is T; `2100 % 100 != 0` is **F** → short-circuits the `&&`; checks `% 400 == 0` → F → outcome F | two branches recorded; c1 path short-circuited; rustc fused the `% 400` check into the chain |
| 2000 | T, F, T → outcome T | same chain, fused `% 400` carries the row to T |
| 1900 | T, F, F → outcome F | confirms the c1=F path with the same outcome as 2100 but distinct row id |

Runtime output (text-format `report --format mcdc`):

```
module: instrumented.wasm
decisions: 1/1 full MC/DC; conditions: 2 proved, 0 gap, 0 dead

decision #0 lib.rs:30: FullMcdc
  truth table:
    row 0: {c0=F}        -> F     (year=2001)
    row 1: {c0=T, c1=T}  -> T     (year=2004)
    row 2: {c0=T, c1=F}  -> F     (year=2100)
    row 3: {c0=T, c1=F}  -> T     (year=2000, % 400 fused with c0&&c1)
    row 4: {c0=T, c1=F}  -> F     (year=1900)
  conditions:
    c0 (branch 0): proved via rows 0+1 (masking)
    c1 (branch 1): proved via rows 1+2 (unique-cause)
```

Sentence by sentence, what every line means:

**`decisions: 1/1 full MC/DC`** — there is one reconstructed
decision in this module; it has full masking MC/DC; the report
ratios are *decisions reaching the criterion / decisions total*.

**`conditions: 2 proved, 0 gap, 0 dead`** — both surviving
conditions have a proving pair; nothing missing; nothing unreached.

**`decision #0 lib.rs:30: FullMcdc`** — `lib.rs:30` is the source
line DWARF attributes the chain to (where the `(year % 4 == 0`
opens). `FullMcdc` is the decision's status — alternatives are
`Partial`, `NoWitness` (decision reached but no proving pair could
form), `Unreached` (zero rows). With v0.9.11's typed-args form this
line attribution is stable across edits; pre-v0.9.11 fixtures using
`core::hint::black_box` attributed to `hint.rs:491` instead.

**`row 0: {c0=F} -> F (year=2001)`** — year 2001 ran. The first
condition `% 4 == 0` evaluated to false. Rust's `&&` short-circuits
on F, so the chain exited without evaluating the second condition.
`c1` is absent from the evaluated map (a don't-care). Decision
outcome: F.

But pause on this row's `c0=F`. Read polarity carefully: at the
**source level** the condition `year % 4 == 0` was indeed false on
year=2001. At the **wasm level** the `br_if` for that condition
fires on FALSE (it's the `&&`-chain early-exit). So `c0=F` here
happens to align with both interpretations because rustc compiles
`&&` as "branch when condition is FALSE; if you didn't branch, keep
going." Section 4 walks through where the two readings *diverge*.

**`row 1: {c0=T, c1=T} -> T (year=2004)`** — both surviving
conditions evaluated. c0=T means `% 4 == 0` was true. c1=T means
`% 100 != 0` was true. Outcome T. No short-circuit on this row.

**`row 2: {c0=T, c1=F} -> F (year=2100)`** — c0=T, c1=F (`% 100`
test failed: 2100 % 100 == 0). Source-level the `&&` short-circuits
and the `||` looks at `% 400 == 0`; rustc fused that third check
into the same chain so it isn't a separate branch in the manifest.
Outcome F (2100 isn't a leap year).

**`row 3: {c0=T, c1=F} -> T (year=2000, % 400 fused with c0&&c1)`**
— same evaluated map as row 2, but outcome T. This is the load-
bearing row: the *outcome* differs from row 2 even though the two
recorded conditions agree. The reason is the fused third condition:
the wasm chain checks `% 400 == 0` after `% 100 != 0` fails. For
2000, `% 400 == 0` is true → outcome T. For 2100, false → outcome
F. The third condition isn't a separate column in the truth table,
but its effect is visible as an outcome flip across rows 2 and 3.

**`row 4: {c0=T, c1=F} -> F (year=1900)`** — confirms row 2's
shape with a different year. 1900 % 4 == 0 (T), 1900 % 100 == 0 so
`!= 0` is F, 1900 % 400 != 0 so the fused third arm is F → outcome
F. Identical evaluated map to rows 2 and 3; same outcome as row 2.

**`c0 (branch 0): proved via rows 0+1 (masking)`** — independent
effect of c0 (the `% 4 == 0` check) is proved by row 0 (`{c0=F}
-> F`) paired with row 1 (`{c0=T, c1=T} -> T`). c0 changes; outcome
changes. Other conditions either don't-care (row 0 never reached
c1) or held constant in c1's pair. Interpretation: `masking` —
because row 0 short-circuited, not every condition was evaluated in
both rows of the pair. The DO-178C masking variant accepts this.

**`c1 (branch 1): proved via rows 1+2 (unique-cause)`** —
independent effect of c1 (the `% 100 != 0` check) is proved by
row 1 (`{c0=T, c1=T} -> T`) paired with row 2 (`{c0=T, c1=F} ->
F`). c0 held constant at T in both; c1 flipped; outcome flipped.
Interpretation: `unique-cause` — both rows fully evaluated both
conditions, so the strict variant applies.

That is one truth table, end to end. Row 3 doesn't show up in
either pair: it isn't needed for the proof, but it's evidence that
the fused third arm produced a distinct outcome from row 2.

## 4. The polarity inversion, named explicitly

This is the single biggest gotcha in reading a witness report and
the v0.10.0 proposal flags it as the load-bearing reviewer-trust
finding. Read this section before you publish a truth table to a
DER (Designated Engineering Representative).

**The mechanic.** When rustc lowers `if a { ... }` to Wasm, it
emits roughly:

```
local.get $a   ;; push a's value
i32.eqz        ;; invert — push !a
br_if N        ;; branch if top-of-stack is true (i.e. if !a)
;; ... `then` body here, only reached when a was TRUE
```

Witness instruments the `br_if` and counts when it fires. The
br_if fires when `!a`, i.e. when `a` is false. Witness records
that as the **condition value** for c-N.

**The consequence.** In the truth table column `c0=T`, the `T`
records "the br_if fired" — which means the early-exit was taken,
which (for an `&&`-chain `br_if` that branches on FALSE) means the
*source condition was FALSE*. The polarities are inverted relative
to what a fresh reader assumes.

**Worked example.** Consider an `&&`-chain decision `a && b`:

- **Source level**: `a=T, b=F → outcome F`.
- **Wasm level** (rustc lowering): a's br_if branches when !a; b's
  br_if branches when !b. For `a=T, b=F`: a's br_if doesn't fire
  (a was T, !a is F); b's br_if fires (b was F, !b is T).
- **Witness records**: `c0=F` (a's br_if didn't fire), `c1=T` (b's
  br_if did fire), outcome F.
- **Reader expectation**: `c0=T, c1=F → outcome F` (matching the
  source values).

The two are exact inverses, condition by condition.

**Why the leap-year fixture's truth table looks "right".** The
example in section 3 shows `row 1: {c0=T, c1=T} -> T (year=2004)`
and that *does* match source semantics for 2004 — both source
conditions are true, outcome is true. That's because the fixture's
canonical rows happen to be ones where the wasm-level br_if fires
align with source truths in a way that makes the inversion
invisible. For an arbitrary `&&`-chain run with a F-then-T row,
the inversion shows up.

**Why witness records the wasm value, not the source value.** Two
reasons. First, witness measures what runs — and what runs is the
`br_if`. Second, MC/DC's independent-effect criterion cares about
*changes* in condition values, not absolute polarity: if you flip
every column in every row, the proving pairs are still proving
pairs. The math is invariant under inversion. So the verdict
("c0 proved via rows 0+1 masking") is correct regardless of
polarity.

**Why a reviewer should still read this section.** The verdict
doesn't lie, but the truth table cells do — to anyone assuming
source polarity. v0.10.0 ships the explainer (this page) plus a
new report-header field, `interpretation_polarity:
"wasm-early-exit"`, so a downstream consumer can detect the
inversion and either show source-equivalent values or present the
table with a polarity note. (v0.10.0 proposal item 4. Behind a
`--wasm-polarity` flag for backward compat.)

Until that field lands, treat the truth-table column header as
"the br_if fired" rather than "the source condition was true," and
the proofs in the conditions block as the load-bearing artefact.

## 5. The MC/DC criterion, in plain language

MC/DC = Modified Condition / Decision Coverage. The criterion has
one sentence at its core:

> Every condition in a decision has been shown to *independently*
> affect the decision's outcome.

Operationally: for each condition c, find two rows in the truth
table such that

1. c has a different value in row A versus row B, and
2. all other evaluated conditions are unchanged (or, under masking,
   short-circuited and provably masked), and
3. the outcome differs between row A and row B.

Such a row pair is a **witness** for c's independent effect. A
condition with at least one witness pair is *proved*. A decision
is *FullMcdc* when every condition is proved.

Worked example, two conditions, four rows.

Decision `a && b`, hypothetical Boolean a, b:

```
row 0: {a=F}        -> F     (short-circuit on a=F)
row 1: {a=T, b=T}   -> T
row 2: {a=T, b=F}   -> F
row 3: {a=F}        -> F     (duplicate of row 0; ignored for proofs)
```

(Here we use **source-level** Boolean values for clarity; the wasm
report inverts them per section 4.)

**Pair for a:** row 0 vs row 1.
- a flips: F → T.
- Outcome flips: F → T.
- Other condition (b): not evaluated in row 0 (short-circuited);
  evaluated as T in row 1. Under unique-cause MC/DC this pair
  doesn't qualify (b isn't held constant — it's missing in one
  row). Under **masking MC/DC** the pair *does* qualify, because
  the short-circuit in row 0 logically masks b's effect. DO-178C
  accepts masking. Witness reports `interpretation: masking`.

**Pair for b:** row 1 vs row 2.
- b flips: T → F.
- Outcome flips: T → F.
- a is held at T in both rows. Strict — both rows fully evaluated
  both conditions. Witness reports `interpretation: unique-cause`.

That's the criterion proved on a four-row run with one condition
in masking interpretation and one in unique-cause. Replace `a, b`
with `year % 4 == 0, year % 100 != 0` and you have section 3's
leap-year analysis exactly.

What the criterion **does not** require:

- That every combination of condition values runs (that's `MC/DC^*`
  / multiple-condition coverage, a stricter criterion).
- That conditions evaluate in any particular order.
- That outcomes match a specification (MC/DC is structural, not
  functional — "did each condition flip the outcome?", not "did
  the function compute the right answer?"). You still need
  property tests / requirement-driven tests on top.

## 6. The signed evidence chain

Two subsections — what v0.9.x signs today, what v0.10.0 will sign.

### What v0.9.x signs today (predicateType `witness-coverage/v1`)

`witness predicate` builds an in-toto Statement with predicateType
`https://pulseengine.eu/witness-coverage/v1`. The body is a
**branch-coverage summary**:

- `total_branches` — count of `BranchEntry` in the manifest.
- `covered_branches` — count of branches with hits ≥ 1.
- `per_function` — branches and covered tallies per function.
- `uncovered` — list of branch ids never hit.

`witness attest` wraps this Statement in a DSSE envelope (sigstore-
compatible JSON) and signs it with your release key. `witness
verify` checks the envelope against the public key and reports the
predicateType to the user.

What's **not** signed in v0.9.x:

- The truth table (rows, evaluated maps, outcomes).
- The condition-level interpretation (`masking` / `unique-cause` /
  `unique-cause-plus-masking`).
- The proving-pair row ids.
- The `original_module` sha256 (the field exists but is `null`).

The MC/DC content lives in `report.json` next to the envelope,
unsigned. A regulator who wants the truth table signed has to
either re-sign `report.json` themselves or wait for v0.10.0.

### What v0.10.0 ships (predicateType `witness-mcdc/v1`)

v0.10.0 adds `witness predicate --kind mcdc`. The Statement's
predicateType becomes `https://pulseengine.eu/witness-mcdc/v1`,
and `predicate.mcdc` carries:

- The full per-decision truth table (every row, every evaluated
  map, every outcome).
- The condition pairs with `interpretation`.
- A canonical-JSON sha256 of the truth-table block, so consumers
  can detect tampering even if they don't re-parse the rows.
- The `interpretation_polarity` field from section 4.

`witness verify --check-content` validates that the inline truth
table matches the signed sha256. `original_module.sha256` is
populated on every predicate (today's `null` is fixed).

`witness-coverage/v1` still ships for branch-only consumers
(codecov, JaCoCo-style). Both predicate types share the same DSSE
+ in-toto envelope shape; only the body differs.

JSON Schemas for both predicate types ship at `docs/schemas/` and
are mirrored at the URLs the reports cite (v0.10.0 proposal item 5).

## 7. Where this comes from — DO-178C and post-preprocessor C

DO-178C is the airworthiness software standard. Its 1992 predecessor
DO-178B introduced MC/DC for Level A software (catastrophic
failure consequence). The 2002 DO-178C revision and its companion
DO-330 / DO-332 stayed compatible. Here's the one piece of context
that matters for understanding witness's positioning.

**The post-preprocessor C precedent.** When DO-178B accepted MC/DC
for C code, the natural question was: which form of the source do
you measure? The C preprocessor expands macros, includes headers,
runs `#if` directives — by the time the compiler sees code, it's
not what the developer wrote. DO-178B accepted measuring
**post-preprocessor C** because that's what the compiler actually
compiles. Coverage of the macro-expanded text is meaningful;
coverage of the un-expanded text is a fiction. The community
litigated this and converged.

**Witness's analogous claim.** Modern Rust compiles to Wasm via
LLVM. By the time the runtime executes anything, rustc has
short-circuited `&&`s, fused `% 400 == 0` into the same `br_if`
chain, inlined helpers, and chosen between `i32.eqz; br_if` (and-
chain) and `local.get; br_if` (or-chain). Coverage of the
**post-rustc Wasm** is what runs; coverage of the un-optimised
source is a fiction. This is the same argument DO-178B accepted
for post-preprocessor C, transposed to a different toolchain.

The leap-year fixture's "source has 3 conditions; report has 2"
divergence is the DO-178C precedent in miniature: the third
condition got fused, and the reportable artefact is the fused
chain, not the source spelling. A reviewer trained on DO-178C C
analysis recognises the shape.

That's the whole positioning argument. Witness is not "MC/DC for
Rust"; it's "MC/DC at the Wasm level, qualified the way
DO-178C qualified post-preprocessor C analysis in 1992." Section
2's vocabulary, section 3's worked walkthrough, section 4's
polarity note, and section 5's criterion are the operational
content; this section is the standards lineage that makes the
operational content admissible.

---

That's the whole concepts page. If you came here from the
quickstart's truth-table block, sections 2 (vocabulary) and 3
(walkthrough) are enough to read your first report. If you came
here as a reviewer, sections 4 (polarity), 6 (signed evidence),
and 7 (DO-178C lineage) are the load-bearing additions on top.
