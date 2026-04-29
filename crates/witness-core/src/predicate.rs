//! Predicate emission for sigil's in-toto attestation bundles.
//!
//! Witness produces an unwrapped in-toto Statement (JSON) carrying a
//! coverage or MC/DC predicate body. Sigil takes this Statement,
//! wraps it in a DSSE envelope, and signs it. Witness stays out of the
//! key-management business.
//!
//! # Predicate types
//!
//! Two predicate types are emitted:
//!
//! - [`PREDICATE_TYPE`] — `https://pulseengine.eu/witness-coverage/v1`
//!   (default). Branch-coverage summary; suitable for branch-only
//!   consumers.
//! - [`MCDC_PREDICATE_TYPE`] — `https://pulseengine.eu/witness-mcdc/v1`
//!   (v0.10.0). Carries the full MC/DC truth tables, condition pairs,
//!   interpretation, and a sha256 binding the envelope payload to the
//!   canonical-JSON-serialised report content. Closes E1 BUG-2 / B1.
//!
//! Sigil reads `predicateType` opaquely (no registry, no schema
//! validation per type — see `docs/research/sigil-predicate-format.md`),
//! so witness can ship today with no sigil-side change.
//!
//! # Subject convention
//!
//! The in-toto Statement's first `subject` is the **instrumented** Wasm
//! module (the artifact that ships into the test pipeline). When the
//! manifest records the pre-instrumentation digest (v0.10.0+), a
//! second subject names the **original** module so the chain back to
//! `source.wasm` is signed too (E1 BUG-3 / B2). Older manifests fall
//! through to the predicate body's `original_module` field.

use crate::mcdc_report::McdcReport;
use crate::report::Report;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

/// In-toto predicate type URL for witness branch-coverage attestations.
pub const PREDICATE_TYPE: &str = "https://pulseengine.eu/witness-coverage/v1";

/// In-toto predicate type URL for witness MC/DC attestations (v0.10.0).
///
/// Schema-publishing subagents target this constant when wiring the
/// JSON-Schema URL — keep them in sync.
pub const MCDC_PREDICATE_TYPE: &str = "https://pulseengine.eu/witness-mcdc/v1";

/// In-toto Statement v1.0. The `predicate` body is held as
/// `serde_json::Value` so a single Statement type round-trips both the
/// coverage and MC/DC predicate kinds; downstream callers parse it into
/// [`CoveragePredicate`] or [`McdcPredicate`] based on `predicate_type`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Statement {
    #[serde(rename = "_type")]
    pub statement_type: String,
    pub subject: Vec<Subject>,
    #[serde(rename = "predicateType")]
    pub predicate_type: String,
    pub predicate: serde_json::Value,
}

impl Statement {
    /// Parse the predicate body as a [`CoveragePredicate`]. Returns an
    /// error if the JSON shape doesn't match — for example, when the
    /// envelope carries the MC/DC predicate type instead.
    pub fn coverage_predicate(&self) -> Result<CoveragePredicate> {
        serde_json::from_value(self.predicate.clone()).map_err(Error::Serde)
    }

    /// Parse the predicate body as an [`McdcPredicate`]. Returns an
    /// error for non-MC/DC envelopes.
    pub fn mcdc_predicate(&self) -> Result<McdcPredicate> {
        serde_json::from_value(self.predicate.clone()).map_err(Error::Serde)
    }
}

/// In-toto Subject — names + digest of an attestation target.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Subject {
    pub name: String,
    pub digest: Digests,
}

/// Cryptographic digests of an artifact. v0.3 uses SHA-256 only;
/// additional algorithms (SHA-512, BLAKE3) can land later as fields.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Digests {
    pub sha256: String,
}

