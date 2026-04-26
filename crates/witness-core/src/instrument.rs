//! Wasm instrumentation — inserts branch counters at every decision point.
//!
//! # v0.1 strategy
//!
//! For each branch instruction (`br_if`, `if/else`, `br_table`) in every local
//! function, allocate a mutable `i32` global initialised to zero and insert
//! instructions that increment it on the *taken* path. Each counter is
//! exported as `__witness_counter_<id>` so any host runtime that can read
//! Wasm globals can extract coverage — no cooperation protocol with the
//! module-under-test is required.
//!
//! The instrumentation patterns:
//!
//! | Source instruction | Counters | Rewrite |
//! |--------------------|----------|---------|
//! | `br_if L`          | 1 (taken) | `local.tee $tmp; if (inc counter) end; local.get $tmp; br_if L` |
//! | `if A else B end`  | 2 (then, else) | prepend counter increment to each arm |
//! | `br_table`         | 1 (executed) | prepend counter increment before `br_table` |
//!
//! `br_table` is counted as a single "executed" point in v0.1. Per-target
//! counting requires reconstructing which arm was taken from the selector,
//! which is a v0.2 concern alongside DWARF-informed decision reconstruction.
//!
//! # Semantic preservation
//!
//! For `br_if`, `local.tee` duplicates the condition onto a local without
//! removing it from the stack; the counter-increment `if` consumes the
//! duplicated value and the original condition remains for the final
//! `br_if`. Stack shape before and after the rewrite is identical.
//!
//! For `if/else`, the prepended instructions have no stack effect (global
//! increment only), so the arm's expected stack shape is preserved.
//!
//! For `br_table`, the prepended instructions have no stack effect, and the
//! selector on top of the stack is untouched.
//!
//! # What v0.1 does NOT do
//!
//! - MC/DC condition decomposition (v0.2, DWARF-informed).
//! - Per-target `br_table` counting (v0.2).
//! - Cross-component coverage for meld-fused modules (v0.4).
//! - Variant-aware scope pruning (v0.4).

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walrus::ir::{
    BinaryOp, Binop, BrIf, Const, GlobalGet, GlobalSet, IfElse, Instr, InstrSeqId, InstrSeqType,
    LoadKind, LocalGet, LocalTee, MemArg, Store, StoreKind, Value,
};
use walrus::{
    ConstExpr, FunctionBuilder, FunctionId, GlobalId, LocalId, MemoryId, Module, ValType,
};

/// Manifest schema version. v0.1 pinned to "1"; v0.2 bumps to "2" for
/// per-target `br_table` entries (`target_index` field). Old v0.1 manifests
/// are still readable; new v0.2 manifests advertise schema "2".
pub const MANIFEST_SCHEMA_VERSION: &str = "2";

/// Exported-global name prefix. Hosts discover counters by iterating exports
/// and matching on this prefix; the suffix is the branch id as decimal.
pub const COUNTER_EXPORT_PREFIX: &str = "__witness_counter_";

/// Helper-function name prefix for the `br_table` per-target dispatcher
/// (DEC-008). Each `br_table` site gets one helper, named with the base
/// branch id of its first target.
pub const BRTABLE_HELPER_PREFIX: &str = "__witness_brtable_";

/// v0.6.1 — per-row condition value. For `BrIf` branches: the evaluated
/// condition value (0 or 1) when reached this row. For `IfThen`/`IfElse`
/// arms: 1 when the arm fired this row, 0 otherwise. Set by instrumentation;
/// zeroed by `__witness_row_reset` between row invocations. Only allocated
/// for `BrIf` / `IfThen` / `IfElse` branches; `BrTable*` branches stay on
/// counter-only per DEC-015.
pub const BRVAL_EXPORT_PREFIX: &str = "__witness_brval_";

/// v0.6.1 — per-row evaluation count. Increments each time the branch is
/// reached this row. The runner reads it after each row invocation to
/// determine "was this condition evaluated this row?" — non-zero means
/// evaluated, zero means short-circuited (absent from `DecisionRow.evaluated`).
pub const BRCNT_EXPORT_PREFIX: &str = "__witness_brcnt_";

/// v0.6.1 — exported helper function the runner calls between row
/// invocations to zero all `BRVAL` / `BRCNT` globals so the next row's
/// captures don't leak prior state.
pub const ROW_RESET_EXPORT: &str = "__witness_row_reset";

/// v0.7.2 — exported memory carrying the per-iteration trace buffer.
/// Each `br_if` instrumentation appends a 4-byte record to this memory
/// at offset `cursor` (where `cursor` is stored at byte offset 0 of
/// the memory itself). Lifts the per-row-globals limitation that
/// caps loop-bearing programs (e.g. httparse) at 0/N full MC/DC,
/// because per-iteration condition vectors are preserved.
///
/// Memory layout:
///   offset 0:    u32 cursor (next write offset, in bytes)
///   offset 4:    u32 capacity (in bytes; runner can read to know when overflow looms)
///   offset 8:    u32 overflow_flag (set by the writer when records would exceed capacity)
///   offset 12:   u32 reserved
///   offset 16+:  records
///
/// Each record (4 bytes, little-endian):
///   bytes 0..2:  u16 branch_id (cap u16; v0.7.2 modules with > 65535 branches
///                 are flagged in the manifest schema as v4 with widened records)
///   byte  2:     u8 value (0 or 1)
///   byte  3:     u8 record_kind (0 = condition, 1 = row-marker, 2 = decision-outcome)
pub const TRACE_MEMORY_EXPORT: &str = "__witness_trace";

/// Trace memory header size in bytes (cursor + capacity + overflow + reserved).
pub const TRACE_HEADER_BYTES: u32 = 16;

/// Trace record size in bytes. Power of two so cursor advance is a
/// single `i32.add 4`.
pub const TRACE_RECORD_BYTES: u32 = 4;

/// Default trace memory size in pages (1 page = 64 KiB). 16 pages =
/// 1 MiB = 262128 records max (after subtracting the 16-byte header
/// from one full page worth = 16384 records minus header overhead).
/// Configurable via the `WITNESS_TRACE_PAGES` env var the host
/// honours when growing the memory; v0.7.2 ships fixed-size.
pub const TRACE_DEFAULT_PAGES: u32 = 16;

/// v0.7.2 — exported helper function: zero the trace cursor +
/// overflow_flag (does not memset the record region; stale data
/// past the new cursor is invisible to the reader and overwritten
/// by next writes). Cheap.
pub const TRACE_RESET_EXPORT: &str = "__witness_trace_reset";

