# v0.5 — witness as a Wasm Component

How witness ships as a Component-Model artifact for v0.5. Sub-agent
investigation 2026-04-24, read-only on witness/sigil/rivet; meld/loom/
kiln not accessible in this thread (open follow-up).

---

## 1. Executive summary

`walrus`, `wasmparser`, `gimli`, `serde*`, `sha2` all compile to
`wasm32-wasip2` — that covers `instrument`, `decisions`, `report`,
`predicate`, `rivet_evidence`, `diff` (the entire pure-data spine).
`wasmtime` does **not** (Cargo.lock shows it pulling cranelift, libc,
memfd, rustix — JIT + mmap + raw syscalls), so `run.rs` stays host-side.
v0.5 carves out a `witness-core` cdylib crate exporting a WIT interface
and ships it as `witness-core-v0.5.0.component.wasm`; the existing
`witness` binary becomes a thin CLI that embeds wasmtime to drive both
the user's module *and* (optionally) witness-core. Sigil's
`wsc-component` already exports `wasm-signatures:wasmsign/signing` so a
`wac`-composed `witness-attest` (instrument → predicate → DSSE-sign) is
the v0.5.1 stretch goal. **Largest compilable subset:** every witness
module except `run.rs`.

---

## 2. Compile-target audit

Source paths are all under `/Users/r/git/pulseengine/witness/src/`.

| Module | wasip2? | Obstacle / refactor |
|---|---|---|
| `lib.rs` / `error.rs` | yes | re-exports + `thiserror` — portable |
| `instrument.rs` | **yes** | `walrus 0.24` is pure Rust over `id-arena`. `pub fn instrument_module` (`instrument.rs:199`) is bytes-in by design; `instrument_file` (FS) stays CLI-side |
| `decisions.rs` | yes | `gimli 0.31` (`read`+`std`) + `wasmparser 0.240`. Pure parsing (`decisions.rs:48`) |
| `report.rs` | yes | `Report::from_record` (`report.rs:59`) is pure; `from_run_file` stays CLI-side |
| `predicate.rs` | yes | `sha2` + `serde_json` + custom RFC-3339 (`predicate.rs:170`). Refactor `build_statement` to take bytes (§3) |
| `rivet_evidence.rs` | yes | `serde_yaml`. `build_evidence` (`rivet_evidence.rs:139`) pure; `RequirementMap::load` → CLI |
| `diff.rs` | yes | serde + manifest sniffing. `diff()` reads files; needs `diff_bytes` core |
| `run.rs` | **NO** | wasmtime 42, wasmtime-wasi, tempfile, `std::process::Command`. Cranelift JIT + mmap + syscalls + subprocess. Stays CLI-side |
| `main.rs` | N/A | CLI driver — host-side |

`Cargo.lock` confirms wasmtime 42.0.2 pulls `cc`, `libc`, `memfd`,
`rustix 1.1.4`, `wasmtime-internal-cranelift` — the JIT-codegen
combination cannot run in a Component-Model guest.

---

## 3. Recommended `witness-core` crate boundary

Split the current single-crate witness into a Cargo workspace:

```text
witness/
  Cargo.toml           # virtual workspace
  witness-core/        # the component crate
    Cargo.toml         # crate-type = ["cdylib"]; depends on walrus, gimli, …
    src/
      lib.rs
      instrument.rs   ← moved verbatim
      decisions.rs    ← moved verbatim
      report.rs        ← `from_run_file` deleted; `Report::from_record` kept
      predicate.rs    ← `build_statement_from_bytes` (new); `build_statement` stays in -cli
      rivet_evidence.rs ← `RequirementMap::from_str` (new); load() stays in -cli
      diff.rs          ← `diff_bytes` (new); `diff()` stays in -cli
      error.rs         ← unchanged
  witness-cli/         # the binary (today's `bin = "witness"`)
    Cargo.toml         # depends on witness-core, wasmtime, wasmtime-wasi, clap, tempfile
    src/
      main.rs
      run.rs           ← stays here; wasmtime + subprocess host
      io.rs            ← thin wrappers: read file → core::fn(bytes), write result
  wit/
    witness.wit        # see §4
```

Refactors needed (small, mechanical):

1. **`predicate::build_statement`**
   change signature from `(report, instrumented_path, original_path,
   harness)` to `build_statement_from_bytes(report,
   instrumented_name: &str, instrumented_bytes: &[u8],
   original: Option<(&str, &[u8])>, harness)`. Today's path-based
   variant becomes a thin wrapper in `witness-cli`.
2. **`diff` module loaders**
   `Manifest::load`, `RunRecord::load`, `Report::from_run_file` —
   rivet-evidence `RequirementMap::load` — all keep the bytes-based
   `serde_*::from_slice` or `from_str` core in the component crate;
   the FS-touching wrappers move to the CLI.
