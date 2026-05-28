//! Static HTML export driver.
//!
//! Walks every page of the dashboard, calling the same
//! `crate::render::render_*` functions the axum handlers use, and
//! writes the resulting HTML to disk under `out_dir`. The output is
//! intended to be served from a static host (GitHub Pages, an S3
//! bucket, `python -m http.server`, opened directly via `file://`)
//! — no axum, no HTMX, no network.
//!
//! Output tree:
//!
//! ```text
//! out_dir/
//!   index.html
//!   verdict/<name>.html              (one per verdict)
//!   decision/<verdict>/<id>.html      (one per decision)
//!   gap/<verdict>/<id>/<cond>.html    (one per condition)
//!   _assets/styles.css
//!   summary.json                      (manifest: pages, verdicts, version)
//! ```
//!
//! Page-depth determines the `href_prefix`:
//! - index (depth 0): `""`
//! - verdict (depth 1): `"../"`
//! - decision (depth 2): `"../../"`
//! - gap (depth 3): `"../../../"`
//!
//! That is the *only* thing the renderer needs to know to produce
//! correct relative links — the body strings the renderer returns
//! are otherwise byte-identical with serve mode.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::data::{self, VerdictBundle};
use crate::layout::{PageOpts, page_with};
use crate::render::{self, RenderContext, RenderResult};
use crate::styles;

/// Options for [`run_export`].
pub struct ExportOpts {
    pub reports_dir: PathBuf,
    pub out_dir: PathBuf,
    /// Optional title-bar / page-title prefix shown above the brand.
    /// `None` → no prefix; pass e.g. `Some("witness · v0.23.0".into())`
    /// to brand the published site.
    pub site_title: Option<String>,
    /// Optional repository root for source-file lookup. When set,
    /// Decision and Gap pages render a ±5-line inline snippet around
    /// the recorded `source_file:source_line`. v0.24+. Missing
    /// files degrade gracefully (snippet suppressed).
    pub source_root: Option<PathBuf>,
}

/// Result of a successful export — useful for logging and CI assertions.
#[derive(Debug, Clone, Default)]
pub struct ExportSummary {
    pub pages_written: usize,
    pub bytes_written: u64,
    pub verdicts: usize,
    pub decisions: usize,
    pub conditions: usize,
    /// v0.24 — number of unique source files for which a full-file
    /// page was emitted. Zero when `--source-root` is unset or no
    /// referenced file is readable.
    pub source_files: usize,
}

