//! Run an instrumented Wasm module and collect coverage counters.
//!
//! # Two execution modes
//!
//! ## Embedded wasmtime (default, since v0.1)
//!
//! witness embeds `wasmtime` and instantiates the instrumented module
//! directly. For each invocation, it calls a user-specified export (the
//! `--invoke` argument) and then iterates the module's exported globals
//! that match the `__witness_counter_` prefix, reading each one's current
//! value. No cooperation from the module-under-test required.
//!
//! ## Subprocess harness (`--harness <cmd>`, since v0.2)
//!
//! When the embedded runtime is not enough — e.g. a `wasm-bindgen-test`
//! suite running in Node, a custom WASI capability profile, or
//! component-model modules — `--harness` spawns a subprocess and lets it
//! drive the runtime. The harness's only cooperation cost is reading the
//! counter snapshot at the end of its run and writing a JSON snapshot
//! file to the path witness supplies via `WITNESS_OUTPUT`.
//!
//! Protocol (file-based handshake, see DEC-009):
//!
//! 1. witness sets three env vars before spawning the harness:
//!    - `WITNESS_MODULE` — path to the instrumented `.wasm`
//!    - `WITNESS_MANIFEST` — path to the `.witness.json` manifest
//!    - `WITNESS_OUTPUT` — path the harness must write its snapshot to
//! 2. The harness loads the module in its native runtime, runs tests, and
//!    before exiting writes a snapshot JSON document of shape
//!    `{"schema": "witness-harness-v1", "counters": {"<branch_id>": <hits>}}`.
//! 3. witness joins the snapshot with the manifest and writes the
//!    full run JSON to the user's `--output` path.
//!
//! # What v0.2 does NOT do
//!
//! - Component-model modules are not yet supported; only core modules.
//! - Calling an export with arguments from the CLI is limited to no-argument
//!   exports in v0.2. Parameterised invocations remain a v0.3 concern.

use crate::instrument::{BranchEntry, COUNTER_EXPORT_PREFIX, Manifest};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use wasmtime::{Config, Engine, Linker, Module, Store, Val};
use wasmtime_wasi::WasiCtxBuilder;
use wasmtime_wasi::p1::WasiP1Ctx;

/// Raw-run output: each branch paired with the counter's final value.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RunRecord {
    pub schema_version: String,
    pub witness_version: String,
    pub module_path: String,
    pub invoked: Vec<String>,
    pub branches: Vec<BranchHit>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BranchHit {
    pub id: u32,
    pub function_index: u32,
    pub function_name: Option<String>,
    pub kind: crate::instrument::BranchKind,
    pub instr_index: u32,
    pub hits: u64,
}

impl RunRecord {
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(Error::Serde)?;
        std::fs::write(path, json).map_err(Error::Io)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).map_err(Error::Io)?;
        serde_json::from_slice(&bytes).map_err(|source| Error::RunOutput {
            path: path.to_path_buf(),
            source,
        })
    }
}

/// Options for `run_module`. Constructed by the CLI layer; exposed as a
/// struct so library callers can drive witness programmatically.
pub struct RunOptions<'a> {
    pub module: &'a Path,
    pub manifest: PathBuf,
    pub output: &'a Path,
    /// Exports to invoke (embedded-runtime mode). Each export must take
    /// no parameters and return zero or one value in v0.2. Ignored when
    /// `harness` is `Some`.
    pub invoke: Vec<String>,
    /// If true, call `_start` automatically before any `invoke` entries
    /// (the WASI "command" convention). Ignored when `harness` is `Some`.
    pub call_start: bool,
    /// Subprocess harness command. When set, witness spawns this command
    /// instead of running the module via embedded wasmtime. The harness
    /// must read `WITNESS_MODULE` / `WITNESS_MANIFEST` and write a
    /// counter snapshot to `WITNESS_OUTPUT`.
    pub harness: Option<String>,
}

/// Counter snapshot the harness writes; the bridge format between
/// subprocess harnesses and witness's run-record assembly.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HarnessSnapshot {
    pub schema: String,
    pub counters: HashMap<String, u64>,
}

