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
        /// v0.9.6 — export to call with positional typed arguments,
        /// e.g. `--invoke-with-args 'is_leap:2024'` or
        /// `--invoke-with-args 'parse:0,12345,3.14'`. Types are taken
        /// from the export's Wasm signature via `func.ty()`; no
        /// type-annotation needed in the spec. May be repeated;
        /// processed after all `--invoke` entries. Eliminates the
        /// `core::hint::black_box` wrapper-export pattern.
        #[arg(long = "invoke-with-args")]
        invoke_with_args: Vec<String>,
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

    /// Emit an in-toto Statement (unwrapped) carrying coverage or MC/DC
    /// data as a witness predicate. Sigil wraps and signs the statement;
    /// witness produces the body.
    ///
    /// `--kind coverage` (default) emits the
    /// `https://pulseengine.eu/witness-coverage/v1` branch-summary
    /// predicate. `--kind mcdc` (v0.10.0) emits the
    /// `https://pulseengine.eu/witness-mcdc/v1` predicate carrying the
    /// full per-decision truth tables, condition pairs, interpretation,
    /// and a sha256 binding the envelope to the canonical-JSON report
    /// — closing the long-standing gap that left the MC/DC verdict
    /// unsigned next to the signed branch summary.
    Predicate {
        /// Path to a run JSON (typically the output of `witness merge`).
        #[arg(long)]
        run: PathBuf,
        /// Path to the instrumented Wasm module (its digest is the
        /// Statement's first subject).
        #[arg(long)]
        module: PathBuf,
        /// Optional: path to the original (pre-instrumentation) module.
        /// When the manifest sitting next to `--module` records
        /// `original_module_sha256` (v0.10.0+ instrument), this flag
        /// can be omitted — the digest is read from the manifest.
        /// When supplied, the file's bytes are re-hashed and the
        /// computed digest takes precedence.
        #[arg(long)]
        original: Option<PathBuf>,
        /// Optional: harness command, recorded in the measurement metadata.
        #[arg(long)]
        harness: Option<String>,
        /// Predicate kind.
        #[arg(long, value_enum, default_value_t = PredicateKind::Coverage)]
        kind: PredicateKind,
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
        /// v0.11.0 — additionally re-derive the canonical-JSON
        /// sha256 of the embedded report and compare to the
        /// `report_sha256` field stored in the predicate body. The
        /// signature already protects `report_sha256` (it's inside
        /// the signed payload), so a mismatch here only catches a
        /// producer that stored the wrong hash. Useful for
        /// auditors who want explicit binding-evidence in their
        /// verification log. No-op for `witness-coverage/v1`
        /// predicates (they don't carry `report_sha256`).
        #[arg(long)]
        check_content: bool,
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

    /// Print the embedded quickstart guide to stdout (v0.9.12+).
    /// The full `docs/quickstart.md` is bundled in the binary at
    /// build time via `include_str!`, so users on a fresh machine
    /// without the repo can still get the 10-minute walkthrough:
    /// `witness quickstart | less` or pipe to a file. The same text
    /// ships at https://github.com/pulseengine/witness/blob/main/docs/quickstart.md.
    Quickstart,

    /// Scaffold a new witness fixture project (v0.9.10+). Writes a
    /// working `Cargo.toml`, `src/lib.rs`, `build.sh`, `run.sh`, and
    /// `.gitignore` into `<dir>/<name>/` (default dir: cwd). The
    /// fixture is a minimal `no_std` Rust crate with five no-arg
    /// `run_row_*` exports wired to a 3-condition decision so
    /// reviewers can see witness reconstruct full MC/DC end-to-end.
    /// Eliminates the fiddly setup of cdylib + wasm32-unknown-unknown
    /// + debuginfo=2 + panic=abort + black_box that new users hit.
    New {
        /// Fixture project name; becomes the directory name + crate
        /// name (kebab-case in Cargo.toml, snake_case in module
        /// references).
        name: String,
        /// Parent directory to create the project under. Default: cwd.
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Overwrite if `<dir>/<name>/` already exists. Without this
        /// flag, witness refuses rather than clobber.
        #[arg(long)]
        force: bool,
    },

    /// Boot the witness-viz HTMX visualiser against a compliance bundle
    /// (v0.9.0). Renders truth tables for every decision in the bundle
    /// at http://127.0.0.1:3037 by default. Spawns the `witness-viz`
    /// binary; set `WITNESS_VIZ_BIN` to override its path. Install with
    /// `cd crates/witness-viz && cargo install --path .` or build the
    /// release artifact alongside `witness`.
    Viz {
        /// Path to the verdict-evidence directory (e.g.
        /// `compliance/verdict-evidence/`). Each subdir must contain
        /// `report.json` and `manifest.json`.
        #[arg(long = "reports-dir")]
        reports_dir: PathBuf,
        /// Port to listen on.
        #[arg(long, default_value_t = 3037)]
        port: u16,
        /// Bind address (default 127.0.0.1 — localhost only).
        #[arg(long, default_value = "127.0.0.1")]
        bind: String,
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
    /// Per-file MC/DC roll-up table (v0.7.1) — usable on httparse-scale runs
    /// where the per-decision detail report is unreadable.
    McdcRollup,
    /// Per-file MC/DC roll-up as JSON (v0.7.1).
    McdcRollupJson,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum DiffFormat {
    /// Machine-readable JSON.
    Json,
    /// Human-readable text (used as the PR-comment body).
    Text,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum PredicateKind {
    /// `https://pulseengine.eu/witness-coverage/v1` — branch summary
    /// (total / covered / per-function / uncovered). v0.9.x default.
    Coverage,
    /// `https://pulseengine.eu/witness-mcdc/v1` — MC/DC truth tables,
    /// condition pairs, interpretation, gap-closure recommendations,
    /// plus a sha256 binding the envelope to the canonical-JSON report.
    /// v0.10.0; closes E1 BUG-2 / B1.
    Mcdc,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    match cli.command {
        Command::Instrument { input, output } => {
            witness_core::instrument::instrument_file(&input, &output)?;
            // v0.9.11 — chatty success. Tester review found instrument /
            // run / predicate / attest were silent on success while
            // keygen / verify were chatty; the asymmetry made users
            // re-run commands thinking they had failed.
            let manifest_path = witness_core::instrument::Manifest::path_for(&output);
            // SAFETY-REVIEW: this is the user-facing CLI; print is the
            // intended channel.
            #[allow(clippy::print_stdout)]
            {
                println!(
                    "wrote {} ({} bytes)",
                    output.display(),
                    std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0)
                );
                println!("wrote {} (manifest)", manifest_path.display());
            }
        }
        Command::Run {
            module,
            manifest,
            output,
            invoke,
            invoke_with_args,
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
                invoke_with_args,
                call_start,
                harness,
            };
            run::run_module(&options)?;
            // v0.9.11 — chatty success.
            #[allow(clippy::print_stdout)]
            {
                println!(
                    "wrote {} ({} bytes)",
                    output.display(),
                    std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0)
                );
            }
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
                ReportFormat::McdcRollup => {
                    let rollup = witness_core::mcdc_report::rollup_from_run_file(&input)?;
                    println!("{}", rollup.to_text());
                }
                ReportFormat::McdcRollupJson => {
                    let rollup = witness_core::mcdc_report::rollup_from_run_file(&input)?;
                    println!("{}", serde_json::to_string_pretty(&rollup)?);
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
            kind,
            output,
        } => {
            // v0.10.0 — when `--original` is omitted, fall back to the
            // pre-instrumentation digest the manifest captured at
            // `instrument` time. This ensures every predicate emitted
            // for an instrumented module carries the chain back to
            // `source.wasm` (E1 BUG-3 / B2 closure).
            let original_module = if original.is_some() {
                None
            } else {
                let manifest_path = witness_core::instrument::Manifest::path_for(&module);
                if manifest_path.exists() {
                    let manifest = witness_core::instrument::Manifest::load(&manifest_path)?;
                    manifest.original_module_sha256.map(|sha| {
                        witness_core::predicate::OriginalModule::from_manifest(
                            &manifest.module_source,
                            sha,
                        )
                    })
                } else {
                    None
                }
            };

            let stmt = match kind {
                PredicateKind::Coverage => {
                    let report = witness_core::report::from_run_file(&run)?;
                    if let Some(om) = original_module {
                        witness_core::predicate::build_statement_with_original(
                            &report,
                            &module,
                            Some(om),
                            harness.as_deref(),
                        )?
                    } else {
                        witness_core::predicate::build_statement(
                            &report,
                            &module,
                            original.as_deref(),
                            harness.as_deref(),
                        )?
                    }
                }
                PredicateKind::Mcdc => {
                    let mcdc = witness_core::mcdc_report::from_run_file(&run)?;
                    if let Some(om) = original_module {
                        witness_core::predicate::build_mcdc_statement_with_original(
                            &mcdc,
                            &module,
                            Some(om),
                            harness.as_deref(),
                        )?
                    } else {
                        witness_core::predicate::build_mcdc_statement(
                            &mcdc,
                            &module,
                            original.as_deref(),
                            harness.as_deref(),
                        )?
                    }
                }
            };
            // v0.11.0 — populate `predicate.measurement.test_cases` from
            // the run record's `invoked` list. The library-level
            // builders don't know about RunRecord.invoked (they take a
            // Report / McdcReport which doesn't carry it), so we
            // enrich the JSON post-build at the CLI layer where both
            // pieces are in scope. Closes E1/P2 finding: an auditor
            // wants row-id ↔ named-export traceability.
            let mut stmt = stmt;
            let record = witness_core::run_record::RunRecord::load(&run)?;
            let test_cases: Vec<serde_json::Value> = record
                .invoked
                .iter()
                .filter(|s| !s.starts_with("__witness_trace_bytes="))
                .enumerate()
                .map(|(i, name)| {
                    serde_json::json!({
                        "row_id": u32::try_from(i).unwrap_or(u32::MAX),
                        "invocation": name,
                    })
                })
                .collect();
            if let Some(measurement) = stmt
                .predicate
                .get_mut("measurement")
                .and_then(|m| m.as_object_mut())
            {
                measurement.insert(
                    "test_cases".to_string(),
                    serde_json::Value::Array(test_cases),
                );
            }
            witness_core::predicate::save_statement(&stmt, &output)?;
            // v0.9.11 — chatty success.
            #[allow(clippy::print_stdout)]
            {
                println!(
                    "wrote {} ({} bytes)",
                    output.display(),
                    std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0)
                );
                println!("  predicate type: {}", stmt.predicate_type);
                println!("  subjects: {}", stmt.subject.len());
            }
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
            // v0.9.11 — chatty success (matches keygen / verify).
            #[allow(clippy::print_stdout)]
            {
                println!(
                    "wrote {} ({} bytes, DSSE envelope)",
                    output.display(),
                    std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0)
                );
                println!(
                    "verify with: witness verify --envelope {} --public-key <path>",
                    output.display()
                );
            }
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
            check_content,
        } => {
            let stmt = witness_core::attest::verify_envelope_file(&envelope, &public_key)?;
            // v0.11.0 — optional content-binding check. Re-canonicalise
            // the embedded report and compare its sha256 to the
            // predicate's `report_sha256`. The signature already
            // protected `report_sha256` (it's inside the signed
            // payload), so a mismatch here means the producer stored
            // a wrong hash, not that the envelope was tampered.
            // Auditors get a separate cite-able line for the binding
            // step.
            let mut content_check_msg: Option<String> = None;
            if check_content {
                if let (Some(report), Some(stored_sha)) = (
                    stmt.predicate.get("report"),
                    stmt.predicate
                        .get("report_sha256")
                        .and_then(|v| v.as_str()),
                ) {
                    let canonical = serde_json::to_vec(report)?;
                    let derived = witness_core::predicate::sha256_hex_pub(&canonical);
                    if derived != stored_sha {
                        anyhow::bail!(
                            "content check failed: stored report_sha256 = {}, derived = {}",
                            stored_sha,
                            derived,
                        );
                    }
                    content_check_msg = Some(format!(
                        "  content: report sha256 matches stored value ({}…)",
                        &derived[..16]
                    ));
                } else {
                    content_check_msg = Some(
                        "  content: predicate has no `report_sha256` field (witness-coverage/v1 envelope; check skipped)"
                            .to_string(),
                    );
                }
            }
            #[allow(clippy::print_stdout)]
            {
                println!(
                    "OK — DSSE envelope {} verifies against {}",
                    envelope.display(),
                    public_key.display(),
                );
                println!("  predicate type: {}", stmt.predicate_type);
                for subject in &stmt.subject {
                    println!(
                        "  subject: {} sha256:{}",
                        subject.name, subject.digest.sha256,
                    );
                }
                if let Some(msg) = content_check_msg {
                    println!("{msg}");
                }
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
        Command::Quickstart => {
            // SAFETY-REVIEW: stdout is the intended channel for end-user docs.
            #[allow(clippy::print_stdout)]
            {
                print!("{}", QUICKSTART_TEXT);
            }
        }
        Command::New { name, dir, force } => {
            let parent = dir.unwrap_or_else(|| std::path::PathBuf::from("."));
            scaffold_fixture(&parent, &name, force)?;
        }
        Command::Viz {
            reports_dir,
            port,
            bind,
        } => {
            run_viz(&reports_dir, port, &bind)?;
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

/// v0.9.10 — scaffold a new witness fixture under `parent/<name>/`.
///
/// Embeds five small text templates: `Cargo.toml`, `src/lib.rs`,
/// `build.sh`, `run.sh`, `.gitignore`. Substitutes `{{NAME}}` for the
/// kebab-case crate name and `{{NAME_SNAKE}}` for the snake-case
/// module name. Writes them with `0644` (or `0755` for the shell
/// scripts) and prints a one-screen "next steps" message.
fn scaffold_fixture(parent: &std::path::Path, name: &str, force: bool) -> Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        anyhow::bail!(
            "fixture name `{name}` must be ASCII alphanumeric / '-' / '_' (it becomes the crate name)"
        );
    }
    let project_dir = parent.join(name);
    if project_dir.exists() && !force {
        anyhow::bail!(
            "{} already exists (pass --force to overwrite)",
            project_dir.display()
        );
    }

    let snake = name.replace('-', "_");
    let kebab = name.replace('_', "-");

    let cargo_toml = SCAFFOLD_CARGO_TOML
        .replace("{{NAME}}", &kebab)
        .replace("{{NAME_SNAKE}}", &snake);
    let lib_rs = SCAFFOLD_LIB_RS
        .replace("{{NAME}}", &kebab)
        .replace("{{NAME_SNAKE}}", &snake);
    let build_sh = SCAFFOLD_BUILD_SH
        .replace("{{NAME}}", &kebab)
        .replace("{{NAME_SNAKE}}", &snake);
    let run_sh = SCAFFOLD_RUN_SH
        .replace("{{NAME}}", &kebab)
        .replace("{{NAME_SNAKE}}", &snake);
    let gitignore = SCAFFOLD_GITIGNORE.to_string();

    std::fs::create_dir_all(project_dir.join("src"))?;
    std::fs::write(project_dir.join("Cargo.toml"), cargo_toml)?;
    std::fs::write(project_dir.join("src").join("lib.rs"), lib_rs)?;
    let build_path = project_dir.join("build.sh");
    let run_path = project_dir.join("run.sh");
    std::fs::write(&build_path, build_sh)?;
    std::fs::write(&run_path, run_sh)?;
    std::fs::write(project_dir.join(".gitignore"), gitignore)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = std::fs::metadata(&build_path)?.permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&build_path, p)?;
        let mut p = std::fs::metadata(&run_path)?.permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&run_path, p)?;
    }

    // SAFETY-REVIEW: this is the user-facing scaffold output; print to
    // stdout is the intended channel.
    #[allow(clippy::print_stdout)]
    {
        println!("Created witness fixture at {}", project_dir.display());
        println!();
        println!("  cd {}", project_dir.display());
        println!("  ./build.sh         # builds verdict_{snake}.wasm");
        println!("  ./run.sh           # instruments + runs + reports + bundles");
        println!("  witness viz --reports-dir verdict-evidence");
        println!();
        println!(
            "Single typed-arg export (`is_leap`) driven by 5 years; 1/1 decisions\n  full MC/DC, 2 conditions proved (rustc fuses the third condition)."
        );
    }

    Ok(())
}