/// v0.7.2 — exported helper function: append a row-marker record
/// `(row_id_lo as branch_id slot, 0, kind=1)`. Called by the runner
/// between row invocations.
pub const TRACE_ROW_MARKER_EXPORT: &str = "__witness_trace_row_marker";

/// v0.7.2 — internal helper function called by per-br_if
/// instrumentation. Takes `(branch_id: i32, value: i32)` and appends
/// a 4-byte record `(branch_id u16, value u8, kind=0 u8)` to the
/// trace memory at cursor, then advances cursor by 4. Not exported —
/// only the instrumented module's br_if sites call it.
const TRACE_RECORD_HELPER_NAME: &str = "__witness_trace_record";

/// Kind of branch a counter is counting.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum BranchKind {
    /// A `br_if` — counter fires when the branch is taken.
    BrIf,
    /// The `then` arm of an `if/else` — counter fires when the consequent runs.
    IfThen,
    /// The `else` arm of an `if/else` — counter fires when the alternative runs.
    IfElse,
    /// A specific `br_table` target arm. The target index is on the
    /// `BranchEntry`. Counter fires when the table dispatches to that arm.
    /// Replaces v0.1's single-counter `BrTable` kind.
    BrTableTarget,
    /// The default arm of a `br_table` (selector >= number of explicit
    /// targets). Counter fires when the default branch is taken.
    BrTableDefault,
}

/// One branch point in the original module. `instr_index` is the position
/// within the containing `InstrSeq` in the pre-instrumentation IR.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BranchEntry {
    pub id: u32,
    pub function_index: u32,
    pub function_name: Option<String>,
    pub kind: BranchKind,
    pub instr_index: u32,
    /// For `BrTableTarget` entries: which target index (0..N) this counter
    /// covers. `None` for non-`br_table` kinds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_index: Option<u32>,
    /// Byte offset of the original source instruction in the
    /// pre-instrumentation Wasm (from walrus `InstrLocId`). Required for
    /// DWARF correlation in v0.2. `None` when the source location is
    /// unavailable (e.g. instruction synthesised, not loaded from a
    /// `.wasm` file).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_offset: Option<u32>,
    /// Walrus `InstrSeqId` encoded as a debug string — diagnostic only.
    pub seq_debug: String,
}

/// Group of `BranchEntry` ids treated as a single source-level decision
/// after DWARF reconstruction. v0.2 emits these for `br_if` sequences that
/// share a source line + lexical decision marker. Empty list means the
/// fallback strict-per-`br_if` interpretation applies.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Decision {
    pub id: u32,
    /// The branch ids that constitute this decision's conditions, ordered
    /// by their position in the `br_if` sequence (first to last).
    pub conditions: Vec<u32>,
    /// Source file path from DWARF, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,
    /// Source line from DWARF, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_line: Option<u32>,
}

/// Manifest written alongside the instrumented Wasm.
#[derive(Serialize, Deserialize, Debug)]
pub struct Manifest {
    pub schema_version: String,
    pub witness_version: String,
    pub module_source: String,
    pub branches: Vec<BranchEntry>,
    /// Source-level decisions reconstructed from `branches` via DWARF.
    /// Empty when DWARF is absent or reconstruction declined to group.
    /// Hosts that don't care about MC/DC can ignore this field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<Decision>,
}

impl Manifest {
    pub fn path_for(output: &Path) -> PathBuf {
        let mut os = output.as_os_str().to_os_string();
        os.push(".witness.json");
        PathBuf::from(os)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).map_err(Error::Io)?;
        serde_json::from_slice(&bytes).map_err(|source| Error::Manifest {
            path: path.to_path_buf(),
            source,
        })
    }
}

/// Instrument the Wasm module at `input`, writing the instrumented module
/// to `output` and a branch manifest alongside it.
pub fn instrument_file(input: &Path, output: &Path) -> Result<()> {
    let original_bytes = std::fs::read(input).map_err(|source| Error::ReadModule {
        path: input.to_path_buf(),
        source,
    })?;
    let mut module = Module::from_buffer(&original_bytes).map_err(|source| Error::ParseModule {
        path: input.to_path_buf(),
        source,
    })?;

    let entries = instrument_module(&mut module, input.to_string_lossy().as_ref())?;

    let wasm = module.emit_wasm();
    std::fs::write(output, wasm).map_err(|source| Error::EmitModule {
        path: output.to_path_buf(),
        source,
    })?;

    // v0.2: attempt DWARF-grounded decision reconstruction. v0.2.0 ships
    // the stub (always empty); v0.2.1 fills in the algorithm. Empty list
    // means hosts use the strict per-br_if fallback.
    let decisions = crate::decisions::reconstruct_decisions(&original_bytes, &entries)?;

    let manifest = Manifest {
        schema_version: MANIFEST_SCHEMA_VERSION.to_string(),
        witness_version: env!("CARGO_PKG_VERSION").to_string(),
        module_source: input.to_string_lossy().into_owned(),
        branches: entries,
        decisions,
    };
    let manifest_path = Manifest::path_for(output);
    let manifest_json = serde_json::to_string_pretty(&manifest).map_err(Error::Serde)?;
    std::fs::write(&manifest_path, manifest_json).map_err(Error::Io)?;

    Ok(())
}