3. **`now_rfc3339`** at `predicate.rs:170` calls `SystemTime::now()`,
   which on `wasm32-wasip2` resolves through `wasi:clocks/wall-clock` —
   that's fine, it just imports a WASI interface in the resulting
   component. No source change needed; document the WASI import.

What stays only in `witness-cli`:

- `run.rs` (wasmtime host execution + subprocess harness)
- `clap`-based argv parsing
- file-system glue (`std::fs::read`, `std::fs::write` of inputs/outputs)
- the `tracing-subscriber` init

Result: `witness-core` has ~1900 LoC of pure algorithm + serde, no
syscalls beyond what WASI exposes (random for nothing, clocks for the
RFC-3339 stamp).

---

## 4. Candidate WIT interface

Drop this at `/Users/r/git/pulseengine/witness/wit/witness.wit`. It
mirrors the existing `pub` surface (`instrument_module`,
`reconstruct_decisions`, `Report::from_record`, `build_statement`,
`build_evidence`, `diff`).

```wit
package pulseengine:witness@0.5.0;

/// Shared types — direct translation of the `pub` data types in
/// `src/instrument.rs:69-141`, `src/run.rs:50-67`, `src/report.rs:22-48`,
/// `src/predicate.rs:34-78`. Each Rust struct → WIT `record`, each
/// Rust enum → WIT `enum` or `variant`.
interface types {
    enum branch-kind {
        br-if, if-then, if-else, br-table-target, br-table-default,
    }

    record branch-entry {
        id: u32, function-index: u32, function-name: option<string>,
        kind: branch-kind, instr-index: u32,
        target-index: option<u32>, byte-offset: option<u32>,
        seq-debug: string,
    }
    record decision {
        id: u32, conditions: list<u32>,
        source-file: option<string>, source-line: option<u32>,
    }
    record manifest {
        schema-version: string, witness-version: string,
        module-source: string,
        branches: list<branch-entry>, decisions: list<decision>,
    }

    record branch-hit {
        id: u32, function-index: u32, function-name: option<string>,
        kind: branch-kind, instr-index: u32, hits: u64,
    }
    record run-record {
        schema-version: string, witness-version: string,
        module-path: string,
        invoked: list<string>, branches: list<branch-hit>,
    }

    record function-report {
        function-index: u32, function-name: option<string>,
        total: u32, covered: u32,
    }
    record uncovered-branch {
        branch-id: u32, function-index: u32,
        function-name: option<string>, instr-index: u32,
        kind: branch-kind,
    }
    record report {
        schema-version: string, witness-version: string,
        module: string,
        total-branches: u32, covered-branches: u32,
        per-function: list<function-report>,
        uncovered: list<uncovered-branch>,
    }

    record digests { sha256: string }
    record subject { name: string, digest: digests }
    record original-module { name: string, digest: digests }
    record measurement {
        harness: option<string>, measured-at: string,
        witness-version: string,
    }
    record coverage-predicate {
        coverage: report, measurement: measurement,
        original-module: option<original-module>,
    }
    record statement {
        statement-type: string, subject: list<subject>,
        predicate-type: string, predicate: coverage-predicate,
    }
}

/// Wasm AST instrumentation — emit instrumented module + branch manifest.
interface instrument {
    use types.{manifest};

    record instrument-result {
        instrumented-wasm: list<u8>,
        manifest: manifest,
    }

    /// Instrument a Wasm module's bytes. Mirrors
    /// `crate::instrument::instrument_module` plus the manifest assembly
    /// today done in `instrument_file` (`src/instrument.rs:161`).
    instrument-module: func(
        module-bytes: list<u8>,
        module-source-label: string,
    ) -> result<instrument-result, string>;
}

/// DWARF-grounded decision reconstruction.
interface decisions {
    use types.{branch-entry, decision};

    /// Mirrors `crate::decisions::reconstruct_decisions`
    /// (`src/decisions.rs:48`).
    reconstruct-decisions: func(
        wasm-bytes: list<u8>,
        branches: list<branch-entry>,
    ) -> result<list<decision>, string>;
}

/// Coverage report aggregation.
interface report {
    use types.{run-record, report as report-record};

    /// Mirrors `Report::from_record` (`src/report.rs:59`).
    from-record: func(record: run-record) -> report-record;

    /// Text-format render. Mirrors `Report::to_text`.
    to-text: func(report: report-record) -> string;
}

/// Coverage-set delta. Records `delta`, `snapshot-meta`,
/// `branch-summary`, `changed-branch`, `coverage-delta` — direct
/// translation of `crate::diff` types (`src/diff.rs:21-77`).
interface diff {
    use types.{branch-entry};
    // ... (records elided; mirror the Rust types verbatim)

    /// Bytes-in version of `crate::diff::diff` (`src/diff.rs`).
    diff-bytes: func(
        base-bytes: list<u8>, base-label: string,
        head-bytes: list<u8>, head-label: string,
    ) -> result<delta, string>;
}

/// in-toto Statement assembly for sigil ingestion.
interface predicate {
    use types.{report, statement};

    /// Mirrors `crate::predicate::build_statement` after the bytes-in
    /// refactor described in v05 §3.
    build-statement: func(
        report: report,
        instrumented-name: string,
        instrumented-bytes: list<u8>,
        original-name: option<string>,
        original-bytes: option<list<u8>>,
        harness: option<string>,
    ) -> result<statement, string>;

    /// JSON serialisation. Caller-side serde is awkward over WIT; expose
    /// the canonical pretty-printed form as a string.
    statement-to-json: func(statement: statement) -> result<string, string>;
}

/// Rivet-shape coverage evidence. Records `requirement-map`,
/// `map-entry`, `coverage-evidence`, `evidence-file` direct-translate
/// from `crate::rivet_evidence` (`src/rivet_evidence.rs:31-103`).
/// Note: `RunMetadata` is flattened into `evidence-file` because WIT
/// nested-record use stays cleaner inline than nested at the WIT
/// boundary, where field renames cost nothing.
interface rivet-evidence {
    use types.{run-record};
    // ... records elided; see the Rust types referenced above.

    /// Mirrors `crate::rivet_evidence::build_evidence`
    /// (`src/rivet_evidence.rs:139`).
    build-evidence: func(
        record: run-record, map: requirement-map,
        source-label: string,
        environment: option<string>, commit: option<string>,
    ) -> result<evidence-file, string>;

    evidence-to-yaml: func(file: evidence-file) -> result<string, string>;
}

/// Library world — pure data, no I/O. Composable into other components.
world witness-core {
    import wasi:clocks/wall-clock@0.2.3;   // for `now_rfc3339` in predicate
    export instrument;
    export decisions;
    export report;
    export diff;
    export predicate;
    export rivet-evidence;
}
```