/// v0.9.12 — quickstart guide embedded at build time. Lets users on
/// a fresh machine without the repo still run `witness quickstart`
/// and get the full 10-minute walkthrough. Source of truth lives at
/// `docs/quickstart.md`; the binary just bundles it.
const QUICKSTART_TEXT: &str = include_str!("../../../docs/quickstart.md");

const SCAFFOLD_CARGO_TOML: &str = r#"[package]
name = "verdict-{{NAME}}"
version = "0.0.0"
edition = "2024"
publish = false
description = "Witness fixture: scaffolded by `witness new`."
license = "Apache-2.0 OR MIT"

# Standalone — not part of any parent workspace. witness instruments
# core modules (wasm32-unknown-unknown produces these); using
# wasm32-wasip2 here would emit a Component witness can't yet
# instrument.
[workspace]

[lib]
crate-type = ["cdylib"]

# debuginfo = true is REQUIRED — DWARF data is what witness uses to
# group br_if branches into source-level decisions. Without it the
# manifest still contains branches, but they don't merge into multi-
# condition Decisions and MC/DC reconstruction degrades to per-
# branch counter coverage.
[profile.release]
debug = true
strip = false
codegen-units = 1
lto = false
panic = "abort"
"#;

const SCAFFOLD_LIB_RS: &str = r#"//! Witness fixture: scaffolded by `witness new {{NAME}}`.
//!
//! Decision under test:  `(year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)`
//!
//! This is the textbook ISO leap-year rule — three short-circuit
//! conditions in mixed AND/OR shape. Modular arithmetic blocks the
//! constant-fold + bitwise-collapse rustc would otherwise apply to
//! plain `(a && b) || c` over bools. Witness reconstructs **one
//! decision with two conditions** (rustc fuses the `% 400 == 0`
//! check into the same `br_if` chain as the first two), and the five
//! rows in run.sh prove independent effect of those two conditions
//! under masking MC/DC (DO-178C accepted).
//!
//! v0.9.11+: this fixture exports a single `is_leap` function and
//! drives it via `--invoke-with-args` (v0.9.6 typed-arg form). That
//! puts the runtime input through the export's parameter rather than
//! `core::hint::black_box`, so DWARF line attribution lands on the
//! predicate's source line (`lib.rs` here) instead of `hint.rs:491`.

