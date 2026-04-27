//! HTMX-powered web visualiser for witness MC/DC compliance bundles.
//!
//! The library exposes [`router`] which builds a fully-wired
//! [`axum::Router`]. The companion `witness-viz` binary in `src/main.rs`
//! parses CLI args and hands the resulting [`AppState`] to this router.

pub mod api;
pub mod data;
pub mod js;
pub mod layout;
pub mod mcp;
pub mod state;
pub mod styles;
pub mod views;

pub use state::AppState;

use axum::Router;
use axum::http::{HeaderValue, header};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use tower_http::cors::CorsLayer;

/// Build the Axum router with all routes wired up against `state`.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(views::index))
        .route("/verdict/{name}", get(views::verdict))
        .route("/decision/{verdict}/{decision_id}", get(views::decision))
        .route("/api/v1/summary", get(api::summary))
        .route("/api/v1/verdicts", get(api::verdicts))
        .route("/api/v1/verdict/{name}", get(api::verdict_detail))
        .route(
            "/api/v1/decision/{verdict}/{decision_id}",
            get(api::decision_detail),
        )
        .route("/mcp", post(mcp::handler))
        .route("/assets/htmx.min.js", get(htmx_asset))
        .route("/assets/styles.css", get(styles_asset))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn htmx_asset() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/javascript"),
        )],
        js::HTMX,
    )
}

async fn styles_asset() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, HeaderValue::from_static("text/css"))],
        styles::CSS,
    )
}
