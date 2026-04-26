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

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use wasmtime::{Config, Engine, Linker, Module, Store, Val};
use wasmtime_wasi::WasiCtxBuilder;
use wasmtime_wasi::p1::WasiP1Ctx;
use witness_core::Result;
use witness_core::error::Error;
use witness_core::instrument::{
    BRCNT_EXPORT_PREFIX, BRVAL_EXPORT_PREFIX, COUNTER_EXPORT_PREFIX, Manifest, ROW_RESET_EXPORT,
    TRACE_HEADER_BYTES, TRACE_MEMORY_EXPORT, TRACE_RESET_EXPORT,
};
use witness_core::run_record::{
    BranchHit, DecisionRecord, DecisionRow, HarnessSnapshot, RunRecord, TraceHealth,
};

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

    // v0.6.1: per-row capture. For each invoked export, reset the per-row
    // globals, run the export, capture its return value as the decision
    // outcome, and read the per-condition (brval, brcnt) pairs.
    let row_reset = instance.get_func(&mut store, ROW_RESET_EXPORT);
    // v0.7.2: trace-buffer reset/inspection. If the module exports the
    // trace memory + reset helper (= post-v0.7.2 instrumentation), call
    // reset before each row and read the cursor watermark after.
    let trace_reset = instance.get_func(&mut store, TRACE_RESET_EXPORT);
    let trace_memory = instance.get_memory(&mut store, TRACE_MEMORY_EXPORT);
    let mut rows_per_decision: BTreeMap<u32, Vec<DecisionRow>> = BTreeMap::new();
    for d in &manifest.decisions {
        rows_per_decision.insert(d.id, Vec::new());
    }
    let mut trace_bytes_total: u64 = 0;
    let mut trace_overflow_seen = false;
    let mut next_row_id: u32 = 0;

    // v0.7.3: build branch_id → (decision_id, condition_index) lookup
    // for the trace-record parser. One entry per condition of every
    // reconstructed decision in the manifest.
    let mut branch_to_decision: HashMap<u32, (u32, u32)> = HashMap::new();
    for d in &manifest.decisions {
        for (idx, &bid) in d.conditions.iter().enumerate() {
            let cond_idx_u32 = u32::try_from(idx).unwrap_or(u32::MAX);
            branch_to_decision.insert(bid, (d.id, cond_idx_u32));
        }
    }

    for name in &options.invoke {
        if let Some(reset) = row_reset {
            reset
                .call(&mut store, &[], &mut [])
                .map_err(|e| Error::Runtime(e.into()))?;
        }
        if let Some(treset) = trace_reset {
            treset
                .call(&mut store, &[], &mut [])
                .map_err(|e| Error::Runtime(e.into()))?;
        }

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

        // SAFETY-REVIEW: only `i32` is interpretable as a bool outcome.
        // Other Wasm value types (`i64`, `f32`, `f64`, `v128`, ref types)
        // are not coerced to bool — outcome stays `None`.
        #[allow(clippy::wildcard_enum_match_arm)]
        let outcome: Option<bool> = results.first().and_then(|v| match v {
            Val::I32(n) => Some(*n != 0),
            _ => None,
        });

        let (brvals, brcnts) = read_per_row_globals(&mut store, &instance)?;

        // v0.7.2: read the trace memory header to learn how many record
        // bytes were written this row. v0.7.3 walks the records and
        // emits one DecisionRow per iteration when trace data is
        // present; falls back to per-row-globals when not.
        let mut trace_iterations: BTreeMap<u32, Vec<BTreeMap<u32, bool>>> = BTreeMap::new();
        if let Some(mem) = trace_memory {
            let data = mem.data(&mut store);
            // SAFETY-REVIEW: the length check guards the indexes; the
            // trace memory's first 12 bytes are the cursor + capacity
            // + overflow_flag header populated by __witness_trace_reset.
            #[allow(clippy::indexing_slicing)]
            if data.len() >= 12 {
                let cursor = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let overflow = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                let bytes_this_row = cursor.saturating_sub(TRACE_HEADER_BYTES);
                trace_bytes_total = trace_bytes_total.saturating_add(u64::from(bytes_this_row));
                if overflow != 0 {
                    trace_overflow_seen = true;
                }
                if bytes_this_row > 0 {
                    trace_iterations = parse_trace_records(data, cursor, &branch_to_decision);
                }
            }
        }

        if !trace_iterations.is_empty() {
            // v0.7.3 — use per-iteration trace data. Each iteration
            // becomes its own DecisionRow with the row's function-
            // return outcome.
            for (dec_id, iters) in trace_iterations {
                for evaluated in iters {
                    rows_per_decision
                        .entry(dec_id)
                        .or_default()
                        .push(DecisionRow {
                            row_id: next_row_id,
                            evaluated,
                            outcome,
                        });
                    next_row_id = next_row_id.saturating_add(1);
                }
            }
        } else {
            // Fallback: v0.6.1 per-row-globals path.
            let row_id = next_row_id;
            next_row_id = next_row_id.saturating_add(1);
            for d in &manifest.decisions {
                let mut evaluated: BTreeMap<u32, bool> = BTreeMap::new();
                for (cond_idx, branch_id) in d.conditions.iter().enumerate() {
                    let cnt = brcnts.get(branch_id).copied().unwrap_or(0);
                    if cnt > 0 {
                        let val = brvals.get(branch_id).copied().unwrap_or(0);
                        let cond_idx_u32 = u32::try_from(cond_idx).unwrap_or(u32::MAX);
                        evaluated.insert(cond_idx_u32, val != 0);
                    }
                }
                rows_per_decision
                    .entry(d.id)
                    .or_default()
                    .push(DecisionRow {
                        row_id,
                        evaluated,
                        outcome,
                    });
            }
        }
    }

    let counter_values = read_counter_globals(&mut store, &instance)?;
    let mut record = build_run_record(&manifest, &counter_values, options.module, invoked);

    // v0.6.1: attach per-decision row tables and trace health.
    let mut decisions: Vec<DecisionRecord> = Vec::with_capacity(manifest.decisions.len());
    for d in &manifest.decisions {
        let rows = rows_per_decision.remove(&d.id).unwrap_or_default();
        decisions.push(DecisionRecord {
            id: d.id,
            source_file: d.source_file.clone(),
            source_line: d.source_line,
            condition_branch_ids: d.conditions.clone(),
            rows,
        });
    }
    let row_total = u64::try_from(options.invoke.len()).unwrap_or(u64::MAX);
    record.decisions = decisions;
    record.trace_health = TraceHealth {
        overflow: trace_overflow_seen,
        rows: row_total,
        // v0.7.2: ambiguous_rows now means "trace memory recorded data
        // beyond what the per-row globals could capture — likely loops".
        // v0.7.3 will replace this with the actual per-iteration row
        // emission. For now, the watermark itself is the signal.
        ambiguous_rows: trace_bytes_total > 0,
    };
    // v0.7.2: append trace-memory bytes-used as a structured note in
    // the invoked list so it's visible in the run JSON until the
    // schema gets a dedicated field in v0.7.3.
    if trace_bytes_total > 0 {
        record
            .invoked
            .push(format!("__witness_trace_bytes={trace_bytes_total}"));
    }
    record.save(options.output)
}

