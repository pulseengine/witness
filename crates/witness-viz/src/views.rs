//! Axum handlers — thin wrappers around the pure renderer in
//! [`crate::render`]. Each handler loads / looks up the requested
//! data, builds a [`crate::render::RenderContext`] for serve mode,
//! calls the appropriate `render::render_*` function, and wraps the
//! returned `body` in page chrome via [`crate::layout::page`].
//!
//! The shape of these handlers is intentionally uniform so the
//! planned `witness viz export` driver can call the same renderer
//! functions with `RenderContext` configured for static output. See
//! [`crate::render`] for the design seam.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

use crate::data;
use crate::layout::{escape, page};
use crate::render::{self, RenderContext};
use crate::state::AppState;

/// GET / — dashboard with totals + verdict scoreboard.
pub async fn index(State(state): State<AppState>) -> Response {
    let verdicts = match data::load_verdicts(state.reports_dir()) {
        Ok(v) => v,
        Err(e) => return error_page("Failed to load verdicts", &e.to_string()),
    };

    let ctx = RenderContext::for_serve(&verdicts, state.reports_dir());
    let out = render::render_overview(&ctx);
    Html(page(&out.title, &out.body)).into_response()
}

/// GET /verdict/{name} — single-verdict drill-down.
pub async fn verdict(Path(name): Path<String>, State(state): State<AppState>) -> Response {
    let bundle = match data::find_verdict(state.reports_dir(), &name) {
        Ok(Some(b)) => b,
        Ok(None) => return not_found("verdict", &name),
        Err(e) => return error_page("Failed to load verdict", &e.to_string()),
    };

    let ctx = RenderContext::for_serve(&[], state.reports_dir());
    let out = render::render_verdict(&ctx, &bundle);
    Html(page(&out.title, &out.body)).into_response()
}

/// GET /decision/{verdict}/{id} — truth table + per-condition pairs.
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
        None => return not_found("decision", &format!("#{decision_id} in {verdict_name}")),
    };

    let ctx = RenderContext::for_serve(&[], state.reports_dir());
    let out = render::render_decision(&ctx, &bundle, decision);
    Html(page(&out.title, &out.body)).into_response()
}

/// GET /gap/{verdict}/{decision_id}/{condition_index} — gap drill-down.
///
/// Reviewer-facing version of MCP's `find_missing_witness`: humans
/// see exactly what an agent would see (needed condition vector,
/// pair rationale, copy-paste test stub).
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

    let condition = match decision
        .conditions
        .iter()
        .find(|c| c.index == condition_index)
    {
        Some(c) => c,
        None => {
            return not_found(
                "condition",
                &format!("c{condition_index} in decision #{decision_id}"),
            );
        }
    };

    let ctx = RenderContext::for_serve(&[], state.reports_dir());
    let out = render::render_gap(&ctx, &bundle, decision, condition);
    Html(page(&out.title, &out.body)).into_response()
}

// ─── error pages ──────────────────────────────────────────────────────

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
    let body = format!("<h1>{}</h1>\n<pre>{}</pre>", escape(title), escape(msg),);
    let html = page(title, &body);
    (StatusCode::INTERNAL_SERVER_ERROR, Html(html)).into_response()
}
