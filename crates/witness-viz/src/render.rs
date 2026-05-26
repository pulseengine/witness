//! Pure-render core for witness-viz pages.
//!
//! This module is the **design seam** rivet uses successfully: both
//! the live HTMX/Axum server (`serve` mode) and the planned static
//! HTML export (`export` mode, see `docs/research/...` and the
//! witness-viz issue tracker) call the same `render_*` functions
//! here. The renderer is sync, takes a borrow-only [`RenderContext`],
//! and returns plain HTML strings — no axum types, no I/O, no HTMX.
//!
//! When static export lands, the only differences between serve and
//! export are wrapped in [`RenderContext::href_prefix`] +
//! [`RenderContext::link_ext`]:
//!
//! - Serve mode: `href_prefix = ""`, `link_ext = ""`. Links resolve
//!   as `/verdict/foo`, axum's router handles them.
//! - Export mode: `href_prefix = "../"` (depth-counted), `link_ext
//!   = ".html"`. Links resolve as `../verdict/foo.html`, browseable
//!   from `file://` and any `gh-pages` subpath alike.
//!
//! See `docs/research/source-map-ingestion.md`-style follow-up note
//! once the export subcommand exists.
//!
//! Status: Step 1A — only [`render_overview`] is migrated here.
//! The `verdict` / `decision` / `gap` pages still live in
//! `views.rs` with axum-typed handlers; their migration is the next
//! step. The seam in this commit proves the pattern works without
//! disturbing the rest of the dashboard.

use std::fmt::Write;
use std::path::Path;

use crate::data::{self, VerdictBundle};
use crate::layout::escape;

/// Borrow-only context the renderer needs. Add fields here as more
/// pages migrate; current single user is [`render_overview`].
pub struct RenderContext<'a> {
    pub verdicts: &'a [VerdictBundle],
    pub reports_dir: &'a Path,
    /// Prefix prepended to every internal `href`. Empty in serve
    /// mode; `"../"`-repeated in export mode based on the depth of
    /// the page being rendered.
    pub href_prefix: &'a str,
    /// Extension appended to internal hrefs. Empty in serve mode
    /// (axum routes `/verdict/foo`); `".html"` in export mode (file
    /// `verdict/foo.html` on disk).
    pub link_ext: &'a str,
}

impl<'a> RenderContext<'a> {
    /// Convenience constructor for serve mode: `href_prefix = "/"`
    /// so internal links are root-relative (`/verdict/foo`) and
    /// `link_ext = ""` so axum's routes resolve them directly.
    pub fn for_serve(verdicts: &'a [VerdictBundle], reports_dir: &'a Path) -> Self {
        Self {
            verdicts,
            reports_dir,
            href_prefix: "/",
            link_ext: "",
        }
    }
}

/// A rendered page — body HTML plus the `<title>` text. Callers
/// (axum handlers, the export driver) wrap this in their own layout.
#[derive(Debug, Clone)]
pub struct RenderResult {
    pub title: String,
    pub body: String,
}