/// v0.7.3 — parse the trace memory's records into per-iteration
/// condition-vector maps grouped by decision_id.
///
/// Walk records in order. For each condition record (kind=0), look
/// up branch_id in `branch_to_decision`. Append the (cond_idx, value)
/// pair to the decision's "current iteration" map. When a duplicate
/// cond_idx appears (= the same condition fires again, meaning the
/// loop iterated), finalize the current iteration and start fresh.
///
/// Records other than `kind=0` (row-marker, decision-outcome) are
/// reserved for future use and skipped here.
fn parse_trace_records(
    data: &[u8],
    cursor: u32,
    branch_to_decision: &HashMap<u32, (u32, u32)>,
) -> BTreeMap<u32, Vec<BTreeMap<u32, bool>>> {
    use witness_core::instrument::{TRACE_HEADER_BYTES, TRACE_RECORD_BYTES};
    let mut current: BTreeMap<u32, BTreeMap<u32, bool>> = BTreeMap::new();
    let mut completed: BTreeMap<u32, Vec<BTreeMap<u32, bool>>> = BTreeMap::new();

    let header_usize = usize::try_from(TRACE_HEADER_BYTES).unwrap_or(usize::MAX);
    let record_usize = usize::try_from(TRACE_RECORD_BYTES).unwrap_or(4);
    let cursor_usize = usize::try_from(cursor).unwrap_or(usize::MAX);
    let mut offset = header_usize;
    let end = cursor_usize.min(data.len());

    while offset.saturating_add(record_usize) <= end {
        // SAFETY-REVIEW: bounded by `offset + record_usize <= end`.
        // The 4 explicit indexes below cover bytes[0..4]; that range is
        // exactly the slice we just took, so the indexes are in-bounds.
        #[allow(clippy::indexing_slicing)]
        let (branch_id, value, kind) = {
            let bytes = &data[offset..offset.saturating_add(record_usize)];
            (
                u32::from(u16::from_le_bytes([bytes[0], bytes[1]])),
                bytes[2] != 0,
                bytes[3],
            )
        };
        offset = offset.saturating_add(record_usize);

        if kind != 0 {
            // row-marker / outcome / reserved — v0.7.3 first pass skips.
            continue;
        }

        let Some(&(dec_id, cond_idx)) = branch_to_decision.get(&branch_id) else {
            continue;
        };

        let entry = current.entry(dec_id).or_default();
        if entry.contains_key(&cond_idx) {
            // Duplicate condition_index for this decision → end of iteration.
            let finished = std::mem::take(entry);
            completed.entry(dec_id).or_default().push(finished);
        }
        // Re-fetch entry (it was just emptied if duplicate fired).
        current.entry(dec_id).or_default().insert(cond_idx, value);
    }

    // Flush trailing in-progress iterations.
    for (dec_id, iter_map) in current {
        if !iter_map.is_empty() {
            completed.entry(dec_id).or_default().push(iter_map);
        }
    }

    completed
}

