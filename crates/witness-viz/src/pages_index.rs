//! Multi-version Pages landing page (DEC-033, REQ-050).
//!
//! `witness-viz pages-index --site-dir <dir>` scans `<dir>` for
//! versioned subdirectories (`vX.Y.Z/`), reads each one's
//! `summary.json` (emitted by `export`), and writes `<dir>/index.html`
//! — a cross-version MC/DC summary table (newest first, with Δ vs the
//! next-older release) plus links into each versioned dashboard.
//!
//! `render_pages_index` is pure (takes rows, returns HTML); the
//! scanning / parsing lives in [`load_site_versions`] so the renderer
//! is unit-testable without a filesystem.

use std::fmt::Write;
use std::path::Path;

use serde::Deserialize;

use crate::layout::escape;
use crate::render::RenderResult;

/// One release's aggregates, as needed by the cross-version table.
#[derive(Debug, Clone)]
pub struct VersionRow {
    pub version: String,
    pub verdicts: u64,
    pub decisions: u64,
    pub decisions_full_mcdc: u64,
    pub conditions_proved: u64,
    pub conditions_gap: u64,
    pub conditions_dead: u64,
    pub source_files: u64,
}

/// Shape of the `summary.json` `export` writes. Only the fields the
/// index needs; `#[serde(default)]` so a pre-v0.26 summary.json (no
/// MC/DC aggregates) still loads with zeros rather than failing.
#[derive(Deserialize)]
struct SummaryJson {
    #[serde(default)]
    verdicts: u64,
    #[serde(default)]
    decisions: u64,
    #[serde(default)]
    decisions_full_mcdc: u64,
    #[serde(default)]
    conditions_proved: u64,
    #[serde(default)]
    conditions_gap: u64,
    #[serde(default)]
    conditions_dead: u64,
    #[serde(default)]
    source_files: u64,
}

/// Parse a `vX.Y.Z` directory name into a sortable triple. Returns
/// `None` for names that aren't a `v`-prefixed three-part version, so
/// non-version subdirs (`_assets`, `source`, …) are skipped.
fn parse_version(name: &str) -> Option<(u64, u64, u64)> {
    let rest = name.strip_prefix('v')?;
    let mut parts = rest.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None; // more than three parts — not our shape
    }
    Some((major, minor, patch))
}

/// Scan `site_dir` for `vX.Y.Z/summary.json`, parse each, return rows
/// sorted **newest first**. Directories without a parseable version
/// name or a readable summary.json are skipped.
pub fn load_site_versions(site_dir: &Path) -> std::io::Result<Vec<VersionRow>> {
    let mut rows: Vec<((u64, u64, u64), VersionRow)> = Vec::new();
    for entry in std::fs::read_dir(site_dir)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(sortkey) = parse_version(&name) else {
            continue;
        };
        let summary_path = entry.path().join("summary.json");
        let Ok(bytes) = std::fs::read(&summary_path) else {
            continue;
        };
        let Ok(s) = serde_json::from_slice::<SummaryJson>(&bytes) else {
            continue;
        };
        rows.push((
            sortkey,
            VersionRow {
                version: name,
                verdicts: s.verdicts,
                decisions: s.decisions,
                decisions_full_mcdc: s.decisions_full_mcdc,
                conditions_proved: s.conditions_proved,
                conditions_gap: s.conditions_gap,
                conditions_dead: s.conditions_dead,
                source_files: s.source_files,
            },
        ));
    }
    // Newest first.
    rows.sort_by_key(|(key, _)| std::cmp::Reverse(*key));
    Ok(rows.into_iter().map(|(_, r)| r).collect())
}

/// Render the cross-version landing page body. Rows must be ordered
/// newest-first (as [`load_site_versions`] returns them); each row's
/// Δ is computed against the next (older) row.
pub fn render_pages_index(rows: &[VersionRow]) -> RenderResult {
    let mut body = String::new();
    body.push_str("<h1>witness — MC/DC coverage across releases</h1>\n");

    if rows.is_empty() {
        body.push_str(
            "<div class=\"empty\">No versioned dashboards found. Each <code>vX.Y.Z/</code> subdirectory should contain a <code>summary.json</code>.</div>\n",
        );
        return RenderResult {
            title: "witness — releases".to_string(),
            body,
        };
    }

    body.push_str(
        "<p class=\"muted\">Each release's MC/DC dashboard, newest first. Δ is versus the next-older release shown.</p>\n",
    );
    body.push_str("<table>\n<thead><tr>");
    for h in [
        "release",
        "verdicts",
        "decisions",
        "full MC/DC",
        "proved",
        "gap",
        "dead",
        "source files",
    ] {
        let _ = write!(body, "<th>{}</th>", escape(h));
    }
    body.push_str("</tr></thead>\n<tbody>\n");

    for (i, row) in rows.iter().enumerate() {
        // Compare against the next (older) row, if any.
        let prev = rows.get(i + 1);
        let _ = write!(
            body,
            r#"<tr><td><a href="{v}/index.html"><code>{v}</code></a></td>"#,
            v = escape(&row.version),
        );
        cell(&mut body, row.verdicts, prev.map(|p| p.verdicts));
        cell(&mut body, row.decisions, prev.map(|p| p.decisions));
        cell(
            &mut body,
            row.decisions_full_mcdc,
            prev.map(|p| p.decisions_full_mcdc),
        );
        cell(
            &mut body,
            row.conditions_proved,
            prev.map(|p| p.conditions_proved),
        );
        cell(&mut body, row.conditions_gap, prev.map(|p| p.conditions_gap));
        cell(
            &mut body,
            row.conditions_dead,
            prev.map(|p| p.conditions_dead),
        );
        cell(&mut body, row.source_files, prev.map(|p| p.source_files));
        body.push_str("</tr>\n");
    }
    body.push_str("</tbody>\n</table>\n");

    RenderResult {
        title: "witness — releases".to_string(),
        body,
    }
}

/// Write one numeric cell with an optional Δ-vs-previous annotation.
fn cell(body: &mut String, value: u64, prev: Option<u64>) {
    match prev {
        Some(p) if p != value => {
            let (sign, mag) = if value >= p {
                ('+', value - p)
            } else {
                ('-', p - value)
            };
            let _ = write!(body, r#"<td>{value} <span class="muted">({sign}{mag})</span></td>"#);
        }
        _ => {
            let _ = write!(body, "<td>{value}</td>");
        }
    }
}
