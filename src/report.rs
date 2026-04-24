//! Coverage report generation from raw run data.
//!
//! # v0.1 output
//!
//! Per-function branch-coverage summary:
//!   - total branches
//!   - covered branches (hit count > 0)
//!   - coverage ratio
//!   - list of uncovered branches with function + offset
//!
//! This is NOT MC/DC yet. It is decision-level branch coverage — the first of
//! five deliveries toward full MC/DC (see [`DESIGN.md`](../DESIGN.md)).

use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub module: String,
    pub total_branches: u32,
    pub covered_branches: u32,
    pub per_function: Vec<FunctionReport>,
    pub uncovered: Vec<UncoveredBranch>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionReport {
    pub function_index: u32,
    pub function_name: Option<String>,
    pub total: u32,
    pub covered: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UncoveredBranch {
    pub branch_id: u32,
    pub function_index: u32,
    pub function_name: Option<String>,
    pub instruction_offset: u32,
    pub kind: BranchKind,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum BranchKind {
    BrIf,
    BrTable,
    If,
}

impl Report {
    pub fn coverage_ratio(&self) -> f64 {
        if self.total_branches == 0 {
            1.0
        } else {
            self.covered_branches as f64 / self.total_branches as f64
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
        if !self.uncovered.is_empty() {
            out.push_str(&format!("\nuncovered branches ({}):\n", self.uncovered.len()));
            for b in &self.uncovered {
                let name = b.function_name.as_deref().unwrap_or("?");
                out.push_str(&format!(
                    "  fn {} ({}) +{} [{:?}]\n",
                    b.function_index, name, b.instruction_offset, b.kind
                ));
            }
        }
        out
    }
}

/// Read a run output file and produce a coverage report.
pub fn from_run_file(path: &Path) -> Result<Report> {
    let _ = path;
    // TODO(v0.1): parse the JSON produced by run::run_harness, aggregate
    // counters by function, build the Report.
    todo!("v0.1 — aggregate run data into report; see DESIGN.md")
}