/// Read the per-row `__witness_brval_<id>` and `__witness_brcnt_<id>`
/// globals into two maps keyed by branch id. Globals not present
/// (e.g. for `BrTable*` branches that don't get per-row capture) are
/// simply absent from the maps.
fn read_per_row_globals(
    store: &mut Store<WasiP1Ctx>,
    instance: &wasmtime::Instance,
) -> Result<(HashMap<u32, i32>, HashMap<u32, i32>)> {
    let mut brvals: HashMap<u32, i32> = HashMap::new();
    let mut brcnts: HashMap<u32, i32> = HashMap::new();
    let names: Vec<String> = instance
        .exports(&mut *store)
        .map(|e| e.name().to_string())
        .collect();
    for name in names {
        let (target, prefix): (&mut HashMap<u32, i32>, &str) =
            if name.starts_with(BRVAL_EXPORT_PREFIX) {
                (&mut brvals, BRVAL_EXPORT_PREFIX)
            } else if name.starts_with(BRCNT_EXPORT_PREFIX) {
                (&mut brcnts, BRCNT_EXPORT_PREFIX)
            } else {
                continue;
            };
        let Some(id_str) = name.strip_prefix(prefix) else {
            continue;
        };
        let Ok(id) = id_str.parse::<u32>() else {
            continue;
        };
        let Some(global) = instance.get_global(&mut *store, &name) else {
            continue;
        };
        // SAFETY-REVIEW: brval/brcnt globals are i32 by construction
        // (instrument.rs allocates them as `ValType::I32`). Any other
        // type at runtime is a tampering signal; skip and continue.
        #[allow(clippy::wildcard_enum_match_arm)]
        if let Val::I32(v) = global.get(&mut *store) {
            target.insert(id, v);
        }
    }
    Ok((brvals, brcnts))
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
