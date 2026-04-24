//! witness — MC/DC-style branch coverage for WebAssembly components.
//!
//! See `README.md` for the full argument, `DESIGN.md` for architecture and
//! the decision-granularity open question, and `artifacts/requirements.yaml`
//! for traced requirements.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "MC/DC-style branch coverage for WebAssembly.",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Verbose output (repeat for more detail).
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,
}

#[derive(Subcommand)]
enum Command {
    /// Instrument a Wasm module with branch counters.
    Instrument {
        /// Path to the input .wasm module.
        input: PathBuf,
        /// Path to write the instrumented module.
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Execute an instrumented module against a test harness and collect counters.
    Run {
        /// Shell command that runs the test harness against the instrumented module.
        #[arg(long)]
        harness: String,
        /// Path to the instrumented module.
        #[arg(long)]
        module: PathBuf,
        /// Path to write the raw counter data (JSON).
        #[arg(short, long, default_value = "witness-run.json")]
        output: PathBuf,
    },

    /// Produce a coverage report from collected counter data.
    Report {
        /// Path to a run output file produced by `witness run`.
        #[arg(long, default_value = "witness-run.json")]
        input: PathBuf,
        /// Output format.
        #[arg(long, value_enum, default_value_t = ReportFormat::Text)]
        format: ReportFormat,
    },
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum ReportFormat {
    /// Human-readable summary.
    Text,
    /// Machine-readable JSON for tools (rivet, CI).
    Json,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    match cli.command {
        Command::Instrument { input, output } => {
            witness::instrument::instrument_file(&input, &output)?;
        }
        Command::Run { harness, module, output } => {
            witness::run::run_harness(&harness, &module, &output)?;
        }
        Command::Report { input, format } => {
            let report = witness::report::from_run_file(&input)?;
            match format {
                ReportFormat::Text => println!("{}", report.to_text()),
                ReportFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
            }
        }
    }

    Ok(())
}

fn init_tracing(verbosity: u8) {
    let level = match verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level)),
        )
        .init();
}