### Type translation notes

| Rust | WIT | Note |
|---|---|---|
| `&[u8]` / `Vec<u8>` | `list<u8>` | WIT has no zero-copy slice; copy on the boundary. |
| `Option<T>` | `option<t>` | direct mapping. |
| `Result<T, Error>` | `result<t, string>` | flatten `Error` to `String` at the boundary. WIT can carry richer error variants but the cross-language consumers don't gain much from them today. |
| `BTreeMap<K, V>` | `list<tuple<k, v>>` | WIT has no map type. Keep as `Vec<(K, V)>` in the binding layer. |
| `&Path` | `string` | Paths cross the boundary as strings; the host side does the OsStr→String lossy convert. |
| `enum BranchKind` | `enum branch-kind` | direct, kebab-case. |
| `struct` | `record` | direct. |
| `enum` with payload | `variant` | direct. |
| `chrono::DateTime` | `string` (RFC 3339) | witness already keeps timestamps as `String`; no change. |

---

## 5. Build pipeline

### Toolchain

Verified locally: `cargo-component`, `wac`, `wit-bindgen` all in
`/Users/r/.cargo/bin/`. `wasmtime` not verified (sandbox blocked the
version query). Fresh-box install:

```bash
cargo install cargo-component wac-cli wit-bindgen-cli --locked
rustup target add wasm32-wasip2
```

### `witness-core/Cargo.toml`

```toml
[package]
name = "witness-core"
version = "0.5.0"
edition = "2024"
rust-version = "1.91"

[lib]
crate-type = ["cdylib"]

[dependencies]
walrus = "0.24"
wasmparser = "0.240"
gimli = { version = "0.31", default-features = false, features = ["read", "std"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
sha2 = "0.10"
thiserror = "2"
anyhow = "1"
wit-bindgen = { version = "0.47", default-features = false, features = ["realloc"] }

[package.metadata.component]
package = "pulseengine:witness"

[package.metadata.component.target]
path = "../wit/witness.wit"
world = "witness-core"

[package.metadata.component.target.dependencies]
"wasi:clocks" = { path = "../wit/deps/clocks" }
```

### Build

```bash
cd witness/witness-core
cargo component build --release
# Produces:
#   target/wasm32-wasip2/release/witness_core.wasm
```

Optional post-process (size shrink + canonicalise):

```bash
wasm-tools component wit target/wasm32-wasip2/release/witness_core.wasm
wasm-opt -Oz -o witness-core.component.wasm \
  target/wasm32-wasip2/release/witness_core.wasm
```

### Invocation

Realistic host is the standalone `wasmtime` CLI:

```bash
wasmtime run --wasm component-model --dir . \
  witness-core.component.wasm \
  --invoke 'instrument-module(<bytes>, "app.wasm")'
```

