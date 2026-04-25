//! Wasmtime-embedded runner for `witness run`.
//!
//! Witness depends on `witness-core` for the run-record types
//! ([`witness_core::run_record::RunRecord`] et al.) and adds the
//! `wasmtime`-based execution path here. The pure-data merge / load /
//! save APIs live in witness-core; only the runtime concerns stay in
//! the binary crate (which can't compile to `wasm32-wasip2` because
//! wasmtime doesn't).
//!
//! # Two execution modes (since v0.2)
//!
//! - **Embedded wasmtime** (default). Instantiate the module, invoke
//!   the user-specified exports, read each `__witness_counter_*`
//!   global, write the run JSON.
//! - **Subprocess harness** (`--harness <cmd>`). Spawn a subprocess
//!   with `WITNESS_MODULE` / `WITNESS_MANIFEST` / `WITNESS_OUTPUT` env
//!   vars set; merge its counter snapshot with the manifest.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use wasmtime::{Config, Engine, Linker, Module, Store, Val};
use wasmtime_wasi::WasiCtxBuilder;
use wasmtime_wasi::p1::WasiP1Ctx;
use witness_core::Result;
use witness_core::error::Error;
use witness_core::instrument::{COUNTER_EXPORT_PREFIX, Manifest};
use witness_core::run_record::{BranchHit, HarnessSnapshot, RunRecord, TraceHealth};

/// Options for `run_module`. Constructed by the CLI layer; exposed as a
/// struct so library callers can drive witness programmatically.
pub struct RunOptions<'a> {
    pub module: &'a Path,
    pub manifest: PathBuf,
    pub output: &'a Path,
    /// Exports to invoke (embedded-runtime mode). Each export must take
    /// no parameters and return zero or one value.
    pub invoke: Vec<String>,
    /// If true, call `_start` automatically before any `invoke` entries
    /// (the WASI "command" convention). Ignored when `harness` is `Some`.
    pub call_start: bool,
    /// Subprocess harness command. When set, witness spawns this command
    /// instead of running the module via embedded wasmtime.
    pub harness: Option<String>,
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
        let ty = func.ty(&store);
        let mut results: Vec<Val> = ty.results().map(|_| Val::I32(0)).collect();
        func.call(&mut store, &[], &mut results)
            .map_err(|e| Error::Runtime(e.into()))?;
        invoked.push(name.clone());
    }

    let counter_values = read_counter_globals(&mut store, &instance)?;
    let record = build_run_record(&manifest, &counter_values, options.module, invoked);
    record.save(options.output)
}

fn run_via_harness(options: &RunOptions<'_>, harness_cmd: &str) -> Result<()> {
    let manifest = Manifest::load(&options.manifest)?;

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

fn build_run_record(
    manifest: &Manifest,
    counter_values: &HashMap<u32, u64>,
    module_path: &Path,
    invoked: Vec<String>,
) -> RunRecord {
    let entries_by_id: HashMap<u32, &witness_core::instrument::BranchEntry> =
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
        decisions: vec![],
        trace_health: TraceHealth::default(),
    }
}

fn read_counter_globals(
    store: &mut Store<WasiP1Ctx>,
    instance: &wasmtime::Instance,
) -> Result<HashMap<u32, u64>> {
    let mut out: HashMap<u32, u64> = HashMap::new();
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
        // SAFETY-REVIEW: counters initialised to 0; reinterpreting the
        // signed two's-complement value as unsigned preserves magnitude
        // for any non-wrapped counter.
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
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use witness_core::instrument::{BranchKind, instrument_module};

    fn instrument_and_emit(
        wat_src: &str,
        out_path: &Path,
    ) -> Vec<witness_core::instrument::BranchEntry> {
        let wasm = wat::parse_str(wat_src).unwrap();
        let mut module = walrus::Module::from_buffer(&wasm).unwrap();
        let entries = instrument_module(&mut module, "test").unwrap();
        std::fs::write(out_path, module.emit_wasm()).unwrap();
        entries
    }

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
            decisions: vec![],
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
            decisions: vec![],
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
            decisions: vec![],
        };
        std::fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();
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
        assert_eq!(then_hit.hits, 7);
    }

    #[test]
    fn harness_subprocess_failure_propagates() {
        let dir = tempdir().unwrap();
        let manifest = Manifest {
            schema_version: "1".to_string(),
            witness_version: "test".to_string(),
            module_source: "fake".to_string(),
            branches: vec![],
            decisions: vec![],
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
