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
        let _ = write!(
            body,
            r#"<tr><td><a href="/verdict/{href}"><code>{name}</code></a></td><td>{branches}</td><td>{dt}</td><td>{df}</td><td class="proved">{cp}</td><td class="gap">{cg}</td><td class="dead">{cd}</td></tr>"#,
            href = escape(&v.name),
            name = escape(&v.name),
            branches = branches,
            dt = o.decisions_total,
            df = o.decisions_full_mcdc,
            cp = o.conditions_proved,
            cg = o.conditions_gap,
            cd = o.conditions_dead,
        );
        body.push('\n');
    }

    let _ = write!(
        body,
        r#"<tr class="total-row"><td>TOTAL</td><td>{tb}</td><td>{dt}</td><td>{df}</td><td>{cp}</td><td>{cg}</td><td>{cd}</td></tr>"#,
        tb = total_branches,
        dt = decisions_total,
        df = decisions_full,
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
    body.push_str(&render_conditions(decision));

    let _ = write!(
        body,
        r#"<p class="back-link"><a href="/verdict/{href}">← back to {name}</a></p>"#,
        href = escape(&verdict_name),
        name = escape(&verdict_name),
    );

    let title = format!("Decision #{decision_id}");
    Html(page(&title, &body)).into_response()
}

// ─── helpers ──────────────────────────────────────────────────────────

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

fn render_conditions(decision: &DecisionReport) -> String {
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
