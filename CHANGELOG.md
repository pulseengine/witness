# Changelog

All notable changes to witness are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [SemVer 2.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.4] — 2026-04-26

### What v0.7.4 closes (the architecture)

v0.7.3 made per-iteration condition vectors visible but reused the
top-level row's function-return as the outcome for every decision.
For decisions in *separately-compiled* called functions, that's
wrong — those functions have their own return values. v0.7.4
adds per-function-call outcome capture: each instrumented function
emits a `kind=2` trace record at every return point carrying
`(function_index, return_value)`.

### Added — `__witness_trace_outcome` helper + return-point instrumentation

A new internal helper function `__witness_trace_outcome(function_idx,
value)` writes a 4-byte record with `kind=2` to the trace memory.

For each local function f satisfying both:
1. f contains at least one `BrIf` decision in its body.
2. f's signature has exactly one i32 result.

The instrumenter walks f's body and:
- Replaces each `Return` instruction with `local.tee tmp; const
  f_idx; local.get tmp; call helper; local.get tmp; return` — captures
  the return value, records it, restores it for the actual return.
- Appends `local.tee tmp; const f_idx; local.get tmp; call helper`
  to the end of the entry block — for the implicit fall-through
  return. The tee leaves the value on the stack as Wasm's implicit
  return semantics expect.

### Updated — runner parser handles `kind=2` records

`parse_trace_records` now treats `kind=2` records distinctly: when
one arrives with function_index F, every in-flight iteration of
every decision belonging to F is finalised with the outcome value
from the record. Decisions whose function never wrote a kind=2
record (because the function had a non-i32 return type, or
trapped, or never reached its return) fall back to the row-level
function-return outcome.

The runner builds two lookups: `branch_to_decision` (for kind=0
records) and `function_to_decisions` (for kind=2 records).

### Result on httparse — same numbers, different reason

| Version | full MC/DC | proved | gap | dead | trace bytes |
|---|---:|---:|---:|---:|---:|
| v0.7.3 (per-row outcomes) | 1/70 | 12 | 52 | 122 | 6328 |
| **v0.7.4 (per-call outcomes)** | **1/70** | **12** | 52 | 122 | **6380** |

The score is unchanged because **rustc inlines aggressively**: most
"interesting" memchr / iter / SWAR decisions are inlined into
`parse_request`, so the wasm-level `function_index` for those
inlined br_ifs is parse_request's index even though the manifest
records `source_file: "memchr.rs"`. Per-wasm-function outcome
capture for the inlined case is the same as the row-level outcome.

The 52 extra trace bytes (= 13 outcome records ÷ ~15 rows ≈ 1
outcome per row) is exactly parse_request's outcome being captured
on every call.

### Why ship anyway

v0.7.4 is structurally correct — for separately-compiled functions
(common in less-optimised builds, or in CI runs with `opt-level = 0`,
or when `#[inline(never)]` is applied to the predicates in the call
graph), per-call outcomes are now captured. The architecture lays
the foundation for v0.8's per-DWARF-inlined-context outcome
tracking, which is the proper fix for aggressively-inlined code
like httparse. The work doesn't compose into something later;
it's the layer below.

### Notes for v0.7.5 / v0.8

- **Per-DWARF-inlined-context outcome tracking** is the next track.
  `function_index` in the manifest is the wasm-level function;
  v0.8 needs to also track the DWARF inlined-subroutine chain so
  decisions inlined from memchr into parse_request get attributed
  back to memchr's logical "outcome" (which doesn't exist as a
  real return value because there is no real call — but can be
  derived from the chain's terminating br_if direction).
- **Multi-result function support**. Currently only single-i32-
  result functions get outcome instrumentation. Rust's
  `Option<usize>` and similar lower to multi-result on
  wasm32-wasip2 sometimes; v0.7.5 could extend the filter.

### Implements / Verifies

- Implements: REQ-034 (architecture for per-call outcomes — the
  v0.6 trace-buffer plan now has all four record kinds wired:
  conditions, row-markers, and outcomes).
- Verifies: leap_year unchanged at 1/1 + 2 proved; httparse
  unchanged at 1/70 + 12 proved (with 52 extra trace bytes
  documenting the captured outcomes).

## [0.7.3] — 2026-04-26

### What v0.7.3 closes (read side)

v0.7.2 shipped the trace-buffer write side. v0.7.3 ships the
runner-side parser that converts the 4-byte trace records into
per-iteration `DecisionRow` entries. The MC/DC reporter then
finds proving pairs across iterations that the per-row-globals
collapse hid.

### Added — `parse_trace_records` in the runner

Reads the trace memory bytes after each row, walks records in
order. For each condition record (kind=0):

1. Looks up `branch_id` in `branch_to_decision` (built from
   manifest `Decision::conditions`).
2. Appends `(condition_index, value)` to the decision's "current
   iteration" map.
3. When a duplicate condition_index appears for the same
   decision (= the loop iterated), finalises the current
   iteration and starts fresh.
4. Trailing in-progress iterations flushed at the end.

The runner now generates one `DecisionRow` per iteration, each
with a fresh `row_id`. Outcome is the function's return value
(per-decision outcome capture is a separate v0.7.x track).

When the trace memory is empty (e.g. a v0.6 fixture predating
v0.7.2 instrumentation), the runner falls back to the per-row-
globals path. So existing verdicts stay backward-compatible.

### Result on httparse

| Version | full MC/DC | proved | gap | dead |
|---|---:|---:|---:|---:|
| v0.7.0 (per-row globals) | 0/70 | 9 | 55 | 122 |
| **v0.7.3 (trace parser)** | **1/70** | **12** | 52 | 122 |

Modest but real. `mod.rs` gained a fully-proved decision; lib.rs and
macros.rs each gained 1-2 proved conditions. The remaining gap is
because outcomes are still uniform per row (function return value);
per-decision outcome capture is the next track.

### Implementation notes

