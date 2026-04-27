use std::fmt::Write as _;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

use crate::data::{self, DecisionReport, TruthRow};
use crate::layout::{escape, page};
use crate::state::AppState;

/// GET / — dashboard with totals + verdict scoreboard.
pub async fn index(State(state): State<AppState>) -> Response {
    let verdicts = match data::load_verdicts(state.reports_dir()) {
        Ok(v) => v,
        Err(e) => return error_page("Failed to load verdicts", &e.to_string()),
    };

    if verdicts.is_empty() {
        let body = format!(
            r#"<h1>witness-viz</h1>
<p class="muted">No verdict bundles found in <code>{}</code>.</p>
<div class="empty">Point <code>--reports-dir</code> at a directory containing one or more verdict folders, each with a <code>report.json</code>.</div>"#,
            escape(&state.reports_dir().display().to_string()),
        );
        return Html(page("Overview", &body)).into_response();
    }

    let mut decisions_total: u64 = 0;
    let mut decisions_full: u64 = 0;
    let mut conditions_total: u64 = 0;
    let mut conditions_proved: u64 = 0;
    let mut conditions_gap: u64 = 0;
    let mut conditions_dead: u64 = 0;

    for v in &verdicts {
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
    for v in &verdicts {
        let o = &v.report.overall;
        let branches = data::branch_count(state.reports_dir(), v);
        total_branches = total_branches.saturating_add(u64::from(branches));
        let bar = render_coverage_bar(
            o.conditions_proved,
            o.conditions_gap,
            o.conditions_dead,
        );
        let _ = write!(
            body,
            r#"<tr><td><a href="/verdict/{href}"><code>{name}</code></a></td><td>{branches}</td><td>{dt}</td><td>{df}/{dt}</td><td>{bar}</td><td class="proved">{cp}</td><td class="gap">{cg}</td><td class="dead">{cd}</td></tr>"#,
            href = escape(&v.name),
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

    Html(page("Overview", &body)).into_response()
}

/// GET /verdict/{name} — single-verdict drill-down.
pub async fn verdict(Path(name): Path<String>, State(state): State<AppState>) -> Response {
    let bundle = match data::find_verdict(state.reports_dir(), &name) {
        Ok(Some(b)) => b,
        Ok(None) => return not_found("verdict", &name),
        Err(e) => return error_page("Failed to load verdict", &e.to_string()),
    };

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
            let gap_count = d
                .conditions
                .iter()
                .filter(|c| c.status == "gap")
                .count();
            let _ = write!(
                body,
                r#"<tr><td><a href="/decision/{verdict}/{id}">#{id}</a></td><td><code>{src}:{line}</code></td><td><span class="status status-{status}">{status_disp}</span></td><td>{nc}</td><td>{gap}</td></tr>"#,
                verdict = escape(&bundle.name),
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

    body.push_str(r#"<p class="back-link"><a href="/">← back to overview</a></p>"#);

    let title = format!("Verdict {}", bundle.name);
    Html(page(&title, &body)).into_response()
}

/// GET /decision/{verdict}/{id} — the truth-table widget.
pub async fn decision(
    Path((verdict_name, decision_id)): Path<(String, u32)>,
    State(state): State<AppState>,
) -> Response {
    let bundle = match data::find_verdict(state.reports_dir(), &verdict_name) {
        Ok(Some(b)) => b,
        Ok(None) => return not_found("verdict", &verdict_name),
        Err(e) => return error_page("Failed to load verdict", &e.to_string()),
    };

    let decision = match bundle.report.decisions.iter().find(|d| d.id == decision_id) {
        Some(d) => d,
        None => {
            return not_found("decision", &format!("#{decision_id} in {verdict_name}"));
        }
    };

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
    body.push_str(&render_conditions(decision, &verdict_name));

    let _ = write!(
        body,
        r#"<p class="back-link"><a href="/verdict/{href}">← back to {name}</a></p>"#,
        href = escape(&verdict_name),
        name = escape(&verdict_name),
    );

    let title = format!("Decision #{decision_id}");
    Html(page(&title, &body)).into_response()
}

/// GET /gap/{verdict}/{decision_id}/{condition_index} — the tutorial-
/// style gap drill-down. Renders the missing-witness row, paired row,
/// rationale prose, and a copy-paste Rust test stub. This is the
/// reviewer-facing version of MCP's `find_missing_witness` — humans
/// see exactly what an agent would see.
pub async fn gap(
    Path((verdict_name, decision_id, condition_index)): Path<(String, u32, u32)>,
    State(state): State<AppState>,
) -> Response {
    let bundle = match data::find_verdict(state.reports_dir(), &verdict_name) {
        Ok(Some(b)) => b,
        Ok(None) => return not_found("verdict", &verdict_name),
        Err(e) => return error_page("Failed to load verdict", &e.to_string()),
    };

    let decision = match bundle.report.decisions.iter().find(|d| d.id == decision_id) {
        Some(d) => d,
        None => return not_found("decision", &format!("#{decision_id} in {verdict_name}")),
    };

    let condition = match decision.conditions.iter().find(|c| c.index == condition_index) {
        Some(c) => c,
        None => {
            return not_found(
                "condition",
                &format!("c{condition_index} in decision #{decision_id}"),
            );
        }
    };

    let mut body = String::new();
    let _ = write!(
        body,
        "<h1>Gap analysis — c{ci} in decision #{did}</h1>\n<p class=\"muted\"><code>{verd}</code> &middot; <code>{src}:{line}</code> &middot; branch <code>{br}</code></p>\n",
        ci = condition_index,
        did = decision_id,
        verd = escape(&verdict_name),
        src = escape(&decision.source_file),
        line = decision.source_line,
        br = condition.branch_id,
    );

    let _ = write!(
        body,
        "<p>Status: <span class=\"status status-{cls}\">{up}</span></p>\n",
        cls = escape(&condition.status),
        up = escape(&condition.status.to_ascii_uppercase()),
    );

    match condition.status.as_str() {
        "proved" => {
            body.push_str("<div class=\"box\">\n");
            if let Some(pair) = condition.pair {
                let interp = condition.interpretation.as_deref().unwrap_or("");
                let _ = write!(
                    body,
                    "<p>Already proved by rows <code>{a}</code> and <code>{b}</code> ({interp}). No action needed.</p>\n",
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
            // gap (or unknown — treat as gap-like)
            render_gap_tutorial(&mut body, &verdict_name, decision, condition);
        }
    }

    let _ = write!(
        body,
        r#"<p class="back-link"><a href="/decision/{verdict}/{did}">← back to decision #{did}</a></p>"#,
        verdict = escape(&verdict_name),
        did = decision_id,
    );

    let title = format!("Gap c{condition_index} in decision #{decision_id}");
    Html(page(&title, &body)).into_response()
}

fn render_gap_tutorial(
    body: &mut String,
    verdict_name: &str,
    decision: &DecisionReport,
    condition: &crate::data::ConditionReport,
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
            let _ = write!(
                body,
                "<p>To prove condition <code>c{ci}</code> independently affects the decision, you need a row where <code>c{ci} = {needed}</code> and the outcome differs from row <code>{rid}</code> (where <code>c{ci} = {current}</code>).</p>\n",
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
                let _ = write!(body, "  c{idx} = {val}\n", idx = c.index, val = val);
            }
            body.push_str("</pre>\n");

            body.push_str("<p class=\"muted\">Conditions marked <code>*</code> were short-circuited in the baseline row; the new test must drive inputs so they get evaluated to either T or F.</p>\n");
        }
        None => {
            body.push_str("<p>No baseline row exists yet — every existing test row short-circuited before reaching this condition. To drive coverage to this condition, expand the test corpus so prior conditions evaluate in the direction that allows control to flow here.</p>\n");
        }
    }

    body.push_str("</div>\n");

    // Copy-paste test stub. We don't have the function signature handy
    // (would require linking the manifest's function_name back to a
    // Rust source declaration), so the stub is structural — the agent
    // or reviewer fills in the call. Worth more than nothing.
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

// ─── helpers ──────────────────────────────────────────────────────────

/// Render a stacked horizontal bar showing the proved / gap / dead
/// split of conditions for a verdict (or total). Pure inline-styled
/// HTML — no extra CSS dependency, embeds in any table cell.
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

fn render_conditions(decision: &DecisionReport, verdict_name: &str) -> String {
    let mut out = String::from("<ul class=\"conditions\">\n");
    for c in &decision.conditions {
        let _ = write!(out, "<li class=\"cond-{status}\">", status = escape(&c.status));
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
                r#" <a class="gap-link" href="/gap/{verdict}/{did}/{ci}">view gap →</a>"#,
                verdict = escape(verdict_name),
                did = decision.id,
                ci = c.index,
            );
        }
        out.push_str("</li>\n");
    }
    out.push_str("</ul>\n");
    out
}

fn not_found(kind: &str, name: &str) -> Response {
    let body = format!(
        r#"<h1>Not found</h1>
<p>No {kind} matches <code>{name}</code>.</p>
<p class="back-link"><a href="/">← back to overview</a></p>"#,
        kind = escape(kind),
        name = escape(name),
    );
    let html = page("Not found", &body);
    (StatusCode::NOT_FOUND, Html(html)).into_response()
}

fn error_page(title: &str, msg: &str) -> Response {
    let body = format!(
        "<h1>{}</h1>\n<pre>{}</pre>",
        escape(title),
        escape(msg),
    );
    let html = page(title, &body);
    (StatusCode::INTERNAL_SERVER_ERROR, Html(html)).into_response()
}
