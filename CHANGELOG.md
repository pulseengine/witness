# Changelog

All notable changes to witness are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [SemVer 2.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
