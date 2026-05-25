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

use crate::data::{self, ConditionReport, DecisionReport, TruthRow, VerdictBundle};
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

    /// Href to the overview page from the current page's depth.
    ///
    /// In serve mode (`link_ext = ""`) this is just `href_prefix`
    /// (`"/"`). In export mode (`link_ext = ".html"`) the overview
    /// is `index.html`, so we synthesise `<prefix>index.html`.
    pub fn link_to_overview(&self) -> String {
        if self.link_ext.is_empty() {
            self.href_prefix.to_string()
        } else {
            format!("{}index{}", self.href_prefix, self.link_ext)
        }
    }

    pub fn link_to_verdict(&self, name: &str) -> String {
        format!("{}verdict/{}{}", self.href_prefix, name, self.link_ext)
    }

    pub fn link_to_decision(&self, verdict: &str, decision_id: u32) -> String {
        format!(
            "{}decision/{}/{}{}",
            self.href_prefix, verdict, decision_id, self.link_ext
        )
    }

    pub fn link_to_gap(&self, verdict: &str, decision_id: u32, condition_index: u32) -> String {
        format!(
            "{}gap/{}/{}/{}{}",
            self.href_prefix, verdict, decision_id, condition_index, self.link_ext
        )
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
            r#"<tr><td><a href="{href}"><code>{name}</code></a></td><td>{branches}</td><td>{dt}</td><td>{df}/{dt}</td><td>{bar}</td><td class="proved">{cp}</td><td class="gap">{cg}</td><td class="dead">{cd}</td></tr>"#,
            href = escape(&ctx.link_to_verdict(&v.name)),
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

/// Render the verdict drill-down (single-verdict scoreboard plus
/// per-decision table linking into the decision page). Returns
/// `body` HTML; the caller wraps it in [`crate::layout::page`].
pub fn render_verdict(ctx: &RenderContext<'_>, bundle: &VerdictBundle) -> RenderResult {
    let o = &bundle.report.overall;
    let mut body = String::new();
    let _ = write!(
        body,
        "<h1>Verdict <code>{}</code></h1>\n<p class=\"muted\">module: {} — schema: {}</p>\n",
        escape(&bundle.name),
        escape(&bundle.report.module),
        escape(&bundle.report.schema),
    );

    body.push_str(&render_cards(&[
        ("Decisions", u64::from(o.decisions_total)),
        ("Full MC/DC", u64::from(o.decisions_full_mcdc)),
        ("Conditions", u64::from(o.conditions_total)),
        ("Proved", u64::from(o.conditions_proved)),
        ("Gap", u64::from(o.conditions_gap)),
        ("Dead", u64::from(o.conditions_dead)),
    ]));

    body.push_str("<h2>Decisions</h2>\n");
    if bundle.report.decisions.is_empty() {
        body.push_str(r#"<div class="empty">No decisions recorded.</div>"#);
    } else {
        body.push_str("<table>\n<thead><tr>");
        for h in ["#", "source", "status", "conditions", "gap"] {
            let _ = write!(body, "<th>{}</th>", escape(h));
        }
        body.push_str("</tr></thead>\n<tbody>\n");

        for d in &bundle.report.decisions {
            let gap_count = d.conditions.iter().filter(|c| c.status == "gap").count();
            let _ = write!(
                body,
                r#"<tr><td><a href="{href}">#{id}</a></td><td><code>{src}:{line}</code></td><td><span class="status status-{status}">{status_disp}</span></td><td>{nc}</td><td>{gap}</td></tr>"#,
                href = escape(&ctx.link_to_decision(&bundle.name, d.id)),
                id = d.id,
                src = escape(&d.source_file),
                line = d.source_line,
                status = escape(&d.status),
                status_disp = escape(&d.status),
                nc = d.conditions.len(),
                gap = gap_count,
            );
            body.push('\n');
        }
        body.push_str("</tbody></table>\n");
    }

    let _ = write!(
        body,
        r#"<p class="back-link"><a href="{href}">← back to overview</a></p>"#,
        href = escape(&ctx.link_to_overview()),
    );

    RenderResult {
        title: format!("Verdict {}", bundle.name),
        body,
    }
}

/// Render the decision-detail page: truth table + per-condition
/// pair list with gap-links. Returns `body` HTML.
pub fn render_decision(
    ctx: &RenderContext<'_>,
    bundle: &VerdictBundle,
    decision: &DecisionReport,
) -> RenderResult {
    let mut body = String::new();
    let _ = write!(
        body,
        "<h1>Decision #{id} — <code>{src}:{line}</code></h1>\n<p>Status: <span class=\"status status-{status}\">{status_disp}</span></p>\n",
        id = decision.id,
        src = escape(&decision.source_file),
        line = decision.source_line,
        status = escape(&decision.status),
        status_disp = escape(&decision.status),
    );

    body.push_str("<h2>Truth table</h2>\n");
    body.push_str(&render_truth_table(decision));

    body.push_str("<h2>Independent-effect pairs</h2>\n");
    body.push_str(&render_conditions(ctx, decision, &bundle.name));

    let _ = write!(
        body,
        r#"<p class="back-link"><a href="{href}">← back to {name}</a></p>"#,
        href = escape(&ctx.link_to_verdict(&bundle.name)),
        name = escape(&bundle.name),
    );

    RenderResult {
        title: format!("Decision #{}", decision.id),
        body,
    }
}

/// Render the gap-drill-down (tutorial-style page explaining what
/// row would close the gap). The same shape MCP exposes via
/// `find_missing_witness`. Returns `body` HTML.
pub fn render_gap(
    ctx: &RenderContext<'_>,
    bundle: &VerdictBundle,
    decision: &DecisionReport,
    condition: &ConditionReport,
) -> RenderResult {
    let mut body = String::new();
    let _ = write!(
        body,
        "<h1>Gap analysis — c{ci} in decision #{did}</h1>\n<p class=\"muted\"><code>{verd}</code> &middot; <code>{src}:{line}</code> &middot; branch <code>{br}</code></p>\n",
        ci = condition.index,
        did = decision.id,
        verd = escape(&bundle.name),
        src = escape(&decision.source_file),
        line = decision.source_line,
        br = condition.branch_id,
    );

    let _ = writeln!(
        body,
        "<p>Status: <span class=\"status status-{cls}\">{up}</span></p>",
        cls = escape(&condition.status),
        up = escape(&condition.status.to_ascii_uppercase()),
    );

    match condition.status.as_str() {
        "proved" => {
            body.push_str("<div class=\"box\">\n");
            if let Some(pair) = condition.pair {
                let interp = condition.interpretation.as_deref().unwrap_or("");
                let _ = writeln!(
                    body,
                    "<p>Already proved by rows <code>{a}</code> and <code>{b}</code> ({interp}). No action needed.</p>",
                    a = pair.first().copied().unwrap_or(0),
                    b = pair.get(1).copied().unwrap_or(0),
                    interp = escape(interp),
                );
            } else {
                body.push_str("<p>Already proved. No action needed.</p>\n");
            }
            body.push_str("</div>\n");
        }
        "dead" => {
            body.push_str("<div class=\"box\">\n");
            body.push_str("<p>Condition is <strong>dead</strong>: the runtime never reached this branch under any test row. The compiler may have folded the predicate, or the call-path is unreachable from the harness.</p>\n");
            body.push_str("<p>Action: confirm by reading the source. If the branch is genuinely unreachable, the dead status is the verdict. If reachable, expand the test corpus to drive code through this branch.</p>\n");
            body.push_str("</div>\n");
        }
        _ => {
            render_gap_tutorial(&mut body, &bundle.name, decision, condition);
        }
    }

    let _ = write!(
        body,
        r#"<p class="back-link"><a href="{href}">← back to decision #{did}</a></p>"#,
        href = escape(&ctx.link_to_decision(&bundle.name, decision.id)),
        did = decision.id,
    );

    RenderResult {
        title: format!("Gap c{} in decision #{}", condition.index, decision.id),
        body,
    }
}

// ─── private helpers ──────────────────────────────────────────────────

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

fn render_truth_table(decision: &DecisionReport) -> String {
    let mut out = String::from("<table class=\"truth-table\">\n<thead><tr><th>row</th>");
    for c in &decision.conditions {
        let _ = write!(
            out,
            "<th>c{idx} <span class=\"br\">br {br}</span></th>",
            idx = c.index,
            br = c.branch_id,
        );
    }
    out.push_str("<th>outcome</th></tr></thead>\n<tbody>\n");

    let gap_indices: std::collections::BTreeSet<u32> = decision
        .conditions
        .iter()
        .filter(|c| c.status == "gap")
        .map(|c| c.index)
        .collect();

    for row in &decision.truth_table {
        let row_class = row_class_for(row, &gap_indices);
        out.push_str("<tr");
        if !row_class.is_empty() {
            let _ = write!(out, r#" class="{row_class}""#);
        }
        out.push('>');
        let _ = write!(out, "<td>{}</td>", row.row_id);
        for c in &decision.conditions {
            let key = c.index.to_string();
            match row.evaluated.get(&key) {
                Some(true) => out.push_str("<td class=\"t\">T</td>"),
                Some(false) => out.push_str("<td class=\"f\">F</td>"),
                None => out.push_str("<td class=\"dontcare\">*</td>"),
            }
        }
        let outcome = if row.outcome {
            "<td class=\"t\">T</td>"
        } else {
            "<td class=\"f\">F</td>"
        };
        out.push_str(outcome);
        out.push_str("</tr>\n");
    }
    out.push_str("</tbody></table>\n");
    out
}

fn row_class_for(row: &TruthRow, gap_indices: &std::collections::BTreeSet<u32>) -> &'static str {
    for idx in gap_indices {
        let key = idx.to_string();
        if !row.evaluated.contains_key(&key) {
            return "row-gap";
        }
    }
    ""
}

fn render_conditions(
    ctx: &RenderContext<'_>,
    decision: &DecisionReport,
    verdict_name: &str,
) -> String {
    let mut out = String::from("<ul class=\"conditions\">\n");
    for c in &decision.conditions {
        let _ = write!(
            out,
            "<li class=\"cond-{status}\">",
            status = escape(&c.status)
        );
        let status_upper = c.status.to_ascii_uppercase();
        let _ = write!(
            out,
            "<code>c{idx}</code> (branch <code>{br}</code>): <strong class=\"{cls}\">{up}</strong>",
            idx = c.index,
            br = c.branch_id,
            cls = escape(&c.status),
            up = escape(&status_upper),
        );
        if let Some(pair) = c.pair {
            let interp = c.interpretation.as_deref().unwrap_or("");
            let _ = write!(
                out,
                " — pair rows <code>{a}</code>, <code>{b}</code> <span class=\"muted\">({interp})</span>",
                a = pair.first().copied().unwrap_or(0),
                b = pair.get(1).copied().unwrap_or(0),
                interp = escape(interp),
            );
        }
        if c.status != "proved" {
            let _ = write!(
                out,
                r#" <a class="gap-link" href="{href}">view gap →</a>"#,
                href = escape(&ctx.link_to_gap(verdict_name, decision.id, c.index)),
            );
        }
        out.push_str("</li>\n");
    }
    out.push_str("</ul>\n");
    out
}

fn render_gap_tutorial(
    body: &mut String,
    verdict_name: &str,
    decision: &DecisionReport,
    condition: &ConditionReport,
) {
    // Find any existing row that has this condition evaluated — that
    // gives us a starting point. The "needed row" is the same row with
    // this condition's value flipped. The other conditions stay as the
    // existing row had them; if a condition was unevaluated (short-
    // circuited away) the agent / reviewer may need to massage inputs
    // so it gets evaluated.
    let key = condition.index.to_string();
    let baseline = decision
        .truth_table
        .iter()
        .find(|r| r.evaluated.contains_key(&key));

    body.push_str("<h2>What you need</h2>\n");
    body.push_str("<div class=\"box\">\n");

    match baseline {
        Some(row) => {
            let current = row.evaluated.get(&key).copied().unwrap_or(false);
            let needed = !current;
            let _ = writeln!(
                body,
                "<p>To prove condition <code>c{ci}</code> independently affects the decision, you need a row where <code>c{ci} = {needed}</code> and the outcome differs from row <code>{rid}</code> (where <code>c{ci} = {current}</code>).</p>",
                ci = condition.index,
                needed = if needed { "T" } else { "F" },
                current = if current { "T" } else { "F" },
                rid = row.row_id,
            );

            body.push_str("<p>Required condition vector:</p>\n<pre>");
            for c in &decision.conditions {
                let k = c.index.to_string();
                let val = if c.index == condition.index {
                    if needed { "T" } else { "F" }
                } else {
                    match row.evaluated.get(&k) {
                        Some(true) => "T",
                        Some(false) => "F",
                        None => "*",
                    }
                };
                let _ = writeln!(body, "  c{idx} = {val}", idx = c.index, val = val);
            }
            body.push_str("</pre>\n");

            body.push_str("<p class=\"muted\">Conditions marked <code>*</code> were short-circuited in the baseline row; the new test must drive inputs so they get evaluated to either T or F.</p>\n");
        }
        None => {
            body.push_str("<p>No baseline row exists yet — every existing test row short-circuited before reaching this condition. To drive coverage to this condition, expand the test corpus so prior conditions evaluate in the direction that allows control to flow here.</p>\n");
        }
    }

    body.push_str("</div>\n");

    body.push_str("<h2>Suggested test stub</h2>\n");
    body.push_str("<pre class=\"stub\">");
    let _ = write!(
        body,
        "#[test]\nfn closes_gap_d{did}_c{ci}() {{\n    // Verdict: {verd}\n    // Source: {src}:{line}\n    // Branch:  {br}\n    //\n    // TODO: drive the function so condition c{ci} evaluates to the\n    // value above and the resulting decision outcome differs from\n    // the existing pair row.\n    todo!(\"witness viz: gap drill-down for d#{did}/c{ci}\");\n}}",
        did = decision.id,
        ci = condition.index,
        verd = escape(verdict_name),
        src = escape(&decision.source_file),
        line = decision.source_line,
        br = condition.branch_id,
    );
    body.push_str("</pre>\n");

    body.push_str("<p class=\"muted\">After adding the test, re-run: <code>witness instrument && witness run --invoke run_row_NEW && witness report</code>. The condition should flip from <code>gap</code> to <code>proved</code>.</p>\n");
}