#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

/// The decision under test. Marked `#[inline(never)]` so the call
/// site keeps a stable DWARF line entry — without this the inliner
/// fuses the predicate into each invocation and the truth table loses
/// its consistent attribution.
#[inline(never)]
fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

// Single typed-arg export. Drive it from run.sh with
// `--invoke-with-args 'is_leap:2001'` (year passed as i32 → cast to
// u32 inside). The wasm signature is `(param i32) (result i32)`;
// witness reads parameter types from `func.ty()` so no annotation is
// needed in the spec.
#[unsafe(no_mangle)]
pub extern "C" fn is_leap(year: i32) -> i32 {
    is_leap_year(year as u32) as i32
}
"#;

const SCAFFOLD_BUILD_SH: &str = r#"#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# wasm32-unknown-unknown produces a *core* module (witness's input
# format). wasm32-wasip2 would produce a Component, which witness
# instrument refuses with v0.9.4's preflight error.
TARGET="${TARGET:-wasm32-unknown-unknown}"

cargo build --release --target "$TARGET"
BUILT="target/${TARGET}/release/verdict_{{NAME_SNAKE}}.wasm"
[ -f "$BUILT" ] || { echo "build did not produce $BUILT" >&2; exit 1; }
cp "$BUILT" "$SCRIPT_DIR/verdict_{{NAME_SNAKE}}.wasm"
echo "built: $SCRIPT_DIR/verdict_{{NAME_SNAKE}}.wasm ($(wc -c < "$SCRIPT_DIR/verdict_{{NAME_SNAKE}}.wasm") bytes)"
"#;

