use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use witness_viz::AppState;
use witness_viz::export::{ExportOpts, run_export};

#[derive(Parser, Debug)]
#[command(
    name = "witness-viz",
    about = "HTMX visualiser for witness MC/DC compliance bundles",
    version,
    arg_required_else_help = false
)]
struct Args {
    #[command(subcommand)]
    cmd: Option<Command>,

    /// Path to a directory laid out like `compliance/verdict-evidence/` —
    /// each subdirectory is a verdict bundle containing `report.json` (and
    /// optionally `manifest.json`). Used when no subcommand is given
    /// (legacy serve mode) and when `serve` is explicit.
    #[arg(long, global = true)]
    reports_dir: Option<PathBuf>,

    /// TCP port to bind on (serve mode).
    #[arg(long, default_value_t = 3037, global = true)]
    port: u16,

    /// Bind address (serve mode).
    #[arg(long, default_value = "127.0.0.1", global = true)]
    bind: String,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run the HTMX dashboard (default when no subcommand is given).
    Serve,

    /// Walk every page and write static HTML to a directory. Output
    /// is browseable from `file://` and deployable to GitHub Pages
    /// or any static host. No HTMX, no API endpoints in the output.
    Export {
        /// Directory to write the static site into. Created if missing;
        /// existing files are overwritten.
        #[arg(long)]
        out: PathBuf,

        /// Optional brand prefix for the page title (e.g. project name +
        /// version). Currently shown only in `<title>`; reserved for
        /// future header use.
        #[arg(long)]
        site_title: Option<String>,

        /// Repository root for source-file lookup (v0.24+). When set,
        /// Decision and Gap pages render an inline `±5 lines` snippet
        /// around `source_file:source_line`. Missing files degrade
        /// gracefully (snippet suppressed; rest of the page renders
        /// unchanged).
        #[arg(long = "source-root")]
        source_root: Option<PathBuf>,
    },

    /// Emit a Markdown MC/DC coverage delta between two report sets
    /// (base vs head) for posting as a PR comment. Each of --base /
    /// --head may be a verdict-evidence directory or a single
    /// report.json (auto-detected). Writes to stdout, or --out FILE.
    /// v0.25+.
    PrComment {
        /// Base (e.g. main) report set: a verdict-evidence dir or a
        /// single report.json.
        #[arg(long)]
        base: PathBuf,
        /// Head (e.g. PR branch) report set.
        #[arg(long)]
        head: PathBuf,
        /// Write the Markdown here instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("witness_viz=info,tower_http=warn"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = Args::parse();

    match args.cmd {
        Some(Command::Export {
            out,
            site_title,
            source_root,
        }) => {
            let reports = args
                .reports_dir
                .as_ref()
                .context("--reports-dir is required for `export`")?;
            if !reports.is_dir() {
                anyhow::bail!("--reports-dir {} is not a directory", reports.display());
            }
            if let Some(ref sr) = source_root
                && !sr.is_dir()
            {
                anyhow::bail!("--source-root {} is not a directory", sr.display());
            }
            let opts = ExportOpts {
                reports_dir: reports.clone(),
                out_dir: out.clone(),
                site_title,
                source_root,
            };
            let summary = run_export(&opts).context("static HTML export")?;
            tracing::info!(
                "wrote {} pages ({} bytes) for {} verdict(s) / {} decision(s) / {} condition(s) → {}",
                summary.pages_written,
                summary.bytes_written,
                summary.verdicts,
                summary.decisions,
                summary.conditions,
                out.display(),
            );
            // CI / scripting consumers parse stdout; tracing output goes
            // to stderr per the EnvFilter init above.
            #[allow(clippy::print_stdout)]
            {
                println!(
                    "exported {} pages → {}",
                    summary.pages_written,
                    out.display()
                );
            }
            Ok(())
        }
        Some(Command::PrComment { base, head, out }) => {
            let base_set = witness_viz::data::load_report_set(&base)
                .with_context(|| format!("loading base report set from {}", base.display()))?;
            let head_set = witness_viz::data::load_report_set(&head)
                .with_context(|| format!("loading head report set from {}", head.display()))?;
            let md = witness_viz::prcomment::render_pr_comment(&base_set, &head_set);
            match out {
                Some(path) => {
                    std::fs::write(&path, md.as_bytes())
                        .with_context(|| format!("writing {}", path.display()))?;
                    tracing::info!("wrote PR-comment Markdown → {}", path.display());
                }
                None => {
                    // The Markdown IS the output — stdout is the contract
                    // (`witness viz-pr-comment ... | gh pr comment --body-file -`).
                    #[allow(clippy::print_stdout)]
                    {
                        print!("{md}");
                    }
                }
            }
            Ok(())
        }
        Some(Command::Serve) | None => serve(args).await,
    }
}

async fn serve(args: Args) -> Result<()> {
    let reports = args
        .reports_dir
        .context("--reports-dir is required for serve mode")?;
    if !reports.is_dir() {
        anyhow::bail!("--reports-dir {} is not a directory", reports.display());
    }

    let state = AppState::new(reports);
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
