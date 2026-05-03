//! DSSE-signed attestation for witness coverage predicates.
//!
//! Wraps an unwrapped in-toto Statement (produced by
//! [`crate::predicate::build_statement`]) in a DSSE envelope signed
//! with an Ed25519 secret key.
//!
//! Built on top of [`wsc_attestation::dsse::DsseEnvelope`] — sigil's
//! lightweight attestation crate (now on crates.io). The output
//! envelope is therefore identical to what sigil would produce, and
//! verifies through `wsc verify`, sigstore cosign, and any
//! in-toto-attestation consumer.
//!
//! Pure Rust, `wasm32-wasip2`-compatible (wsc-attestation's `signing`
//! feature only adds `ed25519-compact` + `ct-codecs`, both pure Rust).

use crate::Result;
use crate::error::Error;
use crate::predicate::Statement;
use std::path::Path;
use wsc_attestation::dsse::DsseEnvelope;

/// Sign an unwrapped Statement and return the DSSE envelope JSON bytes.
///
/// `secret_key_bytes` is a 64-byte Ed25519 secret key (32-byte seed +
/// 32-byte public key). PEM/DER input is a v0.5.1 extension.
pub fn sign_statement(
    statement: &Statement,
    secret_key_bytes: &[u8],
    key_id: Option<&str>,
) -> Result<Vec<u8>> {
    let payload_json = serde_json::to_vec(statement).map_err(Error::Serde)?;
    let mut envelope = DsseEnvelope::new(&payload_json, "application/vnd.in-toto+json");

    let secret_key = ed25519_compact::SecretKey::from_slice(secret_key_bytes).map_err(|e| {
        Error::Runtime(anyhow::anyhow!(
            "secret key must be 64 bytes (Ed25519 seed + public key): {e}"
        ))
    })?;

    envelope.sign_ed25519(&secret_key, key_id.map(str::to_string));

    serde_json::to_vec_pretty(&envelope).map_err(Error::Serde)
}

/// Read an unsigned Statement from `predicate_path`, sign with the key
/// at `secret_key_path`, write the DSSE envelope to `output`.
pub fn sign_predicate_file(
    predicate_path: &Path,
    secret_key_path: &Path,
    output: &Path,
    key_id: Option<&str>,
) -> Result<()> {
    let statement_bytes = std::fs::read(predicate_path).map_err(Error::Io)?;
    let statement: Statement =
        serde_json::from_slice(&statement_bytes).map_err(|source| Error::RunOutput {
            path: predicate_path.to_path_buf(),
            source,
        })?;

    let secret_key_bytes = std::fs::read(secret_key_path).map_err(Error::Io)?;
    let envelope_bytes = sign_statement(&statement, &secret_key_bytes, key_id)?;
    std::fs::write(output, envelope_bytes).map_err(Error::Io)
}

/// Generate a fresh Ed25519 keypair and write the secret + public key
/// to `secret_path` (64 bytes) and `public_path` (32 bytes) respectively.
///
/// Used by the v0.6.4 verdict-suite signing path: the compliance
/// action generates an ephemeral keypair per release, signs every
/// verdict predicate with it, and ships the public key alongside the
/// signed envelopes. The secret key is intentionally short-lived
/// (per-release) so there's no long-term key custody concern.
pub fn generate_keypair_files(secret_path: &Path, public_path: &Path) -> Result<()> {
    let kp = ed25519_compact::KeyPair::generate();
    std::fs::write(secret_path, kp.sk.as_ref()).map_err(Error::Io)?;
    std::fs::write(public_path, kp.pk.as_ref()).map_err(Error::Io)?;
    Ok(())
}

/// File-IO wrapper around [`verify_envelope`]: read the envelope and
/// public key from disk, return the inner Statement on success or a
/// runtime error if the signature does not validate.
pub fn verify_envelope_file(envelope_path: &Path, public_key_path: &Path) -> Result<Statement> {
    let envelope_bytes = std::fs::read(envelope_path).map_err(Error::Io)?;
    let public_key_bytes = std::fs::read(public_key_path).map_err(Error::Io)?;
    verify_envelope(&envelope_bytes, &public_key_bytes)
}

