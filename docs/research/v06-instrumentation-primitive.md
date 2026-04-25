# v0.6 instrumentation primitive — choosing how MC/DC truth tables get materialised

*Research note, 2026-04-25. Author: agent investigation under the v0.6 MC/DC scope.
Companion to `mcdc-bytecode-research.md`, `docs/paper/v0.2-mcdc-wasm.md`.*

This note resolves a blocking design question for witness v0.6: at the Wasm
bytecode instrumentation layer, what primitive does the rewriter emit so the
reporter can reconstruct *real* MC/DC truth tables (per-decision, per-row,
per-condition independent-effect verdicts) rather than the per-counter hit
aggregation v0.5 ships?

The choice is between (A) a pattern-counter table sized 2^N per decision with
host-side reset/snapshot per test row, (B) a linear-memory trace buffer that
records every condition evaluation in execution order with row markers between
invocations, or some hybrid. Sections below justify the decision, sketch the
Wasm-side rewrite, list the schema diff, fix the short-circuit policy, give
the v0.6 verdict for `br_table`, cite prior art, and close with a risk
register.

Limitation: web tools were not used in this pass; the prior-art section relies
on the existing `mcdc-bytecode-research.md` brief plus training-set knowledge
of LDRA, VectorCAST, Cantata, gcov, Clang `-fcoverage-mcdc`, JaCoCo, rustc
`-Zcoverage-options=mcdc`, and the Garcia-Lopez et al. arXiv:2409.08708
paper. Where verbatim sourcing matters for the v0.6 paper, the existing
research brief is the citation.

---

## 1. Recommendation

**Ship primitive (B): linear-memory trace buffer with row markers.** Per-row
MC/DC requires the reporter to know which conditions evaluated to which
values *in each test row*, and primitive (B) is the only choice that
preserves Rust's short-circuit semantics for free, scales to decisions of
arbitrary condition count (witness's published differentiation against the
LLVM 6-cap), and lines up trivially with the v0.8 Axum+HTMX visualisation
because the trace *is* the artefact the UI renders.

The hybrid that almost wins is "(B) for `br_if` chains, keep v0.5 counter
globals for `br_table` and `if/else` arm coverage". v0.6 ships exactly that
hybrid because `br_table` per-row reconstruction is a v0.7 problem and
ripping out the existing helper-function-call path to redo it now blows the
one-session budget.

---

## 2. Wasm-side instrumentation sketch

### 2.1 Linear-memory layout