/// Instrument `module` in place, returning the branch manifest entries.
///
/// v0.6.1: in addition to the v0.5 per-branch counter (sums hits across
/// all rows), allocates per-row `__witness_brval_<id>` and
/// `__witness_brcnt_<id>` globals for `BrIf` / `IfThen` / `IfElse`
/// branches, plus an exported `__witness_row_reset` function that the
/// runner calls between row invocations. `BrTable*` branches keep
/// counter-only instrumentation per DEC-015.
pub fn instrument_module(module: &mut Module, _module_source: &str) -> Result<Vec<BranchEntry>> {
    let scans: Vec<FunctionScan> = collect_scans(module);

    let mut entries: Vec<BranchEntry> = Vec::new();
    let mut counter_globals: Vec<GlobalId> = Vec::new();
    let mut brval_globals: Vec<Option<GlobalId>> = Vec::new();
    let mut brcnt_globals: Vec<Option<GlobalId>> = Vec::new();
    for scan in &scans {
        for branch in &scan.branches {
            let id = u32::try_from(entries.len())
                .map_err(|_| Error::Instrument("branch count exceeds u32::MAX".to_string()))?;
            let counter = module.globals.add_local(
                ValType::I32,
                true,
                false,
                ConstExpr::Value(Value::I32(0)),
            );
            module
                .exports
                .add(&format!("{COUNTER_EXPORT_PREFIX}{id}"), counter);
            counter_globals.push(counter);

            // v0.6.1: allocate per-row capture globals for non-br_table branches.
            let (brval, brcnt) = match branch.kind {
                BranchKind::BrIf | BranchKind::IfThen | BranchKind::IfElse => {
                    let bv = module.globals.add_local(
                        ValType::I32,
                        true,
                        false,
                        ConstExpr::Value(Value::I32(0)),
                    );
                    module
                        .exports
                        .add(&format!("{BRVAL_EXPORT_PREFIX}{id}"), bv);
                    let bc = module.globals.add_local(
                        ValType::I32,
                        true,
                        false,
                        ConstExpr::Value(Value::I32(0)),
                    );
                    module
                        .exports
                        .add(&format!("{BRCNT_EXPORT_PREFIX}{id}"), bc);
                    (Some(bv), Some(bc))
                }
                BranchKind::BrTableTarget | BranchKind::BrTableDefault => (None, None),
            };
            brval_globals.push(brval);
            brcnt_globals.push(brcnt);

            entries.push(BranchEntry {
                id,
                function_index: scan.function_index,
                function_name: scan.function_name.clone(),
                kind: branch.kind,
                instr_index: branch.instr_index,
                target_index: branch.target_index,
                byte_offset: branch.byte_offset,
                seq_debug: format!("{:?}", branch.seq_id),
            });
        }
    }

    // Phase 2: build a `__witness_brtable_<n>` helper function for every
    // br_table site. Helpers must be added before the per-function rewrite
    // because rewrite_function holds a `&mut LocalFunction` that conflicts
    // with module.funcs mutations.
    let helpers = build_brtable_helpers(module, &scans, &counter_globals)?;

    // v0.7.2: build the trace memory + reset/row-marker helpers + the
    // trace_record helper. Built BEFORE the rewrite loop so its
    // FunctionId is stable when rewrite_brif emits a Call.
    let trace_mem = build_trace_infra(module)?;
    let trace_record_helper = build_trace_record_helper(module, trace_mem)?;

    let mut cursor: usize = 0;
    for scan in scans.into_iter() {
        let len = scan.branches.len();
        let end = cursor.saturating_add(len);
        let counters_slice = counter_globals
            .get(cursor..end)
            .ok_or_else(|| Error::Instrument("counter slice out of range".to_string()))?
            .to_vec();
        let brval_slice = brval_globals
            .get(cursor..end)
            .ok_or_else(|| Error::Instrument("brval slice out of range".to_string()))?
            .to_vec();
        let brcnt_slice = brcnt_globals
            .get(cursor..end)
            .ok_or_else(|| Error::Instrument("brcnt slice out of range".to_string()))?
            .to_vec();
        let scan_offset = cursor;
        rewrite_function(
            module,
            scan.function_id,
            &scan.branches,
            &counters_slice,
            &brval_slice,
            &brcnt_slice,
            scan_offset,
            &helpers,
            trace_record_helper,
        )?;
        cursor = end;
    }

    // v0.6.1: build the __witness_row_reset helper function.
    build_row_reset(module, &brval_globals, &brcnt_globals)?;

    Ok(entries)
}

/// Build the `__witness_trace_record(branch_id: i32, value: i32) -> ()`
/// helper that per-br_if instrumentation calls.
fn build_trace_record_helper(module: &mut Module, mem: MemoryId) -> Result<FunctionId> {
    let i32_ty = ValType::I32;
    let mut builder = FunctionBuilder::new(&mut module.types, &[i32_ty, i32_ty], &[]);
    let branch_id_local = module.locals.add(i32_ty);
    let value_local = module.locals.add(i32_ty);
    {
        let mut body = builder.func_body();
        // --- record write at cursor ---
        // stack: [cursor]
        body.instr(Instr::Const(Const {
            value: Value::I32(0),
        }));
        body.instr(Instr::Load(walrus::ir::Load {
            memory: mem,
            kind: LoadKind::I32 { atomic: false },
            arg: MemArg {
                align: 4,
                offset: 0,
            },
        }));
        // packed = (value << 16) | (branch_id & 0xFFFF). kind=0 so high
        // byte stays zero.
        body.instr(Instr::LocalGet(LocalGet { local: value_local }));
        body.instr(Instr::Const(Const {
            value: Value::I32(16),
        }));
        body.instr(Instr::Binop(Binop {
            op: BinaryOp::I32Shl,
        }));
        body.instr(Instr::LocalGet(LocalGet {
            local: branch_id_local,
        }));
        body.instr(Instr::Const(Const {
            value: Value::I32(0xFFFF),
        }));
        body.instr(Instr::Binop(Binop {
            op: BinaryOp::I32And,
        }));
        body.instr(Instr::Binop(Binop {
            op: BinaryOp::I32Or,
        }));
        // stack: [cursor, packed]
        body.instr(Instr::Store(Store {
            memory: mem,
            kind: StoreKind::I32 { atomic: false },
            arg: MemArg {
                align: 4,
                offset: 0,
            },
        }));
        // --- advance cursor: *(0 + 0) = old_cursor + 4 ---
        body.instr(Instr::Const(Const {
            value: Value::I32(0),
        }));
        body.instr(Instr::Const(Const {
            value: Value::I32(0),
        }));
        body.instr(Instr::Load(walrus::ir::Load {
            memory: mem,
            kind: LoadKind::I32 { atomic: false },
            arg: MemArg {
                align: 4,
                offset: 0,
            },
        }));
        body.instr(Instr::Const(Const {
            value: Value::I32(i32::try_from(TRACE_RECORD_BYTES).unwrap_or(4)),
        }));
        body.instr(Instr::Binop(Binop {
            op: BinaryOp::I32Add,
        }));
        body.instr(Instr::Store(Store {
            memory: mem,
            kind: StoreKind::I32 { atomic: false },
            arg: MemArg {
                align: 4,
                offset: 0,
            },
        }));
    }
    let func_id = builder.finish(vec![branch_id_local, value_local], &mut module.funcs);
    // Internal helper — give it a name in the name section but don't
    // export it. The instrumented br_if sites call it by FunctionId.
    module.funcs.get_mut(func_id).name = Some(TRACE_RECORD_HELPER_NAME.to_string());
    Ok(func_id)
}