/// Walk the verdict tree and write static HTML.
///
/// Returns `ExportSummary` for the caller to log / assert. Creates
/// `out_dir` if missing; overwrites existing files.
pub fn run_export(opts: &ExportOpts) -> io::Result<ExportSummary> {
    let verdicts = data::load_verdicts(&opts.reports_dir)
        .map_err(|e| io::Error::other(format!("load verdicts: {e}")))?;

    fs::create_dir_all(&opts.out_dir)?;
    let assets = opts.out_dir.join("_assets");
    fs::create_dir_all(&assets)?;
    fs::write(assets.join("styles.css"), styles::CSS)?;

    let mut summary = ExportSummary {
        verdicts: verdicts.len(),
        ..Default::default()
    };

    // depth 0 — index.html
    {
        let ctx = ctx_for_depth(&verdicts, &opts.reports_dir, opts.source_root.as_deref(), 0);
        let rendered = render::render_overview(&ctx);
        let html = wrap_static(&rendered, 0, opts.site_title.as_deref());
        let path = opts.out_dir.join("index.html");
        summary.bytes_written += write_page(&path, &html)?;
        summary.pages_written += 1;
    }

    for v in &verdicts {
        // depth 1 — verdict/<name>.html
        let verdict_dir = opts.out_dir.join("verdict");
        fs::create_dir_all(&verdict_dir)?;
        let ctx = ctx_for_depth(&[], &opts.reports_dir, opts.source_root.as_deref(), 1);
        let rendered = render::render_verdict(&ctx, v);
        let html = wrap_static(&rendered, 1, opts.site_title.as_deref());
        let path = verdict_dir.join(format!("{}.html", v.name));
        summary.bytes_written += write_page(&path, &html)?;
        summary.pages_written += 1;

        for d in &v.report.decisions {
            // depth 2 — decision/<verdict>/<id>.html
            let dec_dir = opts.out_dir.join("decision").join(&v.name);
            fs::create_dir_all(&dec_dir)?;
            let ctx = ctx_for_depth(&[], &opts.reports_dir, opts.source_root.as_deref(), 2);
            let rendered = render::render_decision(&ctx, v, d);
            let html = wrap_static(&rendered, 2, opts.site_title.as_deref());
            let path = dec_dir.join(format!("{}.html", d.id));
            summary.bytes_written += write_page(&path, &html)?;
            summary.pages_written += 1;
            summary.decisions += 1;

            for c in &d.conditions {
                summary.conditions += 1;
                // v0.24 — skip gap pages for `proved` conditions.
                // Their drill-down would render "already proved, no
                // action needed" — accurate, but not actionable, and
                // they dominate the bundle (~80% of conditions
                // typically). Dead conditions KEEP their drill-down
                // because it explains the "compiler folded the
                // branch / harness can't reach it" investigation
                // that the reviewer still has to do. The link in
                // `render_conditions` is gated identically — see
                // render.rs.
                if c.status == "proved" {
                    continue;
                }
                // depth 3 — gap/<verdict>/<id>/<cond>.html
                let gap_dir = opts
                    .out_dir
                    .join("gap")
                    .join(&v.name)
                    .join(d.id.to_string());
                fs::create_dir_all(&gap_dir)?;
                let ctx = ctx_for_depth(&[], &opts.reports_dir, opts.source_root.as_deref(), 3);
                let rendered = render::render_gap(&ctx, v, d, c);
                let html = wrap_static(&rendered, 3, opts.site_title.as_deref());
                let path = gap_dir.join(format!("{}.html", c.index));
                summary.bytes_written += write_page(&path, &html)?;
                summary.pages_written += 1;
            }
        }
    }

    // v0.24 — full-file source pages (DEC-031). When --source-root is
    // set, walk every Decision across every Verdict, group by
    // (verdict, source_file) → set of source_lines, resolve each via
    // the verdict-scoped resolver (reports carry basename-only
    // source_file from DWARF DW_AT_name), and render
    // syntax-highlighted to `out/source/<verdict>/<source_file>.html`.
    // Verdict-scoping the output path avoids basename collisions (two
    // verdicts each with a `lib.rs`). Unresolvable entries — typically
    // dependency files under ~/.cargo, not vendored under the verdict
    // — are skipped; the snippet on the Decision page degrades
    // identically (render_source_snippet_for shares the resolver).
    if let Some(source_root) = opts.source_root.as_deref() {
        let mut by_file: std::collections::BTreeMap<
            (String, String),
            std::collections::BTreeSet<u32>,
        > = std::collections::BTreeMap::new();
        for v in &verdicts {
            for d in &v.report.decisions {
                if d.source_file.is_empty() || d.source_line == 0 {
                    continue;
                }
                by_file
                    .entry((v.name.clone(), d.source_file.clone()))
                    .or_default()
                    .insert(d.source_line);
            }
        }
        for ((verdict, rel), lines) in &by_file {
            // Sanitise: refuse absolute paths and any `..` traversal in
            // either the verdict name or the source path.
            if rel.starts_with('/')
                || rel.split('/').any(|p| p == "..")
                || verdict.contains('/')
                || verdict == ".."
            {
                tracing::warn!("skipping unsafe source path: {verdict}/{rel}");
                continue;
            }
            let Some(abs) = render::resolve_source_path(source_root, verdict, rel) else {
                continue;
            };
            let Ok(text) = fs::read_to_string(&abs) else {
                continue;
            };
            let marked: Vec<u32> = lines.iter().copied().collect();
            let rendered = render::render_source_page(&text, rel, &marked);
            // Depth-aware: `source/<verdict>/<rel>` — 1 (source) + 1
            // (verdict) + path components of rel.
            let depth = 2 + rel.split('/').count();
            let html = wrap_static(&rendered, depth, opts.site_title.as_deref());
            let out_path = opts
                .out_dir
                .join("source")
                .join(verdict)
                .join(format!("{rel}.html"));
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            summary.bytes_written += write_page(&out_path, &html)?;
            summary.pages_written += 1;
            summary.source_files += 1;
        }
    }

    // Manifest — tiny JSON so CI can assert non-empty / parse / etc.
    let manifest = format!(
        r#"{{
  "tool": "witness-viz",
  "tool_version": "{ver}",
  "pages_written": {pages},
  "bytes_written": {bytes},
  "verdicts": {v},
  "decisions": {d},
  "conditions": {c},
  "source_files": {sf}
}}
"#,
        ver = env!("CARGO_PKG_VERSION"),
        pages = summary.pages_written,
        bytes = summary.bytes_written,
        v = summary.verdicts,
        d = summary.decisions,
        c = summary.conditions,
        sf = summary.source_files,
    );
    fs::write(opts.out_dir.join("summary.json"), manifest)?;

    Ok(summary)
}

fn ctx_for_depth<'a>(
    verdicts: &'a [VerdictBundle],
    reports_dir: &'a Path,
    source_root: Option<&'a Path>,
    depth: usize,
) -> RenderContext<'a> {
    // SAFETY: depth ≤ 3 in our tree → leak is bounded. We use a static
    // table to avoid String allocation in the renderer hot path.
    let href_prefix = match depth {
        0 => "",
        1 => "../",
        2 => "../../",
        3 => "../../../",
        _ => "../../../../", // future-proofing; not reachable today
    };
    RenderContext {
        verdicts,
        reports_dir,
        href_prefix,
        link_ext: ".html",
        source_root,
    }
}

fn wrap_static(r: &RenderResult, depth: usize, _site_title: Option<&str>) -> String {
    let prefix_owned = "../".repeat(depth);
    let asset_prefix = format!("{prefix_owned}_assets/");
    let overview_href = if depth == 0 {
        "index.html".to_string()
    } else {
        format!("{prefix_owned}index.html")
    };
    let opts = PageOpts {
        asset_prefix: &asset_prefix,
        overview_href: &overview_href,
        include_htmx: false,
        include_api_link: false,
    };
    page_with(&r.title, &r.body, &opts)
}

fn write_page(path: &Path, html: &str) -> io::Result<u64> {
    fs::write(path, html.as_bytes())?;
    Ok(u64::try_from(html.len()).unwrap_or(u64::MAX))
}
