# witness — 10-minute quickstart

This is what I wish I had on hand when I tried `witness` for the
first time. It assumes macOS arm64 and a working Rust toolchain
with `wasm32-unknown-unknown` installed.

## What is witness?

`witness` is a CLI that instruments a WebAssembly module, runs it
under embedded `wasmtime` (or a custom harness), and reconstructs
**MC/DC-style branch coverage** from the recorded counter snapshot.
Output: a per-decision truth table, a signed in-toto attestation,
and an LCOV file for codecov.

If you do not know what MC/DC is: think "every condition in a
compound boolean has been independently shown to flip the outcome."
DO-178C signs off on it. `witness` reconstructs MC/DC from runtime
counters without rebuilding your code under a coverage compiler.

## 1. Install

Grab the release tarball for your platform from
https://github.com/pulseengine/witness/releases.

```bash
mkdir -p /tmp/witness-eval && cd /tmp/witness-eval
gh release download v0.9.11 --repo pulseengine/witness \
  --pattern '*aarch64-apple-darwin.tar.gz'
tar -xzf witness-v0.9.11-aarch64-apple-darwin.tar.gz
export PATH="$PWD:$PATH"
witness --version       # witness 0.9.11
witness-viz --version   # witness-viz 0.9.11
```

Both binaries ship in the same tarball. `witness viz` spawns
`witness-viz`, so they need to live next to each other on `$PATH`,
or you set `WITNESS_VIZ_BIN=/abs/path/to/witness-viz`.

The arm64-darwin binary is unsigned — Gatekeeper may quarantine it
on first launch. Run `xattr -d com.apple.quarantine witness
witness-viz` if the OS refuses to execute.

## 2. Scaffold a fixture

```bash
witness new my-fixture --dir /tmp/witness-eval
cd /tmp/witness-eval/my-fixture
./build.sh   # cargo build --release --target wasm32-unknown-unknown
./run.sh     # instrument + run + report + bundle
```

`witness new` writes a tiny `no_std` Rust crate that compiles to
core wasm. The fixture exports a single `is_leap(year: i32) -> i32`
function and drives it from `run.sh` via `--invoke-with-args`
(typed-arg form, v0.9.6+). The runtime input flows through the wasm
parameter rather than `core::hint::black_box`, so DWARF line
attribution lands on the predicate's source line.

You should see:

```
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

wrote run.json (...)
Bundle written under verdict-evidence/. Browse with:
  witness viz --reports-dir verdict-evidence
```

One note on the output:

- The leap-year decision has **three** boolean conditions in the
  source, but witness reports **two**. `witness new --help` warns
  about this: rustc fuses the third (`% 400 == 0`) into the same
  `br_if` chain as the first two. That's expected — the truth table
  is still complete.

## 3. Modify the code, re-run

Edit `src/lib.rs` to widen the predicate or add another row in
`run.sh`. The fixture's typed-args style means **no `run.sh` edit
is needed for new test years — just add another
`--invoke-with-args 'is_leap:1996'` line:

```bash
# in run.sh, append --invoke-with-args 'is_leap:1996'
./run.sh
```

If you DO add a new export (e.g. a different predicate), remember
to invoke it explicitly. `witness run` only calls exports you list
on the command line; new exports are silently ignored. There is no
auto-discovery flag yet (planned for v0.10.x).

The typed-args form (v0.9.6+, default in v0.9.11+ scaffolds)
eliminates the `core::hint::black_box` workaround older fixtures
needed. Syntax: `'export_name:value[,value,...]'`. The type comes
from the export's wasm signature via `func.ty()`; do not annotate
it in the spec (`'is_leap:i32=2024'` is wrong; `'is_leap:2024'` is
right).

## 4. Sign the evidence

```bash
witness instrument verdict_my_fixture.wasm -o inst.wasm
witness run inst.wasm \
    --invoke run_row_0 --invoke run_row_1 --invoke run_row_2 \
    --invoke run_row_3 --invoke run_row_4 -o run.json

witness predicate --run run.json --module inst.wasm -o predicate.json
witness keygen    --secret sk --public pk
witness attest    --predicate predicate.json --secret-key sk -o envelope.json
witness verify    --envelope envelope.json --public-key pk
# OK — DSSE envelope envelope.json verifies against pk
#   predicate type: https://pulseengine.eu/witness-coverage/v1
#   subjects: 1
```

`instrument`, `run`, `predicate`, and `attest` print nothing on
v0.9.11+: every command prints what it wrote on success. Older
versions had `instrument` / `run` / `predicate` / `attest` silent
while `keygen` / `verify` were chatty — that asymmetry is fixed.

The DSSE envelope is sigstore/cosign-compatible. Hand it to your
release artifact pipeline.

## 5. Visualise a compliance bundle

In v0.9.11+ the scaffold's `run.sh` writes a `verdict-evidence/`
layout automatically. Just point `witness viz` at it:

```bash
witness viz --reports-dir verdict-evidence
# witness-viz listening on http://127.0.0.1:3037
```

(In pre-v0.9.11 the layouts didn't match — you can still convert by
hand: `mkdir -p evidence/<name>` then
`witness report --input run.json --format mcdc-json > evidence/<name>/report.json`
and `cp instrumented.wasm.witness.json evidence/<name>/manifest.json`.)

Open http://127.0.0.1:3037 to see the dashboard with per-verdict
progress bars. Click a verdict for the decision list, click a
decision for the truth-table widget, click `view gap` on any
non-proved condition for the **gap drill-down view** — tutorial-
style explanation of the row vector you'd need to close MC/DC for
that condition, plus a copy-paste Rust `#[test]` stub.

There's also a spec-compliant MCP endpoint at `/mcp` exposing three
tools to agents: `get_decision_truth_table`, `find_missing_witness`,
`list_uncovered_conditions`. The full handshake (initialize →
tools/list → tools/call) works in v0.9.11+; pre-v0.9.11 lacked the
`initialize` handler. Quick smoke:

```bash
# initialize
curl -s http://127.0.0.1:3037/mcp -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize",
       "params":{"protocolVersion":"2025-06-18","capabilities":{},
                 "clientInfo":{"name":"curl","version":"0"}}}'

# list tools
curl -s http://127.0.0.1:3037/mcp -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
```

The MCP surface is a strict subset of the HTTP API surface — humans
reviewing a PR see exactly what an agent saw.

## 6. LCOV for codecov

```bash
witness lcov --run run.json --manifest instrumented.wasm.witness.json \
  -o lcov.info
```

In v0.9.11+ the scaffold uses `--invoke-with-args` by default, so
LCOV `BRDA` records are attributed to your source file. Older
fixtures using `core::hint::black_box` wrappers attribute to
`hint.rs`; switch to typed-args to fix.

## What's missing from this guide

- Custom harness mode (`witness run --harness ...`) — the README's
  "Harness-mode protocol" section is the right reference. Minimum
  viable harness is ~10 lines; just write a JSON snapshot to
  `$WITNESS_OUTPUT` matching `{"schema":"witness-harness-v1",
  "counters":{"<id>":<u64>,...}}`.
- `witness merge` for multi-run aggregation.
- `witness diff` for PR-shaped coverage deltas.
