# Changelog

All notable changes to witness are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [SemVer 2.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