impl HarnessSnapshot {
    pub const SCHEMA: &'static str = "witness-harness-v1";

    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).map_err(Error::Io)?;
        serde_json::from_slice(&bytes).map_err(|source| Error::RunOutput {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Convert string-keyed counters to `branch_id -> hits` for merging.
    pub fn into_id_map(self) -> Result<HashMap<u32, u64>> {
        let mut out = HashMap::new();
        for (k, v) in self.counters {
            let id = k.parse::<u32>().map_err(|_| {
                Error::Runtime(anyhow::anyhow!(
                    "harness snapshot contains non-numeric counter id `{k}`"
                ))
            })?;
            out.insert(id, v);
        }
        Ok(out)
    }
}

/// Run the instrumented `module`, writing a `RunRecord` to `options.output`.
pub fn run_module(options: &RunOptions<'_>) -> Result<()> {
    if let Some(cmd) = options.harness.as_deref() {
        return run_via_harness(options, cmd);
    }
    run_via_embedded(options)
}

fn run_via_embedded(options: &RunOptions<'_>) -> Result<()> {
    let manifest = Manifest::load(&options.manifest)?;

    let mut config = Config::new();
    config.wasm_component_model(false);
    let engine = Engine::new(&config).map_err(|e| Error::Runtime(e.into()))?;

    let wasm_bytes = std::fs::read(options.module).map_err(Error::Io)?;
    let module = Module::from_binary(&engine, &wasm_bytes).map_err(|e| Error::Runtime(e.into()))?;

    let wasi = WasiCtxBuilder::new().inherit_stdio().build_p1();
    let mut store: Store<WasiP1Ctx> = Store::new(&engine, wasi);

    let mut linker: Linker<WasiP1Ctx> = Linker::new(&engine);
    wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |cx| cx)
        .map_err(|e| Error::Runtime(e.into()))?;

    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|e| Error::Runtime(e.into()))?;

    let mut invoked: Vec<String> = Vec::new();
    if options.call_start
        && let Some(start) = instance.get_func(&mut store, "_start")
    {
        start
            .call(&mut store, &[], &mut [])
            .map_err(|e| Error::Runtime(e.into()))?;
        invoked.push("_start".to_string());
    }

    for name in &options.invoke {
        let func = instance.get_func(&mut store, name).ok_or_else(|| {
            Error::Runtime(anyhow::anyhow!(
                "export `{name}` not found in instrumented module"
            ))
        })?;
        // v0.1: no arguments, up to one return value. Size results buffer to
        // the export's declared result count.
        let ty = func.ty(&store);
        let mut results: Vec<Val> = ty.results().map(|_| Val::I32(0)).collect();
        func.call(&mut store, &[], &mut results)
            .map_err(|e| Error::Runtime(e.into()))?;
        invoked.push(name.clone());
    }

    // Read each counter global.
    let counter_values = read_counter_globals(&mut store, &instance)?;

    let entries_by_id: HashMap<u32, &BranchEntry> =
        manifest.branches.iter().map(|b| (b.id, b)).collect();
    let mut branches: Vec<BranchHit> = manifest
        .branches
        .iter()
        .map(|b| {
            let hits = counter_values.get(&b.id).copied().unwrap_or(0);
            BranchHit {
                id: b.id,
                function_index: b.function_index,
                function_name: entries_by_id
                    .get(&b.id)
                    .and_then(|e| e.function_name.clone()),
                kind: b.kind,
                instr_index: b.instr_index,
                hits,
            }
        })
        .collect();
    branches.sort_by_key(|b| b.id);

    let record = RunRecord {
        schema_version: manifest.schema_version.clone(),
        witness_version: env!("CARGO_PKG_VERSION").to_string(),
        module_path: options.module.to_string_lossy().into_owned(),
        invoked,
        branches,
    };
    record.save(options.output)
}

/// Subprocess-harness mode entry point.
fn run_via_harness(options: &RunOptions<'_>, harness_cmd: &str) -> Result<()> {
    let manifest = Manifest::load(&options.manifest)?;

    // Create a temp file for the harness's snapshot. We don't reuse the
    // user's --output path because that path holds the *full* RunRecord;
    // the harness only writes the counter slice.
    let snapshot_dir = tempfile::tempdir().map_err(Error::Io)?;
    let snapshot_path = snapshot_dir.path().join("witness-harness-snapshot.json");

    let module_abs = options
        .module
        .canonicalize()
        .map_err(Error::Io)?
        .to_string_lossy()
        .into_owned();
    let manifest_abs = options
        .manifest
        .canonicalize()
        .map_err(Error::Io)?
        .to_string_lossy()
        .into_owned();
    let snapshot_str = snapshot_path.to_string_lossy().into_owned();

    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(harness_cmd)
        .env("WITNESS_MODULE", &module_abs)
        .env("WITNESS_MANIFEST", &manifest_abs)
        .env("WITNESS_OUTPUT", &snapshot_str)
        .status()
        .map_err(Error::Io)?;

    if !status.success() {
        return Err(Error::Harness {
            command: harness_cmd.to_string(),
            code: status.code(),
            stderr: String::new(),
        });
    }

    if !snapshot_path.exists() {
        return Err(Error::Runtime(anyhow::anyhow!(
            "harness completed but did not write a snapshot to WITNESS_OUTPUT ({})",
            snapshot_str
        )));
    }

    let snapshot = HarnessSnapshot::load(&snapshot_path)?;
    if snapshot.schema != HarnessSnapshot::SCHEMA {
        return Err(Error::Runtime(anyhow::anyhow!(
            "harness snapshot schema mismatch: expected `{}`, got `{}`",
            HarnessSnapshot::SCHEMA,
            snapshot.schema
        )));
    }
    let counter_values = snapshot.into_id_map()?;

    let record = build_run_record(&manifest, &counter_values, options.module, vec![]);
    record.save(options.output)
}

