# Rivet → witness quality-pattern mapping

How witness adapts the CI, lint, and release patterns from
[`pulseengine/rivet`](https://github.com/pulseengine/rivet) to a
single-crate scope. Compiled from an exploration pass over rivet's repo
structure and distilled to the decisions that shaped witness v0.1.

## Summary

rivet is a three-crate workspace (`rivet-core`, `rivet-cli`, `etch`) with
Bazel + Cargo, Kani/Verus/Rocq proof tooling, a playwright e2e suite, and
a compliance-report CI pipeline. witness is a single-binary CLI; we keep
the patterns that are universal quality gates and drop the ones that
depend on multi-crate / cross-language / MBSE scope.

## Copied verbatim

| File | Purpose |
|---|---|
| `rustfmt.toml` | `edition = "2024"` |
| `clippy.toml` | `msrv = "1.85"` |
| `deny.toml` | License allow-list + sources/bans; RustSec advisories cleared of rivet-specific ignores |
| `codecov.yml` | Project target 60%, patch target 70% |

## Adapted

| File | What changed |
|---|---|
| `Cargo.toml` | `[workspace]` → `[package]`; SCRC clippy lints moved from `[workspace.lints]` to `[lints]`; `unexpected_cfgs` drops the rivet-only `kani`/`verus` cfg enumeration |
| `.github/workflows/ci.yml` | Same shape as rivet (fmt, clippy, test, msrv, deny, audit, coverage) but **without** the `yaml-lint`, `docs-check`, or `compliance` jobs that depend on rivet's artifact-YAML and dashboard binary. Adds a `test` matrix across Linux/macOS/Windows that rivet does not run because rivet's Bazel path is Linux-only. |
| `.github/workflows/release.yml` | Same five-target matrix (x86_64/aarch64 Linux, x86_64/aarch64 macOS, x86_64 Windows); binary name `witness` instead of `rivet`; no `-p rivet-cli` workspace selector since single crate |

## SCRC clippy lints (39 total) — adopted as-is

Phase 1 (restriction family, DD-059):
`unwrap_used`, `expect_used`, `indexing_slicing`, `arithmetic_side_effects`,
`as_conversions`, `cast_possible_truncation`, `cast_sign_loss`,
`wildcard_enum_match_arm`, `match_wildcard_for_single_variants`, `panic`,
`todo`, `unimplemented`, `dbg_macro`, `print_stdout`, `print_stderr`.

Phase 2 (unsafe-block hygiene + memory-safety, DD-063):
`undocumented_unsafe_blocks`, `multiple_unsafe_ops_per_block`,
`mem_forget`, `mem_replace_with_uninit`, `transmute_undefined_repr`,
`uninit_assumed_init`, `rc_mutex`, `mutex_atomic`, `same_name_method`,
`lossy_float_literal`, `empty_drop`, `exit`.

All `priority = -1` so per-site `#[allow(clippy::xxx)]` overrides work.

## Deliberately omitted

| Feature | Why |
|---|---|
| Bazel (`MODULE.bazel`, `BUILD.bazel`) | witness is single-Cargo; no Rust-Rocq-Verus multi-language build |
| Proofs directory (Kani/Verus/Rocq harnesses) | witness has no metamodel-level invariants to prove at v0.1; re-evaluate at v1.0 when the Check-It qualification artefact is designed |
| `etch` plugin system | witness has no plugin surface; CLI only |
| VS Code extension (`vscode-rivet/`) | not applicable |
| Compliance HTML-report pipeline (`.github/actions/compliance`) | witness does not host artefacts; rivet consumes witness reports, not the reverse |
| `yaml-lint` / `docs-check` CI jobs | witness has no YAML schema or docs-vs-reality gate at v0.1 |
| Fuzz corpus + targets | add in v0.2 when the walrus input-parsing surface stabilises and a corpus makes sense |

## Omitted for now, planned later

- `SAFETY.md` — rivet's 89-line safety posture document. Defer until witness
  has enough surface area to warrant it (likely around v0.3 when rivet
  integration lands).
- Property-based tests with `proptest` — useful for AST transforms in
  witness's rewrite phase, but v0.1's six instrumentation unit tests +
  two round-trip tests are sufficient coverage for the ship milestone.
- `miri` CI job — add when witness grows unsafe code. v0.1 is all safe
  Rust, so miri would only check the dependency closure (high cost, low
  new signal).

## Notes on CI differences worth calling out later

1. rivet's `test` job runs Linux-only and uses `cargo-nextest` with JUnit
   XML upload for compliance-report ingestion. witness runs on three OSes
   and uses plain `cargo test --all-targets`. If witness gains a
   compliance pipeline later, add nextest + JUnit to the Linux matrix cell.
2. rivet uses `dtolnay/rust-toolchain@stable` at `actions/checkout@v6`.
   witness pins to `actions/checkout@v5` (current GA). Bump when v6 GAs.
3. rivet gates coverage on `push` to main only. witness does the same to
   keep PR CI runtimes under the 5-minute cache TTL.
