//! LCOV emission from a `RunRecord`.
//!
//! Hybrid emission per `docs/research/v05-lcov-format.md`: branches
//! with DWARF-correlated `Decision`s in the manifest emit standard
//! `BRDA` records keyed to real source files; branches without DWARF
//! correlation are listed in a sibling overview text rather than
//! synthesised into fake source paths (codecov rejects synthetic paths
//! during git-tree-fixing).
//!
//! Codecov ingests this file as a flagged upload (flag
//! `wasm-bytecode`) — see the v05 brief for the codecov-action snippet.

use crate::Result;
use crate::error::Error;
use crate::instrument::Manifest;
use crate::run_record::{BranchHit, RunRecord};
use std::collections::BTreeMap;
use std::path::Path;

/// Emit LCOV for the DWARF-correlated subset of branches.
///
/// Manifest's `Decision`s carry `(source_file, source_line)`. Each
/// `Decision` becomes one source-file `SF` block; conditions become
/// `BRDA` records.
pub fn emit_lcov(manifest: &Manifest, record: &RunRecord) -> String {
    let hits_by_id: BTreeMap<u32, u64> = record.branches.iter().map(|b| (b.id, b.hits)).collect();

    // Group decisions by source file.
    let mut by_file: BTreeMap<&str, Vec<&crate::instrument::Decision>> = BTreeMap::new();
    for d in &manifest.decisions {
        if let Some(file) = d.source_file.as_deref() {
            // SAFETY-REVIEW: lcov line 0 is rejected by downstream
            // parsers — guard.
            if d.source_line.unwrap_or(0) == 0 {
                continue;
            }
            by_file.entry(file).or_default().push(d);
        }
    }

    let mut out = String::new();
    out.push_str("TN:wasm-bytecode\n");
    for (file, decisions) in by_file {
        out.push_str(&format!("SF:{file}\n"));

        // Function records — group conditions by their function (we
        // approximate via the first condition's function-index in the
        // manifest's BranchEntry for the first condition).
        let mut fn_seen = false;
        for d in &decisions {
            let line = d.source_line.unwrap_or(0);
            // Find a manifest BranchEntry to learn function name.
            let fn_name = manifest
                .branches
                .iter()
                .find(|b| d.conditions.first().is_some_and(|c| *c == b.id))
                .and_then(|b| b.function_name.clone())
                .unwrap_or_else(|| format!("decision_{}", d.id));
            if !fn_seen {
                out.push_str(&format!("FN:{line},{fn_name}\n"));
                let total_hits: u64 = d
                    .conditions
                    .iter()
                    .map(|id| hits_by_id.get(id).copied().unwrap_or(0))
                    .sum();
                out.push_str(&format!("FNDA:{total_hits},{fn_name}\n"));
                fn_seen = true;
            }
        }
        out.push_str("FNF:1\nFNH:1\n");

        // BRDA records: <line>,<block>,<branch>,<taken>
        let mut brf: u32 = 0;
        let mut brh: u32 = 0;
        for d in &decisions {
            let line = d.source_line.unwrap_or(0);
            for (i, cond_id) in d.conditions.iter().enumerate() {
                let hits = hits_by_id.get(cond_id).copied().unwrap_or(0);
                let block = d.id;
                let branch = u32::try_from(i).unwrap_or(u32::MAX);
                out.push_str(&format!("BRDA:{line},{block},{branch},{hits}\n"));
                brf = brf.saturating_add(1);
                if hits > 0 {
                    brh = brh.saturating_add(1);
                }
            }
        }
        out.push_str(&format!("BRF:{brf}\nBRH:{brh}\n"));

        // Line records (one DA per decision line, summing condition hits).
        let mut lf: u32 = 0;
        let mut lh: u32 = 0;
        let mut lines_seen: BTreeMap<u32, u64> = BTreeMap::new();
        for d in &decisions {
            let line = d.source_line.unwrap_or(0);
            let total_hits: u64 = d
                .conditions
                .iter()
                .map(|id| hits_by_id.get(id).copied().unwrap_or(0))
                .sum();
            *lines_seen.entry(line).or_insert(0) = lines_seen
                .get(&line)
                .copied()
                .unwrap_or(0)
                .saturating_add(total_hits);
        }
        for (line, hits) in &lines_seen {
            out.push_str(&format!("DA:{line},{hits}\n"));
            lf = lf.saturating_add(1);
            if *hits > 0 {
                lh = lh.saturating_add(1);
            }
        }
        out.push_str(&format!("LF:{lf}\nLH:{lh}\n"));
        out.push_str("end_of_record\n");
    }

    out
}

