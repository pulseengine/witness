# witness

**MC/DC-style branch coverage for WebAssembly components.**

witness instruments a Wasm module with branch counters, runs a test harness
against it, and emits a coverage report you can read, compare, or feed into
[rivet](https://github.com/pulseengine/rivet) as requirement-to-test evidence
and into [sigil](https://github.com/pulseengine/sigil) as an in-toto coverage
predicate.

> **New here?** [`docs/quickstart.md`](docs/quickstart.md) walks the install
> through first MC/DC truth table in 10 minutes. [`docs/concepts.md`](docs/concepts.md)
> defines every term used in this README — *MC/DC, masking, unique-cause,
> br_if, post-codegen, polarity inversion, DSSE envelope, in-toto* — with
> a worked leap-year example. New users should start there.

## TL;DR — what's MC/DC, and why care?

"Did the test exercise this `if`?" is the wrong question. **Modified
Condition / Decision Coverage** is the right one: for each condition
in a branch — say `a && b && c` — at least one test must flip *just
that condition* and demonstrate that the flip changes the branch's
outcome. Three conditions → at least three meaningful tests, not one
"happy path" that covers the whole expression by accident.

The aviation industry (DO-178C Level A), automotive (ISO 26262 ASIL D),
and medical-device software (IEC 62304 Class C) require MC/DC because
line-coverage doesn't catch the failures that kill people: short-
circuit operands that never get evaluated, fused conditions where one
flip masks another, dead arms the optimiser left in. MC/DC forces
your test corpus to actually distinguish each condition.

**Why care if you're not building a plane?** Same reason regulated
industries demand it: MC/DC tells you whether your tests *would catch*
a bug, not just whether they *touched* the code. It's a sharper
signal than line-coverage at low cost — a few extra tests per
predicate. And once you have it, you can refuse to ship a PR whose
new conditions aren't proved.

Witness measures MC/DC **after rustc + LLVM finish lowering** — on the
actual Wasm the runtime executes. Same DO-178C *"post-preprocessor C"*
precedent, applied to *"post-rustc Wasm"*. See the blog posts below
for the long argument.

### Is this for you?

Witness is **for** you if any of these match: you ship a Wasm module
into a regulated context (avionics, medical, automotive); you want
to know which match arms / branches your test corpus actually
exercises in the form the runtime executes; you want a signed
coverage envelope an auditor can trust; you want an MCP-callable
tool surface so AI agents can close gaps end to end.

Witness is **probably not** for you if you want line/statement
coverage on idiomatic Rust code in a non-regulated context (use
[cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) or
[tarpaulin](https://github.com/xd009642/tarpaulin) instead — both
do a great job and witness deliberately doesn't try to replace them).

The argument for why this tool exists lives in two blog posts:

- [Spec-driven development is half the loop](https://pulseengine.eu/blog/spec-driven-development-is-half-the-loop/)
- [MC/DC for AI-authored Rust is tractable — the variant-pruning argument](https://pulseengine.eu/blog/variant-pruning-rust-mcdc/)

Short version: pattern-matching, `?` desugaring, and cfg expansion have all
resolved by the time a Wasm module exists. Coverage measured at the Wasm
level describes what actually ships, against an instruction set small enough
and formally specified enough that the tool-qualification story moves in your
direction. And the *"post-preprocessor C"* precedent — MC/DC measured on
expanded C rather than pre-expansion source, accepted by DO-178C since 1992 —
is structurally the same move as *"post-rustc Wasm"*.

## Status

Witness is pre-1.0 and ships frequent tagged releases. The
[**CHANGELOG**](CHANGELOG.md) is the source of truth for what's
in any given version — including newly probed languages, schema
revisions, and which fixtures count as Tier A. The
[**latest GitHub Release**](https://github.com/pulseengine/witness/releases/latest)
is the recommended pin for production use.

### What witness measures (and what it doesn't)

Witness counts branches **after rustc + LLVM finish lowering**. That
matters: rustc may *fuse* multiple source-level conditions into a
single `br_if` chain, eliminate dead arms, or constant-fold the
predicate to bitwise arithmetic when the inputs let it. The truth
tables you see are **the post-codegen reality**, not the source
shape. The scaffolded leap-year fixture is the canonical example:
`(year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)` is three
boolean conditions in source; rustc fuses the third into the same
chain, and witness reports two. It's correct — just measured at the
layer where the runtime actually executes. See
[docs/concepts.md](docs/concepts.md) §3-§4 for the worked example
and the polarity convention (the truth-table column `c0=T` records
the wasm `br_if` value, not the source-level condition value).

This is the same move DO-178C made for "post-preprocessor C" in
1992: measure what the compiler emits, not what the engineer typed.

### Stability contract

| Surface | Stability |
|---|---|
| Schema URLs (`witness-mcdc/v1`, `witness-mcdc/v2`, `witness-mcdc/v3`, `witness-coverage/v1`, …) | Stable per version path. Breaking changes bump the version segment; older schema URLs keep validating older predicates. |
| CLI flags + subcommands | Stable unless the CHANGELOG calls out an explicit deprecation in the entry that ships the change. |
| `witness-mcdc-checker` crate (qualifiable kernel) | Stable surface — deliberately kept tiny so it can be audited. |
| `RunRecord` / `Manifest` JSON shape | Stable; serde aliases preserved when fields are renamed (the CHANGELOG notes the alias and when it'll be removed). |
| Rust public API of `witness-core` and `witness-viz` | Use at your own risk until v1.0. Pre-1.0 bumps may change types. |
| MCP wire (`/mcp` JSON-RPC) | Stable. Major adoption changes (e.g. rmcp) gated on spec-feature need. |

v1.0 ships the **Check-It qualification artifact** — a small
qualified checker (the `witness-mcdc-checker` crate today)
validates witness output, audited under DO-330 instead of trying
to qualify the whole pipeline. Until v1.0, witness is positioned as
**supplementary evidence** in a qualification dossier, not primary.

Release discipline: every fix lands in a numbered release with
green CI, signed binaries, and a CHANGELOG entry. For production
use, pin to a specific tag and read the CHANGELOG before bumping.

### The reviewer experience

```
$ witness viz --reports-dir compliance/verdict-evidence/ --port 3037
witness-viz listening on http://127.0.0.1:3037
```

Open the browser to land on a dashboard with the headline numbers,
click a verdict to drill down, click a decision to see the **truth
table** — every row, every condition, gap rows red-bordered,
independent-effect pairs cited inline. Same data over JSON at
`/api/v1/*` and over MCP at `POST /mcp` with three agent-callable
tools (`get_decision_truth_table`, `find_missing_witness`,
`list_uncovered_conditions`). The MCP surface is a strict subset of
the HTTP surface — humans reviewing a PR see exactly what the agent
saw.

The differentiator: where LDRA, VectorCAST, RapiCover, Cantata,
Bullseye and gcov+gcovr ship percentages and gates, witness ships the
truth table — and an agent contract over the same surface. Every
gap-closing test the agent proposes is verifiable: re-run witness, see
the row appear, see the pair turn from `gap` to `proved`.

### Current verdict suite

```
verdict              branches  decisions full      proved  gap   dead   rows
leap_year            2         1         1/1       2       0     0      4
range_overlap        0         0         0/0       0       0     0      3   (optimised to bitwise)
triangle             2         1         1/1       2       0     0      4
state_guard          3         1         1/1       3       0     0      5
mixed_or_and         0         0         0/0       0       0     0      5   (optimised to bitwise)
safety_envelope      4         1         1/1       3       0     0      6
parser_dispatch      33        7         1/7       9       13    5      6
httparse             473       67        7/67      28      46    108    40
nom_numbers          20        3         3/3       6       0     0      28
state_machine        14        5         4/5       11      1     0      27
json_lite            165       29        2/29      26      31    33     28
TOTAL                716       115       21/115    90      91    146
```

**90 conditions proved across 715 br_ifs in real Rust code**, 21 full
MC/DC decisions, 146 dead conditions flagged with row-closure
recommendations the report emits inline. The four real-application
fixtures (httparse, nom_numbers, state_machine, json_lite) account
for 672/716 branches and 104/115 decisions. The seven canonical
fixtures (leap_year through parser_dispatch) provide hand-derivable
oracle truth tables for verifier confidence.

### Version history

See [**CHANGELOG.md**](CHANGELOG.md) for the per-version
"what changed" list — schema bumps, new probed languages,
clustering rule changes, bug fixes. The README intentionally
stays version-agnostic; pinning version detail here just
drifts.

See [DESIGN.md](DESIGN.md) for the roadmap.

Counter values are exposed as exported mutable globals named
`__witness_counter_<id>` (plus `__witness_brval_<id>` /
`__witness_brcnt_<id>` from v0.6.1+), not via a dump function — any
conformant Wasm runtime can read coverage with a two-line
`instance.get_global` call. No cooperation protocol with the
module-under-test is required.

## Show me the proof — verify a release in 60 seconds

Every v0.6.4+ release ships a `compliance-evidence.tar.gz` archive
containing the eleven verdict directories with end-to-end MC/DC
reports, DSSE-signed in-toto Statements per verdict, an ephemeral
public key, and a V-model traceability matrix. Verify it:

```sh
# 1. Download the compliance archive for the latest release.
gh release download v0.8.0 \
  --repo pulseengine/witness \
  --pattern '*compliance-evidence*'

# 2. Extract.
tar -xzf witness-v0.8.0-compliance-evidence.tar.gz

# 3. See the per-verdict scoreboard with proved/gap/dead totals.
cat compliance/verdict-evidence/SUMMARY.txt

# 4. Verify a signed predicate against the verifying key.
witness verify \
  --envelope compliance/verdict-evidence/httparse/signed.dsse.json \
  --public-key compliance/verdict-evidence/verifying-key.pub
```

The verify command prints `OK — DSSE envelope … verifies against …`
and exits zero. Tampering with the envelope or the public key fails
verification with a clear error and a non-zero exit. The `cosign
verify-blob` command works equivalently — the envelope is
standards-compliant DSSE.

The `verdict-evidence/` directory contains:

```
verdict-evidence/
├── SUMMARY.txt                 # one-line-per-verdict status
├── verifying-key.pub           # Ed25519 public key (32 bytes)
├── VERIFY.md                   # verification walkthrough
├── traceability-matrix.json    # V-model matrix (v0.6.5+)
├── traceability-matrix.html    # rendered for human review
└── <verdict-name>/
    ├── source.wasm             # pre-instrumentation
    ├── instrumented.wasm       # post-instrumentation
    ├── manifest.json           # branches + reconstructed decisions
    ├── run.json                # per-row condition vectors (v3 schema)
    ├── report.txt              # MC/DC truth tables (text)
    ├── report.json             # MC/DC report (schema /witness-mcdc/v1)
    ├── predicate.json          # in-toto Statement (unsigned)
    ├── signed.dsse.json        # DSSE envelope (signed)
    └── lcov.info, overview.txt # codecov-ingestible LCOV
```

The eleven verdicts split into two groups:

- **Seven canonical compound-decision shapes** with hand-derivable
  truth tables (leap_year, range_overlap, triangle, state_guard,
  mixed_or_and, safety_envelope, parser_dispatch). These are the
  oracle: a reviewer can verify witness's MC/DC report by eye.
- **Four real-application fixtures** at meaningful scale (httparse,
  nom_numbers, state_machine, json_lite). 672 instrumented branches,
  104 reconstructed decisions across real Rust crate code (RFC 7230
  parser, parser-combinator integer parsing, TLS handshake state
  machine, JSON parser). Their `TRUTH-TABLE.md` files document
  source-author intent + the Wasm-level coverage-lifting story
  (v0.2 paper).

See each verdict's [`TRUTH-TABLE.md`](verdicts/) and `V-MODEL.md`.

## Usage

```sh
# Instrument a Wasm module with branch counters + per-row capture.
witness instrument app.wasm -o app.instrumented.wasm

# Default: embedded wasmtime runner. Each --invoke is one row; witness
# reads counter + per-row globals after each return.
witness run app.instrumented.wasm \
  --invoke run_row_0 --invoke run_row_1 --invoke run_row_2

# Subprocess harness mode. The harness reads WITNESS_MODULE /
# WITNESS_MANIFEST and writes a `witness-harness-v1` snapshot to
# WITNESS_OUTPUT. See "Harness-mode protocol" below for the schema.
witness run app.instrumented.wasm --harness "node tests/runner.mjs"

# Branch-coverage report (text / JSON).
witness report --input witness-run.json
witness report --input witness-run.json --format json

# v0.6+ MC/DC report — truth tables, independent-effect citations,
# gap-closure recommendations.
witness report --input witness-run.json --format mcdc
witness report --input witness-run.json --format mcdc-json

# v0.6+ in-toto coverage predicate + DSSE signing + verification.
witness predicate --run witness-run.json --module app.instrumented.wasm \
  -o predicate.json
witness keygen --secret release.sk --public release.pub
witness attest --predicate predicate.json --secret-key release.sk \
  -o predicate.dsse.json
witness verify --envelope predicate.dsse.json --public-key release.pub

# LCOV emission for codecov.
witness lcov --run witness-run.json --manifest app.instrumented.wasm.witness.json \
  -o lcov.info
```

For a worked example end-to-end, see
[`tests/fixtures/sample-rust-crate/`](tests/fixtures/sample-rust-crate/) —
a tiny `no_std` Rust crate that compiles to Wasm and exercises every
instrumentation pattern (`br_if`, `if/else`, `br_table`). Build it with
`./tests/fixtures/sample-rust-crate/build.sh`, then run
`cargo test --test integration_e2e` to see the round-trip
instrument→run→assert flow against compiler output (not just hand-written
WAT). The fixture's `README.md` documents the entry-point conventions
witness uses for `--invoke`.

## Harness-mode protocol (`witness-harness-v1`)

`witness run --harness <cmd>` is the escape hatch for runtimes other
than embedded wasmtime — Node WASI, custom kiln deployments, hardware
boards. witness spawns the harness via `sh -c` with three env vars set:

```
WITNESS_MODULE   — absolute path to the instrumented .wasm
WITNESS_MANIFEST — absolute path to <module>.witness.json
WITNESS_OUTPUT   — absolute path the harness must write to before exit
```

The harness loads the module, exercises it however it wants, then
writes a JSON file to `WITNESS_OUTPUT` matching:

```json
{
  "schema": "witness-harness-v1",
  "counters": {
    "0": 12,
    "1": 7,
    "2": 0,
    "3": 12
  }
}
```

**Keys are the per-branch decimal IDs** as published in the manifest
(`branches[].id`). **Values are u64 hit counts.** That's the entire
v1 wire format. A 10-line Node WASI harness is enough.

```js
// harness.mjs — minimal witness-harness-v1 implementation
import fs from "node:fs/promises";
import { WASI } from "node:wasi";

const mod = await WebAssembly.compile(
  await fs.readFile(process.env.WITNESS_MODULE),
);
const wasi = new WASI({ version: "preview1" });
const inst = await WebAssembly.instantiate(mod, { wasi_snapshot_preview1: wasi.wasiImport });
inst.exports.run_row_0(); inst.exports.run_row_1(); /* ... */

const counters = {};
for (const [name, val] of Object.entries(inst.exports)) {
  if (name.startsWith("__witness_counter_") && typeof val.value === "bigint") {
    counters[name.replace("__witness_counter_", "")] = Number(val.value);
  }
}
await fs.writeFile(
  process.env.WITNESS_OUTPUT,
  JSON.stringify({ schema: "witness-harness-v1", counters }),
);
```

### v2 schema — full MC/DC from a subprocess (`witness-harness-v2`, v0.9.5+)

`witness-harness-v1` carries counters only, so MC/DC reconstruction
degrades to branch coverage. **v0.9.5 ships v2** — the same wire
format extended with per-row `brvals` / `brcnts` / `trace_b64`,
mirroring exactly what embedded wasmtime mode reads. A v2-aware
harness produces full truth tables identical to embedded.

```json
{
  "schema": "witness-harness-v2",
  "counters": { "0": 7, "1": 3 },
  "rows": [
    {
      "name": "run_row_0",
      "outcome": 1,
      "brvals": { "0": 1, "1": 0 },
      "brcnts": { "0": 1, "1": 1 },
      "trace_b64": "AAAA..."
    },
    {
      "name": "run_row_1",
      "outcome": 0,
      "brvals": { "0": 0, "1": 0 },
      "brcnts": { "0": 1, "1": 1 },
      "trace_b64": "AAAA..."
    }
  ]
}
```

A v2 harness must call `__witness_trace_reset` and
`__witness_row_reset` between rows so each `HarnessRow` carries
isolated post-invocation state. The trace bytes are the raw 64 KiB ×
N pages of `__witness_trace` memory, base64 standard-encoded
(including the 16-byte header).

```js
// harness.mjs — minimal witness-harness-v2 implementation
import fs from "node:fs/promises";
import { WASI } from "node:wasi";

const mod = await WebAssembly.compile(
  await fs.readFile(process.env.WITNESS_MODULE),
);
const wasi = new WASI({ version: "preview1" });
const inst = await WebAssembly.instantiate(mod, { wasi_snapshot_preview1: wasi.wasiImport });
const exp = inst.exports;

const traceMem = exp.__witness_trace;
const rows = [];
const rowNames = ["run_row_0", "run_row_1", "run_row_2"];
for (const name of rowNames) {
  exp.__witness_trace_reset();
  exp.__witness_row_reset();
  const out = exp[name]();

  const brvals = {}, brcnts = {};
  for (const [k, v] of Object.entries(exp)) {
    if (k.startsWith("__witness_brval_")) brvals[k.replace("__witness_brval_", "")] = Number(v.value);
    else if (k.startsWith("__witness_brcnt_")) brcnts[k.replace("__witness_brcnt_", "")] = Number(v.value);
  }
  const trace_b64 = Buffer.from(traceMem.buffer).toString("base64");
  rows.push({ name, outcome: out, brvals, brcnts, trace_b64 });
}

const counters = {};
for (const [k, v] of Object.entries(exp)) {
  if (k.startsWith("__witness_counter_")) counters[k.replace("__witness_counter_", "")] = Number(v.value);
}
await fs.writeFile(
  process.env.WITNESS_OUTPUT,
  JSON.stringify({ schema: "witness-harness-v2", counters, rows }),
);
```

### v1 stays supported (counters-only fallback)

Existing `witness-harness-v1` harnesses keep working unchanged in
v0.9.5+. The schema-string dispatch picks v1's counters-only path,
producing branch coverage like before. Migrate when you need truth
tables — v1 → v2 is a strict superset, no breaking changes to the v1
fields.

## Cross-language reach

witness operates at the wasm + DWARF layer, not at any specific
source language. In principle, any language that compiles to
wasm with debug info is a candidate — see
[docs/cross-language.md](docs/cross-language.md) for the honest
matrix: Rust is verified end-to-end; C is partially verified
(instrumentation works, decision clustering needs a v0.19+
extension to handle clang's `if/else` lowering); C++ / Zig /
Swift / TinyGo / Kotlin/Wasm are documented as "should work,
untested." Probes welcome.

This positions witness alongside but distinct from existing
OSS MC/DC tools:

- **[GCC 14 `-fcondition-coverage`](https://arxiv.org/html/2501.02133v1)**
  (C/C++/D/Rust), **[Coveron](https://coveron.github.io/)**
  (C/C++), and **[linux-mcdc](https://github.com/xlab-uiuc/linux-mcdc)**
  (Linux kernel) all measure at the **source-level** (frontend).
- **[GNATcoverage](https://github.com/AdaCore/gnatcoverage)** (Ada) measures at the
  source level and object level.
- **witness** measures at the **post-codegen wasm bytecode**
  layer. The same DO-178C "post-preprocessor C" precedent
  applies: measure what the compiler emits, not what the
  engineer typed. Different chain layer → additive evidence.

## Where it fits

witness is one piece of a composed pipeline. Each tool owns a narrow mechanical
check; the composition is what the audit trail holds.

| Tool | Role |
|---|---|
| [rivet](https://github.com/pulseengine/rivet) | Requirement ↔ test ↔ coverage traceability validator. Consumes witness reports. |
| [sigil](https://github.com/pulseengine/sigil) | Signs Wasm + emits in-toto SLSA provenance; carries witness reports as coverage predicates as composition matures. |
| [loom](https://github.com/pulseengine/loom) | Post-fusion Wasm optimization with Z3 translation validation — emits the optimized Wasm that witness measures. |
| [meld](https://github.com/pulseengine/meld) | Component fusion — witness can measure coverage on fused modules or individual components. |
| [kiln](https://github.com/pulseengine/kiln) | Wasm runtime — one of the execution options for the test harness. |
| [spar](https://github.com/pulseengine/spar) | Architecture / MBSE layer — not directly involved in coverage, but selects the variant that determines what Wasm gets produced. |

## Build

```sh
cargo build --release
cargo test
```

## Contributing

This project uses [rivet](https://github.com/pulseengine/rivet) for
traceability. Before committing, run `rivet validate` to check artifact
integrity — the pre-commit hook installed by `rivet init --hooks` does this
automatically.

Commit messages use [Conventional Commits](https://www.conventionalcommits.org/):
`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`.

## Related work

witness sits in a populated landscape. The closest precedents measure
coverage at non-source levels (JaCoCo on JVM bytecode, Clang and rustc on
LLVM IR/MIR), and the Wasm ecosystem already has source-level coverage
tools that project through Wasm. None of them measure MC/DC on Wasm
directly, which is the gap witness occupies.

| Tool | Measurement point | MC/DC? | Relationship |
|---|---|---|---|
| [JaCoCo](https://www.eclemma.org/jacoco/trunk/doc/counters.html) | JVM bytecode | No ([per maintainers](https://groups.google.com/g/jacoco/c/b8bAWaWPl6I/m/eMKixUpMCAAJ)) | Direct precedent. The JVM has had bytecode-level branch coverage for two decades and it ships in regulated contexts. witness does the same for Wasm in v0.1 and adds MC/DC reconstruction in v0.2. |
| [Clang source-based MC/DC](https://discourse.llvm.org/t/rfc-source-based-mc-dc-code-coverage/59244) | LLVM IR (annotated from source AST at lowering) | Yes, capped at 6 conditions per decision | Source-level MC/DC. The 6-condition cap is a bitmap-encoding constraint. Different measurement point from witness; complementary. |
| [rustc `-Zcoverage-options=mcdc`](https://github.com/rust-lang/rust/issues/124144) | HIR → MIR (lowered to LLVM coverage) | Yes, capped at 6 conditions per decision | Source-level MC/DC for Rust; inherits the LLVM cap. Covers what the human wrote; witness covers what survives rustc + LLVM into Wasm. Different blind spots. |
| [wasmcov](https://hknio.github.io/wasmcov/) / [minicov](https://github.com/Amanieu/minicov) / [wasm-bindgen-test coverage](https://rustwasm.github.io/docs/wasm-bindgen/wasm-bindgen-test/coverage.html) | LLVM source-level coverage projected through Wasm execution | Inherits LLVM | Source-level coverage *via* Wasm runtime. Useful when source is available and you trust the LLVM-to-Wasm lowering. witness is Wasm-structural — useful when the source is not in scope, or when post-LLVM divergences matter. |
| [Whamm](https://arxiv.org/html/2504.20192) | Wasm bytecode rewriting / engine monitoring | Not coverage-specific | General-purpose Wasm instrumentation DSL (April 2025). I think Whamm could be a future implementation backend for witness's rewrite phase if walrus's ergonomics stop scaling — no immediate action, worth tracking. |
| [Wasabi](https://github.com/danleh/wasabi) | Wasm dynamic analysis | Not coverage-specific | Older Rust-based Wasm instrumentation framework. Precedent for the shape of the tool, no overlap in what it measures. |
| Ferrous / DLR Rust MC/DC (under the [SCRC 2026 Project Goal](https://blog.rust-lang.org/2026/01/14/what-does-it-take-to-ship-rust-in-safety-critical/)) | Rust source / MIR | Yes (planned, DAL A target) | Same chain layer as rustc-mcdc, productised for safety-critical use. Explicitly complementary to witness: different measurement points, different blind spots, additive evidence. |

We adopt all of these. The overdo stance from
[Overdoing the verification chain](https://pulseengine.eu/blog/overdoing-the-verification-chain/)
is that techniques at the same chain layer with non-overlapping blind
spots are paired, not picked between — the cost of running both is CI
budget; the cost of picking one is a certification campaign that stalls
on a missing technique. witness is the post-rustc Wasm measurement
point; rustc-mcdc and Ferrous/DLR are the pre-LLVM source-level
measurement point. Resistance is futile.

### Upstream — equivalence-class inference on legacy binaries

De Luca, De Angelis, Amalfitano, and Cimmino — *Inferring
Equivalence Classes from Legacy Undocumented Embedded Binaries for
ISO 26262-Compliant Testing* ([arXiv:2604.22673](https://arxiv.org/abs/2604.22673)) —
addresses the *input-side* problem that witness assumes solved:
when the test corpus does not exist and the source has been lost,
how do you derive equivalence classes of inputs from a binary that
still has to be re-qualified under ISO 26262? Their work recovers
the input partitions; witness measures MC/DC on the executable
those inputs exercise. Read as a chain: arXiv:2604.22673 produces
the witness vectors a §6.4.2-style structural-coverage argument
needs, and witness measures whether those vectors actually
discriminate the post-codegen decisions the runtime executes. The
two halves are complementary — input-domain reconstruction
upstream, structural-coverage measurement downstream — and form
one shape of evidence chain a 26262 or DO-178C dossier expects.
The citation here is bibliographic; we don't yet integrate with
their pipeline.

## License

Dual-licensed under Apache-2.0 OR MIT. See [LICENSE-APACHE](LICENSE-APACHE)
and [LICENSE-MIT](LICENSE-MIT).