/// Witness's predicate body — the coverage data plus measurement
/// metadata and a back-pointer to the original module.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CoveragePredicate {
    pub coverage: Report,
    pub measurement: Measurement,
    pub original_module: Option<OriginalModule>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Measurement {
    pub harness: Option<String>,
    pub measured_at: String,
    pub witness_version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OriginalModule {
    pub name: String,
    pub digest: Digests,
}

impl OriginalModule {
    /// Resolve an `OriginalModule` from the manifest-recorded
    /// pre-instrumentation digest plus the original module's source
    /// path string. Used by `witness predicate` when the source `.wasm`
    /// no longer needs to live on disk: the digest was captured at
    /// `instrument` time. The path is reduced to a basename so signed
    /// envelopes don't leak the build host's directory layout (E1 F2).
    pub fn from_manifest(module_source: &str, sha256: String) -> Self {
        let name = Path::new(module_source).file_name().map_or_else(
            || module_source.to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        OriginalModule {
            name,
            digest: Digests { sha256 },
        }
    }
}

/// MC/DC predicate body — the full per-decision truth tables plus a
/// content hash binding the envelope to the canonical-JSON report.
///
/// This is what `witness predicate --kind mcdc` emits. Closes E1 BUG-2 /
/// B1: the MC/DC verdict (truth tables, condition pairs, interpretation,
/// gap closures) is now signed in the same envelope rather than sitting
/// unsigned next to a branch-only predicate.
///
/// # Content binding
///
/// The `report_sha256` field hashes the canonical-JSON serialisation of
/// `report` (no whitespace, BTreeMap keys ordered). A consumer can
/// recompute the hash over a copy of the report on disk and match it
/// against the signed envelope's payload to detect tampering. The DSSE
/// signature already covers the entire payload (including the hash);
/// `report_sha256` is the externally-comparable summary so reviewers
/// don't need to byte-equal the inline blob.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McdcPredicate {
    pub report: McdcReport,
    /// SHA-256 of the canonical-JSON serialisation of `report`.
    /// Hex-encoded, lowercase, 64 chars.
    pub report_sha256: String,
    pub measurement: Measurement,
    pub original_module: Option<OriginalModule>,
}

/// Build an in-toto Statement carrying a coverage predicate.
///
/// `instrumented_path` is the path to the instrumented `.wasm` file —
/// its bytes get hashed and named as the Statement's subject.
/// `original_path` is optional; when present, its digest is included
/// in the predicate body as a back-pointer.
///
/// # Reproducibility (v0.10.0)
///
/// Two normalisations apply so identical inputs produce identical
/// predicates across machines (E1 F2, F3):
///
/// - **Path stripping**: the report's `module` field is rewritten to a
///   project-relative path. If `module` starts with the current
///   working directory, that prefix is dropped; otherwise the basename
///   is kept. Subject names already used the basename.
/// - **`SOURCE_DATE_EPOCH`**: when set to a parseable Unix-seconds
///   integer, that value drives the predicate's `measured_at`. Build
///   clock is used otherwise.
pub fn build_statement(
    report: &Report,
    instrumented_path: &Path,
    original_path: Option<&Path>,
    harness: Option<&str>,
) -> Result<Statement> {
    let original_module = original_module_from_path(original_path)?;
    build_statement_with_original(report, instrumented_path, original_module, harness)
}

/// Variant of [`build_statement`] that takes a pre-resolved
/// [`OriginalModule`] (e.g. one whose digest came from the
/// `original_module_sha256` manifest field, not from re-reading the
/// source `.wasm` at predicate time). Closes E1 BUG-3 / B2 path: the
/// `witness instrument` step is the single source of truth for the
/// pre-instrumentation digest; predicate-builders consume that record
/// without needing the source bytes again.
pub fn build_statement_with_original(
    report: &Report,
    instrumented_path: &Path,
    original_module: Option<OriginalModule>,
    harness: Option<&str>,
) -> Result<Statement> {
    let instrumented_bytes = std::fs::read(instrumented_path).map_err(Error::Io)?;
    let instrumented_digest = sha256_hex(&instrumented_bytes);

    let mut coverage = report.clone();
    coverage.module = strip_to_project_relative(&coverage.module);

    let predicate = CoveragePredicate {
        coverage,
        measurement: Measurement {
            harness: harness.map(str::to_string),
            measured_at: reproducible_timestamp(),
            witness_version: env!("CARGO_PKG_VERSION").to_string(),
        },
        original_module: original_module.clone(),
    };

    let mut subjects = vec![Subject {
        name: file_name_string(instrumented_path),
        digest: Digests {
            sha256: instrumented_digest,
        },
    }];
    if let Some(om) = original_module {
        subjects.push(Subject {
            name: om.name,
            digest: om.digest,
        });
    }

    Ok(Statement {
        statement_type: "https://in-toto.io/Statement/v1".to_string(),
        subject: subjects,
        predicate_type: PREDICATE_TYPE.to_string(),
        predicate: serde_json::to_value(&predicate).map_err(Error::Serde)?,
    })
}

/// Read `path` (when supplied) and return an `OriginalModule` whose
/// digest is the SHA-256 of its bytes. None when `path` is None.
fn original_module_from_path(path: Option<&Path>) -> Result<Option<OriginalModule>> {
    match path {
        None => Ok(None),
        Some(p) => {
            let bytes = std::fs::read(p).map_err(Error::Io)?;
            Ok(Some(OriginalModule {
                name: file_name_string(p),
                digest: Digests {
                    sha256: sha256_hex(&bytes),
                },
            }))
        }
    }
}

/// Build an in-toto Statement carrying an MC/DC predicate (v0.10.0).
///
/// `instrumented_path` is the path to the instrumented `.wasm` file —
/// its bytes get hashed and named as the Statement's first subject.
/// When `original_path` is supplied, its digest becomes the second
/// subject and is also recorded in the predicate body for backwards
/// compatibility with consumers that read the predicate-level field.
///
/// The `report` parameter is the MC/DC verdict the runner already
/// produced from a [`crate::run_record::RunRecord`]. Its
/// canonical-JSON serialisation is hashed into `report_sha256` so the
/// envelope's payload is bound to the inline content.
///
/// `module` paths receive the same project-relative normalisation as
/// the coverage path; `SOURCE_DATE_EPOCH` honours the
/// Reproducible-Builds spec for `measured_at`.
pub fn build_mcdc_statement(
    report: &McdcReport,
    instrumented_path: &Path,
    original_path: Option<&Path>,
    harness: Option<&str>,
) -> Result<Statement> {
    let original_module = original_module_from_path(original_path)?;
    build_mcdc_statement_with_original(report, instrumented_path, original_module, harness)
}

/// Variant of [`build_mcdc_statement`] that takes a pre-resolved
/// [`OriginalModule`] (typically constructed from the manifest's
/// `original_module_sha256` field via [`OriginalModule::from_manifest`]).
pub fn build_mcdc_statement_with_original(
    report: &McdcReport,
    instrumented_path: &Path,
    original_module: Option<OriginalModule>,
    harness: Option<&str>,
) -> Result<Statement> {
    let instrumented_bytes = std::fs::read(instrumented_path).map_err(Error::Io)?;
    let instrumented_digest = sha256_hex(&instrumented_bytes);

    let mut normalised = report.clone();
    normalised.module = strip_to_project_relative(&normalised.module);

    // Canonical JSON over the report (no pretty whitespace; BTreeMap
    // fields already serialise in deterministic key order). Bind the
    // envelope payload to this hash so a tampered inline blob is
    // detectable without re-running the suite.
    let canonical = serde_json::to_vec(&normalised).map_err(Error::Serde)?;
    let report_sha256 = sha256_hex(&canonical);

    let predicate = McdcPredicate {
        report: normalised,
        report_sha256,
        measurement: Measurement {
            harness: harness.map(str::to_string),
            measured_at: reproducible_timestamp(),
            witness_version: env!("CARGO_PKG_VERSION").to_string(),
        },
        original_module: original_module.clone(),
    };

    let mut subjects = vec![Subject {
        name: file_name_string(instrumented_path),
        digest: Digests {
            sha256: instrumented_digest,
        },
    }];
    if let Some(om) = original_module {
        subjects.push(Subject {
            name: om.name,
            digest: om.digest,
        });
    }

    Ok(Statement {
        statement_type: "https://in-toto.io/Statement/v1".to_string(),
        subject: subjects,
        predicate_type: MCDC_PREDICATE_TYPE.to_string(),
        predicate: serde_json::to_value(&predicate).map_err(Error::Serde)?,
    })
}

/// Strip an absolute path down to a project-relative form so signed
/// predicates do not leak the build host's directory layout. If `path`
/// starts with the current working directory, that prefix is dropped;
/// otherwise the basename is kept. Relative paths are returned
/// unchanged.
fn strip_to_project_relative(path: &str) -> String {
    let p = Path::new(path);
    if !p.is_absolute() {
        return path.to_string();
    }
    if let Ok(cwd) = std::env::current_dir()
        && let Ok(rel) = p.strip_prefix(&cwd)
    {
        return rel.to_string_lossy().into_owned();
    }
    p.file_name()
        .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned())
}