/// v0.7.2 — add the `__witness_trace` exported memory and the
/// `__witness_trace_reset` / `__witness_trace_row_marker` helper
/// functions. The memory is initialised with capacity in its
/// header (writer-side rolls forward; host-side reads cursor +
/// overflow_flag to know what's been written).
///
/// Returns the MemoryId so callers can pass it to the per-br_if
/// rewrite path that emits trace-record writes.
fn build_trace_infra(module: &mut Module) -> Result<MemoryId> {
    let mem = module.memories.add_local(
        false,
        false,
        TRACE_DEFAULT_PAGES.into(),
        Some(TRACE_DEFAULT_PAGES.into()),
        None,
    );
    module.exports.add(TRACE_MEMORY_EXPORT, mem);

    // __witness_trace_reset(): zero cursor + overflow_flag (offsets 0, 8).
    // Capacity field at offset 4 is set once at module-init time —
    // we write it from the reset helper too so the host can always
    // read a meaningful value after the first reset call.
    let mut reset = FunctionBuilder::new(&mut module.types, &[], &[]);
    {
        let mut body = reset.func_body();
        // cursor = TRACE_HEADER_BYTES (records start after header)
        body.instr(Instr::Const(Const {
            value: Value::I32(0),
        }));
        body.instr(Instr::Const(Const {
            value: Value::I32(i32::try_from(TRACE_HEADER_BYTES).unwrap_or(0)),
        }));
        body.instr(Instr::Store(Store {
            memory: mem,
            kind: StoreKind::I32 { atomic: false },
            arg: MemArg {
                align: 4,
                offset: 0,
            },
        }));
        // capacity = (TRACE_DEFAULT_PAGES * 64KiB) - TRACE_HEADER_BYTES
        let capacity_bytes: u32 = TRACE_DEFAULT_PAGES
            .saturating_mul(65536)
            .saturating_sub(TRACE_HEADER_BYTES);
        body.instr(Instr::Const(Const {
            value: Value::I32(0),
        }));
        body.instr(Instr::Const(Const {
            value: Value::I32(i32::try_from(capacity_bytes).unwrap_or(0)),
        }));
        body.instr(Instr::Store(Store {
            memory: mem,
            kind: StoreKind::I32 { atomic: false },
            arg: MemArg {
                align: 4,
                offset: 4,
            },
        }));
        // overflow_flag = 0
        body.instr(Instr::Const(Const {
            value: Value::I32(0),
        }));
        body.instr(Instr::Const(Const {
            value: Value::I32(0),
        }));
        body.instr(Instr::Store(Store {
            memory: mem,
            kind: StoreKind::I32 { atomic: false },
            arg: MemArg {
                align: 4,
                offset: 8,
            },
        }));
    }
    let reset_id = reset.finish(vec![], &mut module.funcs);
    module.exports.add(TRACE_RESET_EXPORT, reset_id);

    // __witness_trace_row_marker(row_id: i32) -> (): append a record
    // (branch_id slot = row_id_lo, value = 0, kind = 1).
    //
    // Stack-only implementation — walrus's FunctionBuilder doesn't
    // auto-register non-arg locals, so we keep the cursor on stack
    // by reading it twice (write-record path, then advance-cursor
    // path). Two loads at offset 0 is a couple of instructions over
    // a one-load+tee+get version but avoids the local-declaration
    // pitfall.
    let i32_ty = ValType::I32;
    let mut marker = FunctionBuilder::new(&mut module.types, &[i32_ty], &[]);
    let row_id_local = module.locals.add(i32_ty);
    {
        let mut body = marker.func_body();
        // --- record write at cursor ---
        // stack: [cursor]
        body.instr(Instr::Const(Const {
            value: Value::I32(0),
        }));
        body.instr(Instr::Load(walrus::ir::Load {
            memory: mem,
            kind: LoadKind::I32 { atomic: false },
            arg: MemArg {
                align: 4,
                offset: 0,
            },
        }));
        // packed value: (kind=1 << 24) | (row_id & 0xFFFF)
        body.instr(Instr::LocalGet(LocalGet {
            local: row_id_local,
        }));
        body.instr(Instr::Const(Const {
            value: Value::I32(0xFFFF),
        }));
        body.instr(Instr::Binop(Binop {
            op: BinaryOp::I32And,
        }));
        body.instr(Instr::Const(Const {
            value: Value::I32(0x01_00_00_00),
        }));
        body.instr(Instr::Binop(Binop {
            op: BinaryOp::I32Or,
        }));
        // stack: [cursor, packed]
        body.instr(Instr::Store(Store {
            memory: mem,
            kind: StoreKind::I32 { atomic: false },
            arg: MemArg {
                align: 4,
                offset: 0,
            },
        }));
        // stack: [] after store consumes both

        // --- advance cursor: *(mem + 0) = cursor + 4 ---
        // Stack must end at: [addr=0, value=new_cursor], then i32.store consumes both.
        body.instr(Instr::Const(Const {
            value: Value::I32(0),
        }));
        // stack: [0]
        body.instr(Instr::Const(Const {
            value: Value::I32(0),
        }));
        body.instr(Instr::Load(walrus::ir::Load {
            memory: mem,
            kind: LoadKind::I32 { atomic: false },
            arg: MemArg {
                align: 4,
                offset: 0,
            },
        }));
        // stack: [0, cursor]
        body.instr(Instr::Const(Const {
            value: Value::I32(i32::try_from(TRACE_RECORD_BYTES).unwrap_or(4)),
        }));
        body.instr(Instr::Binop(Binop {
            op: BinaryOp::I32Add,
        }));
        // stack: [0, new_cursor]
        body.instr(Instr::Store(Store {
            memory: mem,
            kind: StoreKind::I32 { atomic: false },
            arg: MemArg {
                align: 4,
                offset: 0,
            },
        }));
    }
    let marker_id = marker.finish(vec![row_id_local], &mut module.funcs);
    module.exports.add(TRACE_ROW_MARKER_EXPORT, marker_id);

    Ok(mem)
}

/// Build the `__witness_row_reset()` exported function that zeroes every
/// `__witness_brval_*` and `__witness_brcnt_*` global. The runner calls
/// this between row invocations so the next row's captures don't carry
/// residual state from the prior row.
fn build_row_reset(
    module: &mut Module,
    brval_globals: &[Option<GlobalId>],
    brcnt_globals: &[Option<GlobalId>],
) -> Result<()> {
    let mut builder = FunctionBuilder::new(&mut module.types, &[], &[]);
    {
        let mut body = builder.func_body();
        for &g in brval_globals.iter().chain(brcnt_globals.iter()).flatten() {
            body.instr(Instr::Const(Const {
                value: Value::I32(0),
            }));
            body.instr(Instr::GlobalSet(GlobalSet { global: g }));
        }
    }
    let func_id = builder.finish(vec![], &mut module.funcs);
    module.exports.add(ROW_RESET_EXPORT, func_id);
    Ok(())
}

