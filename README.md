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

**v0.1 — in progress.** Scope is deliberately small; see [DESIGN.md](DESIGN.md)
for the incremental v0.1→v1.0 roadmap. Core tracking issue:
[pulseengine.eu #29](https://github.com/pulseengine/pulseengine.eu/issues/29).

## Usage (v0.1 target)

```sh
# Instrument a Wasm module with branch counters.
witness instrument app.wasm -o app.instrumented.wasm

# Run your test harness against the instrumented module.
witness run --harness "cargo test --target wasm32-wasi" --module app.instrumented.wasm

# Produce a coverage report.
witness report
```

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

## Relationship to Rust-level MC/DC (Ferrous / DLR)

**Complementary, not competitive.** When the Ferrous/DLR MC/DC tool for Rust
lands, witness does not become obsolete. Different measurement points have
different blind spots:

- Rust-level MC/DC measures *source* decisions (what the human wrote).
- witness measures the *post-compile Wasm* (what actually ships).
- Translation validation (loom's Z3 TV) bridges the two levels.
- Coverage at both levels is additive evidence — the multi-level discipline
  DO-178C has accepted since 1992.

The overdo principle: adopt both tools, not one. Resistance is futile.

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

## License

Dual-licensed under Apache-2.0 OR MIT. See [LICENSE-APACHE](LICENSE-APACHE)
and [LICENSE-MIT](LICENSE-MIT).
