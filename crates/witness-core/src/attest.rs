//! DSSE-signed attestation for witness coverage predicates.
//!
//! Wraps an unwrapped in-toto Statement (produced by [`crate::predicate::build_statement`])
//! in a DSSE envelope signed with an Ed25519 secret key. The output
//! envelope is byte-compatible with sigil's `wsc verify` and with any
//! tool that consumes DSSE-wrapped in-toto Statements (cosign,
//! sigstore, in-toto-attestation).
//!
//! Depends on `wsc-attestation` (sigil's lightweight attestation
//! crate) for the DSSE primitives + Ed25519 binding. Wasm-compatible
//! under `wasm32-wasip2`.
//!
//! Per the v0.5 wsc-integration brief
//! (`docs/research/v05-wsc-integration.md`), witness keeps its own
//! `Statement` / `CoveragePredicate` types and only borrows the DSSE
//! wrapper, avoiding coupling to sigil's higher-level
//! `TransformationAttestation` builder.

use crate::Result;
use crate::error::Error;
use crate::predicate::Statement;
use std::path::Path;

/// Sign an unwrapped Statement and return the DSSE envelope JSON bytes.
///
/// `secret_key_bytes` is a 64-byte Ed25519 secret key (32-byte seed +
/// 32-byte public key); accept the raw form for now and extend to PEM
/// in v0.5.1.
pub fn sign_statement(
    statement: &Statement,
    secret_key_bytes: &[u8],
    key_id: Option<&str>,
) -> Result<Vec<u8>> {
    let payload_json = serde_json::to_vec(statement).map_err(Error::Serde)?;
    let mut envelope =
        wsc_attestation::dsse::DsseEnvelope::new(&payload_json, "application/vnd.in-toto+json");

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

/// Verify a DSSE envelope produced by [`sign_statement`] against the
/// matching Ed25519 public key. Returns the inner Statement on success.
pub fn verify_envelope(envelope_bytes: &[u8], public_key_bytes: &[u8]) -> Result<Statement> {
    let envelope: wsc_attestation::dsse::DsseEnvelope =
        serde_json::from_slice(envelope_bytes).map_err(Error::Serde)?;
    let public_key = ed25519_compact::PublicKey::from_slice(public_key_bytes).map_err(|e| {
        Error::Runtime(anyhow::anyhow!(
            "public key must be 32 bytes (Ed25519): {e}"
        ))
    })?;
    let valid = envelope
        .verify_ed25519(&public_key)
        .map_err(|e| Error::Runtime(anyhow::anyhow!("DSSE verify failed: {e:?}")))?;
    if !valid {
        return Err(Error::Runtime(anyhow::anyhow!(
            "DSSE signature verification returned false"
        )));
    }
    let payload = envelope
        .payload_bytes()
        .map_err(|e| Error::Runtime(anyhow::anyhow!("DSSE payload decode: {e:?}")))?;
    serde_json::from_slice(&payload).map_err(Error::Serde)
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
        CoveragePredicate, Digests, Measurement, PREDICATE_TYPE, Statement, Subject,
    };
    use crate::report::{FunctionReport, Report};

    fn fake_statement() -> Statement {
        Statement {
            statement_type: "https://in-toto.io/Statement/v1".to_string(),
            subject: vec![Subject {
                name: "x.wasm".to_string(),
                digest: Digests {
                    sha256: "abc123".to_string(),
                },
            }],
            predicate_type: PREDICATE_TYPE.to_string(),
            predicate: CoveragePredicate {
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
                },
                original_module: None,
            },
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
}
