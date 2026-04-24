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

    #[error("io error")]
    Io(#[from] std::io::Error),

    #[error("serde error")]
    Serde(#[from] serde_json::Error),
}