/// Render the overview page (Codecov-style dashboard landing): a
/// stats-cards row + a per-verdict table with stacked coverage bars.
///
/// Returns `body` HTML; the caller wraps it in [`crate::layout::page`].
pub fn render_overview(ctx: &RenderContext<'_>) -> RenderResult {
    if ctx.verdicts.is_empty() {
        let body = format!(
            r#"<h1>witness-viz</h1>
<p class="muted">No verdict bundles found in <code>{}</code>.</p>
<div class="empty">Point <code>--reports-dir</code> at a directory containing one or more verdict folders, each with a <code>report.json</code>.</div>"#,
            escape(&ctx.reports_dir.display().to_string()),
        );
        return RenderResult {
            title: "Overview".to_string(),
            body,
        };
    }

    let mut decisions_total: u64 = 0;
    let mut decisions_full: u64 = 0;
    let mut conditions_total: u64 = 0;
    let mut conditions_proved: u64 = 0;
    let mut conditions_gap: u64 = 0;
    let mut conditions_dead: u64 = 0;

    for v in ctx.verdicts {
        let o = &v.report.overall;
        decisions_total = decisions_total.saturating_add(u64::from(o.decisions_total));
        decisions_full = decisions_full.saturating_add(u64::from(o.decisions_full_mcdc));
        conditions_total = conditions_total.saturating_add(u64::from(o.conditions_total));
        conditions_proved = conditions_proved.saturating_add(u64::from(o.conditions_proved));
        conditions_gap = conditions_gap.saturating_add(u64::from(o.conditions_gap));
        conditions_dead = conditions_dead.saturating_add(u64::from(o.conditions_dead));
    }

    let mut body = String::new();
    body.push_str("<h1>Compliance overview</h1>\n");

    body.push_str(&render_cards(&[
        ("Decisions", decisions_total),
        ("Full MC/DC", decisions_full),
        ("Conditions proved", conditions_proved),
        ("Gap", conditions_gap),
        ("Dead", conditions_dead),
    ]));

    body.push_str("<h2>Verdicts</h2>\n");
    body.push_str("<table>\n<thead><tr>");
    for h in [
        "verdict",
        "branches",
        "decisions",
        "full MC/DC",
        "coverage",
        "proved",
        "gap",
        "dead",
    ] {
        let _ = write!(body, "<th>{}</th>", escape(h));
    }
    body.push_str("</tr></thead>\n<tbody>\n");

    let mut total_branches: u64 = 0;
    for v in ctx.verdicts {
        let o = &v.report.overall;
        let branches = data::branch_count(ctx.reports_dir, v);
        total_branches = total_branches.saturating_add(u64::from(branches));
        let bar = render_coverage_bar(o.conditions_proved, o.conditions_gap, o.conditions_dead);
        let _ = write!(
            body,
            r#"<tr><td><a href="{prefix}verdict/{href}{ext}"><code>{name}</code></a></td><td>{branches}</td><td>{dt}</td><td>{df}/{dt}</td><td>{bar}</td><td class="proved">{cp}</td><td class="gap">{cg}</td><td class="dead">{cd}</td></tr>"#,
            prefix = ctx.href_prefix,
            href = escape(&v.name),
            ext = ctx.link_ext,
            name = escape(&v.name),
            branches = branches,
            dt = o.decisions_total,
            df = o.decisions_full_mcdc,
            bar = bar,
            cp = o.conditions_proved,
            cg = o.conditions_gap,
            cd = o.conditions_dead,
        );
        body.push('\n');
    }

    let total_bar = render_coverage_bar(
        u32_or_max(conditions_proved),
        u32_or_max(conditions_gap),
        u32_or_max(conditions_dead),
    );
    let _ = write!(
        body,
        r#"<tr class="total-row"><td>TOTAL</td><td>{tb}</td><td>{dt}</td><td>{df}/{dt}</td><td>{bar}</td><td>{cp}</td><td>{cg}</td><td>{cd}</td></tr>"#,
        tb = total_branches,
        dt = decisions_total,
        df = decisions_full,
        bar = total_bar,
        cp = conditions_proved,
        cg = conditions_gap,
        cd = conditions_dead,
    );
    body.push_str("\n</tbody>\n</table>\n");

    RenderResult {
        title: "Overview".to_string(),
        body,
    }
}

// ─── private helpers (also used by views.rs for the un-migrated pages;
// duplication is intentional during the staged migration and will be
// removed when the other pages move here) ────────────────────────────

fn render_coverage_bar(proved: u32, gap: u32, dead: u32) -> String {
    let total = proved.saturating_add(gap).saturating_add(dead);
    if total == 0 {
        return String::from(r#"<div class="cov-bar empty"></div>"#);
    }
    let p_pct = (u64::from(proved).saturating_mul(100)) / u64::from(total);
    let g_pct = (u64::from(gap).saturating_mul(100)) / u64::from(total);
    let d_pct = 100u64.saturating_sub(p_pct).saturating_sub(g_pct);
    format!(
        r#"<div class="cov-bar" title="proved {proved} / gap {gap} / dead {dead}">
<span class="seg-proved" style="width:{pp}%"></span><span class="seg-gap" style="width:{gp}%"></span><span class="seg-dead" style="width:{dp}%"></span>
</div>"#,
        proved = proved,
        gap = gap,
        dead = dead,
        pp = p_pct,
        gp = g_pct,
        dp = d_pct,
    )
}

fn u32_or_max(v: u64) -> u32 {
    if v > u64::from(u32::MAX) {
        u32::MAX
    } else {
        // SAFETY: bounds-checked above.
        #[allow(clippy::cast_possible_truncation)]
        let r = v as u32;
        r
    }
}

fn render_cards(items: &[(&str, u64)]) -> String {
    let mut out = String::from("<div class=\"cards\">\n");
    for (label, n) in items {
        let _ = write!(
            out,
            r#"<div class="card"><div class="num">{n}</div><div class="label">{label}</div></div>"#,
            n = n,
            label = escape(label),
        );
        out.push('\n');
    }
    out.push_str("</div>\n");
    out
}
