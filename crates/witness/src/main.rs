//! witness — MC/DC-style branch coverage for WebAssembly components.
//!
//! See `README.md` for the full argument, `DESIGN.md` for architecture and
//! the decision-granularity open question, and `artifacts/requirements.yaml`
//! for traced requirements.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use witness::run;

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

    /// Execute an instrumented module and collect counters.
    ///
    /// Default mode embeds wasmtime and invokes user-specified exports.
    /// `--harness <cmd>` switches to subprocess mode: witness spawns the
    /// command with WITNESS_MODULE / WITNESS_MANIFEST / WITNESS_OUTPUT env
    /// vars set, and the harness writes a counter snapshot before exit.
    Run {
        /// Path to the instrumented module.
        module: PathBuf,
        /// Path to the branch manifest (defaults to `<module>.witness.json`).
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// Path to write the raw counter data (JSON).
        #[arg(short, long, default_value = "witness-run.json")]
        output: PathBuf,
        /// Export to call (no arguments, any number of return values).
        /// May be repeated; exports are invoked in the order given.
        /// Ignored when `--harness` is set.
        #[arg(long = "invoke")]
        invoke: Vec<String>,
        /// Call the `_start` WASI entry-point before `--invoke` targets.
        /// Ignored when `--harness` is set.
        #[arg(long)]
        call_start: bool,
        /// Subprocess harness command. When set, witness spawns this
        /// command via `sh -c` with WITNESS_MODULE / WITNESS_MANIFEST /
        /// WITNESS_OUTPUT env vars; the harness must write a counter
        /// snapshot to WITNESS_OUTPUT before exiting.
        #[arg(long)]
        harness: Option<String>,
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

    /// Merge multiple run JSON files into one by summing per-branch counters.
    ///
    /// Inputs must share the same instrumented module (same `module_path`,
    /// same branch list). `invoked` lists are concatenated in input order.
    /// Use this to combine runs from multiple test binaries or harness
    /// invocations before producing one coverage report.
    Merge {
        /// Run JSON files to merge.
        inputs: Vec<PathBuf>,
        /// Output path for the merged run JSON.
        #[arg(short, long, default_value = "witness-merged.json")]
        output: PathBuf,
    },

    /// Emit an in-toto Statement (unwrapped) carrying the coverage as a
    /// `https://pulseengine.eu/witness-coverage/v1` predicate. Sigil
    /// wraps and signs the statement; witness produces the body.
    Predicate {
        /// Path to a run JSON (typically the output of `witness merge`).
        #[arg(long)]
        run: PathBuf,
        /// Path to the instrumented Wasm module (its digest is the
        /// Statement's subject).
        #[arg(long)]
        module: PathBuf,
        /// Optional: path to the original (pre-instrumentation) module;
        /// its digest is recorded in the predicate body.
        #[arg(long)]
        original: Option<PathBuf>,
        /// Optional: harness command, recorded in the measurement metadata.
        #[arg(long)]
        harness: Option<String>,
        /// Output path for the JSON Statement.
        #[arg(short, long, default_value = "witness-predicate.json")]
        output: PathBuf,
    },

    /// Compute the coverage / branch-set delta between two manifests
    /// or run records. Used by the `witness-delta.yml` PR workflow.
    Diff {
        /// Base snapshot (manifest or run JSON).
        #[arg(long)]
        base: PathBuf,
        /// Head snapshot (manifest or run JSON).
        #[arg(long)]
        head: PathBuf,
        /// Output path for the JSON delta document.
        #[arg(short, long, default_value = "witness-delta.json")]
        output: PathBuf,
        /// Output format: json (default) or text.
        #[arg(long, value_enum, default_value_t = DiffFormat::Json)]
        format: DiffFormat,
    },

    /// Sign an unwrapped Statement (from `witness predicate`) and
    /// produce a DSSE envelope. Compatible with sigil's `wsc verify`,
    /// cosign, sigstore, and any in-toto-attestation consumer.
    Attest {
        /// Path to the Statement JSON (output of `witness predicate`).
        #[arg(long)]
        predicate: PathBuf,
        /// Path to the Ed25519 secret key (raw 64 bytes: 32-byte seed
        /// + 32-byte public key, no PEM in v0.5; PEM/DER in v0.5.1).
        #[arg(long)]
        secret_key: PathBuf,
        /// Optional key identifier embedded in the DSSE signature.
        #[arg(long)]
        key_id: Option<String>,
        /// Output path for the DSSE envelope JSON.
        #[arg(short, long, default_value = "witness-envelope.json")]
        output: PathBuf,
    },

    /// Generate a fresh Ed25519 keypair (raw 64-byte secret + 32-byte
    /// public). v0.6.4 — used by the verdict-suite signing path which
    /// generates an ephemeral key per release.
    Keygen {
        /// Output path for the secret key (64 bytes).
        #[arg(long)]
        secret: PathBuf,
        /// Output path for the public key (32 bytes).
        #[arg(long)]
        public: PathBuf,
    },

    /// Verify a DSSE envelope (from `witness attest`) against the
    /// matching Ed25519 public key. Exits non-zero on signature failure.
    /// v0.6.4 — closes the verification side of the signed-evidence loop.
    Verify {
        /// Path to the DSSE envelope JSON.
        #[arg(long)]
        envelope: PathBuf,
        /// Path to the Ed25519 public key (32 bytes).
        #[arg(long)]
        public_key: PathBuf,
    },

    /// Emit LCOV from a run JSON for codecov ingestion.
    /// DWARF-correlated decisions emit BRDA records; uncorrelated
    /// branches go in a sibling overview text file (per
    /// docs/research/v05-lcov-format.md).
    Lcov {
        /// Path to a run JSON.
        #[arg(long)]
        run: PathBuf,
        /// Path to the manifest (defaults to `<run>.witness.json` style:
        /// the manifest must accompany the run).
        #[arg(long)]
        manifest: PathBuf,
        /// Output path for the LCOV file.
        #[arg(short, long, default_value = "lcov.info")]
        output: PathBuf,
        /// Output path for the sibling overview text.
        #[arg(long, default_value = "witness-overview.txt")]
        overview: PathBuf,
    },

    /// Emit rivet-shape coverage evidence YAML, partitioned by a
    /// branch→artefact mapping. Output is consumable by rivet's
    /// `CoverageStore` (landing in the rivet upstream PR coordinated
    /// with this v0.3 release).
    RivetEvidence {
        /// Path to a run JSON (typically the output of `witness merge`).
        #[arg(long)]
        run: PathBuf,
        /// Path to the requirement-map YAML (mappings of branch ids to
        /// rivet artefact ids). See docs/research/rivet-evidence-consumer.md.
        #[arg(long = "requirement-map")]
        requirement_map: PathBuf,
        /// Optional environment label for the run-metadata block.
        #[arg(long)]
        environment: Option<String>,
        /// Optional commit SHA for the run-metadata block.
        #[arg(long)]
        commit: Option<String>,
        /// Output path for the YAML evidence file.
        #[arg(short, long, default_value = "witness-coverage-evidence.yaml")]
        output: PathBuf,
    },
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum ReportFormat {
    /// Human-readable branch-coverage summary.
    Text,
    /// Machine-readable JSON branch coverage for tools (rivet, CI).
    Json,
    /// Human-readable MC/DC truth tables, verdicts, gap analysis (v0.6).
    Mcdc,
    /// Machine-readable MC/DC JSON, schema https://pulseengine.eu/witness-mcdc/v1 (v0.6).
    McdcJson,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum DiffFormat {
    /// Machine-readable JSON.
    Json,
    /// Human-readable text (used as the PR-comment body).
    Text,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    match cli.command {
        Command::Instrument { input, output } => {
            witness_core::instrument::instrument_file(&input, &output)?;
        }
        Command::Run {
            module,
            manifest,
            output,
            invoke,
            call_start,
            harness,
        } => {
            let manifest =
                manifest.unwrap_or_else(|| witness_core::instrument::Manifest::path_for(&module));
            let options = run::RunOptions {
                module: &module,
                manifest,
                output: &output,
                invoke,
                call_start,
                harness,
            };
            run::run_module(&options)?;
        }
        Command::Report { input, format } => {
            // SAFETY-REVIEW: CLI's job is to write the report to stdout;
            // `println!` is the intended output channel for end users.
            #[allow(clippy::print_stdout)]
            match format {
                ReportFormat::Text => {
                    let report = witness_core::report::from_run_file(&input)?;
                    println!("{}", report.to_text());
                }
                ReportFormat::Json => {
                    let report = witness_core::report::from_run_file(&input)?;
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
                ReportFormat::Mcdc => {
                    let report = witness_core::mcdc_report::from_run_file(&input)?;
                    println!("{}", report.to_text());
                }
                ReportFormat::McdcJson => {
                    let report = witness_core::mcdc_report::from_run_file(&input)?;
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
            }
        }
        Command::Merge { inputs, output } => {
            witness_core::run_record::merge_files(&inputs, &output)?;
        }
        Command::Predicate {
            run,
            module,
            original,
            harness,
            output,
        } => {
            let report = witness_core::report::from_run_file(&run)?;
            let stmt = witness_core::predicate::build_statement(
                &report,
                &module,
                original.as_deref(),
                harness.as_deref(),
            )?;
            witness_core::predicate::save_statement(&stmt, &output)?;
        }
        Command::Diff {
            base,
            head,
            output,
            format,
        } => {
            let delta = witness_core::diff::diff(&base, &head)?;
            // SAFETY-REVIEW: CLI prints to stdout for human consumers.
            #[allow(clippy::print_stdout)]
            match format {
                DiffFormat::Json => {
                    let json = serde_json::to_string_pretty(&delta)?;
                    std::fs::write(&output, &json)?;
                    println!("{json}");
                }
                DiffFormat::Text => {
                    let text = witness_core::diff::delta_to_text(&delta);
                    std::fs::write(&output, &text)?;
                    println!("{text}");
                }
            }
        }
        Command::Attest {
            predicate,
            secret_key,
            key_id,
            output,
        } => {
            witness_core::attest::sign_predicate_file(
                &predicate,
                &secret_key,
                &output,
                key_id.as_deref(),
            )?;
        }
        Command::Keygen { secret, public } => {
            witness_core::attest::generate_keypair_files(&secret, &public)?;
            #[allow(clippy::print_stdout)]
            {
                println!("wrote secret key: {}", secret.display());
                println!("wrote public key: {}", public.display());
            }
        }
        Command::Verify {
            envelope,
            public_key,
        } => {
            let stmt = witness_core::attest::verify_envelope_file(&envelope, &public_key)?;
            #[allow(clippy::print_stdout)]
            {
                println!(
                    "OK — DSSE envelope {} verifies against {}",
                    envelope.display(),
                    public_key.display(),
                );
                println!("  predicate type: {}", stmt.predicate_type);
                println!("  subjects: {}", stmt.subject.len());
            }
        }
        Command::Lcov {
            run,
            manifest,
            output,
            overview,
        } => {
            let manifest_loaded = witness_core::instrument::Manifest::load(&manifest)?;
            let record = witness_core::run_record::RunRecord::load(&run)?;
            witness_core::lcov::emit_lcov_files(&manifest_loaded, &record, &output, &overview)?;
        }
        Command::RivetEvidence {
            run,
            requirement_map,
            environment,
            commit,
            output,
        } => {
            let record = witness_core::run_record::RunRecord::load(&run)?;
            let map = witness_core::rivet_evidence::RequirementMap::load(&requirement_map)?;
            let flat = map.flatten()?;
            let file = witness_core::rivet_evidence::build_evidence(
                &record,
                &flat,
                "witness rivet-evidence",
                environment.as_deref(),
                commit.as_deref(),
            )?;
            witness_core::rivet_evidence::save_evidence(&file, &output)?;
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