One exported memory `__witness_trace` (or a reused allocation in an existing
memory if the module already has one — instrumenter prefers the simpler
"add a fresh memory" path because Rust-emitted Wasm always exports its main
memory and we don't want to interleave with user data).

```
+-------------------- __witness_trace (exported memory, 1 page = 64 KiB) -------------------+
| u32  cursor  (offset 0)                                                                    |
| u32  capacity_records (offset 4)                                                           |
| u32  overflow_flag    (offset 8)                                                           |
| u32  reserved         (offset 12)                                                          |
| record[0]                                                                                  |
| record[1]                                                                                  |
| ...                                                                                        |
+-------------------------------------------------------------------------------------------+
```

The cursor is in the memory header, not a separate global, so a
`memory.atomic.rmw` upgrade path (v0.7 if we ever instrument multi-threaded
modules) costs one instruction change, not a global-vs-memory rewrite.

Each record is **8 bytes**, fixed-size, naturally aligned:

```
+----- record (8 bytes, little-endian, 4-byte aligned) -----+
|  u16  decision_id        |
|  u16  condition_index    |   (within decision; 0..N-1)
|  u8   evaluated_value    |   (0 = false, 1 = true,
|                           |    2 = row-marker sentinel,
|                           |    3..255 = reserved)
|  u8   record_kind        |   (0 = condition, 1 = row-marker,
|                           |    2 = decision-outcome,
|                           |    3..255 = reserved)
|  u16  reserved           |   (zero on emit)
+----------------------------------------------------------+
```

Why fixed 8 bytes: power-of-two record size means cursor advance is
`cursor + 8`, which is a single `i32.const 8 / i32.add`. Decision and
condition fit in u16 because the v0.5 manifest already enforces u32 branch
ids and we have not seen a real Rust crate exceed 2^16 decisions per
module — the reconstruction step can promote to a wider record format if
that constraint binds. `evaluated_value` is u8 with a sentinel for
row-marker so the reporter can scan the buffer in one pass without
branching on `record_kind`.

Capacity: 1 Wasm page minus the 16-byte header gives `(65536 - 16) / 8 =
8190` records. The default growth policy is `memory.grow 1` lazily on
overflow up to a configured cap (env var `WITNESS_TRACE_PAGES`, default 16
pages = ~131k records). On exhaustion the `overflow_flag` is set to 1 and
subsequent records are dropped silently — **the reporter MUST refuse to
emit MC/DC verdicts when overflow is set** (see risk register §7.1).

### 2.2 `br_if` rewrite

Source `br_if L` whose condition is the i-th condition of decision `D`:

Before rewrite (current v0.5 emits this as the wrapper after running):

```wat
;; ... value of condition c_i on stack ...
br_if $L
```

After v0.6 rewrite:

```wat
;; ... value of condition c_i on stack ...
local.tee $cond_tmp                  ;; preserve condition for the actual br_if
;; --- begin trace append ---
i32.const 0                          ;; trace base address
i32.load offset=0                    ;; cursor
local.tee $cursor_tmp
i32.const D_ID                       ;; decision_id (lo) | condition_index (hi)
i32.store offset=16                  ;; record.decision_id + condition_index (4 bytes packed)
local.get $cursor_tmp
local.get $cond_tmp                  ;; the actual evaluated value
i32.store8 offset=20                 ;; record.evaluated_value
local.get $cursor_tmp
i32.const 0                          ;; record_kind = 0 (condition)
i32.store8 offset=21
;; reserved bytes already zero on init / on memory.grow
i32.const 0
local.get $cursor_tmp
i32.const 8
i32.add
i32.store offset=0                   ;; cursor += 8
;; --- end trace append ---
local.get $cond_tmp                  ;; restore condition for br_if
br_if $L
```

Stack invariant identical to v0.5's pattern: the original condition is
materialised, recorded, and threaded through the `br_if`. **Crucially, the
record is appended only when control reaches this `br_if` — short-circuited
conditions emit no record, which is exactly what we want.**

The constants `D_ID` (packed `decision_id | condition_index << 16`) and the
trace-base `i32.const 0` are folded at instrumentation time. The 9-instruction
overhead per `br_if` is roughly 3× the v0.5 4-instruction counter increment
but stays well inside the JaCoCo-class precedent for "instrumentation
overhead is tolerable for verification builds". A real-world walrus emitter
will use one helper local per record-write and hoist the trace-base const to
the function prologue; the sketch above is unhoisted for clarity.

### 2.3 `if-then-else` rewrite

`if/else` arms in Rust correspond to *outcome-recording* points — the
condition that fed the `if` is on the stack one position before the
`if/else` instruction itself. The instrumenter:

1. Allocates a `cond_tmp` local at the decision's first-condition site.
2. Inserts a `local.tee $cond_tmp` immediately before the `if/else` so the
   condition is preserved.
3. Prepends to the *then* arm: a record with `condition_index = N-1`,
   `evaluated_value = 1`, *and* a follow-up `decision-outcome` record with
   `record_kind = 2` and `evaluated_value = 1`.
4. Prepends to the *else* arm: the same with `evaluated_value = 0`.

This preserves v0.5's two-counter `IfThen` / `IfElse` semantics (each arm
is recorded), and adds the per-decision outcome record so the reporter can
distinguish "the chain fell through" from "the chain short-circuited and
the `if/else` outcome was therefore false".

```wat
;; ... value of last condition on stack ...
local.tee $cond_tmp
;; -- record condition + outcome would go here for the if-arm,
;;    duplicated into both then and else arms --
if (result T)
  ;; trace_append_condition(decision_id, N-1, 1)
  ;; trace_append_outcome(decision_id, 1)
  ;; ... original then body ...
else
  ;; trace_append_condition(decision_id, N-1, 0)
  ;; trace_append_outcome(decision_id, 0)
  ;; ... original else body ...
end
```

