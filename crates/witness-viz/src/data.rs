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
    /// v0.10.0 — renamed from `ambiguous_rows` upstream. Accept both
    /// names on deserialise so v0.9.x reports keep loading; emit with
    /// the new name.
    #[serde(alias = "ambiguous_rows")]
    pub trace_parser_active: bool,
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

/// Load a report set from a path that is *either* a verdict-evidence
/// directory (one subdir per verdict, each with `report.json`) *or* a
/// single `report.json` file. Used by `witness viz-pr-comment` where
/// `--base` / `--head` may point at either shape. A single file
/// becomes a one-element `Vec` whose verdict name is the file's parent
/// directory name (or the file stem if there's no meaningful parent).
pub fn load_report_set(path: &std::path::Path) -> anyhow::Result<Vec<VerdictBundle>> {
    if path.is_dir() {
        return load_verdicts(path);
    }
    if path.is_file() {
        let bytes = std::fs::read(path)?;
        let report: McdcReport = serde_json::from_slice(&bytes)
            .map_err(|e| anyhow::anyhow!("parsing {}: {e}", path.display()))?;
        // Name: prefer the parent dir name (verdict-evidence layout puts
        // report.json under <verdict>/), else the file stem.
        let name = path
            .parent()
            .and_then(|p| p.file_name())
            .filter(|n| *n != "." && !n.is_empty())
            .or_else(|| path.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "report".to_string());
        return Ok(vec![VerdictBundle { name, report }]);
    }
    anyhow::bail!("{} is neither a directory nor a file", path.display())
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

/// One frame of a DWARF inline call chain: where the inlined branch
/// was called from. v0.27.
#[derive(Debug, Clone)]
pub struct InlineFrame {
    pub call_file: String,
    pub call_line: u32,
}

/// Per-branch provenance, joined from `manifest.json` by branch id
/// (== the report condition's `branch_id`). v0.27 — explains *why* a
/// condition exists: which function it lives in, whether it's a
/// `br_if` or a `br_table` arm, and (rarely) its inline call chain.
#[derive(Debug, Clone, Default)]
pub struct BranchProvenance {
    /// Demangled function name the branch lives in (e.g.
    /// `verdict_json_lite::parse_primitive`).
    pub function: String,
    /// Branch kind from the manifest: `br_if`, `br_table_target`,
    /// `br_table_default`, …
    pub kind: String,
    /// DWARF inline call chain, outermost (call site) first. Empty
    /// for the common non-inlined case.
    pub inline_chain: Vec<InlineFrame>,
}

/// Load per-branch provenance for a verdict from its `manifest.json`.
/// Returns `branch_id → BranchProvenance`. Empty map when the
/// manifest is absent or unparseable (the Decision page degrades to
/// no provenance, same as pre-v0.27). v0.27.
pub fn load_branch_provenance(
    reports_dir: &std::path::Path,
    name: &str,
) -> BTreeMap<u32, BranchProvenance> {
    let mut out = BTreeMap::new();
    let manifest = reports_dir.join(name).join("manifest.json");
    let Ok(bytes) = std::fs::read(&manifest) else {
        return out;
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return out;
    };

    // branches[] → function_name + kind, keyed by `id`.
    if let Some(branches) = value.get("branches").and_then(|b| b.as_array()) {
        for b in branches {
            let Some(id) = b.get("id").and_then(serde_json::Value::as_u64) else {
                continue;
            };
            let id = u32::try_from(id).unwrap_or(u32::MAX);
            let raw_fn = b
                .get("function_name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            // `{:#}` drops the trailing `::h<hash>` disambiguator.
            let function = format!("{:#}", rustc_demangle::demangle(raw_fn));
            let kind = b
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            out.insert(
                id,
                BranchProvenance {
                    function,
                    kind,
                    inline_chain: Vec::new(),
                },
            );
        }
    }

    // branch_inline_chains: { "<id>": [{call_file, call_line}, …] }.
    // Merge onto the entries already built from branches[].
    if let Some(chains) = value
        .get("branch_inline_chains")
        .and_then(|c| c.as_object())
    {
        for (k, frames_v) in chains {
            let Ok(id) = k.parse::<u32>() else { continue };
            let frames: Vec<InlineFrame> = frames_v
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|f| {
                            Some(InlineFrame {
                                call_file: f.get("call_file")?.as_str()?.to_string(),
                                call_line: u32::try_from(
                                    f.get("call_line")?.as_u64().unwrap_or(0),
                                )
                                .unwrap_or(0),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            out.entry(id).or_default().inline_chain = frames;
        }
    }

    out
}
