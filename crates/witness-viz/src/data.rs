use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Deserialize, Serialize, Clone)]
pub struct McdcReport {
    pub schema: String,
    pub witness_version: String,
    pub module: String,
    pub overall: Overall,
    pub decisions: Vec<DecisionReport>,
    #[serde(default)]
    pub trace_health: Option<TraceHealth>,
}

#[derive(Deserialize, Serialize, Clone, Copy)]
pub struct Overall {
    pub decisions_total: u32,
    pub decisions_full_mcdc: u32,
    pub conditions_total: u32,
    pub conditions_proved: u32,
    pub conditions_gap: u32,
    pub conditions_dead: u32,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct DecisionReport {
    pub id: u32,
    pub source_file: String,
    pub source_line: u32,
    pub conditions: Vec<ConditionReport>,
    pub truth_table: Vec<TruthRow>,
    pub status: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ConditionReport {
    pub index: u32,
    pub branch_id: u32,
    pub status: String,
    #[serde(default)]
    pub interpretation: Option<String>,
    #[serde(default)]
    pub pair: Option<[u32; 2]>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct TruthRow {
    pub row_id: u32,
    pub evaluated: BTreeMap<String, bool>,
    pub outcome: bool,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct TraceHealth {
    pub overflow: bool,
    pub rows: u32,
    pub ambiguous_rows: bool,
}

pub struct VerdictBundle {
    pub name: String,
    pub report: McdcReport,
}

pub fn load_verdicts(reports_dir: &std::path::Path) -> anyhow::Result<Vec<VerdictBundle>> {
    let mut out = Vec::new();
    if !reports_dir.is_dir() {
        return Ok(out);
    }
    let mut entries: Vec<_> = std::fs::read_dir(reports_dir)?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let report_path = entry.path().join("report.json");
        if !report_path.is_file() {
            continue;
        }
        let bytes = std::fs::read(&report_path)?;
        let report: McdcReport = match serde_json::from_slice(&bytes) {
            Ok(r) => r,
            Err(_) => continue,
        };
        out.push(VerdictBundle {
            name: entry.file_name().to_string_lossy().into_owned(),
            report,
        });
    }
    Ok(out)
}

pub fn find_verdict(
    reports_dir: &std::path::Path,
    name: &str,
) -> anyhow::Result<Option<VerdictBundle>> {
    let safe = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.');
    if !safe {
        return Ok(None);
    }
    let dir = reports_dir.join(name);
    if !dir.is_dir() {
        return Ok(None);
    }
    let report_path = dir.join("report.json");
    if !report_path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(&report_path)?;
    let report: McdcReport = match serde_json::from_slice(&bytes) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    Ok(Some(VerdictBundle {
        name: name.to_string(),
        report,
    }))
}

/// Branches per-verdict — read manifest.json if present, fall back to the
/// number of unique condition branch_ids in the report.
pub fn branch_count(reports_dir: &std::path::Path, verdict: &VerdictBundle) -> u32 {
    if let Some(n) = manifest_branch_count(reports_dir, &verdict.name) {
        return n;
    }
    // Fallback: count unique branch_ids.
    let mut ids = std::collections::BTreeSet::new();
    for d in &verdict.report.decisions {
        for c in &d.conditions {
            ids.insert(c.branch_id);
        }
    }
    u32::try_from(ids.len()).unwrap_or(0)
}

fn manifest_branch_count(reports_dir: &std::path::Path, name: &str) -> Option<u32> {
    let manifest = reports_dir.join(name).join("manifest.json");
    let bytes = std::fs::read(&manifest).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let branches = value.get("branches")?.as_array()?;
    u32::try_from(branches.len()).ok()
}