/// Assemble a `RunRecord` from a manifest and counter snapshot. Shared
/// between embedded and subprocess execution paths.
fn build_run_record(
    manifest: &Manifest,
    counter_values: &HashMap<u32, u64>,
    module_path: &Path,
    invoked: Vec<String>,
) -> RunRecord {
    let entries_by_id: HashMap<u32, &BranchEntry> =
        manifest.branches.iter().map(|b| (b.id, b)).collect();
    let mut branches: Vec<BranchHit> = manifest
        .branches
        .iter()
        .map(|b| {
            let hits = counter_values.get(&b.id).copied().unwrap_or(0);
            BranchHit {
                id: b.id,
                function_index: b.function_index,
                function_name: entries_by_id
                    .get(&b.id)
                    .and_then(|e| e.function_name.clone()),
                kind: b.kind,
                instr_index: b.instr_index,
                hits,
            }
        })
        .collect();
    branches.sort_by_key(|b| b.id);

    RunRecord {
        schema_version: manifest.schema_version.clone(),
        witness_version: env!("CARGO_PKG_VERSION").to_string(),
        module_path: module_path.to_string_lossy().into_owned(),
        invoked,
        branches,
    }
}

fn read_counter_globals(
    store: &mut Store<WasiP1Ctx>,
    instance: &wasmtime::Instance,
) -> Result<HashMap<u32, u64>> {
    let mut out: HashMap<u32, u64> = HashMap::new();
    // Snapshot the export names first to end the immutable borrow before
    // we take the mutable &mut store each get_global needs.
    let names: Vec<String> = instance
        .exports(&mut *store)
        .map(|e| e.name().to_string())
        .collect();
    for name in names {
        let Some(id_str) = name.strip_prefix(COUNTER_EXPORT_PREFIX) else {
            continue;
        };
        let Ok(id) = id_str.parse::<u32>() else {
            continue;
        };
        let Some(global) = instance.get_global(&mut *store, &name) else {
            continue;
        };
        let value = global.get(&mut *store);
        // SAFETY-REVIEW: counters are initialised to 0 and only ever
        // incremented by 1, so reinterpreting the signed two's-complement
        // value as unsigned preserves the magnitude for any non-wrapped
        // counter. If a counter has wrapped (2^31 hits), we would need u64
        // semantics anyway and the reinterpretation is the right choice.
        #[allow(
            clippy::wildcard_enum_match_arm,
            clippy::cast_sign_loss,
            clippy::as_conversions
        )]
        let hits = match value {
            Val::I32(v) => u64::from(v as u32),
            Val::I64(v) => v as u64,
            other => {
                return Err(Error::Runtime(anyhow::anyhow!(
                    "counter `{name}` has unexpected type {other:?}"
                )));
            }
        };
        out.insert(id, hits);
    }
    Ok(out)
}