/// Resolve the timestamp embedded in the predicate. Honours
/// `SOURCE_DATE_EPOCH` (Reproducible-Builds spec) when set to a
/// non-negative integer that fits in `u64`; otherwise returns the
/// current wall-clock time as RFC 3339. A malformed value falls
/// through to the wall clock so a misconfigured environment does not
/// silently block envelope generation.
fn reproducible_timestamp() -> String {
    if let Ok(raw) = std::env::var("SOURCE_DATE_EPOCH")
        && let Ok(secs) = raw.trim().parse::<u64>()
    {
        return rfc3339_from_unix(secs);
    }
    now_rfc3339()
}

/// Save a Statement to disk as pretty-printed JSON.
pub fn save_statement(statement: &Statement, path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(statement).map_err(Error::Serde)?;
    std::fs::write(path, json).map_err(Error::Io)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    hex_encode(&digest)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for &b in bytes {
        // SAFETY-REVIEW: `b` is u8 (0..=255); upper/lower nibble are 0..=15;
        // indexing into a 16-element table cannot panic. The `as char` cast
        // converts a hex-digit byte (0x30..=0x66) to its ASCII char — the
        // value range is in the ASCII subset where the conversion is exact.
        #[allow(clippy::indexing_slicing, clippy::as_conversions)]
        {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            out.push(HEX[usize::from(b >> 4)] as char);
            out.push(HEX[usize::from(b & 0x0f)] as char);
        }
    }
    out
}