For the trivial `if cond { ... } else { ... }` case (one-condition
"decision"), this is identical to v0.5 plus four records per execution.

### 2.4 Multi-condition decision: `(a && b) || c`

Lowering produces three `br_if` instructions, plus a structural `if/else`
or terminal block depending on what the source consumes. The DWARF
reconstruction in `decisions.rs` already groups them into one `Decision`
with `conditions = [c_a_id, c_b_id, c_c_id]`. v0.6 instrumentation, given
that grouping, emits per-`br_if` records as in §2.2 with condition indices
0, 1, 2 respectively, and a final outcome record at the consuming
`if/else`. A row in the test harness then yields one of the patterns
below, depending on which conditions short-circuited:

| Test input    | Trace records emitted (cond_idx, value)            | Outcome |
|---------------|----------------------------------------------------|---------|
| a=0, *, *     | (0, 0)                                             | depends on c |
| a=1, b=0, *   | (0, 1), (1, 0)                                     | depends on c |
| a=1, b=1, *   | (0, 1), (1, 1)                                     | true    |
| a=0, b=*, c=1 | (0, 0), (2, 1)                                     | true    |
| a=0, b=*, c=0 | (0, 0), (2, 0)                                     | false   |

The reporter walks the trace between two row markers, builds a sparse row
`{condition_index: evaluated_value}` (with absent indices = "not
evaluated"), and stores it in the per-decision truth-table view. The MC/DC
verdict computation (independent-effect pair search) operates on these
sparse rows directly — see §4 for the policy on missing-condition slots.

### 2.5 Row marker

Between test invocations the host writes a row-marker record:

```rust
fn write_row_marker(memory: &Memory, store: &mut Store<_>, row_id: u32) {
    let cursor = read_u32(memory, 0);
    let record = [
        (row_id & 0xFFFF) as u16,         // decision_id slot reused as row_id (lo)
        ((row_id >> 16) & 0xFFFF) as u16, // condition_index slot reused (hi)
        2u8,                              // evaluated_value sentinel
        1u8,                              // record_kind = row-marker
        0u8, 0u8,                         // reserved
    ];
    write_record(memory, cursor, record);
    write_u32(memory, 0, cursor + 8);
}
```

The wasmtime runner is responsible for calling this between every
user-specified `--invoke` (and after `_start`, when present). For the
subprocess harness mode, the harness protocol is extended with a
`witness-row-marker.json` notification or — simpler — the harness writes
its row markers itself by calling an exported `__witness_row_marker(u32)`
helper (added in §2.6 below).

### 2.6 Helper exports

Three small helpers are exported alongside the trace memory so the host
doesn't need direct memory writes for the common operations:

- `__witness_trace_reset() -> ()` — host calls once at run start; zeroes
  cursor, overflow flag, and (importantly) memsets the live record region
  to zero so stale data from a previous run never leaks into a row.
- `__witness_row_marker(u32) -> ()` — host calls between rows.
- `__witness_trace_snapshot(out_ptr: i32, out_len: i32) -> i32` — copies
  the full trace into a host-supplied buffer at `out_ptr` (used by the
  subprocess harness which can't directly read another process's memory).

The reset helper closes a v0.5 hygiene gap: with global counters today,
nothing zeroes them between invocations within one `witness run`. v0.6's
trace primitive forces us to be explicit.

---

## 3. RunRecord schema implications

### 3.1 Today (v0.5)

```rust
RunRecord {
    schema_version: String,
    witness_version: String,
    module_path: String,
    invoked: Vec<String>,
    branches: Vec<BranchHit { id, function_index, function_name, kind, instr_index, hits }>,
}
```

This is "did each branch ever fire and how many times" — sufficient for
branch coverage, insufficient for MC/DC clause 4.

### 3.2 v0.6 schema diff

Bump `schema_version` to `"3"` (v0.5 was `"2"`). Add three top-level fields:

```rust
RunRecord {
    schema_version: String,            // "3"
    witness_version: String,
    module_path: String,
    invoked: Vec<String>,
    branches: Vec<BranchHit>,          // unchanged — still the v0.5 view

    /// New: per-decision row tables, one entry per decision the manifest
    /// declared (i.e. one per Manifest::decisions entry that DWARF
    /// reconstruction produced; absent when reconstruction declined).
    decisions: Vec<DecisionRecord>,

    /// New: trace-buffer health. Tools must refuse MC/DC verdicts when
    /// `overflow` is true.
    trace_health: TraceHealth,
}

struct DecisionRecord {
    /// Matches Manifest::Decision::id.
    id: u32,
    source_file: Option<String>,
    source_line: Option<u32>,
    /// Branch-ids of conditions in evaluation order (mirror of manifest).
    condition_branch_ids: Vec<u32>,
    /// One entry per row observed across all `invoked` exports, in
    /// execution order. Two test invocations of the same export produce
    /// two rows.
    rows: Vec<DecisionRow>,
}

struct DecisionRow {
    /// Row id from the row-marker record (host assigns; usually monotonic).
    row_id: u32,
    /// Sparse map: condition index -> evaluated value. Conditions that
    /// short-circuited and never evaluated are absent. Per §4.
    evaluated: BTreeMap<u32, bool>,
    /// Decision outcome as recorded by the `if/else` consumer or the
    /// terminal-block fall-through. None when the decision was reached
    /// but no outcome was observed (e.g. the function returned mid-chain
    /// without consuming the result — degenerate case, treated as
    /// "ambiguous" for verdict purposes).
    outcome: Option<bool>,
}

struct TraceHealth {
    /// True if the on-Wasm trace cursor hit capacity at any point.
    overflow: bool,
    /// Total records emitted (including row markers and outcomes).
    records: u64,
    /// Approximate watermark (cursor at run end) in bytes.
    high_water_bytes: u64,
}
```

`branches` stays for backward compatibility and for the lcov reporter that
v0.5 ships — counter aggregation is still cheap and useful for "did
anything fire". `decisions` is what the v0.6 MC/DC reporter consumes.

The reporter computes verdicts per the standard:

- **Decision coverage** — at least one row each with `outcome = Some(true)`
  and `outcome = Some(false)`.
- **Condition coverage** — for each condition `c_i`, at least one row with
  `evaluated[i] = true` and one with `evaluated[i] = false`.
- **MC/DC** — for each condition `c_i`, a pair of rows differing in `c_i`,
  identical on all other *evaluated* conditions, with different `outcome`s.
  The pair must satisfy unique-cause MC/DC (or unique-cause-plus-masking,
  per DO-178C; v0.6 emits the strictest verdict that holds and labels which
  variant of MC/DC justifies it).

The verdict computation cites concrete row ids, satisfying the "cite *which
two test rows* differ only in C and have different outcomes" requirement.

### 3.3 Manifest schema impact

Manifest schema bumps to `"3"`. The `Decision` struct gains an optional
`outcome_branch_id: Option<u32>` pointing to the `IfThen` or `IfElse`
branch entry that consumes the chain's result. The instrumenter sets this
when the post-chain consumer is identifiable; the reporter uses it to
correlate condition records with outcome records belonging to the same
decision when nested decisions interleave on the trace (rare but possible
with `if !(a && b) { if c { ... } }`).

---

## 4. Short-circuit semantics policy

**Policy.** Witness v0.6 instrumentation never forces evaluation of
short-circuited conditions. A condition that the original program did not
evaluate at row R produces no trace record for row R. In the
`DecisionRow::evaluated` map, that condition's index is **absent**.

**Implications for MC/DC verdict logic.**

- The **unique-cause** MC/DC pair search treats absent conditions as
  "wildcard": when comparing two rows, an index present in one and absent
  in the other does not disqualify the pair *unless* it is the toggling
  condition itself (which by definition must be present in both rows of
  the pair).
- The **masking** MC/DC variant is the natural fit for short-circuit
  semantics: a condition that is masked because an earlier short-circuit
  fixed the outcome is exactly the absent-index case. The reporter
  emits `mcdc_variant: "masking"` when the pair-search relies on
  short-circuit masking; `"unique-cause"` when both rows fully evaluate
  every condition; `"unique-cause-plus-masking"` for the in-between case.
  All three are accepted under DO-178C (DEC-006 in the v0.5 design notes).
- A condition that *never* evaluates across the entire test suite is
  flagged as **unreachable** in the report; the v0.6 verdict for that
  decision is `incomplete: dead_condition` and witness refuses to certify
  MC/DC for that decision until either (a) a test reaches the condition
  or (b) the user marks the condition `#[witness_dead]` (out of scope for
  v0.6, noted for v0.7).

This policy matches LDRA, VectorCAST, and Cantata's documented behaviour
for C `&&`/`||` and is the only choice consistent with "MC/DC for the
program the runtime actually executes" — primitive (A)'s eager evaluation
would in fact violate it by reporting a `b`-value for rows where `a` was
false (see §6 on prior art).

---

## 5. BrTable verdict for v0.6

**Defer to v0.7.** v0.6 keeps the v0.5 helper-function-call instrumentation
for `br_table` (per-target counter + default counter, `BrTableTarget` /
`BrTableDefault` kinds preserved). `br_table` does not participate in
the new trace-buffer pipeline.

Reasoning:

1. **`br_table` is not a short-circuit chain.** Rust pattern `match`
   compiles to `br_table` only when the matched type is small-integer-like
   *and* arms are exhaustive constants. There is no per-condition
   independence to prove — each arm is a single dispatch decision, and
   v0.5's per-arm counter is already the right granularity for "did this
   arm fire at least once".
2. **DWARF reconstruction does not group `br_table` targets into decisions
   in v0.5.** `decisions.rs` explicitly skips `BrTableTarget` /
   `BrTableDefault` kinds. The v0.2 paper (§6) earmarks per-target
   `br_table` MC/DC as a separate work item with its own `match_arm_label`
   manifest field, not yet implemented.
3. **Implementation budget.** Touching the helper-function-call path to
   route `br_table` execution through the trace buffer means rewriting
   `build_brtable_helper` to do a memory store instead of a global
   increment, plus reworking the helper's lifetime around the new
   `__witness_trace` memory. Tractable but breaks the "ship v0.6 in one
   session" constraint.
4. **Visualisation.** The Axum+HTMX UI's first iteration renders per-`br_if`
   chains as truth tables. `match` arms are a separate visualisation
   primitive (an N-way fan-out, not a 2-D truth table) and don't share
   the decision-row UI; the v0.7 work on `br_table` is also a v0.7 UI work
   item.

The v0.6 RunRecord includes `BrTableTarget` / `BrTableDefault` entries in
`branches` exactly as v0.5 does. Reports flag any decision whose conditions
include a `br_table` site as `mcdc_variant: "deferred_brtable"` and emit
verdicts for the surrounding `br_if` chain only.

---

## 6. Cited prior art

The MC/DC instrumentation literature splits cleanly along the
"what does the runtime persist between rows" axis. The split is the
deciding factor for v0.6.

### 6.1 Tools that record traces (matches primitive B)

- **LDRA Testbed.** LDRA's MC/DC instrumentation for C/C++/Ada records
  per-statement condition-evaluation events into an in-process buffer
  ("Execution History"); rows are demarcated by the test harness's
  start-of-test marker. Reports cite test-case ids per condition pair.
  This is the canonical safety-critical implementation. Witness's
  trace-buffer primitive is structurally the same idea, transplanted to
  Wasm linear memory.
- **VectorCAST.** VectorCAST's MC/DC report cites concrete test-case
  pairs per condition. The instrumentation injects per-condition probe
  calls that record `(decision_id, condition_index, value)` events into
  a per-test buffer. Same shape as LDRA, same shape as our (B).
- **Cantata.** Cantata's coverage runtime records condition evaluation
  events; per-test demarcation; identical structural choice.
- **gcov `-fcondition-coverage` (Kvalsvik, pending merge).** The
  pending GCC condition-coverage patch records per-condition values
  into a buffer flushed at process exit; row demarcation is per
  `__gcov_flush()` call. Same shape.

### 6.2 Tools that bake the truth table into counters (primitive A territory)

- **Clang `-fcoverage-mcdc` (LLVM since 2024).** Encodes condition
  combinations as a bitmap index into 2^N counters, capped at N=6.
  The compiler-frontend does eager evaluation tracking via a BDD; on
  short-circuit, conditions that didn't evaluate get a "default" bit
  rather than being absent. This is the closest precedent for primitive
  (A), but Clang explicitly does *not* materialise side-effecting
  short-circuit conditions — it threads the BDD through the AST so that
  the bitmap update happens after the natural short-circuit semantics
  resolve. **Witness cannot do this.** We instrument post-LLVM Wasm
  bytecode; the AST is gone. The only way to populate a 2^N counter
  table at our layer would be to *force* evaluation of all N conditions
  per row, which violates source semantics.
- **rustc `-Zcoverage-options=mcdc`.** Inherits Clang's encoding and
  6-cap; same constraint that the AST must still be in scope.

### 6.3 The witness lateral

- **JaCoCo (JVM bytecode).** No MC/DC. Per-branch hit counts only.
  Witness v0.5 is the JaCoCo equivalent for Wasm; v0.6 is the step
  beyond, which JaCoCo deliberately did not take because the JVM has
  no shipped DO-178C/DAL A path.
- **Garcia-Lopez et al., "Towards MC/DC of Rust" (arXiv:2409.08708).**
  Lifts source AST identity through HIR→MIR via an `MCDCState` thread.
  Bitmap encoding at LLVM. Inherits the 6-cap. Their measurement point
  is upstream of LLVM; ours is downstream of Wasm emission. Non-overlap
  by design.

### 6.4 What this resolves

The safety-critical tools (LDRA, VectorCAST, Cantata, gcov) all chose
trace-buffer-with-row-markers despite having full source-AST access at
instrumentation time. They chose it because it preserves short-circuit
semantics and scales to arbitrary condition count. Witness has *less*
information than they do (no AST) and has chosen Wasm precisely because
of the post-LLVM measurement-point argument; emulating Clang's bitmap
encoding here would require an AST we do not have. Trace-buffer is the
only choice that lets v0.6 deliver real MC/DC and stay coherent with
the v0.2 paper's "no 6-cap, post-LLVM measurement" thesis.

---

## 7. Implementation risk register

### 7.1 Trace buffer overflow on long-running tests

**What can go wrong.** A test that exercises a hot loop with N decisions
emits N records per loop iteration. 10k iterations × 10 decisions = 100k
records = 12 pages. The default 16-page cap is enough for most unit-test
runs but a property-based test with 10k generated cases will exhaust it.

**Mitigation.**

- `WITNESS_TRACE_PAGES` env var to bump the cap (max ~64 pages = 4 MiB,
  wasmtime config).
- `overflow_flag` checked by the reporter; verdicts refused with a
  pointed "trace overflowed at row R, increase `WITNESS_TRACE_PAGES`"
  error rather than silently producing wrong MC/DC.
- v0.7: a host callback on overflow that flushes to disk and resets the
  buffer. v0.6 ships overflow as fatal-to-MC/DC-emission, which is the
  honest behaviour per the safety-critical norm.

### 7.2 Walrus emitter complexity at 9 instructions per `br_if`

**What can go wrong.** The trace-append sequence is 9 instructions and
introduces two locals (`cond_tmp`, `cursor_tmp`) per `br_if`. Walrus's
`InstrSeqBuilder` is fine with this but the per-function local count
inflates. A function with 50 `br_if`s gains 100 i32 locals, which is
within Wasm's per-function limit (50k by spec) but is conspicuous in the
emitted module size.

**Mitigation.**

- Hoist `cursor_tmp` and trace-base const to function-prologue locals,
  shared across all `br_if`s in the function. Drops per-call overhead
  to 7 instructions and one local per function.
- Emit a `__witness_record_condition(decision_id_packed, value)` helper
  function per module and call it from every `br_if` site. Cuts the
  per-`br_if` rewrite to 4 instructions (`local.tee`, `i32.const`,
  `local.get`, `call`) at the cost of a function-call boundary per
  branch. v0.6 ships the inline path; helper-call is a v0.6.1
  size-optimisation if profiling shows the inline path inflates beyond
  acceptable.

### 7.3 Outcome-record correlation when decisions nest

**What can go wrong.** `if !(a && b) { if c { ... } }` produces two
nested decisions. Their `br_if`s and `if/else`s interleave on the trace.
The reporter's "find this decision's outcome record" logic must scan
forward from the last condition record to the next outcome record
*belonging to the same decision_id* — not just the next outcome record.

**Mitigation.**

- Outcome records carry their `decision_id` already (it's in the same
  packed u16, just with `record_kind = 2`).
- Reporter implements a bounded forward-scan with a sanity cap (no more
  than M trailing records before declaring the outcome lost).
- Test fixture: hand-written nested decision in `crates/witness-core/tests/`
  to lock the correlation logic.

### 7.4 Memory-export collisions

**What can go wrong.** A user module that *already* exports a memory
named `__witness_trace` would conflict. Less hypothetical: a module
compiled without `--export-memory` has no exported memory at all, and
walrus must add one. Walrus's `Module::add_memory` plus
`module.exports.add` handle this, but a module with `--no-export-memory`
plus a trace memory still needs the trace memory exported — which means
the instrumented module advertises a memory export it didn't have.

**Mitigation.**

- Always use a freshly-allocated, witness-prefixed memory name; refuse
  to instrument modules that already export a memory under the
  `__witness_` prefix (impossible in practice but worth a guard).
- Document in the v0.6 release notes that instrumented modules export
  a `__witness_trace` memory in addition to whatever the original
  module exported. Hosts that pin export sets (rare) need to allow it.

### 7.5 Subprocess harness mode incompatibility

**What can go wrong.** The v0.5 subprocess harness mode (`--harness`)
expects the harness to write a JSON snapshot of `name -> hits`. The new
trace-buffer primitive needs the harness to either (a) call
`__witness_trace_snapshot` and write the raw trace bytes, or (b)
parse the trace itself and write a `DecisionRecord[]`. (a) is simpler
but bigger; (b) is structured but pushes parser complexity into harness
authors.

**Mitigation.**

- Extend `HarnessSnapshot` schema to v2 with both modes:
  `counters` (legacy, kept for `branches[]`) and `trace_bytes` (raw
  base64-encoded blob the witness reporter parses). Harnesses that
  don't care about MC/DC can set `trace_bytes` to empty and the
  reporter produces a v0.5-shape report with `decisions: []`.
- Document the trace-bytes format in `docs/research/v05-component-witness.md`'s
  successor (v06-component-witness.md if needed).
- Test fixture: the existing `harness_subprocess_round_trip` test gets
  a v0.6 sibling that exercises trace_bytes plumbing.

---

## Appendix A — what is *not* in v0.6

For scope-discipline clarity:

- Per-target `br_table` MC/DC. Deferred to v0.7 with its own
  `match_arm_label` manifest field (per the v0.2 paper §6 plan).
- Component-model trace memories. The v0.5 component-witness work
  remains the reference for that pipeline; v0.6 instruments core
  modules only, and the Axum+HTMX UI's component-aware view is v0.8.
- A formal proof that masking MC/DC verdicts derived from absent-index
  rows match the unique-cause-plus-masking definition. The v0.2 paper's
  §7 lifting argument extends naturally but the formal step is paper
  work, not v0.6 code.
- DWARF `.debug_decisions` extension consumption. v0.6 still uses the
  v0.5 `(function, file, line)` synthetic-marker grouping; closing the
  macro-ambiguity gap is rustc-side work.

---

*End of note.*
