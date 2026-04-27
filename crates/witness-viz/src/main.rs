use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use tracing_subscriber::EnvFilter;

use witness_viz::AppState;

#[derive(Parser, Debug)]
#[command(
    name = "witness-viz",
    about = "HTMX visualiser for witness MC/DC compliance bundles",
    version
)]
struct Args {
    /// Path to a directory laid out like `compliance/verdict-evidence/` —
    /// each subdirectory is a verdict bundle containing `report.json` (and
    /// optionally `manifest.json`).
    #[arg(long)]
    reports_dir: PathBuf,

    /// TCP port to bind on.
    #[arg(long, default_value_t = 3037)]
    port: u16,

    /// Bind address.
    #[arg(long, default_value = "127.0.0.1")]
    bind: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("witness_viz=info,tower_http=warn"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = Args::parse();

    if !args.reports_dir.is_dir() {
        anyhow::bail!(
            "--reports-dir {} is not a directory",
            args.reports_dir.display()
        );
    }

    let state = AppState::new(args.reports_dir.clone());
    let router = witness_viz::router(state);

    let addr = format!("{}:{}", args.bind, args.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    let actual = listener.local_addr()?;

    tracing::info!(
        "witness-viz listening on http://{}:{}",
        args.bind,
        actual.port()
    );

    axum::serve(listener, router).await?;
    Ok(())
}
