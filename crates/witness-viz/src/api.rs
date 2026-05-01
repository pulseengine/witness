use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::json;

use crate::data::{self, DecisionReport, McdcReport};
use crate::state::AppState;

#[derive(Serialize)]
pub struct VerdictSummary {
    pub name: String,
    pub branches: u32,
    pub decisions_total: u32,
    pub decisions_full_mcdc: u32,
    pub conditions_total: u32,
    pub conditions_proved: u32,
    pub conditions_gap: u32,
    pub conditions_dead: u32,
    pub status: String,
}

/// GET /api/v1/summary — aggregate totals.
pub async fn summary(State(state): State<AppState>) -> Response {
    let verdicts = match data::load_verdicts(state.reports_dir()) {
        Ok(v) => v,
        Err(e) => return json_err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    let mut decisions_total: u64 = 0;
    let mut decisions_full: u64 = 0;
    let mut conditions_total: u64 = 0;
    let mut conditions_proved: u64 = 0;
    let mut conditions_gap: u64 = 0;
    let mut conditions_dead: u64 = 0;
    let mut total_branches: u64 = 0;

    for v in &verdicts {
        let o = &v.report.overall;
        decisions_total = decisions_total.saturating_add(u64::from(o.decisions_total));
        decisions_full = decisions_full.saturating_add(u64::from(o.decisions_full_mcdc));
        conditions_total = conditions_total.saturating_add(u64::from(o.conditions_total));
        conditions_proved = conditions_proved.saturating_add(u64::from(o.conditions_proved));
        conditions_gap = conditions_gap.saturating_add(u64::from(o.conditions_gap));
        conditions_dead = conditions_dead.saturating_add(u64::from(o.conditions_dead));
        total_branches =
            total_branches.saturating_add(u64::from(data::branch_count(state.reports_dir(), v)));
    }

    Json(json!({
        "verdicts": verdicts.len(),
        "branches": total_branches,
        "decisions_total": decisions_total,
        "decisions_full_mcdc": decisions_full,
        "conditions_total": conditions_total,
        "conditions_proved": conditions_proved,
        "conditions_gap": conditions_gap,
        "conditions_dead": conditions_dead,
    }))
    .into_response()
}

/// GET /api/v1/verdicts — one entry per verdict bundle.
pub async fn verdicts(State(state): State<AppState>) -> Response {
    let verdicts = match data::load_verdicts(state.reports_dir()) {
        Ok(v) => v,
        Err(e) => return json_err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    let mut out = Vec::with_capacity(verdicts.len());
    for v in &verdicts {
        let o = &v.report.overall;
        let status = if o.decisions_total > 0 && o.decisions_total == o.decisions_full_mcdc {
            "full_mcdc"
        } else if o.decisions_full_mcdc > 0 {
            "partial_mcdc"
        } else {
            "no_mcdc"
        };
        out.push(VerdictSummary {
            name: v.name.clone(),
            branches: data::branch_count(state.reports_dir(), v),
            decisions_total: o.decisions_total,
            decisions_full_mcdc: o.decisions_full_mcdc,
            conditions_total: o.conditions_total,
            conditions_proved: o.conditions_proved,
            conditions_gap: o.conditions_gap,
            conditions_dead: o.conditions_dead,
            status: status.to_string(),
        });
    }
    Json(out).into_response()
}

/// GET /api/v1/verdict/{name} — full report.
pub async fn verdict_detail(Path(name): Path<String>, State(state): State<AppState>) -> Response {
    match data::find_verdict(state.reports_dir(), &name) {
        Ok(Some(b)) => Json::<McdcReport>(b.report).into_response(),
        Ok(None) => json_err(StatusCode::NOT_FOUND, "verdict not found"),
        Err(e) => json_err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// GET /api/v1/decision/{verdict}/{decision_id} — single decision.
pub async fn decision_detail(
    Path((verdict_name, decision_id)): Path<(String, u32)>,
    State(state): State<AppState>,
) -> Response {
    match data::find_verdict(state.reports_dir(), &verdict_name) {
        Ok(Some(bundle)) => {
            match bundle
                .report
                .decisions
                .into_iter()
                .find(|d| d.id == decision_id)
            {
                Some(d) => Json::<DecisionReport>(d).into_response(),
                None => json_err(StatusCode::NOT_FOUND, "decision not found"),
            }
        }
        Ok(None) => json_err(StatusCode::NOT_FOUND, "verdict not found"),
        Err(e) => json_err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// GET /healthz — liveness probe for container deployments. Returns
/// 200 + a JSON body with the crate version. Does NOT touch the
/// reports directory — must succeed even if the volume is unmounted.
pub async fn healthz() -> Response {
    Json(json!({
        "status": "ok",
        "service": "witness-viz",
        "version": env!("CARGO_PKG_VERSION"),
    }))
    .into_response()
}

fn json_err(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({"error": msg}))).into_response()
}