/// Build `__witness_brtable_<n>` helper functions, one per br_table site.
/// Returns a map keyed by absolute counter index of the FIRST counter in
/// each br_table group, to the helper FunctionId.
///
/// SAFETY-REVIEW: `i`, `j`, `global_idx`, `group_start`, `n_total`,
/// `abs_start` are all bounded by `scans.len()` and `counter_globals.len()`,
/// which themselves are bounded by `usize::MAX` for any realistic Wasm
/// module. The arithmetic is wraparound-safe via `saturating_add` /
/// `saturating_sub`; indexing uses `.get()` / `.ok_or_else()` to surface
/// out-of-range as an instrumentation error rather than panicking.
#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]
fn build_brtable_helpers(
    module: &mut Module,
    scans: &[FunctionScan],
    counter_globals: &[GlobalId],
) -> Result<HashMap<usize, FunctionId>> {
    let mut helpers: HashMap<usize, FunctionId> = HashMap::new();
    let mut helper_seq: u32 = 0;
    let mut global_idx: usize = 0;

    for scan in scans {
        let mut i = 0;
        while i < scan.branches.len() {
            let site = &scan.branches[i];
            if site.kind == BranchKind::BrTableTarget {
                let group_start = i;
                let group_seq = site.seq_id;
                let group_idx = site.instr_index;
                let mut j = i;
                while j < scan.branches.len()
                    && scan.branches[j].seq_id == group_seq
                    && scan.branches[j].instr_index == group_idx
                {
                    j = j.saturating_add(1);
                }
                let n_total = j.saturating_sub(group_start);
                let abs_start = global_idx.saturating_add(group_start);
                let abs_end = abs_start.saturating_add(n_total);
                let group_globals: Vec<GlobalId> = counter_globals
                    .get(abs_start..abs_end)
                    .ok_or_else(|| {
                        Error::Instrument("br_table counter group out of range".to_string())
                    })?
                    .to_vec();
                let helper_id = build_brtable_helper(module, &group_globals, helper_seq)?;
                helpers.insert(abs_start, helper_id);
                helper_seq = helper_seq.saturating_add(1);
                i = j;
            } else {
                i = i.saturating_add(1);
            }
        }
        global_idx = global_idx.saturating_add(scan.branches.len());
    }

    Ok(helpers)
}

/// Build a single helper function:
///
/// ```wat
/// (func $__witness_brtable_n (param $sel i32) (result i32)
///   ;; for each explicit target i in 0..N:
///   local.get $sel
///   i32.const i
///   i32.eq
///   if
///     ;; counter[i] += 1
///   end
///   ;; default arm: selector >= N (unsigned)
///   local.get $sel
///   i32.const N
///   i32.ge_u
///   if
///     ;; counter[N] += 1   (the default counter)
///   end
///   local.get $sel
/// )
/// ```
fn build_brtable_helper(
    module: &mut Module,
    counters: &[GlobalId],
    helper_seq: u32,
) -> Result<FunctionId> {
    if counters.is_empty() {
        return Err(Error::Instrument(
            "br_table helper requires at least the default counter".to_string(),
        ));
    }
    let n_explicit = counters.len().saturating_sub(1);
    let default_counter = *counters
        .last()
        .ok_or_else(|| Error::Instrument("br_table helper missing default counter".to_string()))?;
    let n_explicit_i32 = i32::try_from(n_explicit)
        .map_err(|_| Error::Instrument("br_table target count exceeds i32::MAX".to_string()))?;

    let i32_ty = ValType::I32;
    let mut builder = FunctionBuilder::new(&mut module.types, &[i32_ty], &[i32_ty]);
    let selector_local = module.locals.add(i32_ty);

    {
        let mut body = builder.func_body();
        for (i, &counter) in counters.iter().take(n_explicit).enumerate() {
            let target_idx = i32::try_from(i).map_err(|_| {
                Error::Instrument("br_table target index exceeds i32::MAX".to_string())
            })?;
            body.instr(Instr::LocalGet(LocalGet {
                local: selector_local,
            }));
            body.instr(Instr::Const(Const {
                value: Value::I32(target_idx),
            }));
            body.instr(Instr::Binop(Binop {
                op: BinaryOp::I32Eq,
            }));
            body.if_else(
                InstrSeqType::Simple(None),
                |then| {
                    counter_inc_into(then, counter);
                },
                |_else| {},
            );
        }
        // Default arm: selector >= N (unsigned).
        body.instr(Instr::LocalGet(LocalGet {
            local: selector_local,
        }));
        body.instr(Instr::Const(Const {
            value: Value::I32(n_explicit_i32),
        }));
        body.instr(Instr::Binop(Binop {
            op: BinaryOp::I32GeU,
        }));
        body.if_else(
            InstrSeqType::Simple(None),
            |then| {
                counter_inc_into(then, default_counter);
            },
            |_else| {},
        );
        // Return selector unchanged.
        body.instr(Instr::LocalGet(LocalGet {
            local: selector_local,
        }));
    }

    let func_id = builder.finish(vec![selector_local], &mut module.funcs);
    let export_name = format!("{BRTABLE_HELPER_PREFIX}{helper_seq}");
    module.exports.add(&export_name, func_id);
    Ok(func_id)
}

/// Append `counter += 1` to the given instruction-sequence builder.
fn counter_inc_into(seq: &mut walrus::InstrSeqBuilder<'_>, counter: GlobalId) {
    seq.instr(Instr::GlobalGet(GlobalGet { global: counter }));
    seq.instr(Instr::Const(Const {
        value: Value::I32(1),
    }));
    seq.instr(Instr::Binop(Binop {
        op: BinaryOp::I32Add,
    }));
    seq.instr(Instr::GlobalSet(GlobalSet { global: counter }));
}

struct BranchSite {
    seq_id: InstrSeqId,
    instr_index: u32,
    kind: BranchKind,
    /// For `BrTableTarget` sites only: which target index this counter
    /// covers. `None` for non-`br_table` kinds and for `BrTableDefault`.
    target_index: Option<u32>,
    /// Original wasm bytecode offset of the source instruction, when
    /// available (walrus `InstrLocId.data()`). `None` for synthetic
    /// instructions or modules built without source-map data.
    byte_offset: Option<u32>,
}

struct FunctionScan {
    function_id: FunctionId,
    function_index: u32,
    function_name: Option<String>,
    branches: Vec<BranchSite>,
}

fn collect_scans(module: &Module) -> Vec<FunctionScan> {
    let mut out = Vec::new();
    for (index, (fid, lf)) in module.funcs.iter_local().enumerate() {
        let mut sites = Vec::new();
        walk_collect(lf, lf.entry_block(), &mut sites);
        if sites.is_empty() {
            continue;
        }
        let function_index = u32::try_from(index).unwrap_or(u32::MAX);
        let function_name = module.funcs.get(fid).name.clone();
        out.push(FunctionScan {
            function_id: fid,
            function_index,
            function_name,
            branches: sites,
        });
    }
    out
}

