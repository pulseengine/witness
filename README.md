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

**v0.1.0 shipped 2026-04-24.** Per-`br_if` / per-`if-else` branch coverage,
embedded wasmtime runner, exported-mutable-globals counter mechanism. v0.2
adds DWARF-grounded MC/DC reconstruction, per-target `br_table` counting,
and a subprocess `--harness` escape hatch. See [DESIGN.md](DESIGN.md) for
the incremental v0.1→v1.0 roadmap. Core tracking issue:
[pulseengine.eu #29](https://github.com/pulseengine/pulseengine.eu/issues/29).

Counter values are exposed as exported mutable globals named
`__witness_counter_<id>`, not via a dump function — any conformant Wasm
runtime can read coverage with a two-line `instance.get_global` call. No
cooperation protocol with the module-under-test is required.

## Usage

```sh
# Instrument a Wasm module with branch counters.
witness instrument app.wasm -o app.instrumented.wasm

# Default: embedded wasmtime runner. Invoke one or more no-argument
# exports; witness reads counter globals after they return.
witness run app.instrumented.wasm --invoke run_tests

# Subprocess harness mode (v0.2). The harness reads WITNESS_MODULE /
# WITNESS_MANIFEST and writes a counter snapshot to WITNESS_OUTPUT
# before exiting.
witness run app.instrumented.wasm --harness "node tests/runner.mjs"

# Produce a coverage report (text or JSON).
witness report --input witness-run.json
witness report --input witness-run.json --format json
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