#[cfg(test)]
// SAFETY-REVIEW: tests use `.unwrap()` / `.expect()` intentionally to
// surface failures as panics.
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod tests {
    use super::*;
    use crate::instrument::{BranchKind, instrument_module};
    use tempfile::tempdir;

    fn instrument_and_emit(wat_src: &str, out_path: &Path) -> Vec<BranchEntry> {
        let wasm = wat::parse_str(wat_src).unwrap();
        let mut module = walrus::Module::from_buffer(&wasm).unwrap();
        let entries = instrument_module(&mut module, "test").unwrap();
        std::fs::write(out_path, module.emit_wasm()).unwrap();
        entries
    }

    /// End-to-end: instrument a module, run the `then` arm via a no-arg
    /// thunk, verify the correct counter fires and the `else` counter does
    /// not. v0.1 only supports no-argument exports, so the test WAT exposes
    /// `hit_then` / `hit_else` thunks that wrap the real `choose(i32)` fn.
    #[test]
    fn round_trip_if_else_then_arm() {
        let wat_src = r#"
            (module
              (func $choose (param i32) (result i32)
                local.get 0
                if (result i32)
                  i32.const 42
                else
                  i32.const 99
                end)
              (func (export "hit_then") (result i32)
                i32.const 1
                call $choose)
              (func (export "hit_else") (result i32)
                i32.const 0
                call $choose))
        "#;
        let dir = tempdir().unwrap();
        let wasm_path = dir.path().join("prog.wasm");
        let manifest_path = dir.path().join("prog.wasm.witness.json");
        let run_path = dir.path().join("run.json");
        let entries = instrument_and_emit(wat_src, &wasm_path);
        let manifest = Manifest {
            schema_version: "1".to_string(),
            witness_version: "test".to_string(),
            module_source: wasm_path.to_string_lossy().into_owned(),
            branches: entries,
        };
        std::fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();

        let options = RunOptions {
            module: &wasm_path,
            manifest: manifest_path,
            output: &run_path,
            invoke: vec!["hit_then".to_string()],
            call_start: false,
            harness: None,
        };
        run_module(&options).unwrap();

        let record = RunRecord::load(&run_path).unwrap();
        let then_hit = record
            .branches
            .iter()
            .find(|b| b.kind == BranchKind::IfThen)
            .expect("then branch");
        let else_hit = record
            .branches
            .iter()
            .find(|b| b.kind == BranchKind::IfElse)
            .expect("else branch");
        assert_eq!(then_hit.hits, 1, "then arm should fire once");
        assert_eq!(else_hit.hits, 0, "else arm should not fire");
    }

    #[test]
    fn round_trip_br_if_taken() {
        let dir = tempdir().unwrap();
        let wasm_path = dir.path().join("prog.wasm");
        let manifest_path = dir.path().join("prog.wasm.witness.json");
        let run_path = dir.path().join("run.json");

        // br_if with a block result: value on stack first, then condition,
        // then br_if. If taken, block yields the value; if not, we drop it
        // and fall through to the next path.
        let wat_src = r#"
            (module
              (func (export "take_branch") (result i32)
                (block $exit (result i32)
                  i32.const 42
                  i32.const 1
                  br_if $exit
                  drop
                  i32.const 99)))
        "#;
        let entries = instrument_and_emit(wat_src, &wasm_path);
        let manifest = Manifest {
            schema_version: "1".to_string(),
            witness_version: "test".to_string(),
            module_source: wasm_path.to_string_lossy().into_owned(),
            branches: entries,
        };
        std::fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();

        let options = RunOptions {
            module: &wasm_path,
            manifest: manifest_path,
            output: &run_path,
            invoke: vec!["take_branch".to_string()],
            call_start: false,
            harness: None,
        };
        run_module(&options).unwrap();

        let record = RunRecord::load(&run_path).unwrap();
        let brif_hit = record
            .branches
            .iter()
            .find(|b| b.kind == BranchKind::BrIf)
            .expect("br_if branch");
        assert_eq!(brif_hit.hits, 1, "br_if taken path should fire once");
    }

    /// Subprocess-harness round-trip. The harness here is a tiny shell
    /// script that writes a fake snapshot — proves the file-handshake
    /// protocol works without needing a real wasm-bindgen-test
    /// installation in CI.
    #[test]
    fn harness_subprocess_round_trip() {
        let wat_src = r#"
            (module
              (func (export "f") (param i32) (result i32)
                local.get 0
                if (result i32)
                  i32.const 1
                else
                  i32.const 0
                end))
        "#;
        let dir = tempdir().unwrap();
        let wasm_path = dir.path().join("prog.wasm");
        let manifest_path = dir.path().join("prog.wasm.witness.json");
        let run_path = dir.path().join("run.json");
        let entries = instrument_and_emit(wat_src, &wasm_path);
        let manifest = Manifest {
            schema_version: "1".to_string(),
            witness_version: "test".to_string(),
            module_source: wasm_path.to_string_lossy().into_owned(),
            branches: entries,
        };
        std::fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();

        // The harness pretends to have run the module and observed the
        // then-arm taken (counter 0 = 7 hits) and the else-arm not taken
        // (counter 1 absent → defaults to 0 in the merge step).
        let harness_cmd = r#"cat > "$WITNESS_OUTPUT" <<'EOF'
{
  "schema": "witness-harness-v1",
  "counters": { "0": 7 }
}
EOF"#;

        let options = RunOptions {
            module: &wasm_path,
            manifest: manifest_path,
            output: &run_path,
            invoke: vec![],
            call_start: false,
            harness: Some(harness_cmd.to_string()),
        };
        run_module(&options).unwrap();

        let record = RunRecord::load(&run_path).unwrap();
        let then_hit = record
            .branches
            .iter()
            .find(|b| b.kind == BranchKind::IfThen)
            .expect("then branch");
        assert_eq!(then_hit.hits, 7, "harness-supplied counter 0 should be 7");
        let else_hit = record
            .branches
            .iter()
            .find(|b| b.kind == BranchKind::IfElse)
            .expect("else branch");
        assert_eq!(else_hit.hits, 0, "absent counter defaults to 0");
    }

    /// Per-target br_table round-trip. The instrumented module fires only
    /// the counter for the actually-taken target. We exercise three
    /// distinct selectors via three exported thunks.
    #[test]
    fn round_trip_br_table_per_target() {
        // br_table with 2 explicit targets + 1 default. selector=0 → exit
        // outer block via target 0 (label 0 from inner block, returns 100);
        // selector=1 → target 1 (label 1, returns 200); selector >= 2 →
        // default (label 2, returns 300).
        let wat_src = r#"
            (module
              (func $sel (param $s i32) (result i32)
                (block $default
                  (block $b
                    (block $a
                      local.get $s
                      br_table $a $b $default
                    )
                    i32.const 100
                    return
                  )
                  i32.const 200
                  return
                )
                i32.const 300)
              (func (export "hit_target_0") (result i32)
                i32.const 0
                call $sel)
              (func (export "hit_target_1") (result i32)
                i32.const 1
                call $sel)
              (func (export "hit_default") (result i32)
                i32.const 5
                call $sel))
        "#;
        let dir = tempdir().unwrap();
        let wasm_path = dir.path().join("prog.wasm");
        let manifest_path = dir.path().join("prog.wasm.witness.json");

        let entries = instrument_and_emit(wat_src, &wasm_path);
        let manifest = Manifest {
            schema_version: "2".to_string(),
            witness_version: "test".to_string(),
            module_source: wasm_path.to_string_lossy().into_owned(),
            branches: entries,
        };
        std::fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();

        // Run hit_target_0 — only target-0 counter should fire.
        let run_path = dir.path().join("run0.json");
        let options = RunOptions {
            module: &wasm_path,
            manifest: manifest_path.clone(),
            output: &run_path,
            invoke: vec!["hit_target_0".to_string()],
            call_start: false,
            harness: None,
        };
        run_module(&options).unwrap();
        let record = RunRecord::load(&run_path).unwrap();
        let hits_target_0_total: u64 = record
            .branches
            .iter()
            .filter(|b| b.kind == BranchKind::BrTableTarget)
            .map(|b| b.hits)
            .sum();
        let hits_default: u64 = record
            .branches
            .iter()
            .filter(|b| b.kind == BranchKind::BrTableDefault)
            .map(|b| b.hits)
            .sum();
        assert_eq!(
            hits_target_0_total, 1,
            "exactly one target counter should fire on selector=0"
        );
        assert_eq!(hits_default, 0, "default counter should not fire");

        // Run hit_default — only default counter should fire.
        let run_path_def = dir.path().join("run_def.json");
        let options_def = RunOptions {
            module: &wasm_path,
            manifest: manifest_path,
            output: &run_path_def,
            invoke: vec!["hit_default".to_string()],
            call_start: false,
            harness: None,
        };
        run_module(&options_def).unwrap();
        let record_def = RunRecord::load(&run_path_def).unwrap();
        let default_hits: u64 = record_def
            .branches
            .iter()
            .filter(|b| b.kind == BranchKind::BrTableDefault)
            .map(|b| b.hits)
            .sum();
        assert_eq!(default_hits, 1, "default counter fires on selector=5");
    }

    #[test]
    fn harness_subprocess_failure_propagates() {
        let dir = tempdir().unwrap();
        let manifest = Manifest {
            schema_version: "1".to_string(),
            witness_version: "test".to_string(),
            module_source: "fake".to_string(),
            branches: vec![],
        };
        let manifest_path = dir.path().join("manifest.json");
        let module_path = dir.path().join("prog.wasm");
        let run_path = dir.path().join("run.json");
        std::fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();
        std::fs::write(&module_path, b"\x00asm\x01\x00\x00\x00").unwrap();

        let options = RunOptions {
            module: &module_path,
            manifest: manifest_path,
            output: &run_path,
            invoke: vec![],
            call_start: false,
            harness: Some("exit 1".to_string()),
        };
        let result = run_module(&options);
        assert!(matches!(result, Err(Error::Harness { .. })));
    }
}