- Iteration boundary detection is conservative — "duplicate
  condition_index" may incorrectly join two semantically distinct
  iterations if the second iteration short-circuits at the same
  condition that fired last. v0.7.x will switch to row-marker-based
  boundaries (the `__witness_trace_row_marker` helper exists already
  but isn't yet emitted between iterations).
- Records other than `kind=0` (row-marker, decision-outcome) are
  reserved and skipped by this pass. Becomes load-bearing in the
  next iteration of the iteration-boundary detection.

### Notes for v0.7.4+

- **Per-decision outcome capture** is the largest remaining
  unlock. Currently every decision's outcome is the row's
  function-return value, which is correct only for the top-level
  predicate decision. Sub-decisions (memchr's compound predicates,
  iter.rs's bounds checks) have actual outcomes the trace buffer
  doesn't yet record.
- **Row-marker-based iteration boundaries** would be more accurate
  than "duplicate condition_index". The instrumenter already exports
  `__witness_trace_row_marker(u32)`; v0.7.4 wires the runner to call
  it between rows, and the parser splits on those markers instead.
- **Trace buffer overflow handling** at v0.9-scale workloads —
  currently the writer sets `overflow_flag` and silently drops, the
  reporter refuses MC/DC verdicts on overflow. v0.9 might add a
  host-callback flush.

### Implements / Verifies

- Implements: REQ-034 (substance — trace-buffer primitive end-to-end).
- Verifies: leap_year unchanged (1/1 full MC/DC, 2 proved); httparse
  improved from 0/70 + 9 proved to 1/70 + 12 proved with no other
  changes.

## [0.7.2] — 2026-04-26

### What v0.7.2 closes (write side)

v0.7.0 hit the limitation Agent A's research warned about: per-row
globals can capture only the last value per condition per row, so
loop-bearing programs (httparse) end up with `0/N full MC/DC`
because every iteration's evaluated map collapses into the last
iteration's. v0.7.2 ships the **write side** of the linear-memory
trace buffer that lifts this limitation.

The runner-side parser that converts trace records into per-iteration
`DecisionRow` entries is the v0.7.3 follow-up.

### Added — trace memory, helper exports

Each instrumented module now exports a 16-page (1 MiB) trace memory
plus three helper functions:

- `__witness_trace`: 16-page exported memory. Header at offsets 0-15
  (cursor, capacity, overflow_flag, reserved); records starting at
  offset 16.
- `__witness_trace_reset()`: zeros cursor + overflow_flag, sets
  capacity. The runner calls this between row invocations.
- `__witness_trace_row_marker(row_id: i32)`: writes a row-marker
  record. Reserved for v0.7.3+ when iteration boundaries get
  emitted at row transitions.
- `__witness_trace_record(branch_id: i32, value: i32)`: internal
  helper called by per-br_if instrumentation. Writes a 4-byte
  record `(branch_id u16, value u8, kind=0 u8)` to trace memory at
  cursor, advances cursor.

### Added — per-br_if trace-record writes

`rewrite_brif` was extended to emit `i32.const branch_id;
local.get tmp; call __witness_trace_record` after the brval/brcnt
sequence. Stack-neutral (consumes 2, pushes 0); the v0.5 invariant
that the tee'd condition stays on the stack for the if-counter-inc
that follows is preserved.

### Added — runner reads the trace watermark

`witness run` now reads the trace memory header after each row
invocation and reports the bytes-used watermark in
`RunRecord.invoked` as `__witness_trace_bytes=N`. Sets
`trace_health.ambiguous_rows = true` when any trace activity is
seen — flag for the reporter that v0.7.3's per-iteration parser
should be applied to this run.

### Verified end-to-end

```
$ witness instrument verdicts/leap_year/verdict_leap_year.wasm -o lyt.wasm
$ witness run lyt.wasm --invoke run_row_0..3 -o lyt.run.json
trace_health: {'overflow': False, 'rows': 4, 'ambiguous_rows': True}
trace: __witness_trace_bytes=28      → 7 records / 4 rows / 2 br_ifs each ≈ 1.75 r/row

$ witness instrument verdicts/httparse/verdict_httparse.wasm -o hp.wasm
$ witness run hp.wasm --invoke run_row_0..14 -o hp.run.json
trace_health: {'overflow': False, 'rows': 15, 'ambiguous_rows': True}
trace: __witness_trace_bytes=6328     → 1582 records across 15 rows
```

httparse's **1582 records** across 15 rows is exactly the
per-iteration data per-row globals could not capture. v0.7.3 parses
this into per-iteration DecisionRow entries; the MC/DC reporter
then finds proving pairs that the per-row-globals collapse hid.

### Implementation notes

- Helper-function approach (not inline). Each per-br_if site is
  3 instructions (const + local.get + call) instead of ~15 inline
  instructions. Trade-off: function-call overhead per branch in
  exchange for smaller module size + simpler stack-typing review.
  v0.7.x can switch to inline if profiling shows the call dominates.
- Module file size growth ≈ 100 bytes for the trace infrastructure
  (memory + 3 helpers) + 12 bytes per `BrIf` site (3 instructions
  ≈ 4 bytes each). httparse's 481 br_ifs add ~5.7 KB of
  instrumentation; the bulk of `verdict_httparse.wasm`'s 677 KB
  is httparse itself.
- Multiple-memory support required at the runtime side. wasmtime
  42 supports it natively (the wasi-preview2 import set already
  uses multiple memories). Older runtimes that don't support
  `multi_memory` will reject witness-instrumented modules; the
  runtime check is upstream of any witness-specific failure.

### Notes for v0.7.3

- Runner-side trace parser: read the trace memory after each row,
  walk the records, group by decision (via manifest's branch→
  decision map), split into iterations (next condition_idx
  appearing equal-or-less-than the previous starts a new
  iteration), emit one DecisionRow per iteration. Outcome stays the
  function return value (per-decision outcome capture is a
  separate v0.7.x track).
- MC/DC reporter changes: handle the case where one row produces
  multiple iterations of the same decision. Each iteration is a
  candidate for pair-finding.

### Implements / Verifies

- Implements: REQ-034 (substance — first half: trace-buffer
  primitive on the write side).
- Verifies: leap_year produces 7 records, httparse produces 1582
  records — trace memory is being written by per-br_if
  instrumentation as designed.

## [0.7.1] — 2026-04-26

### What v0.7.1 closes

v0.7.0's httparse demo proved witness scales to a real Rust crate
(70 decisions, 481 br_ifs) but the per-decision report at that
size — 1519 lines — is unreadable. v0.7.1 adds module-rollup
report mode: 13 lines instead of 1519, per-file decisions/
conditions table sorted by decision count.

### Added — `witness report --format mcdc-rollup` and `mcdc-rollup-json`

```
$ witness report --input httparse.run.json --format mcdc-rollup
module: httparse.wasm
overall: 0/70 full MC/DC; conditions: 9 proved, 55 gap, 122 dead (186 total)

source file                               decisions  full mcdc     proved        gap       dead
---------------------------------------- ---------- ---------- ---------- ---------- ----------
lib.rs                                           18          0          1         18         25
iter.rs                                          16          0          1         16         28
macros.rs                                        10          0          3         13         15
count.rs                                          6          0          0          0         12
mod.rs                                            6          0          1          1         16
num.rs                                            3          0          0          0         10
validations.rs                                    3          0          2          3          4
const_ptr.rs                                      2          0          0          2          2
swar.rs                                           2          0          1          2          2
converts.rs                                       1          0          0          0          2
index.rs                                          1          0          0          0          2
panic.rs                                          1          0          0          0          2
result.rs                                         1          0          0          0          2
```

The user reads this as: "lib.rs has 18 decisions; 1 condition is
independently witnessed, 18 have gaps a new test row would close,
25 are dead (never reached by the suite)". One-line-per-file, the
most-decision-rich files at the top.

JSON variant emits the same structure for tooling consumption,
schema URL `https://pulseengine.eu/witness-mcdc/v1/rollup`.

### Implementation

- New `McdcRollup` and `FileRollup` structs in
  `witness-core::mcdc_report`.
- `McdcRollup::from_report(&McdcReport)` walks decisions, buckets by
  `source_file`, sums per-bucket condition tallies. Sorts by
  decision-count descending.
- New `mcdc_report::rollup_from_run_file(path)` convenience.
- CLI: two new variants in the `ReportFormat` enum.
- Long file paths (e.g. inlined-stdlib paths in httparse) get
  `…suffix` truncation in the text output to keep the table
  column-aligned at 40 chars.

### Notes for v0.7.2

- The per-row-globals limitation that caps httparse's full-MC/DC
  count at 0/70 is still in effect. v0.7.2 plans the trace-buffer
  primitive switch.

### Implements / Verifies

- Implements: REQ-029-substance — the report is now actually
  reviewable at scale, where v0.7.0's per-decision text was not.

## [0.7.0] — 2026-04-26

### What v0.7.0 closes

Two substantive items: (a) the v0.5–v0.6 release pipeline shipped a
13 KB `witness-core.wasm` build smoke-test as if it were a usable
component — it had no `extern "C"` exports and no WIT interface, so
linker dead-code-elimination threw away the entire library; (b) the
v0.6.x verdict suite covered seven canonical compound-decision
shapes but no real-application demo. v0.7.0 closes both.

### Added — `crates/witness-component`

A new workspace member that builds witness-core's MC/DC reporter as
a **real Component Model component** for `wasm32-wasip2`. WIT
interface at `crates/witness-component/wit/world.wit`:

```wit
package pulseengine:witness@0.7.0;

interface reporter {
    report-text: func(run-json: string) -> result<string, string>;
    report-json: func(run-json: string) -> result<string, string>;
    verify-envelope: func(envelope-json: string, public-key: list<u8>)
        -> result<string, string>;
}

world component { export reporter; }
```

`wasm-tools component wit` confirms the exports land
(`pulseengine:witness/reporter@0.7.0`). The emitted artefact is
**~400 KB** vs the v0.5–v0.6 13 KB stub — a 30× size jump that's
witness-core's actual code being kept by the linker because the
WIT interface gives it reachable entry points.

### Replaced — release asset name

- v0.5 / v0.6: `witness-core-vX.Y.Z-wasm32-wasip2.wasm` (13 KB stub).
- **v0.7.0+**: `witness-component-vX.Y.Z-wasm32-wasip2.wasm` (~400 KB
  real component).

Asset-name change makes the format change visible to downstream
consumers tracking release artefacts. The release notes call it out
explicitly.

### Added — `verdicts/httparse` real-application fixture

A new verdict crate that depends on the `httparse` crate (RFC 7230
HTTP/1.x parser) and exposes 15 `run_row_<n>` exports driving the
parser with representative request and response shapes. Witness
reconstructs **~70 decisions across 481 br_ifs** spanning
`httparse/lib.rs`, `httparse/iter.rs`, `httparse/macros.rs`, and
inlined stdlib code (`core/src/iter`, `core/str`, `swar.rs`, etc.).

This is "witness on a real Rust crate" — the verdict suite's 7
existing canonical shapes are still useful for verifying the
reporter's correctness, but httparse demonstrates witness on
something with a real test surface.

#### What httparse reveals — and the per-row-globals limitation

Running the suite on httparse reveals that **0 of 70 decisions
achieve full MC/DC** under our 15 test rows. The cause is a known
limitation of the v0.6.1 per-row-globals primitive:

- httparse's parsing loops hit each `br_if` *multiple times per
  test row* (once per iteration).
- Our `__witness_brval_<id>` global captures only the LAST value
  per row.
- So the recorded `evaluated[i]` for each condition is the value at
  loop termination, identical across rows that traverse similar
  loop paths.
- Pair-finding can't find independent-effect proofs because the
  toggling-condition rows produce identical `evaluated` maps.

**This is exactly the loop-case Agent A's research warned about**
and recommended the linear-memory trace buffer as the long-term
fix (`docs/research/v06-instrumentation-primitive.md` §2.5, §7.1).
v0.7.0 documents the limitation clearly; v0.7.x or v0.8 ships the
trace-buffer primitive that lifts it.

For now, the httparse report is still useful as a **reachability /
dead-condition picture**: which decisions in the parser are
exercised by which test rows, which conditions never fire across
the suite, which gaps a new test row would close.

### Updated — verdict-suite regression gate

CI's `verdict-suite` job adds httparse to the assertion list:
expected ≥ 30 decisions reconstructed (the count varies by rustc
version; 30 is a generous floor). leap_year + triangle + state_guard
+ safety_envelope + parser_dispatch each still asserted ≥ 1.

### Implements / Verifies

- Closes the witness-core wasm overclaim from v0.5/v0.6 by shipping
  a real Component Model component (REQ-031 substance, not just
  packaging).
- Adds httparse as the v0.9 destination workload anchor (per
  `docs/research/v07-scaling-roadmap.md` recommendation).
- Verifies: `wasm-tools validate` + `wasm-tools component wit` on
  the emitted `witness_component.wasm`; verdict suite with httparse
  produces 70 decisions / 481 branches / 1050 row records.

### Notes for v0.7.x

- **Module-rollup MC/DC report mode** for usability at httparse
  scale (1519-line per-decision report → per-file roll-up). v0.7.1.
- **Trace-buffer instrumentation primitive** to lift the per-row-
  globals limitation on loop-bearing code. v0.7.x or v0.8.
- **Per-decision outcome capture** (currently the function return
  value is applied uniformly to every decision; a sub-decision's
  actual outcome may differ from the top-level return). Likely
  v0.7.x alongside the trace buffer.
- **Component verification harness** — wasmtime-driven integration
  test that loads `witness_component.wasm`, calls `reporter:report-
  text`, asserts the output. v0.7.1.

## [0.6.9] — 2026-04-26

### What v0.6.9 closes

The v0.6.4–v0.6.8 work shipped a complete signed-evidence pipeline,
but the threat model — what a witness signature proves, what it
*doesn't*, why ephemeral keys — was scattered across CHANGELOG
narratives. v0.6.9 consolidates it into a `SECURITY.md` that
adopters can cite when scoping witness's role in their qualification
chain.

### Added — `SECURITY.md`

Five sections:

1. **What a signature proves** — predicate body integrity, signer
   key identity, instrumented-module digest binding.
2. **What a signature does NOT prove** — no long-term key
   continuity, no source-binding, no test-suite representativeness,
   no LLVM lowering soundness (the v0.2 paper's coverage-lifting
   open problem, deferred to v1.0's Check-It pattern).
3. **The ephemeral-key approach** — pros and cons, when adopters
   should layer their own signing chain on top (sigstore Fulcio,
   HSM/KMS).
4. **Key sizes and algorithms** — Ed25519, raw 64+32 byte format,
   PEM/DER deferred to v0.7.
5. **Reporting security issues** — `security@pulseengine.eu`,
   security-relevant code paths.

### Why this matters for v0.6.x adopters

Safety-critical adopters need to scope witness in their
qualification chain. The signature's claims are precise but
narrow: "this evidence was produced by the release pipeline that
wrote this verifying-key.pub". Anything broader requires
composition with surrounding tools (build provenance, sigstore,
HSM-backed signing). SECURITY.md states that boundary explicitly
so adopters don't over-claim.

### Closing the v0.6.x ratchet

This is likely the **last v0.6.x sub-version**. The series
ratcheted from v0.6.0's "consumer side only" through v0.6.8's
"release self-verifies its own bundle". The v0.7 work — scaling to
real applications, BrTable per-row MC/DC, visualisation, sigstore
integration — is substantively different in shape from v0.6.x
ratchets and warrants its own planning cycle (the existing v07
research brief at `docs/research/v07-scaling-roadmap.md` is the
starting point).

Future v0.6.x patch releases remain available for fixes.

### Implements / Verifies

- Documents: the v0.6.4 sign + v0.6.7 verify + v0.6.8 self-verify
  loop's threat model and scope.

## [0.6.8] — 2026-04-26

### What v0.6.8 closes

v0.6.4 added DSSE signing; v0.6.7 documented how to verify a release.
v0.6.8 closes the loop in the other direction: every release pipeline
now self-verifies the compliance archive it just built. If the
signing path regresses (key not bundled, envelope corrupted, verify
command broken), the release fails before publication rather than
shipping broken evidence to users.

### Added — self-verify step in `.github/actions/compliance/action.yml`

After the V-model trace matrix is written and before the README, the
action runs `witness verify` against one signed envelope from the
just-built bundle, against the bundled `verifying-key.pub`. The
specific envelope tried is `leap_year/signed.dsse.json` — the
canonical demo, expected to produce a verdict on every release.
Falls back to triangle / state_guard / safety_envelope /
parser_dispatch if leap_year is missing for some reason.

The step is conditionally a no-op if signing was skipped (no
`verifying-key.pub` in the bundle) — keeps the action backward-
compatible with the v0.5 dev-mode invocation that doesn't run the
verdict suite.

### Why this matters

It's defence in depth in the strictest sense: the failure mode being
prevented is "release ships, downstream consumer downloads, runs
witness verify, gets failure, files a bug — but we already shipped".
Self-verify makes that scenario impossible because the release
pipeline is the first downstream consumer.

It also doubles as a smoke test for `witness verify` itself: the
verify command runs in the release pipeline against real production
evidence on every release. If a future change to the attest / verify
code breaks compatibility with already-produced bundles, the next
release fails closed.

### Implements / Verifies

- Verifies: the v0.6.4 sign + v0.6.7 verify loop self-attests on
  every release, no manual intervention required.

## [0.6.7] — 2026-04-26

### What v0.6.7 closes

The v0.6.x series shipped a complete signed-evidence pipeline across
six versions, but the README was still pinned to v0.1.0 — visitors
landed on the repo and saw none of it. v0.6.7 fixes that: the README
features a "Show me the proof — verify a release in 60 seconds"
recipe walking through download → extract → verify → see truth
tables.

### Updated — README.md

- **Status** section replaced with a v0.6.x ratchet table summarising
  what each sub-release added.
- **Show me the proof** section before the usage block. Walks through
  `gh release download`, extract, `cat SUMMARY.txt`, `witness verify`.
  Includes a directory tree of the bundle so users know what each
  file is for.
- **Usage** section updated to demonstrate the v0.6 commands:
  `witness report --format mcdc`, `keygen`, `attest`, `verify`,
  `lcov` — replacing the v0.1.0-era examples.

### Verified end-to-end against the production v0.6.4 release

Downloaded v0.6.4's compliance archive from GitHub, extracted, and
ran `witness verify` against `leap_year/signed.dsse.json` and
`parser_dispatch/signed.dsse.json` against the bundled
`verifying-key.pub`. Both verify with `OK`. The published release
ships exactly what the README documents — no broken promises.

### Notes for v0.6.8 / v0.7

- The `Related work` section in the README still doesn't mention
  RapiCover (the closest commercial competitor identified by the
  v0.9 research agent). Worth adding for v0.6.8.
- The `Where it fits` table predates the v0.6.x ecosystem reality —
  loom and meld don't yet emit the offset-translation maps witness
  needs for post-loom / post-meld coverage; that's v0.7+ work
  pending the upstream issues from v0.5.

### Implements / Verifies

- Ratifies the v0.6.x ladder for repo visitors. No code changes —
  this release exists to put the showable proof on the front page.

## [0.6.6] — 2026-04-26

### What v0.6.6 closes

The v0.6.3 release added a verdict-suite regression gate to CI on
main, but a PR-time view of "did this PR regress the suite?" was
missing. v0.6.5's verdict-suite-delta note flagged this as a
v0.6.6 candidate; v0.6.6 ships it.

### Added — `verdict-suite-delta` job in `.github/workflows/witness-delta.yml`

Triggers on PRs touching `crates/`, `verdicts/`, `Cargo.toml`, or
either of the two delta-related files. Steps:

1. Checks out the base (main) and head (PR) revisions side by side.
2. Builds witness in release mode against each.
3. Runs `verdicts/run-suite.sh` against both with `SIGN=0` (PR
   delta does not need signed envelopes — that's the release
   pipeline's job).
4. Walks both `delta-head/` and `delta-base/` directory trees,
   reads each verdict's `report.json`, builds a per-verdict
   comparison table with columns:
   - base full/total decisions
   - head full/total decisions
   - head conditions (proved / gap / dead)
   - status: improvement / unchanged / regression / new
5. Posts the table as a PR comment, replacing any prior comment
   tagged with the `<!-- witness-verdict-delta -->` marker.
6. Uploads the delta directory as an artefact for inspection.

The job runs `continue-on-error: true` so a verdict-suite failure
doesn't block PR merging — the comment is the signal, not the
status check. A regression is flagged in **bold** in the comment
body so reviewers can't miss it.

### Updated — paths-filter on the delta workflow

The original filter only triggered on `src/`, `tests/`, and
`Cargo.toml`. v0.6.6 expands to include `crates/`, `verdicts/`, and
the action workflows themselves. PRs that touch the verdict suite
or the compliance pipeline now correctly fire the delta job.

### Notes for v0.7

- The same comparison logic could fire weekly against `main` to
  catch *upstream* regressions (e.g. a rustc upgrade that changes
  optimiser behaviour and silently moves a verdict from full-MC/DC
  to partial). That's a v0.7 ergonomics item alongside the
  scaling-roadmap work.
- The signing path (v0.6.4) and the trace matrix (v0.6.5) are
  release-time only. A v0.7 candidate is to surface the trace
  matrix on PR delta too — a single HTML artefact a reviewer can
  open to see the cross-references for the PR's branch state.

### Implements / Verifies

- Implements: REQ-022 (coverage-delta PR workflow) — extended from
  v0.4's manifest-only delta to include the verdict-suite roll-up.
- Verifies: comment-bot logic uses the existing
  `actions/github-script@v7` pattern; comment marker dedupes on
  re-runs.

## [0.6.5] — 2026-04-26

### What v0.6.5 closes

REQ-032 — every release ships a JSON + HTML traceability matrix
generated from `artifacts/*.yaml` at release time, bundled into the
compliance archive. v0.6.0 declared the requirement; v0.6.5 ships
the implementation.

Plus: parser_dispatch's `TRUTH-TABLE.md` gains a "Post-rustc
Wasm-level reality" section documenting why the report shows 7
decisions instead of the source author's intended 1 — the v0.2
paper's coverage-lifting argument in concrete form.

### Added — `.github/actions/compliance/trace-matrix.py`

Pure-Python (PyYAML only) script that reads the rivet artefact
graph and the verdict-evidence directory and emits two files:

- `traceability-matrix.json` (schema URL
  `https://pulseengine.eu/witness-trace-matrix/v1`) carrying
  totals + per-requirement satisfied-by-feature / supporting-
  decision lists + per-verdict MC/DC roll-up + signed-envelope
  flag.
- `traceability-matrix.html` styled for human review with a
  verdict-suite table at the top and a requirements table below.

Composite-action wiring installs PyYAML via `apt python3-yaml`
preferentially, falling back to `pip --break-system-packages` for
runners that lack the apt package.

### Added — Wasm-level-reality section in `parser_dispatch/TRUTH-TABLE.md`

The source-level table at the top of the file documents the
predicate as the author wrote it (5 conditions, hand-derived rows,
under-masking pair structure). The new section at the bottom
documents what witness's report actually finds:

- 7 decisions (1 in `lib.rs`, 5 across `memchr.rs`, 1 split via
  inlining of the byte-search exit structure)
- 33 br_ifs total
- 5 dead conditions because `memchr`'s SIMD path requires inputs
  longer than our 6 test rows provide

The discrepancy is the v0.2 paper's coverage-lifting argument made
concrete: post-rustc Wasm coverage measures what the optimizer
left, including stdlib internals invoked by user code. The user is
responsible for scoping which decisions are part of their MC/DC
claim.

### Compliance bundle now contains

| File | Purpose |
|---|---|
| `verdict-evidence/<name>/*` | Per-verdict instrument-run-report-predicate-signed chain (v0.6.3+, v0.6.4 added signing) |
| `verdict-evidence/SUMMARY.txt` | One-line-per-verdict status table |
| `verdict-evidence/verifying-key.pub` | Ed25519 public key (v0.6.4+) |
| `verdict-evidence/VERIFY.md` | Verification walkthrough (v0.6.4+) |
| `traceability-matrix.json` | V-model matrix machine-readable (v0.6.5+) |
| `traceability-matrix.html` | V-model matrix human-readable (v0.6.5+) |
| `predicates/`, `manifests/` | Legacy v0.5 directories (still present for compatibility) |
| `coverage-report.{json,txt}` | Top-level coverage report (when run-json input is provided) |

### Implements / Verifies

- Implements: REQ-032 (V-model traceability matrix in every release).
- Verifies: matrix renders against the actual v0.6.4 verdict-evidence
  bundle locally — 39 requirements, 17 features, 22 design-decisions,
  7 verdicts (5 with non-zero decisions), all with signed envelopes.

## [0.6.4] — 2026-04-26

### What v0.6.4 closes

v0.6.3 populated the compliance bundle with real per-verdict
evidence — manifest, run record, MC/DC report, unwrapped in-toto
predicate per verdict. v0.6.4 adds the **signature** layer: each
verdict's predicate is wrapped in a DSSE envelope and signed with an
ephemeral Ed25519 keypair generated fresh for the release. The
verifying public key ships in the bundle. Tampering with the
predicate body, the envelope, or the key fails verification.

### Added — `witness keygen` and `witness verify` CLI commands

Two new subcommands close the signing loop:

- `witness keygen --secret SK --public PK` — generate a fresh
  Ed25519 keypair (raw 64-byte secret + 32-byte public). Used by the
  verdict-suite signing path; also available for users who want to
  sign their own predicates.
- `witness verify --envelope E --public-key PK` — validate a DSSE
  envelope against an Ed25519 public key. Exits zero with `OK` on
  match, non-zero (with a clear error) on mismatch. Standards-
  compliant DSSE means `cosign verify-blob` works equivalently.

### Added — `witness-core::attest::generate_keypair_files`

Public library API for keypair generation. Mirrors the existing
`sign_predicate_file` and `verify_envelope_file` shapes (file in /
file out / `Result<()>`). Lets downstream tooling embed the same
ephemeral-key flow.

### Added — `witness-core::attest::verify_envelope_file`

File-IO wrapper around the existing `verify_envelope` byte-slice API.
Reads the envelope and public key from disk, returns the inner
in-toto Statement on success.

### Updated — `verdicts/run-suite.sh`

When `SIGN=1` (default), the script:

1. Generates an ephemeral Ed25519 keypair via `witness keygen`.
2. Writes the public key to `<bundle>/verifying-key.pub`.
3. For each verdict's `predicate.json`, runs `witness attest` to
   produce `<verdict>/signed.dsse.json` with `key_id` =
   `witness-suite/<verdict>`.
4. Discards the secret key on exit (in a `mktemp` directory cleaned
   up by a `trap`).
5. Writes a `<bundle>/VERIFY.md` documenting the verification
   command for both `witness verify` and `cosign verify-blob`.

Setting `SIGN=0` skips the signing path — useful for fast local
iteration.

### Verifies — round-trip end-to-end

Local sign-verify proven on the leap_year and parser_dispatch
envelopes:

```
$ witness verify \
    --envelope leap_year/signed.dsse.json \
    --public-key verifying-key.pub
OK — DSSE envelope leap_year/signed.dsse.json verifies against verifying-key.pub
  predicate type: https://pulseengine.eu/witness-coverage/v1
  subjects: 1
```

Tampering the public key (XOR first byte) correctly fails:

```
Error: wasm runtime error: DSSE verify failed: VerificationFailed
exit=1
```

### Why ephemeral keys

Per-release ephemeral keys avoid long-term key custody. The verifying
key is shipped in the compliance bundle. The secret key is generated
fresh in CI, used to sign, then discarded. A signature thus proves
"this evidence was produced by the release pipeline that wrote this
verifying-key.pub" — exactly the V-model claim. Long-term key
management (rotation, attestation chains, sigstore Fulcio integration)
is v0.7+ work.

### Implements / Verifies

- Implements: REQ-031 (witness-core.wasm signed release asset — the
  pattern now applies uniformly to verdict predicates too).
- Verifies: round-trip sign + verify works for every verdict that
  produces a non-empty predicate; tampered key correctly rejected.

## [0.6.3] — 2026-04-26

### What v0.6.3 closes

v0.6.0 promised real per-verdict signed evidence in the release archive
(REQ-033) but shipped a structural placeholder: empty `predicates/`
and `manifests/` directories. v0.6.1 made the per-row instrumentation
work end-to-end; v0.6.2 made 5 of 7 verdicts produce reports. v0.6.3
finally **populates the compliance bundle** with that evidence and
adds a CI regression gate so the suite stays green across rustc
upgrades.

### Added — `verdicts/run-suite.sh`

End-to-end driver script. Invoked locally (`./verdicts/run-suite.sh
some-out-dir`) and from the compliance action. For each of the seven
verdicts:

1. Builds with `wasm32-unknown-unknown` (core module — walrus can
   rewrite; `wasm32-wasip2` produces Components walrus doesn't yet
   handle).
2. Instruments with the v0.6.1 per-row primitive.
3. Runs every `run_row_<n>` export.
4. Emits text + JSON MC/DC reports.
5. Builds the unwrapped in-toto Statement (signing is v0.6.4 once
   release-key management is wired in).
6. Emits LCOV + sibling overview when DWARF surfaces decisions.

A `SUMMARY.txt` rolls up branches / decisions / full-MC/DC counts:

```
verdict              branches   decisions    full-mcdc
-------              --------   ---------    ---------
leap_year            2          1            1/1
range_overlap        0          0            0/0
triangle             2          1            1/1
state_guard          3          1            1/1
mixed_or_and         0          0            0/0
safety_envelope      4          1            1/1
parser_dispatch      33         7            1/7
```

### Added — populated compliance bundle

`.github/actions/compliance` now invokes `verdicts/run-suite.sh` and
nests the output under `compliance/verdict-evidence/<name>/`. Each
release's compliance archive contains:

- The seven verdict directories with their full instrument-run-report
  chain.
- `SUMMARY.txt` at the top of the bundle.
- The original (now non-empty) `predicates/` and `manifests/`
  directories.

Closes REQ-033 ("compliance bundle populated with real evidence") in
substance, not just structurally.

### Added — `verdict-suite` CI regression gate

New `verdict-suite` job in `ci.yml`:

- Builds witness in release mode.
- Adds `wasm32-unknown-unknown` target.
- Runs the suite script.
- **Asserts** that `leap_year`, `triangle`, `state_guard`,
  `safety_envelope`, and `parser_dispatch` each produce >= 1
  reconstructed decision. A regression (e.g. a future rustc that
  fully optimises one of these verdicts to bitwise) fails CI on main.
- Uploads `verdict-evidence/` as an artefact.

`range_overlap` and `mixed_or_and` are deliberately excluded from the
gate — their pure-boolean conditions are expected to be fully
optimised to `i32.and` and produce zero branches at the Wasm level.

### Notes for v0.6.4

- DSSE-sign each verdict's predicate with a release-time key. Pulls
  `wsc-attestation` into the action, manages the key via GitHub
  Secrets.
- Add a `verdict-suite-delta` PR-level CI job that diffs the
  `decisions / conditions / full-mcdc` counts vs `main` and posts the
  delta to the PR conversation. Useful for catching subtle optimiser
  regressions earlier than the regression gate fires.

### Implements / Verifies

- Implements: REQ-033 (compliance bundle populated with real
  per-verdict evidence).
- Verifies: the verdict-suite CI gate exists and fails closed when
  any of the five "should-have-decisions" verdicts regresses to zero.

## [0.6.2] — 2026-04-26

### What v0.6.2 closes

v0.6.1 made the per-row instrumentation work end-to-end on the
`leap_year` verdict, where rustc happens to attribute all surviving
`br_if`s to the same source line. Verdicts whose conditions span
multiple source lines (`state_guard`, `triangle`, `safety_envelope`,
`parser_dispatch`) had surviving `br_if`s in their manifests but
**zero reconstructed decisions** — `decisions::group_into_decisions`
required strict same-line equality for the grouping criterion, which
short-circuit chains formatted across multiple lines do not satisfy.

v0.6.2 relaxes the criterion: br_ifs in the same `(function, file)`
whose source lines fall within `MAX_DECISION_LINE_SPAN = 10` cluster
into one Decision. Walks branches in branch-id order (= source-walk
emission order), starts a new cluster when the next br_if is outside
the line window. False-grouping is bounded — adjacent decisions
separated by a > 10-line gap stay separate.

### Result

| Verdict | branches | decisions | full MC/DC | notes |
|---|---:|---:|---|---|
| `leap_year` | 2 | 1 | 1 | unchanged from v0.6.1 |
| `state_guard` | 3 | 1 | 1 | **new in v0.6.2** |
| `triangle` | 2 | 1 | 1 | **new in v0.6.2** |
| `safety_envelope` | 4 | 1 | 1 (3 conds) | **new in v0.6.2** |
| `parser_dispatch` | 33 | **7** | 1 | **new in v0.6.2** — finds decisions in `memchr` library calls automatically |
| `range_overlap` | 0 | 0 | n/a | optimised to `i32.and` (bitwise), nothing to measure |
| `mixed_or_and` | 0 | 0 | n/a | optimised to bitwise; nothing to measure |

### parser_dispatch is the standout

The `parser_dispatch` verdict's `s.contains(b'@')` call lowered into
the `memchr` library's byte-search loops, which themselves contain
compound boolean conditions. `decisions::reconstruct_decisions` picks
them up automatically:

```
$ witness report --input parser_dispatch.run.json --format mcdc
decisions: 1/7 full MC/DC; conditions: 7 proved, 15 gap, 5 dead

decision #0 lib.rs:37: Partial
  c0 (branch 3): proved via rows 1+4 (masking)
  c1 (branch 10): DEAD — never evaluated in any row
  c2 (branch 11): DEAD — never evaluated in any row
  c3 (branch 17): GAP — try a row {c0=T, c3=T} (outcome must differ from row 4)
decision #1 lib.rs:58: FullMcdc
  c0 (branch 18): proved via rows 2+4 (masking)
  c1 (branch 19): proved via rows 3+5 (unique-cause)
decision #2 memchr.rs:40: Partial
  c0 (branch 0): proved via rows 0+4 (masking)
  c1 (branch 1): proved via rows 1+5 (masking)
  c2 (branch 2): GAP — try a row {c0=T, c1=T, c2=F} (outcome must differ from row 4)
...
```

This is "witness on real code, not toys" — the predicate is six
test rows of 4-condition URL-authority validation, but the underlying
implementation drags in the standard library's compound predicates,
and witness reports MC/DC on all of them with cited row pairs and
closure recommendations.

### Implementation

- `decisions::MAX_DECISION_LINE_SPAN: u32 = 10` — public constant so
  consumers can document the threshold in V-model briefs.
- `group_into_decisions` rewritten as a two-pass algorithm: resolve
  br_if entries to `(function, file, line)`, then bucket by
  `(function, file)` and cluster within each bucket using the
  adjacent-line span.
- Two new unit tests: `group_into_decisions_clusters_adjacent_lines`
  (4 br_ifs on lines 23-26 → one Decision) and
  `group_into_decisions_splits_on_large_gap` (two clusters separated
  by a 49-line gap stay separate).

### Implements / Verifies

- Implements: REQ-027, REQ-028, REQ-029 — extends the v0.6.0 schema
  + reporter to cover the broader range of compound-decision
  lowerings rustc emits.
- Verifies: 5 verdicts (leap_year, state_guard, triangle,
  safety_envelope, parser_dispatch) produce non-empty MC/DC reports
  with cited row pairs.

### Notes for v0.6.3 / v0.7

- `range_overlap` and `mixed_or_and` produce zero branches because
  rustc fully optimises their pure-boolean conditions to bitwise
  arithmetic. v0.7's "compiler hint" work could ask rustc to emit
  branches for these patterns when an opt-in attribute is present —
  per the v0.2 paper's "witness-and-checker" stance. Out of scope
  for v0.6.x.
- `parser_dispatch` shows 5 dead conditions across 6 test rows; the
  verdict's `TRUTH-TABLE.md` should be revised to align expected
  rows with what the post-rustc lowering actually exposes (rather
  than the source-level decisions originally documented).

## [0.6.1] — 2026-04-26

### What v0.6.1 closes

v0.6.0 shipped the consumer side (schema, reporter, verdict-suite oracles)
and explicitly deferred the runtime instrumentation to v0.6.1. v0.6.1 is
that runtime path: real per-row condition capture during `witness run`,
real `RunRecord.decisions` populated from execution rather than hand-
curated, real end-to-end demonstrable MC/DC on the canonical leap_year
verdict.

### Added — per-row instrumentation

- **Per-condition exported globals**: each `BrIf` / `IfThen` / `IfElse`
  branch now allocates two additional globals alongside its existing
  `__witness_counter_<id>`:
  - `__witness_brval_<id>` (i32) — the condition's evaluated value
    (0 or 1) when reached this row, or `1` for fired arms.
  - `__witness_brcnt_<id>` (i32) — count of evaluations this row;
    non-zero means evaluated, zero means short-circuited (absent
    from `DecisionRow.evaluated`). `BrTable*` branches keep
    counter-only instrumentation per DEC-015.
- **`__witness_row_reset` exported function**: emitted by every
  instrumentation pass. Zeros all `brval` / `brcnt` globals so the
  next row's captures don't leak prior state.

### Added — runner row-by-row capture

- `witness run` (embedded wasmtime path) now, for each `--invoke`:
  1. Calls `__witness_row_reset` to clear per-row state.
  2. Invokes the export, capturing the return value as the row's
     decision outcome (when the export returns an `i32`).
  3. Reads the per-row `brval` / `brcnt` globals.
  4. For each `Decision` in the manifest, builds a `DecisionRow`
     populated with the per-condition values evaluated this row.
- `RunRecord.decisions` is now populated from execution; the
  `mcdc_report` reporter consumes it directly with no manual curation.

### End-to-end demonstrable on `verdicts/leap_year`

Building the leap_year verdict, instrumenting it, running all 4 row
exports, and asking for the MC/DC report produces:

```
$ witness instrument verdicts/leap_year/verdict_leap_year.wasm -o leap.wasm
$ witness run leap.wasm --invoke run_row_0 ... --invoke run_row_3 -o run.json
$ witness report --input run.json --format mcdc
module: leap.wasm
decisions: 1/1 full MC/DC; conditions: 2 proved, 0 gap, 0 dead

decision #0 lib.rs:46: FullMcdc
  truth table:
    row 0: {c0=T} -> F
    row 1: {c0=F, c1=T} -> T
    row 2: {c0=F, c1=F} -> F
    row 3: {c0=F, c1=F} -> T
  conditions:
    c0 (branch 0): proved via rows 0+1 (masking)
    c1 (branch 1): proved via rows 1+2 (unique-cause)
```

Two conditions, both proved with cited row pairs, full MC/DC at the
Wasm bytecode level.

### Why some verdicts report zero decisions

The leap_year decision `(year%4==0 && year%100!=0) || year%400==0`
lowered to two `br_if` instructions plus inline arithmetic for the
third condition. That's why the report shows two conditions rather
than three: the third was elided by rustc's optimizer into the
fall-through computation. This is exactly the v0.2 paper's coverage-
lifting thesis — post-rustc Wasm coverage measures *what the
optimizer left as branches*, not the source-level condition count.

For verdicts whose conditions are all side-effect-free comparisons
(e.g. `a && b` over pure booleans), rustc may lower `&&` to a single
`i32.and` instruction and eliminate branches entirely. `range_overlap`
and similar verdicts produce zero `BrIf` entries in the manifest as a
result. The reporter correctly reports zero decisions — that is the
honest measurement at this point. Source-level MC/DC for these
predicates is the rustc-mcdc tool's territory; witness covers what
survives lowering. The "overdo stance" (DEC-005) — adopt both, do
not pick one.

The remaining verdicts (`triangle`, `state_guard`, `mixed_or_and`,
`safety_envelope`, `parser_dispatch`) have varying numbers of
surviving branches depending on rustc's lowering choices for their
specific shapes. Their `TRUTH-TABLE.md` files document the
hand-derived source-level MC/DC; the witness report shows the
Wasm-level MC/DC. The discrepancy between the two is itself
evidence of how aggressive the optimizer's elision is — useful
data for the v0.7 work on inlined-subroutine handling and decision
reconstruction extension.

### Implements / Verifies

- Implements: REQ-034 (on-Wasm trace-buffer instrumentation; v0.6.1
  uses per-row globals as the simplest correct primitive instead of
  the linear-memory trace buffer recommended by Agent A — both
  satisfy the requirement, the per-row globals are simpler when each
  row invokes the predicate exactly once).
- Implements: FEAT-015 (the runtime side of the v0.6 redo).
- Verifies: leap_year verdict end-to-end pipeline produces the
  expected Wasm-level MC/DC report (1 decision, 2 conditions, full
  MC/DC under masking).

### Notes for v0.6.2

- Investigate why state_guard / triangle / mixed_or_and decisions
  don't always reconstruct under DWARF-based grouping despite having
  surviving br_ifs. Likely fix: relax the `(function, source_file,
  source_line)` grouping criterion to handle inlined-subroutine line
  attribution.
- Consider whether the per-row-globals primitive should evolve toward
  the linear-memory trace buffer (Agent A's recommendation) once
  v0.7's scaling work surfaces hot-loop overflow patterns.
- Per-target br_table MC/DC reconstruction (DEC-015 deferral).

## [0.6.0] — 2026-04-25

### What v0.6 is — and what it is not

v0.5.0 shipped DWARF-grouped branch coverage but the report layer
computed *per-branch hit counts*, not MC/DC truth tables. The CHANGELOG
described it as MC/DC; that was an overclaim. v0.6 is the redo: the
schema, the reporter, the verdict suite, and the V-model artefact graph
that real MC/DC requires. The on-Wasm instrumentation that captures
per-row condition vectors lands as a v0.6.1 follow-up — see "Deferred
to v0.6.1" below.

### Added — schema and reporter

- **`RunRecord` schema v3**: new `decisions: Vec<DecisionRecord>` and
  `trace_health: TraceHealth` fields (REQ-027, FEAT-012, DEC-013).
  `DecisionRecord` carries per-decision `rows: Vec<DecisionRow>`; each
  `DecisionRow` has a sparse `evaluated: BTreeMap<u32, bool>` so
  short-circuited conditions are first-class evidence (DEC-014). v0.5
  records (schema "2") still load — both new fields default to empty.
- **`witness-core::mcdc_report` module**: per-decision truth tables,
  independent-effect citations under masking MC/DC (DO-178C accepted
  variant), gap analysis with row-closure recommendations (REQ-028,
  REQ-029). 6 unit tests covering all canonical decision shapes pass.
- **`witness report --format mcdc`** and **`--format mcdc-json`**:
  CLI surface for the new reporter. Schema URL
  `https://pulseengine.eu/witness-mcdc/v1`.

### Added — verdict suite (REQ-030, FEAT-012, DEC-016)

The `verdicts/` directory contains seven canonical compound-decision
verdicts, each as a self-contained Rust crate that compiles to
`wasm32-wasip2`. Each verdict ships:

- `Cargo.toml` — standalone, opts out of the witness workspace.
- `src/lib.rs` — the predicate plus `run_row_<n>` exports, one per
  test row.
- `TRUTH-TABLE.md` — the **expected** MC/DC analysis, hand-derived,
  with a machine-readable JSON block. Acts as the verification oracle
  for the `mcdc_report` reporter.
- `V-MODEL.md` — one-page traceability: REQ → DEC → conditions → rows
  → evidence.
- `build.sh` — standalone build to `wasm32-wasip2`.

The seven verdicts and their shapes:

| Verdict | Decision | Conds | Rows |
|---|---|---|---|
| `leap_year` | `(y%4==0 && y%100!=0) \|\| y%400==0` | 3 | 4 |
| `range_overlap` | `a.start <= b.end && b.start <= a.end` | 2 | 3 |
| `triangle` | Myers-paper "not a triangle" check (3-cond OR) | 3 | 4 |
| `state_guard` | TLS handshake guard (4-cond AND chain) | 4 | 5 |
| `mixed_or_and` | `(a\|\|b) && (c\|\|d)` | 4 | 5 |
| `safety_envelope` | 5-cond automotive envelope (beyond LLVM 6-cap) | 5 | 6 |
| `parser_dispatch` | RFC 3986 URL authority validator (real-world anchor) | 5 | 6 |

The reporter's correctness has been verified by reproducing each
verdict's hand-derived `TRUTH-TABLE.md` from a hand-curated
`DecisionRecord` in unit tests.

### Added — V-model artefact graph (REQ-032, FEAT-014, DEC-017)

- 7 new requirements (REQ-027..033)
- 3 new features (FEAT-012..014)
- 6 new design decisions (DEC-013..018), with DEC-013 documenting the
  trace-buffer instrumentation primitive recommendation from the
  `v06-instrumentation-primitive` research brief.
- `rivet validate` PASS across the workspace.

### Added — research roadmap (4 parallel agent docs, ~19k words total)

- `docs/research/v06-instrumentation-primitive.md` — chooses linear-
  memory trace buffer with row markers as the v0.6.1 instrumentation
  primitive. Wasm-side rewrite sketch, schema diff, short-circuit
  semantics policy, BrTable v0.7 deferral, prior-art citations,
  implementation risk register.
- `docs/research/v07-scaling-roadmap.md` — destination workload pick:
  `seanmonstar/httparse` (~1500 decisions, clean wasm32-wasip2 build).
  v0.7 capability list (streaming counter encoding, i64 saturating
  counters, inlined-subroutine DWARF, auto-generated synthetic
  requirements, module-rollup default report). Top scaling risk:
  DWARF parsing memory at scale.
- `docs/research/v08-visualisation-roadmap.md` — architecture call:
  `wstd-axum` + `maud` + HTMX 2.x, runnable as `wasmtime serve` or
  composed via `wac plug`. AI-agent surface = REST+JSON content
  negotiation plus `rmcp` MCP transport mounted on the same Axum
  router. Playwright tests reuse rivet's pattern; visualiser
  visualises its own coverage (the v0.8 demo screenshot).
- `docs/research/v09-soa-and-agent-ux.md` — competitive scan
  (LDRA, VectorCAST, Cantata, BullseyeCoverage, Squore, gcov+gcovr).
  v0.9 positioning: first MC/DC tool with end-to-end signed evidence
  and agent-native MCP API. Top 3 superiority features identified.
  Biggest competitive risk: RapiCover already has unbounded
  conditions plus DO-178C heritage for C/C++/Ada.

### Deferred to v0.6.1

- **On-Wasm instrumentation that captures per-row data.** The
  trace-buffer rewrite from `v06-instrumentation-primitive.md` is
  scoped for v0.6.1. v0.6.0 ships the consumer side (schema +
  reporter + verdict suite oracles + CLI). The `witness instrument`
  subcommand still emits v0.5-style per-counter instrumentation;
  v0.6.1 extends it with the trace primitive so `witness run`
  produces populated `RunRecord.decisions`.
- **End-to-end verdict execution.** Each verdict's `src/lib.rs`,
  `TRUTH-TABLE.md`, and `V-MODEL.md` are in place; `cargo build
  --target wasm32-wasip2` against each verdict crate produces a
  `.wasm`. The reporter's correctness has been verified against the
  hand-derived truth tables in unit tests, and the verdicts' V-MODEL
  evidence chains will be populated by `compliance` when v0.6.1's
  instrumentation lands.

### Why ship the foundation as 0.6.0

The schema, reporter, verdict-suite oracles, and V-model artefact
graph are independent of the instrumentation runtime path. Shipping
them as v0.6.0 lets downstream consumers (rivet, sigil, agent
integrations) build against the v3 schema and the
`witness-mcdc/v1` predicate type now, while the instrumentation
work continues in the v0.6.1 release. The verdicts' `TRUTH-TABLE.md`
files are the verification oracles v0.6.1 will reproduce.

### Implements / Verifies

- Implements: REQ-027 (truth-table emission), REQ-028 (independent-
  effect citations), REQ-029 (gap-closure recommendations), REQ-030
  (verdict suite — scaffolded), REQ-032 (V-model traceability —
  artefact graph), REQ-033 (compliance bundle structure).
- Implements: FEAT-012 (real MC/DC reporter — consumer side),
  FEAT-014 (V-model artefact graph).
- Verifies: 6 mcdc_report unit tests reproduce each canonical verdict
  shape's expected truth table and pair-finding outcomes.

## [0.5.0] — 2026-04-25

### Added

- **Workspace split.** Single-crate `witness` becomes a workspace with
  `crates/witness-core` (pure-data algorithms; `wasm32-wasip2`-buildable)
  and `crates/witness` (CLI binary plus the wasmtime-using runner).
  All algorithm modules — instrument, decisions, diff, predicate,
  report, rivet_evidence, run_record, lcov, attest — live in
  witness-core. Only main.rs + run.rs (wasmtime embedder) stay in the
  binary crate.
- **`witness lcov`** subcommand (REQ-023). Emits LCOV from a
  `RunRecord` per the
  [v0.5 LCOV format brief](docs/research/v05-lcov-format.md). Hybrid
  emission: DWARF-correlated `Decision`s become standard `BRDA`
  records keyed to real source files; uncorrelated branches go in a
  sibling overview text. Codecov-ingestible as `flag: wasm-bytecode`.
- **`witness attest`** subcommand (REQ-024). Wraps an unwrapped
  in-toto Statement (from `witness predicate`) in a DSSE envelope
  signed with an Ed25519 secret key. Compatible with sigil's
  `wsc verify`, sigstore cosign, and any in-toto-attestation
  consumer. Implementation depends on the workspace `wsc-attestation`
  path-dep into `pulseengine/sigil`.
- **Wasm-target artefact.** `cargo build -p witness-core --target
  wasm32-wasip2` produces `target/wasm32-wasip2/release/witness_core.wasm`,
  uploaded as a CI artefact and (in release builds) attached to the
  GitHub release. The full Component Model build with WIT bindings
  is the v0.6 stretch goal.
- **CI dogfood loop.** New `dogfood` job builds the
  `sample-rust-crate` fixture, instruments it with the freshly-built
  witness, runs every `run_*` export, and emits LCOV. Uploads to
  codecov with `flag: wasm-bytecode` for side-by-side comparison
  with the existing `flag: rust-source` LCOV (cargo-llvm-cov).
- **`witness-core` Wasm-target CI job.** Verifies witness-core
  compiles to `wasm32-wasip2` on every push to main; uploads the
  resulting `.wasm` artefact.
- **Loom + meld upstream issue drafts** at
  `docs/research/v05-loom-meld-upstream.md` ready for the maintainer
  to file. Both ask for DWARF preservation plus a byte-offset
  translation map so witness v0.6 can correlate post-loom / post-meld
  Wasm to source-level decisions.

### Research output

- `docs/research/v05-blog-principles.md` (placeholder; previously
  `v04-blog-principles.md` covers the same corpus).
- `docs/research/v05-lcov-format.md` — codecov-flag-compatible LCOV
  emission; recommends hybrid C strategy (BRDA for correlated, text
  overview for uncorrelated).
- `docs/research/v05-wsc-integration.md` — wsc-attestation API
  surface, Cargo dep model, witness-attest subcommand sketch. Confirmed
  wasm32 compatibility under the `signing` feature.
- `docs/research/v05-component-witness.md` — component-model build
  path; confirms cargo-component, wac, wit-bindgen all installed
  locally; identifies wasmtime as the only host-only dep.
- `docs/research/v05-loom-meld-upstream.md` — issue drafts for the
  upstream tools.

### Changed

- The `coverage` CI job now uploads with `flag: rust-source` so the
  new bytecode-coverage upload (`flag: wasm-bytecode`) renders
  side-by-side in codecov.
- Workspace pulls `wsc-attestation` from a sibling
  `pulseengine/sigil` checkout (path dep). Will become a regular
  crates.io dep when wsc-attestation publishes.
- Direct `ed25519-compact` dep added to witness-core for keypair
  generation in tests and direct use by `attest.rs`.

### Implements / Verifies

- Implements: REQ-023 (witness lcov), REQ-024 (witness attest), plus
  the v0.5 workspace-split and dogfood-CI requirements (REQ-025,
  REQ-026 in the artefact set).

### Deferred to v0.6

- DWARF preservation through loom optimisation (gated on the upstream
  loom issue).
- DWARF preservation through meld fusion (gated on the upstream meld
  issue).
- Full Component Model build with WIT interface and `wac`-based
  composition with sigil's wsc component for in-process signing.

## [0.4.0] — 2026-04-25

### Added

- **DWARF-grounded MC/DC reconstruction** (FEAT-011, REQ-005, REQ-006,
  REQ-016). `decisions::reconstruct_decisions` now parses Wasm DWARF
  custom sections via `gimli` and `wasmparser`, builds a
  `(byte_offset → file, line)` map per compilation unit, and groups
  `BrIf` `BranchEntry`s sharing a `(function, file, line)` key into
  source-level `Decision`s. Strict per-`br_if` fallback when DWARF is
  absent. Lifted from v0.2.1 plan; v0.2.1 is therefore not released as
  a separate version.
- **`witness diff` subcommand** (REQ-020). Computes added / removed /
  changed branches and (when both inputs are runs) coverage-percentage
  delta. Schema URL `https://pulseengine.eu/witness-delta/v1`. Both
  JSON and text output. Required by the v0.4 PR delta workflow.
- **`witness-delta.yml` PR workflow** (REQ-022). Triggers on every PR
  touching `src/` / `tests/` / `Cargo.toml`. Checks out base + head,
  builds the head witness, runs `witness diff` on whatever manifests
  the fixture pipeline emits, attaches the delta JSON+text as a PR
  artefact. `continue-on-error` throughout — never blocks merge.
- **`actions/compliance` composite action** (REQ-021). Mirrors rivet's
  equivalent. Generates a tar.gz evidence bundle on release containing
  coverage report, in-toto predicates per module, branch manifests,
  and a README. Wired into `release.yml` between `build-binaries` and
  `create-github-release` as a new `compliance` job; the resulting
  archive is attached to the GitHub release alongside the binaries.

### Research output

- `docs/research/v04-blog-principles.md` — survey of every published
  pulseengine.eu post and the principles witness must adopt; 4756
  words across 14/16 posts; 20-item adoption checklist; voice
  mechanics catalogued.
- `docs/research/v04-ci-ports.md` — adaptation brief for
  rivet-delta.yml and the rivet compliance composite action; full
  YAML drafts for both witness-side workflow files.
- `docs/research/v04-compiler-qualification-reduction.md` — 451-line
  brief: ISO 26262-8 §11.4.5 substitution argument for ASIL B
  (works), DAL B (weaker), DAL A (broken). Most surprising finding:
  the TCL framework explicitly yields TCL 1 — "no qualification
  required" — when TI 1 *or* TD 1 holds; the work is in establishing
  TD 1, not in carving an exception.
- `docs/research/v04-mythos-slop-audit.md` — quick-pass slop audit
  using the methodology from
  <http://127.0.0.1:1024/blog/mythos-slop-hunt/>. Two P1 findings
  applied (deleted `report::save_json`; removed direct `tracing` dep).
  Two P2 findings kept as consumer-facing constants. Twelve P3
  findings documented as intentional defensiveness.

### Removed

- `report::save_json` — orphan-slop, no callers (P1 slop-hunt finding).
- `tracing = "0.1"` direct dependency — only `tracing-subscriber` is
  actively used (P1 slop-hunt finding).

### Deferred to v0.5

- Component-model coverage (was nominal v0.4; needs walrus or wac
  component support).
- Post-cfg / post-meld / post-loom measurement points (depends on
  loom's translation-validation evidence shape, which is itself
  evolving).
- A Wasm Component Model fixture for end-to-end testing (folded with
  the above).

### Implements / Verifies

- Implements: REQ-005, REQ-006, REQ-016, REQ-020, REQ-021, REQ-022
- Implements: FEAT-011 (v0.4 feature wrapper)

## [0.3.0] — 2026-04-25

### Added

- **`witness merge`** subcommand. Aggregates per-branch counters across
  multiple `witness run` outputs (one per test binary or harness
  invocation). Validates that all inputs share the same instrumented
  module and branch list before summing. Five new tests + four proptest
  properties (commutativity, monotonicity, sum-preservation,
  single-record identity).
- **`witness predicate`** subcommand. Emits an unwrapped in-toto
  Statement v1.0 carrying the coverage report as a
  `https://pulseengine.eu/witness-coverage/v1` predicate. Subject is
  the instrumented module (sha256); the original module's digest goes
  in the predicate body. Sigil reads the predicate type opaquely (no
  registry, no schema validation per type — see
  `docs/research/sigil-predicate-format.md`), so witness emits today
  with no sigil-side change. 5 unit tests including known-vector
  SHA-256 and RFC 3339 timestamp calibration.
- **`witness rivet-evidence`** subcommand. Emits coverage in the
  `https://pulseengine.eu/witness-rivet-evidence/v1` schema, partitioned
  by a user-supplied `branch_id → artefact_id` mapping YAML. The
  schema mirrors rivet's existing `ResultStore` shape so the consumer
  can be a near-drop-in copy. 4 unit tests + 2 proptest properties on
  RequirementMap flattening.
- **rivet upstream consumer** on the
  `feat/witness-coverage-evidence-consumer` branch in
  `pulseengine/rivet`. Adds `rivet-core::coverage_evidence::CoverageStore`
  mirroring `ResultStore`, plus 9 unit tests, plus the new
  `Error::CoverageEvidence` variant. 491 LOC. 780 rivet-core tests
  pass; clippy/fmt/deny clean. Branch is **left local for review** —
  not pushed to origin.
- **`docs/research/rivet-evidence-consumer.md`** and
  **`docs/research/sigil-predicate-format.md`** — evidence-of-design
  briefs that established the schemas before the code was written.
- **Rust→Wasm test fixture** under `tests/fixtures/sample-rust-crate/`.
  A minimal `no_std` Rust crate that compiles to Wasm and exercises
  every instrumentation pattern (`br_if`, `if/else`, `br_table`).
  Eight integration tests in `tests/integration_e2e.rs` runtime-skip
  if the fixture isn't built; `./tests/fixtures/sample-rust-crate/build.sh`
  is the one-shot builder for CI.

### Quality bar (REQ-019, FEAT-010)

- **Property-based tests** via `proptest` (new dev-dependency). 8
  properties covering merge invariants, serde round-trip of `Manifest`
  / `RunRecord`, and `RequirementMap::flatten` semantics. CI's
  `proptest-extended` job on main runs with `PROPTEST_CASES=2048`.
- **Mutation testing** via `cargo-mutants`. New CI job `mutants` runs
  on main as informational (continue-on-error: true). Configuration
  in `.cargo/mutants.toml` constrains mutation to the witness library
  and skips test modules.
- **Miri** CI job runs nightly miri with `-Zmiri-tree-borrows` over the
  pure-Rust modules (`report::*`, `decisions::*`, predicate's SHA
  vector + RFC 3339 path). The walrus / wasmtime FFI surface is
  excluded — miri's foreign-call constraints make it more noise than
  signal there.
- **Coverage threshold raised** to 75% project / 80% patch
  (`codecov.yml`).

### Implements / Verifies (rivet trailers)

- Implements: REQ-007, REQ-008, REQ-017, REQ-018, REQ-019
- Implements: FEAT-003 (rivet/sigil integration), FEAT-010 (quality bar)

### Notes

- v0.2.1 (DWARF reconstruction algorithm body) remains an any-time
  release. The schema is forward-compatible — when v0.2.1 lands, the
  rivet-evidence and predicate emitters automatically populate
  `decisions: [...]` for hosts that consume MC/DC.
- rivet integration is end-to-end **once the rivet upstream branch is
  pushed and a rivet release cuts**. The witness output is correctly
  shaped today; the rivet consumer code is on a feature branch.

## [0.2.0] — 2026-04-25

### Added

- **Subprocess harness mode** (`witness run --harness <cmd>`). Spawns a
  user-supplied command via `sh -c` with `WITNESS_MODULE` /
  `WITNESS_MANIFEST` / `WITNESS_OUTPUT` env vars set; the harness writes
  a counter snapshot to `WITNESS_OUTPUT` before exiting. Witness merges
  the snapshot with the manifest to produce the final run JSON. Escape
  hatch for runtimes the embedded wasmtime cannot accommodate
  (browser-based tests, custom WASI capability profiles, etc.).
  Implements REQ-014 / FEAT-006 / DEC-009.
- **Per-target `br_table` counting** (REQ-013 / FEAT-007 / DEC-008). v0.1's
  single-counter "executed" instrumentation is replaced with one counter
  per target plus one for the default arm. A generated
  `__witness_brtable_<n>` helper function dispatches on the selector via
  i32.eq chain (or i32.ge_u for the default), increments the matching
  counter, and returns the selector unchanged for the original
  `br_table` to dispatch. `BranchKind::BrTable` is removed; replaced by
  `BrTableTarget` (with `target_index: u32`) and `BrTableDefault`.
- **Manifest schema v2** (`schema_version: "2"`). Adds:
  - `BranchEntry.byte_offset: Option<u32>` — original wasm bytecode
    offset from walrus's `InstrLocId`. Required for DWARF correlation.
  - `BranchEntry.target_index: Option<u32>` — for `BrTableTarget` only.
  - `Manifest.decisions: Vec<Decision>` — DWARF-grounded source-level
    decisions reconstructed from `br_if` sequences. Empty when DWARF is
    absent or the v0.2.0 stub is in effect.
- **No artificial condition-count cap** (REQ-015). Witness uses exported
  globals, not LLVM's bitmap encoding, and supports decisions of any
  size.
- **`docs/paper/v0.2-mcdc-wasm.md`** — 8.2k-word paper draft covering
  motivation, formal MC/DC at Wasm, the reconstruction algorithm, the
  coverage-lifting soundness argument (DEC-010), comparison with
  rustc-mcdc / Clang / wasmcov / Whamm, and regulatory framing. Six
  sourcing TODOs for closed-access primary references (DO-178C clause,
  Chilenski & Miller 1994, Vilkomir & Bowen, Pnueli et al., DWARF
  spec).
- **README**: new "Related work" section with seven-row comparison
  table; status updated to "v0.1.0 shipped 2026-04-24"; usage examples
  refreshed to show both `--invoke` and `--harness` modes.

### Changed

- **MSRV unchanged at 1.91** (matches wasmtime 42's transitive floor).
- `Module` is loaded via `from_buffer` rather than `from_file` so the
  original bytes are available to the (stubbed) DWARF reconstructor.

### Stubbed (lands in v0.2.1)

- **DWARF-grounded reconstruction algorithm body** (DEC-012). v0.2.0
  ships the schema and the fallback path; the algorithm itself
  (`src/decisions.rs::reconstruct_decisions`) currently returns an
  empty list, leaving hosts on the strict per-`br_if` interpretation.
  The algorithm is documented in `docs/paper/v0.2-mcdc-wasm.md`. The
  schema is forward-compatible; v0.2.1 will fill the stub without a
  schema bump.

### Implements / Verifies (rivet trailers)

- Implements: REQ-013, REQ-014, REQ-015, REQ-016
- Verifies: REQ-004 (semantic preservation; round-trip tests pass for
  br_if, if-else, br_table)

## [0.1.0] — 2026-04-24

### Added

- `witness instrument <in.wasm> -o <out.wasm>` — walrus-based branch-counter
  insertion at every `br_if`, `if-else`, and `br_table` in every local
  function. Counter values are exposed as exported mutable globals named
  `__witness_counter_<id>`. Emits a sidecar manifest JSON describing each
  branch's function index, instruction index within its sequence, and
  kind.
- `witness run <instrumented.wasm> --invoke <export>` — built-in wasmtime
  runner that instantiates the module, invokes the requested no-argument
  export(s), reads all counter globals, and writes a raw run JSON.
  WASI-preview1 is wired with `inherit_stdio`; `--call-start` runs the
  WASI `_start` command-style entry-point.
- `witness report --input <run.json>` — branch-coverage report in human
  text or JSON. Per-function aggregation, deterministic uncovered-branch
  ordering.
- Library crate `witness::{instrument, run, report, error}` for callers
  that want to drive the pipeline programmatically (rivet integration in
  v0.3 will use this entry-point).
- SCRC Phase 1 + 2 clippy lints enforced workspace-wide; `cargo clippy
  --all-targets -- -D warnings` is a hard CI gate.
- Cross-platform CI: fmt, clippy, test matrix (Linux/macOS/Windows),
  MSRV (1.85), cargo-deny, cargo-audit, coverage via cargo-llvm-cov +
  codecov.
- Release workflow: tag-triggered cross-compiles for five targets
  (x86_64/aarch64 Linux, x86_64/aarch64 macOS, x86_64 Windows) with
  SHA256 checksums and auto-generated release notes.

### Design notes

- **Counter mechanism.** v0.1 exposes counters as exported mutable
  globals rather than a `__witness_dump_counters` function that
  serialises to linear memory. The exported-global path removes any
  cooperation-protocol requirement on the module-under-test and makes
  the runtime-side extraction a two-line `instance.get_global` for every
  conformant Wasm host. A dump-function escape hatch can be added later
  if an embedder requires a single exit point.
- **`br_table` granularity.** v0.1 counts `br_table` as a single
  "executed" point, not per-target. Per-target counting is a v0.2
  concern alongside DWARF-informed decision reconstruction; counting
  each target requires reconstructing which arm was taken from the
  selector, which materially complicates the rewrite without
  information DWARF-in-Wasm will give us cheaply in v0.2.
- **Harness model.** v0.1 ships the wasmtime-embedded runner only;
  subprocess-harness mode (`--harness <cmd>`) is deferred to v0.2 for
  modules that need a richer runtime.

### Research briefings

- `docs/research/rivet-template-mapping.md` — mapping of rivet's CI,
  lint, and release patterns adapted to witness's single-crate scope.
- `docs/research/overdo-alignment.md` — alignment brief extracting
  design constraints C1–C7 from the *Overdoing the verification chain*
  blog post the project's AGENTS.md cites.
