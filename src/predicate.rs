//! Coverage predicate emission for sigil's in-toto attestation bundles.
//!
//! Witness produces an unwrapped in-toto Statement (JSON) carrying the
//! coverage data as the predicate body. Sigil takes this Statement,
//! wraps it in a DSSE envelope, and signs it. Witness stays out of the
//! key-management business.
//!
//! # Predicate type
//!
//! [`PREDICATE_TYPE`] — `https://pulseengine.eu/witness-coverage/v1`.
//! Sigil reads this opaquely (no registry, no schema validation per
//! type — see `docs/research/sigil-predicate-format.md`), so witness
//! can ship today with no sigil-side change.
//!
//! # Subject convention
//!
//! The in-toto Statement's `subject` is the **instrumented** Wasm
//! module (the artifact that ships into the test pipeline). The
//! original (pre-instrumentation) module's digest goes in the
//! predicate body's `original_module` field, mirroring the
//! transformation/transcoding convention sigil already uses
//! (`src/lib/src/transcoding.rs:200–220`).

use crate::report::Report;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

/// In-toto predicate type URL for witness coverage attestations.
pub const PREDICATE_TYPE: &str = "https://pulseengine.eu/witness-coverage/v1";

/// In-toto Statement v1.0 generic over predicate body type.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Statement {
    #[serde(rename = "_type")]
    pub statement_type: String,
    pub subject: Vec<Subject>,
    #[serde(rename = "predicateType")]
    pub predicate_type: String,
    pub predicate: CoveragePredicate,
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

/// Build an in-toto Statement carrying a coverage predicate.
///
/// `instrumented_path` is the path to the instrumented `.wasm` file —
/// its bytes get hashed and named as the Statement's subject.
/// `original_path` is optional; when present, its digest is included
/// in the predicate body as a back-pointer.
pub fn build_statement(
    report: &Report,
    instrumented_path: &Path,
    original_path: Option<&Path>,
    harness: Option<&str>,
) -> Result<Statement> {
    let instrumented_bytes = std::fs::read(instrumented_path).map_err(Error::Io)?;
    let instrumented_digest = sha256_hex(&instrumented_bytes);

    let original_module = match original_path {
        None => None,
        Some(p) => {
            let bytes = std::fs::read(p).map_err(Error::Io)?;
            Some(OriginalModule {
                name: file_name_string(p),
                digest: Digests {
                    sha256: sha256_hex(&bytes),
                },
            })
        }
    };

    Ok(Statement {
        statement_type: "https://in-toto.io/Statement/v1".to_string(),
        subject: vec![Subject {
            name: file_name_string(instrumented_path),
            digest: Digests {
                sha256: instrumented_digest,
            },
        }],
        predicate_type: PREDICATE_TYPE.to_string(),
        predicate: CoveragePredicate {
            coverage: report.clone(),
            measurement: Measurement {
                harness: harness.map(str::to_string),
                measured_at: now_rfc3339(),
                witness_version: env!("CARGO_PKG_VERSION").to_string(),
            },
            original_module,
        },
    })
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
    p.file_name()
        .map_or_else(|| p.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned())
}

/// RFC 3339 / ISO 8601 timestamp using only `std`. v0.3 keeps witness
/// off the `chrono` / `time` dependency for build-cost reasons; the
/// resulting string is the seconds-precision UTC time. `pub` so the
/// rivet-evidence emitter can reuse the same timestamp format.
pub fn now_rfc3339() -> String {
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

        format!(
            "{y:04}-{m:02}-{d:02}T{hour:02}:{minute:02}:{second:02}Z"
        )
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
        assert!(stmt.subject[0].digest.sha256.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(stmt.predicate.measurement.harness.as_deref(), Some("cargo test"));
        assert!(stmt.predicate.original_module.is_none());
    }

    #[test]
    fn statement_includes_original_module_when_supplied() {
        let dir = tempfile::tempdir().unwrap();
        let inst = dir.path().join("app.instrumented.wasm");
        let orig = dir.path().join("app.wasm");
        std::fs::write(&inst, b"\x00asm\x01\x00\x00\x00").unwrap();
        std::fs::write(&orig, b"\x00asm\x01\x00\x00\x00").unwrap();
        let stmt = build_statement(&fake_report(), &inst, Some(&orig), None).unwrap();
        let om = stmt.predicate.original_module.unwrap();
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
        assert_eq!(parsed.subject[0].digest.sha256, stmt.subject[0].digest.sha256);
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
}