/// Verify a DSSE envelope produced by [`sign_statement`] against the
/// matching Ed25519 public key. Returns the inner Statement on success.
pub fn verify_envelope(envelope_bytes: &[u8], public_key_bytes: &[u8]) -> Result<Statement> {
    // v0.10.3 — error messages no longer wrap as "wasm runtime error"
    // (E1 BUG-5/BUG-6). Each failure mode gets a dedicated Error
    // variant so reviewers can tell envelope corruption from
    // signature mismatch from key shape.
    let envelope: DsseEnvelope = serde_json::from_slice(envelope_bytes)
        .map_err(|e| Error::EnvelopeMalformed(e.to_string()))?;
    let public_key = ed25519_compact::PublicKey::from_slice(public_key_bytes)
        .map_err(|e| Error::KeyMalformed(e.to_string()))?;
    let valid = envelope
        .verify_ed25519(&public_key)
        .map_err(|e| Error::SignatureInvalid(format!("{e:?}")))?;
    if !valid {
        return Err(Error::SignatureInvalid(
            "envelope's signature failed Ed25519 verification".to_string(),
        ));
    }
    let payload = envelope
        .payload_bytes()
        .map_err(|e| Error::PayloadDecode(format!("{e:?}")))?;
    serde_json::from_slice(&payload).map_err(|e| Error::EnvelopeMalformed(e.to_string()))
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
    use crate::predicate::{
        CoveragePredicate, Digests, MCDC_PREDICATE_TYPE, McdcPredicate, Measurement,
        PREDICATE_TYPE, Statement, Subject, build_mcdc_statement,
    };
    use crate::report::{FunctionReport, Report};

    fn fake_statement() -> Statement {
        // The Statement carries `predicate` as `serde_json::Value` so a
        // single envelope type round-trips both the coverage and MC/DC
        // predicate kinds (v0.10.0). Build the typed body and serialise.
        let predicate = CoveragePredicate {
            coverage: Report {
                schema_version: "2".to_string(),
                witness_version: "0.5.0".to_string(),
                module: "x.wasm".to_string(),
                total_branches: 1,
                covered_branches: 1,
                per_function: vec![FunctionReport {
                    function_index: 0,
                    function_name: None,
                    total: 1,
                    covered: 1,
                }],
                uncovered: vec![],
            },
            measurement: Measurement {
                harness: None,
                measured_at: "2026-04-25T00:00:00Z".to_string(),
                witness_version: "0.5.0".to_string(),
                toolchain: None,
                test_cases: vec![],
            },
            original_module: None,
        };
        Statement {
            statement_type: "https://in-toto.io/Statement/v1".to_string(),
            subject: vec![Subject {
                name: "x.wasm".to_string(),
                digest: Digests {
                    sha256: "abc123".to_string(),
                },
            }],
            predicate_type: PREDICATE_TYPE.to_string(),
            predicate: serde_json::to_value(&predicate).unwrap(),
        }
    }

    #[test]
    fn sign_then_verify_round_trip() {
        let stmt = fake_statement();
        let key_pair = ed25519_compact::KeyPair::generate();
        let envelope = sign_statement(&stmt, key_pair.sk.as_ref(), Some("test")).unwrap();

        let recovered = verify_envelope(&envelope, key_pair.pk.as_ref()).unwrap();
        assert_eq!(recovered.predicate_type, PREDICATE_TYPE);
        assert_eq!(recovered.subject.len(), 1);
        assert_eq!(recovered.subject[0].digest.sha256, "abc123");
    }

    #[test]
    fn verify_rejects_wrong_key() {
        let stmt = fake_statement();
        let signing_key = ed25519_compact::KeyPair::generate();
        let other_key = ed25519_compact::KeyPair::generate();
        let envelope = sign_statement(&stmt, signing_key.sk.as_ref(), None).unwrap();
        let result = verify_envelope(&envelope, other_key.pk.as_ref());
        assert!(result.is_err());
    }

    #[test]
    fn sign_rejects_bad_secret_key_length() {
        let stmt = fake_statement();
        let result = sign_statement(&stmt, &[0u8; 16], None);
        assert!(matches!(result, Err(Error::Runtime(_))));
    }

    /// Integration round-trip for the v0.10.0 MC/DC predicate type:
    /// build → sign → verify → re-parse the truth tables. Closes E1
    /// BUG-2 verification side; complements the unit test in
    /// `predicate.rs`.
    #[test]
    fn mcdc_predicate_sign_then_verify_round_trip() {
        use crate::mcdc_report::McdcReport;
        use crate::run_record::{DecisionRecord, DecisionRow, RunRecord, TraceHealth};
        use std::collections::BTreeMap;

        // Minimal RunRecord: one full-MC/DC decision (3 conditions, 4 rows).
        let row = |id: u32, evaluated: &[(u32, bool)], outcome: Option<bool>| DecisionRow {
            row_id: id,
            evaluated: evaluated.iter().copied().collect::<BTreeMap<_, _>>(),
            outcome,
            raw_brvals: BTreeMap::new(),
        };
        let record = RunRecord {
            schema_version: "3".to_string(),
            witness_version: "test".to_string(),
            module_path: "app.wasm".to_string(),
            invoked: vec![],
            branches: vec![],
            decisions: vec![DecisionRecord {
                id: 0,
                source_file: Some("leap_year.rs".to_string()),
                source_line: Some(20),
                inline_context: None,
                condition_branch_ids: vec![100, 101, 102],
                rows: vec![
                    row(0, &[(0, false), (2, false)], Some(false)),
                    row(1, &[(0, true), (1, true)], Some(true)),
                    row(2, &[(0, true), (1, false), (2, false)], Some(false)),
                    row(3, &[(0, true), (1, false), (2, true)], Some(true)),
                ],
            }],
            trace_health: TraceHealth::default(),
        };
        let mcdc = McdcReport::from_record(&record);

        let dir = tempfile::tempdir().unwrap();
        let inst = dir.path().join("app.instrumented.wasm");
        std::fs::write(&inst, b"\x00asm\x01\x00\x00\x00").unwrap();

        let stmt = build_mcdc_statement(&mcdc, &inst, None, Some("cargo test")).unwrap();

        let key_pair = ed25519_compact::KeyPair::generate();
        let envelope = sign_statement(&stmt, key_pair.sk.as_ref(), Some("v0.10-test")).unwrap();

        let recovered = verify_envelope(&envelope, key_pair.pk.as_ref()).unwrap();
        assert_eq!(recovered.predicate_type, MCDC_PREDICATE_TYPE);
        assert_eq!(recovered.subject.len(), 1);

        // Truth tables survived sign + verify intact.
        let predicate: McdcPredicate = recovered.mcdc_predicate().unwrap();
        assert_eq!(predicate.report.overall.decisions_full_mcdc, 1);
        assert_eq!(predicate.report.overall.conditions_proved, 3);
        assert_eq!(predicate.report.decisions[0].truth_table.len(), 4);

        // Content hash still matches the canonical-JSON serialisation
        // of the report we recovered — proves the in-envelope binding.
        // v0.11.0 — canonicalise via to_value() first so the bytes
        // match the producer's BTreeMap-sorted form. Direct
        // to_vec on the struct would emit field-declaration order.
        let report_value = serde_json::to_value(&predicate.report).unwrap();
        let canonical = serde_json::to_vec(&report_value).unwrap();
        let recomputed = {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(&canonical);
            let bytes = h.finalize();
            let mut out = String::with_capacity(64);
            for &b in bytes.iter() {
                out.push_str(&format!("{b:02x}"));
            }
            out
        };
        assert_eq!(predicate.report_sha256, recomputed);
    }
}
