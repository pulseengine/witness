# witness

**MC/DC-style branch coverage for WebAssembly components.**

witness instruments a Wasm module with branch counters, runs a test harness
against it, and emits a coverage report you can read, compare, or feed into
[rivet](https://github.com/pulseengine/rivet) as requirement-to-test evidence
and into [sigil](https://github.com/pulseengine/sigil) as an in-toto coverage
predicate.

The argument for why this tool exists lives in two blog posts:

- [Spec-driven development is half the loop](https://pulseengine.eu/blog/spec-driven-development-is-half-the-loop/)
- *MC/DC for AI-authored Rust is tractable — the variant-pruning argument* (draft)

Short version: pattern-matching, `?` desugaring, and cfg expansion have all
resolved by the time a Wasm module exists. Coverage measured at the Wasm
level describes what actually ships, against an instruction set small enough
and formally specified enough that the tool-qualification story moves in your
direction. And the *"post-preprocessor C"* precedent — MC/DC measured on
expanded C rather than pre-expansion source, accepted by DO-178C since 1992 —
is structurally the same move as *"post-rustc Wasm"*.

## Status

**v0.6.x is the current release line** (latest tag — see
[releases](https://github.com/pulseengine/witness/releases)). The
v0.6.x sub-versions ratcheted from "consumer-side schema only" up to
a complete signed-evidence pipeline:

| Version | What it added |
|---|---|
| **v0.6.0** | Real MC/DC reporter, schema v3, verdict-suite scaffolding, V-model artefact graph |
| **v0.6.1** | On-Wasm per-row instrumentation; leap_year verdict end-to-end |
| **v0.6.2** | Adjacent-line decision clustering — 5 of 7 verdicts produce reports |
| **v0.6.3** | Populated compliance bundle + verdict-suite CI regression gate |
| **v0.6.4** | DSSE-signed verdict predicates with ephemeral release keys |
| **v0.6.5** | V-model traceability matrix in compliance bundle |
| **v0.6.6** | Verdict-suite delta posted to PRs as a comment |

Earlier versions (v0.1.0–v0.5.0, all shipped between 2026-04-24 and
2026-04-25): branch coverage, DWARF reconstruction, rivet evidence
emission, sigil predicate format, workspace split, LCOV emission.
See [DESIGN.md](DESIGN.md) for the incremental roadmap and
[`docs/roadmap.md`](docs/roadmap.md) for v0.7+.

Counter values are exposed as exported mutable globals named
`__witness_counter_<id>` (plus `__witness_brval_<id>` /
`__witness_brcnt_<id>` from v0.6.1+), not via a dump function — any
conformant Wasm runtime can read coverage with a two-line
`instance.get_global` call. No cooperation protocol with the
module-under-test is required.

## Show me the proof — verify a release in 60 seconds

Every v0.6.4+ release ships a `compliance-evidence.tar.gz` archive
containing seven verdict directories with end-to-end MC/DC reports
plus a DSSE-signed in-toto Statement per verdict and an ephemeral
public key. Verify it:

```sh
# 1. Download the compliance archive for the latest release.
gh release download v0.6.6 \
  --repo pulseengine/witness \
  --pattern '*compliance-evidence*'

# 2. Extract.
tar -xzf witness-v0.6.6-compliance-evidence.tar.gz

# 3. See the per-verdict roll-up.
cat compliance/verdict-evidence/SUMMARY.txt

# 4. Verify a signed predicate against the verifying key.
witness verify \
  --envelope compliance/verdict-evidence/leap_year/signed.dsse.json \
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

The seven verdicts cover canonical compound-decision shapes from
2-condition AND through 5-condition mixed AND/OR plus a real-world
URL-authority validator that surfaces decisions in the Rust standard
library's `memchr`. See each verdict's
[`TRUTH-TABLE.md`](verdicts/) for the source-level truth table and,
where the Wasm-level reality differs, a "post-rustc Wasm-level
reality" section explaining the v0.2 paper's coverage-lifting
argument in concrete form.

## Usage

```sh
# Instrument a Wasm module with branch counters + per-row capture.
witness instrument app.wasm -o app.instrumented.wasm

# Default: embedded wasmtime runner. Each --invoke is one row; witness
# reads counter + per-row globals after each return.
witness run app.instrumented.wasm \
  --invoke run_row_0 --invoke run_row_1 --invoke run_row_2

# Subprocess harness mode. The harness reads WITNESS_MODULE /
# WITNESS_MANIFEST and writes a counter snapshot to WITNESS_OUTPUT.
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

## License

Dual-licensed under Apache-2.0 OR MIT. See [LICENSE-APACHE](LICENSE-APACHE)
and [LICENSE-MIT](LICENSE-MIT).
