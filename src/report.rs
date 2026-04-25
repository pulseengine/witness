//! Coverage report generation from raw run data.
//!
//! # v0.1 output
//!
//! Per-function branch-coverage summary:
//!   - total branches
//!   - covered branches (hit count > 0)
//!   - coverage ratio
//!   - list of uncovered branches with function + instruction index
//!
//! Deterministic: identical `RunRecord` input produces identical output.
//! BTreeMap keyed by `function_index` and an explicit sort on uncovered
//! branches keep HashMap iteration order from leaking into output.

use crate::instrument::BranchKind;
use crate::run::{BranchHit, RunRecord};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Report {
    pub schema_version: String,
    pub witness_version: String,
    pub module: String,
    pub total_branches: u32,
    pub covered_branches: u32,
    pub per_function: Vec<FunctionReport>,
    pub uncovered: Vec<UncoveredBranch>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionReport {
    pub function_index: u32,
    pub function_name: Option<String>,
    pub total: u32,
    pub covered: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UncoveredBranch {
    pub branch_id: u32,
    pub function_index: u32,
    pub function_name: Option<String>,
    pub instr_index: u32,
    pub kind: BranchKind,
}

impl Report {
    pub fn coverage_ratio(&self) -> f64 {
        if self.total_branches == 0 {
            1.0
        } else {
            f64::from(self.covered_branches) / f64::from(self.total_branches)
        }
    }

    pub fn from_record(record: &RunRecord) -> Self {
        let total = u32::try_from(record.branches.len()).unwrap_or(u32::MAX);
        let covered = u32::try_from(record.branches.iter().filter(|b| b.hits > 0).count())
            .unwrap_or(u32::MAX);

        let mut per_fn: BTreeMap<u32, (Option<String>, u32, u32)> = BTreeMap::new();
        for b in &record.branches {
            let entry = per_fn
                .entry(b.function_index)
                .or_insert_with(|| (b.function_name.clone(), 0, 0));
            // SAFETY-REVIEW: total branches per function is bounded by the
            // manifest entry count, which fits in u32 by construction.
            entry.1 = entry.1.saturating_add(1);
            if b.hits > 0 {
                entry.2 = entry.2.saturating_add(1);
            }
        }
        let per_function = per_fn
            .into_iter()
            .map(|(idx, (name, total, covered))| FunctionReport {
                function_index: idx,
                function_name: name,
                total,
                covered,
            })
            .collect();

        let mut uncovered: Vec<UncoveredBranch> = record
            .branches
            .iter()
            .filter(|b: &&BranchHit| b.hits == 0)
            .map(|b: &BranchHit| UncoveredBranch {
                branch_id: b.id,
                function_index: b.function_index,
                function_name: b.function_name.clone(),
                instr_index: b.instr_index,
                kind: b.kind,
            })
            .collect();
        uncovered.sort_by_key(|u| (u.function_index, u.instr_index, u.branch_id));

        Report {
            schema_version: record.schema_version.clone(),
            witness_version: env!("CARGO_PKG_VERSION").to_string(),
            module: record.module_path.clone(),
            total_branches: total,
            covered_branches: covered,
            per_function,
            uncovered,
        }
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("module: {}\n", self.module));
        out.push_str(&format!(
            "coverage: {}/{} ({:.1}%)\n",
            self.covered_branches,
            self.total_branches,
            self.coverage_ratio() * 100.0,
        ));
        if !self.per_function.is_empty() {
            out.push_str("\nper function:\n");
            for f in &self.per_function {
                let name = f.function_name.as_deref().unwrap_or("(anon)");
                let ratio = if f.total == 0 {
                    1.0
                } else {
                    f64::from(f.covered) / f64::from(f.total)
                };
                out.push_str(&format!(
                    "  fn {} ({}): {}/{} ({:.1}%)\n",
                    f.function_index,
                    name,
                    f.covered,
                    f.total,
                    ratio * 100.0,
                ));
            }
        }
        if !self.uncovered.is_empty() {
            out.push_str(&format!(
                "\nuncovered branches ({}):\n",
                self.uncovered.len()
            ));
            for b in &self.uncovered {
                let name = b.function_name.as_deref().unwrap_or("(anon)");
                out.push_str(&format!(
                    "  fn {} ({}) instr +{} [{:?}] id={}\n",
                    b.function_index, name, b.instr_index, b.kind, b.branch_id,
                ));
            }
        }
        out
    }
}

pub fn from_run_file(path: &Path) -> Result<Report> {
    let record = RunRecord::load(path)?;
    Ok(Report::from_record(&record))
}

pub fn save_json(report: &Report, path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(report).map_err(Error::Serde)?;
    std::fs::write(path, json).map_err(Error::Io)
}

#[cfg(test)]
// SAFETY-REVIEW: tests use `.unwrap()` / indexing intentionally; panic on
// failure is the desired test-signal behaviour.
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod tests {
    use super::*;
    use crate::run::BranchHit;

    fn hit(id: u32, hits: u64, kind: BranchKind, fn_idx: u32, fn_name: &str) -> BranchHit {
        BranchHit {
            id,
            function_index: fn_idx,
            function_name: Some(fn_name.to_string()),
            kind,
            instr_index: id,
            hits,
        }
    }

    fn sample_record() -> RunRecord {
        RunRecord {
            schema_version: "1".to_string(),
            witness_version: "test".to_string(),
            module_path: "sample.wasm".to_string(),
            invoked: vec!["f".to_string()],
            branches: vec![
                hit(0, 1, BranchKind::IfThen, 0, "f"),
                hit(1, 0, BranchKind::IfElse, 0, "f"),
                hit(2, 3, BranchKind::BrIf, 1, "g"),
                hit(3, 0, BranchKind::BrTableDefault, 1, "g"),
            ],
        }
    }

    #[test]
    fn aggregates_per_function() {
        let record = sample_record();
        let report = Report::from_record(&record);
        assert_eq!(report.total_branches, 4);
        assert_eq!(report.covered_branches, 2);
        assert_eq!(report.per_function.len(), 2);
        assert_eq!(report.per_function[0].function_index, 0);
        assert_eq!(report.per_function[0].total, 2);
        assert_eq!(report.per_function[0].covered, 1);
        assert_eq!(report.per_function[1].function_index, 1);
        assert_eq!(report.per_function[1].total, 2);
        assert_eq!(report.per_function[1].covered, 1);
    }

    #[test]
    fn lists_uncovered_deterministically() {
        let report = Report::from_record(&sample_record());
        assert_eq!(report.uncovered.len(), 2);
        assert_eq!(report.uncovered[0].branch_id, 1);
        assert_eq!(report.uncovered[1].branch_id, 3);
    }

    #[test]
    fn coverage_ratio_handles_empty_module() {
        let empty = RunRecord {
            schema_version: "1".to_string(),
            witness_version: "test".to_string(),
            module_path: "empty.wasm".to_string(),
            invoked: vec![],
            branches: vec![],
        };
        let report = Report::from_record(&empty);
        assert_eq!(report.total_branches, 0);
        assert!((report.coverage_ratio() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn text_format_contains_headline_ratio() {
        let report = Report::from_record(&sample_record());
        let text = report.to_text();
        assert!(text.contains("coverage: 2/4"));
        assert!(text.contains("uncovered branches (2)"));
    }
}