/// Emit a sibling overview text listing branches that DWARF
/// reconstruction did NOT cover. Useful as a CI artefact alongside the
/// LCOV file.
pub fn emit_overview(manifest: &Manifest, record: &RunRecord) -> String {
    let hits_by_id: BTreeMap<u32, u64> = record.branches.iter().map(|b| (b.id, b.hits)).collect();
    let in_decision: std::collections::HashSet<u32> = manifest
        .decisions
        .iter()
        .flat_map(|d| d.conditions.iter().copied())
        .collect();

    let uncorrelated: Vec<&BranchHit> = record
        .branches
        .iter()
        .filter(|b| !in_decision.contains(&b.id))
        .collect();

    let mut out = String::new();
    out.push_str("# witness — branches not covered by DWARF reconstruction\n");
    out.push_str(&format!(
        "# {} branches; {} in Decisions; {} uncorrelated\n\n",
        record.branches.len(),
        record.branches.len().saturating_sub(uncorrelated.len()),
        uncorrelated.len(),
    ));
    if uncorrelated.is_empty() {
        out.push_str("All branches correlated. No overview entries.\n");
        return out;
    }
    out.push_str("function_index | instr_index | kind | hits\n");
    out.push_str("---|---|---|---\n");
    for b in &uncorrelated {
        let _ = hits_by_id.get(&b.id);
        out.push_str(&format!(
            "{} | {} | {:?} | {}\n",
            b.function_index, b.instr_index, b.kind, b.hits,
        ));
    }
    out
}

/// Emit LCOV + overview to two paths. Both are text files.
pub fn emit_lcov_files(
    manifest: &Manifest,
    record: &RunRecord,
    lcov_path: &Path,
    overview_path: &Path,
) -> Result<()> {
    std::fs::write(lcov_path, emit_lcov(manifest, record)).map_err(Error::Io)?;
    std::fs::write(overview_path, emit_overview(manifest, record)).map_err(Error::Io)?;
    Ok(())
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
    use crate::instrument::{BranchEntry, BranchKind, Decision, Manifest};

    fn manifest_with_two_decisions() -> Manifest {
        Manifest {
            schema_version: "2".to_string(),
            witness_version: "test".to_string(),
            module_source: "x.wasm".to_string(),
            original_module_sha256: None,
            branches: vec![
                BranchEntry {
                    id: 0,
                    function_index: 1,
                    function_name: Some("f".to_string()),
                    kind: BranchKind::BrIf,
                    instr_index: 5,
                    target_index: None,
                    byte_offset: Some(100),
                    seq_debug: String::new(),
                },
                BranchEntry {
                    id: 1,
                    function_index: 1,
                    function_name: Some("f".to_string()),
                    kind: BranchKind::BrIf,
                    instr_index: 6,
                    target_index: None,
                    byte_offset: Some(105),
                    seq_debug: String::new(),
                },
                // Uncorrelated entry (no Decision).
                BranchEntry {
                    id: 2,
                    function_index: 2,
                    function_name: None,
                    kind: BranchKind::IfThen,
                    instr_index: 1,
                    target_index: None,
                    byte_offset: None,
                    seq_debug: String::new(),
                },
            ],
            decisions: vec![Decision {
                id: 0,
                conditions: vec![0, 1],
                source_file: Some("src/lib.rs".to_string()),
                source_line: Some(42),
                chain_kind: crate::instrument::ChainKind::default(),
                inline_context: None,
            }],
        }
    }

    fn record_for(hits: &[u64]) -> RunRecord {
        RunRecord {
            schema_version: "2".to_string(),
            witness_version: "test".to_string(),
            module_path: "x.wasm".to_string(),
            invoked: vec![],
            branches: hits
                .iter()
                .enumerate()
                .map(|(i, &h)| BranchHit {
                    id: u32::try_from(i).unwrap(),
                    function_index: 1,
                    function_name: Some("f".to_string()),
                    kind: BranchKind::BrIf,
                    instr_index: u32::try_from(i).unwrap(),
                    hits: h,
                })
                .collect(),
            decisions: vec![],
            trace_health: Default::default(),
        }
    }

    #[test]
    fn lcov_includes_correlated_decisions() {
        let m = manifest_with_two_decisions();
        let r = record_for(&[3, 0, 1]);
        let lcov = emit_lcov(&m, &r);
        assert!(lcov.starts_with("TN:wasm-bytecode\n"));
        assert!(lcov.contains("SF:src/lib.rs"));
        assert!(lcov.contains("BRDA:42,0,0,3"));
        assert!(lcov.contains("BRDA:42,0,1,0"));
        assert!(lcov.contains("BRF:2"));
        assert!(lcov.contains("BRH:1"));
        assert!(lcov.contains("end_of_record"));
    }

    #[test]
    fn overview_lists_uncorrelated() {
        let m = manifest_with_two_decisions();
        let r = record_for(&[3, 0, 7]);
        let overview = emit_overview(&m, &r);
        assert!(overview.contains("3 branches; 2 in Decisions; 1 uncorrelated"));
        // The third branch in `record_for` is BrIf (record-side kind);
        // the fact that it survives uncorrelated is the point of the test.
        assert!(overview.contains("| BrIf |"));
    }

    #[test]
    fn lcov_skips_line_zero_decisions() {
        let mut m = manifest_with_two_decisions();
        m.decisions[0].source_line = Some(0);
        let r = record_for(&[3, 0, 1]);
        let lcov = emit_lcov(&m, &r);
        // Line 0 decision skipped → only TN header, no SF block.
        assert_eq!(lcov, "TN:wasm-bytecode\n");
    }
}
