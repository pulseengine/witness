//! Run an instrumented Wasm module and collect coverage counters.
//!
//! # v0.1 strategy
//!
//! witness embeds `wasmtime` and instantiates the instrumented module
//! directly. For each invocation, it calls a user-specified export (the
//! `--invoke` argument) and then iterates the module's exported globals
//! that match the `__witness_counter_` prefix, reading each one's current
//! value.
//!
//! This bypasses the "cooperation protocol" problem entirely: nothing about
//! the module-under-test has to know about witness. The only observation
//! surface is the Wasm global-export interface, which every conformant
//! runtime supports.
//!
//! # What v0.1 does NOT do
//!
//! - WASI-preview1/2 host imports are wired with a default context (stdio
//!   inherited, no filesystem). For modules that need richer WASI, the v0.2
//!   `--harness <cmd>` subprocess mode is the right escape hatch.
//! - Component-model modules are not yet supported; only core modules.
//! - Calling an export with arguments from the CLI is limited to no-argument
//!   exports in v0.1. Parameterised invocations are v0.2.

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
    /// Exports to invoke, in order. Each export must take no parameters and
    /// return no more than one value in v0.1.
    pub invoke: Vec<String>,
    /// If true, call `_start` automatically before any `invoke` entries
    /// (the WASI "command" convention).
    pub call_start: bool,
}

/// Run the instrumented `module`, writing a `RunRecord` to `options.output`.
pub fn run_module(options: &RunOptions<'_>) -> Result<()> {
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
}