fn file_name_string(p: &Path) -> String {
    p.file_name().map_or_else(
        || p.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// RFC 3339 / ISO 8601 timestamp using only `std`. v0.3 keeps witness
/// off the `chrono` / `time` dependency for build-cost reasons; the
/// resulting string is the seconds-precision UTC time. `pub` so the
/// rivet-evidence emitter can reuse the same timestamp format.
///
/// v0.10.0 — honours `SOURCE_DATE_EPOCH` (per
/// <https://reproducible-builds.org/docs/source-date-epoch/>) when
/// set in the environment. Reviewers re-running the same instrumented
/// module + harness against the same `SOURCE_DATE_EPOCH` get a
/// byte-identical predicate.
pub fn now_rfc3339() -> String {
    if let Ok(epoch) = std::env::var("SOURCE_DATE_EPOCH")
        && let Ok(secs) = epoch.trim().parse::<u64>()
    {
        return rfc3339_from_unix(secs);
    }
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    rfc3339_from_unix(secs)
}

/// Convert a Unix timestamp (seconds since 1970-01-01 UTC) to an RFC 3339
/// string `YYYY-MM-DDTHH:MM:SSZ`. Civil-date conversion via Howard Hinnant's
/// algorithm — pure arithmetic, no allocation, no leap seconds (sub-second
/// precision is irrelevant for witness's measurement timestamps).
fn rfc3339_from_unix(secs: u64) -> String {
    // SAFETY-REVIEW: arithmetic on u64 timestamps in the year-2026..2200
    // range; no realistic overflow path. Unsigned division is well-defined.
    #[allow(clippy::arithmetic_side_effects, clippy::integer_division)]
    {
        let days = secs / 86_400;
        let secs_of_day = secs % 86_400;
        let hour = secs_of_day / 3_600;
        let minute = (secs_of_day / 60) % 60;
        let second = secs_of_day % 60;

        // Howard Hinnant's "civil_from_days" algorithm (public domain).
        let z = i64::try_from(days).unwrap_or(0).saturating_add(719_468);
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era.saturating_mul(400);
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };

        format!("{y:04}-{m:02}-{d:02}T{hour:02}:{minute:02}:{second:02}Z")
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod tests {
    use super::*;
    use crate::report::FunctionReport;

    fn fake_report() -> Report {
        Report {
            schema_version: "2".to_string(),
            witness_version: "test".to_string(),
            module: "app.wasm".to_string(),
            total_branches: 4,
            covered_branches: 3,
            per_function: vec![FunctionReport {
                function_index: 0,
                function_name: Some("f".to_string()),
                total: 4,
                covered: 3,
            }],
            uncovered: vec![],
        }
    }

    #[test]
    fn statement_builds_with_sha256_subject() {
        let dir = tempfile::tempdir().unwrap();
        let inst = dir.path().join("app.instrumented.wasm");
        std::fs::write(&inst, b"\x00asm\x01\x00\x00\x00").unwrap();
        let stmt = build_statement(&fake_report(), &inst, None, Some("cargo test")).unwrap();
        assert_eq!(stmt.statement_type, "https://in-toto.io/Statement/v1");
        assert_eq!(stmt.predicate_type, PREDICATE_TYPE);
        assert_eq!(stmt.subject.len(), 1);
        assert_eq!(stmt.subject[0].name, "app.instrumented.wasm");
        assert_eq!(stmt.subject[0].digest.sha256.len(), 64);
        assert!(
            stmt.subject[0]
                .digest
                .sha256
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        );
        let pred = stmt.coverage_predicate().unwrap();
        assert_eq!(pred.measurement.harness.as_deref(), Some("cargo test"));
        assert!(pred.original_module.is_none());
    }

    #[test]
    fn statement_includes_original_module_when_supplied() {
        let dir = tempfile::tempdir().unwrap();
        let inst = dir.path().join("app.instrumented.wasm");
        let orig = dir.path().join("app.wasm");
        std::fs::write(&inst, b"\x00asm\x01\x00\x00\x00").unwrap();
        std::fs::write(&orig, b"\x00asm\x01\x00\x00\x00").unwrap();
        let stmt = build_statement(&fake_report(), &inst, Some(&orig), None).unwrap();
        // Two subjects: instrumented + original (E1 BUG-3 closure).
        assert_eq!(stmt.subject.len(), 2);
        let pred = stmt.coverage_predicate().unwrap();
        let om = pred.original_module.unwrap();
        assert_eq!(om.name, "app.wasm");
        assert_eq!(om.digest.sha256.len(), 64);
    }

    #[test]
    fn statement_round_trips_via_serde_json() {
        let dir = tempfile::tempdir().unwrap();
        let inst = dir.path().join("a.wasm");
        std::fs::write(&inst, b"\x00asm\x01\x00\x00\x00").unwrap();
        let stmt = build_statement(&fake_report(), &inst, None, None).unwrap();
        let json = serde_json::to_string(&stmt).unwrap();
        let parsed: Statement = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.predicate_type, stmt.predicate_type);
        assert_eq!(
            parsed.subject[0].digest.sha256,
            stmt.subject[0].digest.sha256
        );
    }

    #[test]
    fn rfc3339_format_is_well_formed() {
        // 1970-01-01T00:00:00Z (epoch) is the calibration anchor.
        let s0 = rfc3339_from_unix(0);
        assert_eq!(s0, "1970-01-01T00:00:00Z");
        // 2026-04-25 00:00:00 UTC is 56 years (incl. 14 leap days) plus
        // Jan(31)+Feb(28)+Mar(31)+24 days into 2026: 20568 days × 86400.
        let s1 = rfc3339_from_unix(20_568 * 86_400);
        assert_eq!(s1, "2026-04-25T00:00:00Z");
        // Format invariants for any value.
        let s2 = rfc3339_from_unix(1_700_000_000);
        assert_eq!(s2.len(), 20);
        assert_eq!(&s2[4..5], "-");
        assert_eq!(&s2[7..8], "-");
        assert_eq!(&s2[10..11], "T");
        assert_eq!(&s2[13..14], ":");
        assert_eq!(&s2[16..17], ":");
        assert!(s2.ends_with('Z'));
    }

    #[test]
    fn sha256_known_vector() {
        // sha256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
        let h = sha256_hex(b"abc");
        assert_eq!(
            h,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    /// `SOURCE_DATE_EPOCH` must drive the predicate's `measured_at`
    /// exactly so identical inputs sign to byte-identical envelopes
    /// regardless of the build clock (E1 F3, v0.10.0 must-ship #12).
    ///
    /// SAFETY-REVIEW: this test mutates process env to verify the
    /// reproducibility contract; it is single-threaded by virtue of
    /// touching one well-known variable. We restore the prior value on
    /// exit so neighbouring tests in the same binary stay isolated.
    /// Bundled with the path-stripping assertion (#12 strip) so both
    /// reproducibility behaviours move together.
    #[test]
    fn source_date_epoch_pins_predicate_timestamp_and_strips_paths() {
        // Save and clear so we own the variable for this test.
        let prior = std::env::var("SOURCE_DATE_EPOCH").ok();
        // SAFETY: setting env in a single-threaded test; restored below.
        unsafe {
            std::env::set_var("SOURCE_DATE_EPOCH", "1700000000");
        }

        let dir = tempfile::tempdir().unwrap();
        let inst = dir.path().join("app.instrumented.wasm");
        std::fs::write(&inst, b"\x00asm\x01\x00\x00\x00").unwrap();

        // Synthetic absolute module path that does NOT live under cwd
        // — exercises the basename-fallback branch.
        let mut report = fake_report();
        report.module = "/nowhere/not/under/cwd/app.wasm".to_string();

        let stmt = build_statement(&report, &inst, None, None).unwrap();
        let pred = stmt.coverage_predicate().unwrap();

        // Reproducibility-1: timestamp pinned by SOURCE_DATE_EPOCH.
        assert_eq!(
            pred.measurement.measured_at, "2023-11-14T22:13:20Z",
            "SOURCE_DATE_EPOCH=1700000000 must yield exactly 2023-11-14T22:13:20Z"
        );
        // Reproducibility-2: absolute non-cwd path collapses to basename.
        assert_eq!(pred.coverage.module, "app.wasm");

        // Each branch carries its own SAFETY comment so the clippy
        // `multiple_unsafe_ops_per_block` and `undocumented_unsafe_blocks`
        // lints are both satisfied.
        match prior {
            Some(v) => {
                // SAFETY: restoring the variable to its prior state in
                // the same single-threaded test.
                unsafe { std::env::set_var("SOURCE_DATE_EPOCH", v) }
            }
            None => {
                // SAFETY: clearing the variable since no prior value
                // existed; same single-threaded test.
                unsafe { std::env::remove_var("SOURCE_DATE_EPOCH") }
            }
        }
    }

    /// Module paths that sit under the current working directory must
    /// be rewritten to a project-relative form (cwd prefix dropped).
    /// This is the common case on a developer machine and on CI.
    #[test]
    fn module_path_under_cwd_becomes_project_relative() {
        let cwd = std::env::current_dir().unwrap();
        let abs = cwd.join("target").join("artifact.wasm");
        let abs_str = abs.to_string_lossy().into_owned();

        let stripped = strip_to_project_relative(&abs_str);
        // Must lose the cwd prefix; must preserve the relative tail.
        assert!(
            !stripped.starts_with(cwd.to_string_lossy().as_ref()),
            "expected cwd-relative result, got {stripped}"
        );
        assert!(stripped.ends_with("artifact.wasm"));
    }

    /// Relative paths must pass through untouched — they already are
    /// project-relative.
    #[test]
    fn relative_module_path_passes_through() {
        assert_eq!(
            strip_to_project_relative("verdicts/leap_year/instrumented.wasm"),
            "verdicts/leap_year/instrumented.wasm"
        );
        assert_eq!(strip_to_project_relative("app.wasm"), "app.wasm");
    }

    /// v0.10.0 — `SOURCE_DATE_EPOCH` env var pins the timestamp for
    /// reproducible builds. Per
    /// <https://reproducible-builds.org/docs/source-date-epoch/>,
    /// 1700000000 is 2023-11-14T22:13:20Z.
    #[test]
    fn rfc3339_honours_source_date_epoch() {
        // SAFETY: env var mutation is `unsafe` under multi-threaded
        // tests; cargo runs tests in-process. We mutate a single var
        // with a value no other test touches.
        unsafe {
            std::env::set_var("SOURCE_DATE_EPOCH", "1700000000");
        }
        let stamp = now_rfc3339();
        assert_eq!(
            stamp, "2023-11-14T22:13:20Z",
            "SOURCE_DATE_EPOCH must override the wall clock"
        );
        // SAFETY: same justification.
        unsafe {
            std::env::remove_var("SOURCE_DATE_EPOCH");
        }
    }

    /// Garbage SOURCE_DATE_EPOCH values must not crash; fall back to
    /// the system clock.
    #[test]
    fn rfc3339_garbage_source_date_epoch_falls_back() {
        // SAFETY: see rfc3339_honours_source_date_epoch.
        unsafe {
            std::env::set_var("SOURCE_DATE_EPOCH", "not-a-number");
        }
        let stamp = now_rfc3339();
        assert!(stamp.ends_with('Z'), "still RFC3339-shaped: {stamp}");
        // SAFETY: see above.
        unsafe {
            std::env::remove_var("SOURCE_DATE_EPOCH");
        }
    }

    // --------------------------------------------------------------
    // v0.10.0 — MC/DC predicate (item 1, E1 BUG-2 / B1 closure).
    // --------------------------------------------------------------

    use crate::mcdc_report::McdcReport;
    use crate::run_record::{DecisionRecord, DecisionRow, RunRecord, TraceHealth};
    use std::collections::BTreeMap;

    fn fake_run_record_full_plus_gap() -> RunRecord {
        // Decision A: leap-year 3-condition full MC/DC (4 rows).
        // Decision B: same shape, missing the row that would close c2 →
        //             c2 lands as Gap with a recommended closure.
        let row = |id: u32, evaluated: &[(u32, bool)], outcome: Option<bool>| DecisionRow {
            row_id: id,
            evaluated: evaluated.iter().copied().collect::<BTreeMap<_, _>>(),
            outcome,
        };
        let full = DecisionRecord {
            id: 0,
            source_file: Some("leap_year.rs".to_string()),
            source_line: Some(20),
            condition_branch_ids: vec![100, 101, 102],
            rows: vec![
                row(0, &[(0, false), (2, false)], Some(false)),
                row(1, &[(0, true), (1, true)], Some(true)),
                row(2, &[(0, true), (1, false), (2, false)], Some(false)),
                row(3, &[(0, true), (1, false), (2, true)], Some(true)),
            ],
        };
        let gap = DecisionRecord {
            id: 1,
            source_file: Some("leap_year.rs".to_string()),
            source_line: Some(40),
            condition_branch_ids: vec![200, 201, 202],
            rows: vec![
                row(0, &[(0, false), (2, false)], Some(false)),
                row(1, &[(0, true), (1, true)], Some(true)),
                row(2, &[(0, true), (1, false), (2, false)], Some(false)),
                // missing the row that would prove c2 independent-effect.
            ],
        };
        RunRecord {
            schema_version: "3".to_string(),
            witness_version: "test".to_string(),
            module_path: "app.wasm".to_string(),
            invoked: vec![],
            branches: vec![],
            decisions: vec![full, gap],
            trace_health: TraceHealth::default(),
        }
    }

    /// Item 1 unit test: build an MC/DC predicate from a run record
    /// containing one full-MC/DC + one gap decision, deserialise, and
    /// confirm the truth tables, condition pairs, and gap-closure
    /// recommendations all round-trip through serde + the Statement
    /// envelope.
    #[test]
    fn mcdc_predicate_round_trips_truth_tables_and_gaps() {
        let dir = tempfile::tempdir().unwrap();
        let inst = dir.path().join("app.instrumented.wasm");
        std::fs::write(&inst, b"\x00asm\x01\x00\x00\x00").unwrap();

        let record = fake_run_record_full_plus_gap();
        let mcdc = McdcReport::from_record(&record);

        let stmt =
            build_mcdc_statement(&mcdc, &inst, None, Some("cargo test")).expect("build mcdc");
        assert_eq!(stmt.predicate_type, MCDC_PREDICATE_TYPE);
        assert_eq!(stmt.subject.len(), 1);
        assert_eq!(stmt.subject[0].name, "app.instrumented.wasm");

        // JSON round-trip: write, read, parse the predicate body.
        let json = serde_json::to_string(&stmt).unwrap();
        let parsed: Statement = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.predicate_type, MCDC_PREDICATE_TYPE);
        let predicate = parsed.mcdc_predicate().expect("mcdc predicate");

        // Overall counts survive the trip.
        assert_eq!(predicate.report.overall.decisions_total, 2);
        assert_eq!(predicate.report.overall.decisions_full_mcdc, 1);
        assert_eq!(predicate.report.overall.conditions_proved, 5);
        assert_eq!(predicate.report.overall.conditions_gap, 1);

        // Decision 0 — full MC/DC: every condition has a proving pair.
        let d0 = predicate
            .report
            .decisions
            .iter()
            .find(|d| d.id == 0)
            .unwrap();
        assert!(matches!(
            d0.status,
            crate::mcdc_report::DecisionStatus::FullMcdc
        ));
        assert_eq!(d0.truth_table.len(), 4);
        for c in &d0.conditions {
            assert!(matches!(
                c.status,
                crate::mcdc_report::ConditionStatus::Proved
            ));
            assert!(c.pair.is_some());
            assert!(c.interpretation.is_some());
        }

        // Decision 1 — partial: c2 is the gap, with a closure
        // recommendation pointing at row 2.
        let d1 = predicate
            .report
            .decisions
            .iter()
            .find(|d| d.id == 1)
            .unwrap();
        assert!(matches!(
            d1.status,
            crate::mcdc_report::DecisionStatus::Partial
        ));
        let c2 = d1.conditions.iter().find(|c| c.index == 2).unwrap();
        assert!(matches!(
            c2.status,
            crate::mcdc_report::ConditionStatus::Gap
        ));
        let closure = c2.gap_closure.as_ref().unwrap();
        assert_eq!(closure.paired_with_row, 2);
        assert_eq!(closure.evaluated.get(&2), Some(&true));

        // Content hash binds the envelope to the canonical-JSON report.
        assert_eq!(predicate.report_sha256.len(), 64);
        assert!(
            predicate
                .report_sha256
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        );
        let canonical = serde_json::to_vec(&predicate.report).unwrap();
        assert_eq!(predicate.report_sha256, sha256_hex(&canonical));
    }

    /// Item 2: the manifest-derived flow. `witness instrument` records
    /// the pre-instrumentation digest in the manifest; `witness
    /// predicate` reads it back and emits the second Statement
    /// subject without needing the source `.wasm` on disk.
    /// Verified for both the coverage and MC/DC predicate kinds.
    #[test]
    fn original_module_from_manifest_produces_second_subject() {
        let dir = tempfile::tempdir().unwrap();
        let inst = dir.path().join("app.instrumented.wasm");
        std::fs::write(&inst, b"\x00asm\x01\x00\x00\x00").unwrap();

        // Synthetic manifest digest — what `witness instrument` would
        // have written. Exercises the from_manifest path without
        // round-tripping through walrus.
        let manifest_path_str = "/Users/dev/project/source.wasm";
        let manifest_digest =
            "deadbeefcafebabe1111222233334444555566667777888899990000aaaabbbb".to_string();
        let om = OriginalModule::from_manifest(manifest_path_str, manifest_digest.clone());
        // Path stripped to basename (no host directory leak).
        assert_eq!(om.name, "source.wasm");
        assert_eq!(om.digest.sha256, manifest_digest);

        // Coverage kind.
        let stmt_cov = build_statement_with_original(
            &fake_report(),
            &inst,
            Some(om.clone()),
            Some("cargo test"),
        )
        .unwrap();
        assert_eq!(stmt_cov.subject.len(), 2);
        assert_eq!(stmt_cov.subject[1].name, "source.wasm");
        assert_eq!(stmt_cov.subject[1].digest.sha256, manifest_digest);

        // MC/DC kind.
        let mcdc = McdcReport::from_record(&fake_run_record_full_plus_gap());
        let stmt_mcdc =
            build_mcdc_statement_with_original(&mcdc, &inst, Some(om), Some("cargo test")).unwrap();
        assert_eq!(stmt_mcdc.subject.len(), 2);
        assert_eq!(stmt_mcdc.subject[1].name, "source.wasm");
        assert_eq!(stmt_mcdc.subject[1].digest.sha256, manifest_digest);
    }

    /// Item 2 round-trip: when an `original_path` is supplied, the
    /// MC/DC Statement must carry TWO subjects with distinct digests
    /// (instrumented + original). Closes E1 BUG-3 for the MC/DC kind.
    #[test]
    fn mcdc_predicate_emits_two_subjects_when_original_supplied() {
        let dir = tempfile::tempdir().unwrap();
        let inst = dir.path().join("app.instrumented.wasm");
        let orig = dir.path().join("app.wasm");
        // Distinct bytes so digests differ — the whole point of the
        // chain back to source.wasm is that the digests don't collide.
        std::fs::write(&inst, b"\x00asm\x01\x00\x00\x00instrumented").unwrap();
        std::fs::write(&orig, b"\x00asm\x01\x00\x00\x00original").unwrap();

        let record = fake_run_record_full_plus_gap();
        let mcdc = McdcReport::from_record(&record);
        let stmt =
            build_mcdc_statement(&mcdc, &inst, Some(&orig), None).expect("build mcdc with orig");

        assert_eq!(stmt.subject.len(), 2, "expected instrumented + original");
        assert_eq!(stmt.subject[0].name, "app.instrumented.wasm");
        assert_eq!(stmt.subject[1].name, "app.wasm");
        assert_ne!(
            stmt.subject[0].digest.sha256, stmt.subject[1].digest.sha256,
            "instrumented and original must hash differently"
        );

        let predicate = stmt.mcdc_predicate().unwrap();
        let om = predicate.original_module.as_ref().unwrap();
        assert_eq!(om.name, "app.wasm");
        assert_eq!(om.digest.sha256, stmt.subject[1].digest.sha256);
    }
}
