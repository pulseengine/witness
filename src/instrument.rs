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
use std::path::{Path, PathBuf};
use walrus::ir::{
    BinaryOp, Binop, BrIf, Const, GlobalGet, GlobalSet, IfElse, Instr, InstrSeqId, InstrSeqType,
    LocalGet, LocalTee, Value,
};
use walrus::{ConstExpr, FunctionId, GlobalId, LocalId, Module, ValType};

/// Manifest schema version. Bump on breaking changes; v0.1 pins to "1".
pub const MANIFEST_SCHEMA_VERSION: &str = "1";

/// Exported-global name prefix. Hosts discover counters by iterating exports
/// and matching on this prefix; the suffix is the branch id as decimal.
pub const COUNTER_EXPORT_PREFIX: &str = "__witness_counter_";

/// Kind of branch a counter is counting.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BranchKind {
    /// A `br_if` — counter fires when the branch is taken.
    BrIf,
    /// The `then` arm of an `if/else` — counter fires when the consequent runs.
    IfThen,
    /// The `else` arm of an `if/else` — counter fires when the alternative runs.
    IfElse,
    /// A `br_table` — counter fires once per execution, regardless of target.
    /// Per-target counting is v0.2.
    BrTable,
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
    /// Walrus `InstrSeqId` encoded as a debug string — diagnostic only.
    pub seq_debug: String,
}

/// Manifest written alongside the instrumented Wasm.
#[derive(Serialize, Deserialize, Debug)]
pub struct Manifest {
    pub schema_version: String,
    pub witness_version: String,
    pub module_source: String,
    pub branches: Vec<BranchEntry>,
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
    let mut module = Module::from_file(input).map_err(|source| Error::ParseModule {
        path: input.to_path_buf(),
        source,
    })?;

    let entries = instrument_module(&mut module, input.to_string_lossy().as_ref())?;

    let wasm = module.emit_wasm();
    std::fs::write(output, wasm).map_err(|source| Error::EmitModule {
        path: output.to_path_buf(),
        source,
    })?;

    let manifest = Manifest {
        schema_version: MANIFEST_SCHEMA_VERSION.to_string(),
        witness_version: env!("CARGO_PKG_VERSION").to_string(),
        module_source: input.to_string_lossy().into_owned(),
        branches: entries,
    };
    let manifest_path = Manifest::path_for(output);
    let manifest_json = serde_json::to_string_pretty(&manifest).map_err(Error::Serde)?;
    std::fs::write(&manifest_path, manifest_json).map_err(Error::Io)?;

    Ok(())
}

/// Instrument `module` in place, returning the branch manifest entries.
pub fn instrument_module(module: &mut Module, _module_source: &str) -> Result<Vec<BranchEntry>> {
    let scans: Vec<FunctionScan> = collect_scans(module);

    let mut entries: Vec<BranchEntry> = Vec::new();
    let mut counter_globals: Vec<GlobalId> = Vec::new();
    for scan in &scans {
        for branch in &scan.branches {
            let id = u32::try_from(entries.len())
                .map_err(|_| Error::Instrument("branch count exceeds u32::MAX".to_string()))?;
            let global = module.globals.add_local(
                ValType::I32,
                true,
                false,
                ConstExpr::Value(Value::I32(0)),
            );
            let export_name = format!("{COUNTER_EXPORT_PREFIX}{id}");
            module.exports.add(&export_name, global);
            counter_globals.push(global);
            entries.push(BranchEntry {
                id,
                function_index: scan.function_index,
                function_name: scan.function_name.clone(),
                kind: branch.kind,
                instr_index: branch.instr_index,
                seq_debug: format!("{:?}", branch.seq_id),
            });
        }
    }

    let mut cursor: usize = 0;
    for scan in scans.into_iter() {
        let len = scan.branches.len();
        // SAFETY-REVIEW: `cursor` and `len` are both <= counter_globals.len()
        // by construction — counter_globals is built in the exact iteration
        // above with one element per BranchSite across scans.
        let end = cursor.saturating_add(len);
        let slice = counter_globals
            .get(cursor..end)
            .ok_or_else(|| Error::Instrument("counter slice out of range".to_string()))?
            .to_vec();
        rewrite_function(module, scan.function_id, &scan.branches, &slice)?;
        cursor = end;
    }

    Ok(entries)
}