const SCAFFOLD_RUN_SH: &str = r#"#!/usr/bin/env bash
# End-to-end pipeline: build → instrument → run → report.
# Expected outcome: 1 decision reconstructed; 2 conditions proved
# under masking MC/DC (rustc fuses the third — see lib.rs comment).
#
# v0.9.11+: also emits the verdict-evidence/{{NAME_SNAKE}}/ layout
# that `witness viz` consumes, so the next step ("./run.sh && witness
# viz --reports-dir verdict-evidence") works without any glue.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

WITNESS="${WITNESS:-witness}"

./build.sh
"$WITNESS" instrument verdict_{{NAME_SNAKE}}.wasm -o instrumented.wasm
# v0.9.11+: typed-arg form. Each --invoke-with-args call passes one
# i32 to `is_leap`. Five years cover the truth table:
#   2001  → c0=F                      → outcome F
#   2004  → c0=T, c1=T                → outcome T (a&&b carries)
#   2100  → c0=T, c1=F, c2=F          → outcome F
#   2000  → c0=T, c1=F, c2=T          → outcome T (% 400 fused with a&&b)
#   1900  → c0=T, c1=F, c2=F          → outcome F (independent of 2100)
"$WITNESS" run instrumented.wasm \
    --invoke-with-args 'is_leap:2001' \
    --invoke-with-args 'is_leap:2004' \
    --invoke-with-args 'is_leap:2100' \
    --invoke-with-args 'is_leap:2000' \
    --invoke-with-args 'is_leap:1900' \
    -o run.json