fn walk_collect(func: &walrus::LocalFunction, seq_id: InstrSeqId, out: &mut Vec<BranchSite>) {
    let seq = func.block(seq_id);
    for (index, (instr, loc)) in seq.instrs.iter().enumerate() {
        let i = u32::try_from(index).unwrap_or(u32::MAX);
        let byte_offset = if loc.is_default() {
            None
        } else {
            Some(loc.data())
        };
        // SAFETY-REVIEW: walrus has 50+ Instr variants; only those
        // containing nested InstrSeqIds need handling (Block, Loop,
        // IfElse), and branch-count candidates (BrIf, BrTable). Listing
        // every other variant explicitly would churn on every walrus
        // minor-version bump.
        #[allow(clippy::wildcard_enum_match_arm)]
        match instr {
            Instr::BrIf(_) => out.push(BranchSite {
                seq_id,
                instr_index: i,
                kind: BranchKind::BrIf,
                target_index: None,
                byte_offset,
            }),
            Instr::IfElse(IfElse {
                consequent,
                alternative,
            }) => {
                out.push(BranchSite {
                    seq_id,
                    instr_index: i,
                    kind: BranchKind::IfThen,
                    target_index: None,
                    byte_offset,
                });
                out.push(BranchSite {
                    seq_id,
                    instr_index: i,
                    kind: BranchKind::IfElse,
                    target_index: None,
                    byte_offset,
                });
                walk_collect(func, *consequent, out);
                walk_collect(func, *alternative, out);
            }
            Instr::BrTable(bt) => {
                let n = u32::try_from(bt.blocks.len()).unwrap_or(u32::MAX);
                for t in 0..n {
                    out.push(BranchSite {
                        seq_id,
                        instr_index: i,
                        kind: BranchKind::BrTableTarget,
                        target_index: Some(t),
                        byte_offset,
                    });
                }
                out.push(BranchSite {
                    seq_id,
                    instr_index: i,
                    kind: BranchKind::BrTableDefault,
                    target_index: None,
                    byte_offset,
                });
            }
            Instr::Block(b) => walk_collect(func, b.seq, out),
            Instr::Loop(l) => walk_collect(func, l.seq, out),
            _ => {}
        }
    }
}