struct BranchSite {
    seq_id: InstrSeqId,
    instr_index: u32,
    kind: BranchKind,
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
    for (index, (instr, _loc)) in seq.instrs.iter().enumerate() {
        let i = u32::try_from(index).unwrap_or(u32::MAX);
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
            }),
            Instr::IfElse(IfElse {
                consequent,
                alternative,
            }) => {
                out.push(BranchSite {
                    seq_id,
                    instr_index: i,
                    kind: BranchKind::IfThen,
                });
                out.push(BranchSite {
                    seq_id,
                    instr_index: i,
                    kind: BranchKind::IfElse,
                });
                walk_collect(func, *consequent, out);
                walk_collect(func, *alternative, out);
            }
            Instr::BrTable(_) => out.push(BranchSite {
                seq_id,
                instr_index: i,
                kind: BranchKind::BrTable,
            }),
            Instr::Block(b) => walk_collect(func, b.seq, out),
            Instr::Loop(l) => walk_collect(func, l.seq, out),
            _ => {}
        }
    }
}

fn rewrite_function(
    module: &mut Module,
    function_id: FunctionId,
    sites: &[BranchSite],
    counters: &[GlobalId],
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
    // entries at the same position (one IfThen, one IfElse) that we rewrite
    // together.
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

        match kind_at {
            PeekKind::BrIf => {
                let (idx_in_sites, _) = *group
                    .first()
                    .ok_or_else(|| Error::Instrument("empty br_if group".to_string()))?;
                let counter = counter_at(idx_in_sites)?;
                let tmp = brif_tmp.ok_or_else(|| {
                    Error::Instrument("br_if site without brif_tmp local".to_string())
                })?;
                rewrite_brif(func, seq_id, at, counter, tmp);
            }
            PeekKind::IfElse => {
                let then_counter = group
                    .iter()
                    .find(|(_, k)| matches!(k, BranchKind::IfThen))
                    .map(|(i, _)| counter_at(*i))
                    .transpose()?;
                let else_counter = group
                    .iter()
                    .find(|(_, k)| matches!(k, BranchKind::IfElse))
                    .map(|(i, _)| counter_at(*i))
                    .transpose()?;
                rewrite_ifelse(func, seq_id, at, then_counter, else_counter);
            }
            PeekKind::BrTable => {
                let (idx_in_sites, _) = *group
                    .first()
                    .ok_or_else(|| Error::Instrument("empty br_table group".to_string()))?;
                let counter = counter_at(idx_in_sites)?;
                rewrite_brtable(func, seq_id, at, counter);
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

/// Rewrite `br_if L` at `(seq_id, index)` into:
/// `local.tee tmp; if (inc counter) end; local.get tmp; br_if L`.
fn rewrite_brif(
    func: &mut walrus::LocalFunction,
    seq_id: InstrSeqId,
    index: usize,
    counter: GlobalId,
    tmp: LocalId,
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

    let replacement = [
        Instr::LocalTee(LocalTee { local: tmp }),
        Instr::IfElse(IfElse {
            consequent: inc_seq,
            alternative: empty_seq,
        }),
        Instr::LocalGet(LocalGet { local: tmp }),
        Instr::BrIf(BrIf { block: target }),
    ];
    let instrs = &mut func.block_mut(seq_id).instrs;
    for (offset, instr) in replacement.into_iter().enumerate() {
        // SAFETY-REVIEW: `offset` is <= replacement.len() (=4) and
        // `index` is an in-bounds position into the sequence's instrs.
        let pos = index.saturating_add(offset);
        instrs.insert(pos, (instr, Default::default()));
    }
}

fn rewrite_ifelse(
    func: &mut walrus::LocalFunction,
    seq_id: InstrSeqId,
    index: usize,
    then_counter: Option<GlobalId>,
    else_counter: Option<GlobalId>,
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
        prepend_counter_inc(func, then_id, c);
    }
    if let Some(c) = else_counter {
        prepend_counter_inc(func, else_id, c);
    }
}

fn rewrite_brtable(
    func: &mut walrus::LocalFunction,
    seq_id: InstrSeqId,
    index: usize,
    counter: GlobalId,
) {
    let increments = counter_inc_instrs(counter);
    let instrs = &mut func.block_mut(seq_id).instrs;
    for (offset, instr) in increments.into_iter().enumerate() {
        // SAFETY-REVIEW: `offset` is <= 4 and `index` is in bounds.
        let pos = index.saturating_add(offset);
        instrs.insert(pos, (instr, Default::default()));
    }
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

fn prepend_counter_inc(func: &mut walrus::LocalFunction, seq_id: InstrSeqId, counter: GlobalId) {
    let increments = counter_inc_instrs(counter);
    let instrs = &mut func.block_mut(seq_id).instrs;
    for (offset, instr) in increments.into_iter().enumerate() {
        instrs.insert(offset, (instr, Default::default()));
    }
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
    fn enumerates_br_table_as_one_branch() {
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
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, BranchKind::BrTable);
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