"$WITNESS" report --input run.json --format mcdc

# v0.9.11 — emit the bundle layout `witness viz` expects, so a fresh
# user goes from `witness new` → `./run.sh` → `witness viz` with no
# manual glue. Each verdict gets its own subdir under verdict-evidence/.
EVIDENCE_DIR="verdict-evidence/{{NAME_SNAKE}}"
mkdir -p "$EVIDENCE_DIR"
"$WITNESS" report --input run.json --format mcdc-json \
    > "$EVIDENCE_DIR/report.json"
cp instrumented.wasm.witness.json "$EVIDENCE_DIR/manifest.json"
echo
echo "Bundle written under verdict-evidence/. Browse with:"
echo "  witness viz --reports-dir verdict-evidence"
"#;

const SCAFFOLD_GITIGNORE: &str =
    "target/\nCargo.lock\n*.wasm\n*.witness.json\nrun.json\nverdict-evidence/\n";

fn run_viz(reports_dir: &std::path::Path, port: u16, bind: &str) -> Result<()> {
    let bin = std::env::var_os("WITNESS_VIZ_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("witness-viz"));
    let status = std::process::Command::new(&bin)
        .arg("--reports-dir")
        .arg(reports_dir)
        .arg("--port")
        .arg(port.to_string())
        .arg("--bind")
        .arg(bind)
        .status()
        .map_err(|e| {
            anyhow::anyhow!(
                "failed to spawn witness-viz at '{}': {e}\n  hint: install with `cd crates/witness-viz && cargo install --path .`, or set WITNESS_VIZ_BIN to the binary path.",
                bin.display()
            )
        })?;
    if !status.success() {
        anyhow::bail!("witness-viz exited with {status}");
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
    // v0.9.4 — silence walrus's `name-section parse` warning by default.
    // Walrus emits `WARN walrus::module: in name section: function index 0
    // is out of bounds for local` for every well-formed cdylib produced
    // by stable rustc, which makes good output look broken (tester
    // review). At -v or higher we restore the default for diagnostics.
    let walrus_filter = if verbosity >= 1 { "" } else { ",walrus=error" };
    let default_directive = format!("{level}{walrus_filter}");
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_directive)),
        )
        .init();
}