/// `counter_offset` is the position of `sites[0]`'s counter within the
/// global counter list — used to look up the right br_table helper from
/// `helpers` (keyed by absolute counter index).
#[allow(clippy::too_many_arguments)]
fn rewrite_function(
    module: &mut Module,
    function_id: FunctionId,
    sites: &[BranchSite],
    counters: &[GlobalId],
    brvals: &[Option<GlobalId>],
    brcnts: &[Option<GlobalId>],
    counter_offset: usize,
    helpers: &HashMap<usize, FunctionId>,
    trace_record_helper: FunctionId,
) -> Result<()> {
    let has_brif = sites.iter().any(|s| s.kind == BranchKind::BrIf);
    let brif_tmp: Option<LocalId> = if has_brif {
        Some(module.locals.add(ValType::I32))
    } else {
        None
    };

    // SAFETY-REVIEW: only Local kinds have a body to rewrite; Import and
    // Uninitialized kinds have no IR to mutate.
    #[allow(clippy::wildcard_enum_match_arm)]
    let func = match &mut module.funcs.get_mut(function_id).kind {
        walrus::FunctionKind::Local(lf) => lf,
        _ => return Ok(()),
    };

    // Group sites by (seq_id, instr_index). IfElse emits two BranchSite
    // entries at the same position (one IfThen, one IfElse); BrTable emits
    // N+1 entries (N targets + 1 default). We rewrite each group as one.
    let mut by_index: std::collections::BTreeMap<(InstrSeqId, u32), Vec<(usize, BranchKind)>> =
        std::collections::BTreeMap::new();
    for (idx, site) in sites.iter().enumerate() {
        by_index
            .entry((site.seq_id, site.instr_index))
            .or_default()
            .push((idx, site.kind));
    }

    // Execute in reverse so later indices in the same sequence are rewritten
    // before earlier ones — keeps instr_index valid as we splice.
    #[allow(clippy::type_complexity)]
    let rewrite_ops: Vec<((InstrSeqId, u32), Vec<(usize, BranchKind)>)> =
        by_index.into_iter().rev().collect();

    for ((seq_id, instr_index), group) in rewrite_ops {
        let at = usize::try_from(instr_index).unwrap_or(usize::MAX);
        // SAFETY-REVIEW: BrIf/IfElse/BrTable are the branch kinds produced
        // by `walk_collect`; any other variant here is a logic bug.
        #[allow(clippy::wildcard_enum_match_arm)]
        let kind_at = match func.block(seq_id).instrs.get(at) {
            Some((Instr::BrIf(_), _)) => PeekKind::BrIf,
            Some((Instr::IfElse(_), _)) => PeekKind::IfElse,
            Some((Instr::BrTable(_), _)) => PeekKind::BrTable,
            other => {
                return Err(Error::Instrument(format!(
                    "expected branch instruction at seq {seq_id:?} index {instr_index}, got {other:?}"
                )));
            }
        };

        let counter_at = |site_idx: usize| -> Result<GlobalId> {
            counters
                .get(site_idx)
                .copied()
                .ok_or_else(|| Error::Instrument(format!("counter index {site_idx} out of range")))
        };
        // SAFETY-REVIEW: indices are bounded by `sites.len()` and the
        // brval/brcnt slices are constructed with the same length as
        // `counters`. `.get()` returns `None` for out-of-bounds, surfacing
        // as `None` (not a panic) for non-instrumented kinds.
        let brval_at =
            |site_idx: usize| -> Option<GlobalId> { brvals.get(site_idx).copied().flatten() };
        let brcnt_at =
            |site_idx: usize| -> Option<GlobalId> { brcnts.get(site_idx).copied().flatten() };

        match kind_at {
            PeekKind::BrIf => {
                let (idx_in_sites, _) = *group
                    .first()
                    .ok_or_else(|| Error::Instrument("empty br_if group".to_string()))?;
                let counter = counter_at(idx_in_sites)?;
                let brval = brval_at(idx_in_sites);
                let brcnt = brcnt_at(idx_in_sites);
                let tmp = brif_tmp.ok_or_else(|| {
                    Error::Instrument("br_if site without brif_tmp local".to_string())
                })?;
                let absolute_branch_id =
                    u32::try_from(counter_offset.saturating_add(idx_in_sites)).unwrap_or(u32::MAX);
                rewrite_brif(
                    func,
                    seq_id,
                    at,
                    counter,
                    brval,
                    brcnt,
                    tmp,
                    Some((trace_record_helper, absolute_branch_id)),
                );
            }
            PeekKind::IfElse => {
                let then_idx = group
                    .iter()
                    .find(|(_, k)| matches!(k, BranchKind::IfThen))
                    .map(|(i, _)| *i);
                let else_idx = group
                    .iter()
                    .find(|(_, k)| matches!(k, BranchKind::IfElse))
                    .map(|(i, _)| *i);
                let then_counter = then_idx.map(counter_at).transpose()?;
                let else_counter = else_idx.map(counter_at).transpose()?;
                let then_brval = then_idx.and_then(brval_at);
                let then_brcnt = then_idx.and_then(brcnt_at);
                let else_brval = else_idx.and_then(brval_at);
                let else_brcnt = else_idx.and_then(brcnt_at);
                rewrite_ifelse(
                    func,
                    seq_id,
                    at,
                    then_counter,
                    else_counter,
                    then_brval,
                    then_brcnt,
                    else_brval,
                    else_brcnt,
                );
            }
            PeekKind::BrTable => {
                // The first counter index in the group is the helper key.
                let (idx_in_sites, _) = *group
                    .first()
                    .ok_or_else(|| Error::Instrument("empty br_table group".to_string()))?;
                let absolute = counter_offset.saturating_add(idx_in_sites);
                let helper = helpers.get(&absolute).copied().ok_or_else(|| {
                    Error::Instrument(format!(
                        "no br_table helper registered for counter offset {absolute}"
                    ))
                })?;
                rewrite_brtable(func, seq_id, at, helper);
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
enum PeekKind {
    BrIf,
    IfElse,
    BrTable,
}

/// Rewrite `br_if L` at `(seq_id, index)`.
///
/// v0.5 emitted: `local.tee tmp; if (inc counter) end; local.get tmp; br_if L`.
///
/// v0.6.1 prepends per-row capture writes when `brval` and `brcnt` are
/// available: `local.tee tmp; local.get tmp; global.set brval; brcnt += 1;
/// local.get tmp; if (inc counter) end; local.get tmp; br_if L`.
///
/// Stack invariant identical: the original condition value flows through
/// the rewrite unchanged.
#[allow(clippy::too_many_arguments)]
fn rewrite_brif(
    func: &mut walrus::LocalFunction,
    seq_id: InstrSeqId,
    index: usize,
    counter: GlobalId,
    brval: Option<GlobalId>,
    brcnt: Option<GlobalId>,
    tmp: LocalId,
    trace: Option<(FunctionId, u32)>,
) {
    // SAFETY-REVIEW: caller has already peeked at `(seq_id, index)` and
    // confirmed it is a `BrIf`; any other variant reaching here is a logic
    // bug in `rewrite_function`.
    #[allow(clippy::wildcard_enum_match_arm)]
    let target = {
        let instrs = &mut func.block_mut(seq_id).instrs;
        let (instr, _loc) = instrs.remove(index);
        match instr {
            Instr::BrIf(BrIf { block }) => block,
            _ => unreachable!("peek validated br_if"),
        }
    };

    let inc_seq = counter_inc_seq(func, counter);
    let empty_seq = func
        .builder_mut()
        .dangling_instr_seq(InstrSeqType::Simple(None))
        .id();

    // Build the replacement instruction list. Begin with the v0.5 tee.
    let mut replacement: Vec<Instr> = Vec::with_capacity(12);
    replacement.push(Instr::LocalTee(LocalTee { local: tmp }));
    // v0.6.1 brval write: brval = tmp value.
    if let Some(bv) = brval {
        replacement.push(Instr::LocalGet(LocalGet { local: tmp }));
        replacement.push(Instr::GlobalSet(GlobalSet { global: bv }));
    }
    // v0.6.1 brcnt increment.
    if let Some(bc) = brcnt {
        replacement.push(Instr::GlobalGet(GlobalGet { global: bc }));
        replacement.push(Instr::Const(Const {
            value: Value::I32(1),
        }));
        replacement.push(Instr::Binop(Binop {
            op: BinaryOp::I32Add,
        }));
        replacement.push(Instr::GlobalSet(GlobalSet { global: bc }));
    }
    // v0.7.2 trace-record-call: i32.const branch_id; local.get tmp;
    // call __witness_trace_record. Stack-neutral (consumes 2, pushes
    // 0). Preserves the v0.5 invariant that the tee'd cond is still
    // on the stack for the if-counter-inc that follows.
    if let Some((helper, branch_id)) = trace {
        let bid = i32::try_from(branch_id).unwrap_or(i32::MAX);
        replacement.push(Instr::Const(Const {
            value: Value::I32(bid),
        }));
        replacement.push(Instr::LocalGet(LocalGet { local: tmp }));
        replacement.push(Instr::Call(walrus::ir::Call { func: helper }));
    }
    // v0.5 counter-on-taken pattern. The tee'd value is still on the
    // stack from `local.tee tmp` (and the brval/brcnt sequences are
    // stack-neutral), so we don't re-fetch tmp before the if.
    replacement.push(Instr::IfElse(IfElse {
        consequent: inc_seq,
        alternative: empty_seq,
    }));
    // Restore condition for the original br_if.
    replacement.push(Instr::LocalGet(LocalGet { local: tmp }));
    replacement.push(Instr::BrIf(BrIf { block: target }));

    let instrs = &mut func.block_mut(seq_id).instrs;
    for (offset, instr) in replacement.into_iter().enumerate() {
        // SAFETY-REVIEW: `offset` is bounded by `replacement.len()` (≤ 12)
        // and `index` is an in-bounds position into the sequence's instrs.
        let pos = index.saturating_add(offset);
        instrs.insert(pos, (instr, Default::default()));
    }
}

#[allow(clippy::too_many_arguments)]
fn rewrite_ifelse(
    func: &mut walrus::LocalFunction,
    seq_id: InstrSeqId,
    index: usize,
    then_counter: Option<GlobalId>,
    else_counter: Option<GlobalId>,
    then_brval: Option<GlobalId>,
    then_brcnt: Option<GlobalId>,
    else_brval: Option<GlobalId>,
    else_brcnt: Option<GlobalId>,
) {
    // SAFETY-REVIEW: caller has peeked and confirmed IfElse at this position.
    #[allow(clippy::wildcard_enum_match_arm)]
    let (then_id, else_id) = match func.block(seq_id).instrs.get(index) {
        Some((
            Instr::IfElse(IfElse {
                consequent,
                alternative,
            }),
            _,
        )) => (*consequent, *alternative),
        _ => unreachable!("peek validated if_else"),
    };

    if let Some(c) = then_counter {
        prepend_arm_capture(func, then_id, c, then_brval, then_brcnt);
    }
    if let Some(c) = else_counter {
        prepend_arm_capture(func, else_id, c, else_brval, else_brcnt);
    }
}

/// Prepend per-arm v0.6.1 capture: `brval = 1` (this arm fired this row)
/// and `brcnt += 1`, plus the v0.5 counter increment.
fn prepend_arm_capture(
    func: &mut walrus::LocalFunction,
    seq_id: InstrSeqId,
    counter: GlobalId,
    brval: Option<GlobalId>,
    brcnt: Option<GlobalId>,
) {
    let mut prelude: Vec<Instr> = Vec::with_capacity(12);
    if let Some(bv) = brval {
        prelude.push(Instr::Const(Const {
            value: Value::I32(1),
        }));
        prelude.push(Instr::GlobalSet(GlobalSet { global: bv }));
    }
    if let Some(bc) = brcnt {
        prelude.push(Instr::GlobalGet(GlobalGet { global: bc }));
        prelude.push(Instr::Const(Const {
            value: Value::I32(1),
        }));
        prelude.push(Instr::Binop(Binop {
            op: BinaryOp::I32Add,
        }));
        prelude.push(Instr::GlobalSet(GlobalSet { global: bc }));
    }
    for instr in counter_inc_instrs(counter) {
        prelude.push(instr);
    }
    let instrs = &mut func.block_mut(seq_id).instrs;
    for (offset, instr) in prelude.into_iter().enumerate() {
        instrs.insert(offset, (instr, Default::default()));
    }
}

/// Rewrite a `br_table` site by inserting `call $helper` immediately
/// before it. The helper consumes the selector, increments the counter
/// matching the selector's value (one of N target counters or the default
/// counter), and pushes the selector back so the original `br_table`
/// dispatches as before.
fn rewrite_brtable(
    func: &mut walrus::LocalFunction,
    seq_id: InstrSeqId,
    index: usize,
    helper: FunctionId,
) {
    let call_instr = Instr::Call(walrus::ir::Call { func: helper });
    let instrs = &mut func.block_mut(seq_id).instrs;
    instrs.insert(index, (call_instr, Default::default()));
}

fn counter_inc_seq(func: &mut walrus::LocalFunction, counter: GlobalId) -> InstrSeqId {
    let mut seq = func
        .builder_mut()
        .dangling_instr_seq(InstrSeqType::Simple(None));
    for instr in counter_inc_instrs(counter) {
        seq.instr(instr);
    }
    seq.id()
}

fn counter_inc_instrs(counter: GlobalId) -> [Instr; 4] {
    [
        Instr::GlobalGet(GlobalGet { global: counter }),
        Instr::Const(Const {
            value: Value::I32(1),
        }),
        Instr::Binop(Binop {
            op: BinaryOp::I32Add,
        }),
        Instr::GlobalSet(GlobalSet { global: counter }),
    ]
}

#[cfg(test)]
// SAFETY-REVIEW: tests use `.unwrap()` / `.expect()` / `.[0]` intentionally —
// test failures should surface as panics, not silently swallow errors.
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod tests {
    use super::*;

    fn wat_to_module(wat_src: &str) -> Module {
        let wasm = wat::parse_str(wat_src).expect("valid wat");
        Module::from_buffer(&wasm).expect("walrus parse")
    }

    #[test]
    fn enumerates_if_else_as_two_branches() {
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
        let mut module = wat_to_module(wat_src);
        let entries = instrument_module(&mut module, "test").unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.kind == BranchKind::IfThen));
        assert!(entries.iter().any(|e| e.kind == BranchKind::IfElse));
    }

    #[test]
    fn enumerates_br_if_as_one_branch() {
        let wat_src = r#"
            (module
              (func (export "f") (param i32)
                block
                  local.get 0
                  br_if 0
                end))
        "#;
        let mut module = wat_to_module(wat_src);
        let entries = instrument_module(&mut module, "test").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, BranchKind::BrIf);
    }

    #[test]
    fn enumerates_br_table_as_per_target_plus_default() {
        // Wasm `br_table` syntax: `br_table target0 target1 ... default`.
        // `br_table 0 1 2` therefore has TWO explicit targets (labels 0, 1)
        // and a default (label 2). Walrus exposes this as `bt.blocks =
        // [target0, target1]` and `bt.default = label2`.
        let wat_src = r#"
            (module
              (func (export "f") (param i32)
                block
                  block
                    block
                      local.get 0
                      br_table 0 1 2
                    end
                  end
                end))
        "#;
        let mut module = wat_to_module(wat_src);
        let entries = instrument_module(&mut module, "test").unwrap();
        assert_eq!(
            entries.len(),
            3,
            "2 explicit targets + 1 default = 3 entries"
        );
        let target_count = entries
            .iter()
            .filter(|e| e.kind == BranchKind::BrTableTarget)
            .count();
        let default_count = entries
            .iter()
            .filter(|e| e.kind == BranchKind::BrTableDefault)
            .count();
        assert_eq!(target_count, 2);
        assert_eq!(default_count, 1);
        let mut idx: Vec<u32> = entries
            .iter()
            .filter_map(|e| {
                if e.kind == BranchKind::BrTableTarget {
                    e.target_index
                } else {
                    None
                }
            })
            .collect();
        idx.sort_unstable();
        assert_eq!(idx, vec![0, 1]);
    }

    #[test]
    fn exports_one_counter_per_branch() {
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
        let mut module = wat_to_module(wat_src);
        instrument_module(&mut module, "test").unwrap();
        let counter_exports = module
            .exports
            .iter()
            .filter(|e| e.name.starts_with(COUNTER_EXPORT_PREFIX))
            .count();
        assert_eq!(counter_exports, 2);
    }

    #[test]
    fn instrumented_module_re_parses() {
        let wat_src = r#"
            (module
              (func (export "f") (param i32) (result i32)
                local.get 0
                if (result i32)
                  local.get 0
                  i32.const 0
                  i32.ne
                  if (result i32)
                    i32.const 2
                  else
                    i32.const 3
                  end
                else
                  i32.const 0
                end))
        "#;
        let mut module = wat_to_module(wat_src);
        instrument_module(&mut module, "test").unwrap();
        let bytes = module.emit_wasm();
        Module::from_buffer(&bytes).expect("re-parse instrumented wasm");
    }

    #[test]
    fn empty_module_instruments_to_zero_branches() {
        let wat_src = r#"(module (func (export "f")))"#;
        let mut module = wat_to_module(wat_src);
        let entries = instrument_module(&mut module, "test").unwrap();
        assert_eq!(entries.len(), 0);
    }
}