For real workflows, wrap in a host-side script (file → `list<u8>` →
file). The witness-cli binary in v0.5 keeps `run` host-side via the
`wasmtime` Rust crate (it has to JIT a *different* user module —
embedding witness-as-component on top of that would be two layers of
guest, over-engineered).

---

## 6. Release-asset shape

Existing release flow (referenced in `docs/research/v04-ci-ports.md`)
ships:

- `witness` (host binary, per-platform)
- `witness-coverage-v0.5.0.json` (sample predicate)
- `witness-rivet-evidence-v0.5.0.yaml` (sample evidence)

Add for v0.5:

```text
witness-core-v0.5.0.component.wasm        # the component itself
witness-core-v0.5.0.component.wasm.wit    # extracted WIT (for consumers)
witness-core-v0.5.0.component.wasm.sha256 # in-archive checksum
```

Place them in the same GitHub Release as the host binaries, under
`assets/`. Naming follows the pattern sigil already uses for
`wsc-component-v0.8.0.component.wasm`. The `.wit` companion comes from
`wasm-tools component wit <component>` and lets downstream consumers
pin against the exact interface without parsing the embedded WIT.

OCI-registry upload is a v0.6 concern (sigil's `wsc-component` isn't
on a registry yet either; once one of them lands an OCI publish step,
the other can copy it).

---

## 7. Composition with wsc (sigil's signing component)

Sigil's `wsc-component` (`sigil/src/component/`) exports
`wasm-signatures:wasmsign/signing@0.2.6` (world `signing-lib`,
`sigil/wit/worlds.wit:38`). v0.5 stretch goal: a `witness-attest`
composed component:

```wit
// witness/wit/composed.wit
package pulseengine:attest@0.5.0;
world witness-attest {
    import wasi:clocks/wall-clock@0.2.3;
    export attest-coverage: func(
        module-bytes: list<u8>, run-record: list<u8>,
        secret-key: list<u8>, harness: option<string>,
    ) -> result<list<u8>, string>;   // DSSE envelope JSON
}
```

```bash
wac plug --plug witness-core-v0.5.0.component.wasm \
         --plug wsc-component-v0.8.0.component.wasm \
         --output witness-attest-v0.5.0.component.wasm composed.wit
```

Ship `witness-core` standalone first; compose in v0.5.1 once the
WIT-package-URL resolution between `pulseengine:witness@0.5.0` and
`wasm-signatures:wasmsign@0.2.6` is exercised end-to-end.

---

## 8. v0.5 ships vs defers

**Ships:** workspace split (`witness-core` cdylib + `witness-cli`
binary) with shared `wit/witness.wit`; `witness-core-v0.5.0.component.wasm`
as a release asset alongside its extracted `.wit` companion; a
`component-build.yml` CI job (`cargo component build --release` +
`wasm-tools validate`); `docs/component-usage.md` with `wasmtime`-CLI
and Rust-host invocation snippets.

**Defers:**

1. Composed `witness-attest` (witness-core + wsc-component via `wac`) —
   sketched above; needs an end-to-end smoke test, slip to v0.5.1.
2. `run` as a component — requires wasmtime-in-wasmtime; over-engineered.
3. OCI-registry publish — wait for sigil to land first, copy workflow.
4. JS bindings via `jco transpile` — should Just Work, exercise in v0.6.
5. meld / loom / kiln component audit — sandbox blocked listing those
   repos in this run; rivet's workspace has no `walrus`/`wit-bindgen`/
   `cdylib` references, so witness-core is likely the second
   pulseengine component (after sigil's `wsc-component`).

---

## Blockers

- **wasmtime does not compile to wasm32-wasip2.** No workaround
  short of rewriting the JIT-based `run` mode against a different
  runtime (interp-only, e.g. `wasmi` 0.40+, which *does* compile to
  `wasip2`). Out of scope for v0.5 — ship `run` as a host-only
  feature. This is the only structural blocker.
- **No other blockers.** All other dependencies (`walrus`, `gimli`,
  `wasmparser`, `serde`, `sha2`, `serde_yaml`) are pure-Rust crates
  that compile cleanly to `wasm32-wasip2`.

---

## Citations

`witness/Cargo.toml:53-90` (deps); `witness/Cargo.lock` (wasmtime 42
native deps); `witness/src/lib.rs:37-46`; `witness/src/instrument.rs:199`;
`witness/src/decisions.rs:48`; `witness/src/predicate.rs:86`;
`witness/src/rivet_evidence.rs:139`; `witness/src/report.rs:59`;
`witness/src/run.rs:40-47` (wasmtime imports proving wasm32
incompatibility); `sigil/wit/signing.wit:8`; `sigil/wit/worlds.wit:38`;
`sigil/src/component/Cargo.toml:11-19`;
`sigil/src/component/src/lib.rs:1-156`.
