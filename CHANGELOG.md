# Changelog

All notable changes to witness are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [SemVer 2.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
