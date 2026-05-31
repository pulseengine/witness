//! Error types for witness.

use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to read Wasm module at {path}")]
    ReadModule {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse Wasm module at {path}")]
    ParseModule {
        path: PathBuf,
        #[source]
        source: anyhow::Error,
    },

    #[error("failed to emit instrumented Wasm to {path}")]
    EmitModule {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("harness command failed: {command}\nexit code: {code:?}\nstderr: {stderr}")]
    Harness {
        command: String,
        code: Option<i32>,
        stderr: String,
    },

    #[error("run output file malformed at {path}")]
    RunOutput {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("manifest file malformed at {path}")]
    Manifest {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("wasm runtime error: {0}")]
    Runtime(#[source] anyhow::Error),

    #[error("instrumentation error: {0}")]
    Instrument(String),

    /// v0.28 — input is a Wasm Component and witness could not
    /// auto-extract a single core module to instrument. witness
    /// transparently unbundles the SOLE embedded core module of a
    /// component (the wasm32-wasip2 leaf-function case, where the core
    /// has no Component-Model imports and runs under the preview1
    /// runtime). `detail` says why this input didn't qualify — no
    /// core module, or several (a preview1→p2 adapter beside the main
    /// module, i.e. the syscall-heavy case). Superseded the v0.9.4
    /// `InputIsComponent` hard-error.
    #[error(
        "could not auto-unbundle Wasm Component '{path}': {detail}\n  \
         witness auto-extracts the core module from single-module \
         components (the wasm32-wasip2 leaf-function case). For this \
         input, build for wasm32-unknown-unknown or wasm32-wasip1 \
         instead, or extract manually:\n    \
         wasm-tools component unbundle '{path}' --module-dir out/\n    \
         witness instrument out/<module>.wasm"
    )]
    ComponentUnbundle { path: PathBuf, detail: String },

    /// v0.9.4 — YAML/TOML/etc. config parse failure. Distinct from
    /// `Runtime` so consumers can tell schema problems apart from
    /// wasmtime trap-style errors. Tester review found rivet-evidence
    /// reporting YAML parse errors as "wasm runtime error".
    #[error("requirement-map config malformed at {path}")]
    RequirementMap {
        path: PathBuf,
        #[source]
        source: anyhow::Error,
    },

    /// v0.10.3 — DSSE envelope verification or parsing failure. v0.9.x
    /// routed all of these through `Error::Runtime`, so reviewers saw
    /// "wasm runtime error: DSSE verify failed" — misleading because
    /// no wasm runtime is involved. E1 BUG-5 / BUG-6 / F5.
    ///
    /// Distinct messages:
    /// - `EnvelopeMalformed` — file isn't valid DSSE JSON (probably
    ///   truncated, mis-encoded, or not an envelope at all).
    /// - `SignatureInvalid` — envelope is well-formed but the
    ///   signature does not verify against the supplied public key.
    /// - `KeyMalformed` — the public-key file is the wrong size or
    ///   shape (Ed25519 keys must be exactly 32 bytes).
    /// - `PayloadDecode` — signature is fine, but the inner payload
    ///   couldn't be base64-decoded.
    #[error("DSSE envelope is malformed or truncated: {0}")]
    EnvelopeMalformed(String),
    #[error("DSSE signature did not verify against the supplied public key: {0}")]
    SignatureInvalid(String),
    #[error("Ed25519 public key is malformed (must be exactly 32 bytes): {0}")]
    KeyMalformed(String),
    #[error("DSSE payload could not be decoded: {0}")]
    PayloadDecode(String),

    #[error("io error")]
    Io(#[from] std::io::Error),

    #[error("serde error")]
    Serde(#[from] serde_json::Error),
}
