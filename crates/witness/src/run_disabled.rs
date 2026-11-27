//! Stand-in for `run` when witness is built without the `runtime`
//! feature (no embedded wasmtime, e.g. for wasm32-wasip2).
//!
//! The option types mirror `run.rs`, so the CLI compiles unchanged;
//! `run_module` refuses. A field added to `run::RunOptions` must be
//! added here too, or the no-runtime build fails (CI builds it).

use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

/// Mirrors `run::RunOptions`; see run.rs for each field's meaning.
#[derive(Debug)]
pub struct RunOptions<'a> {
    pub module: &'a Path,
    pub manifest: PathBuf,
    pub output: &'a Path,
    pub invoke: Vec<String>,
    pub invoke_with_args: Vec<String>,
    pub call_start: bool,
    pub invoke_all: bool,
    pub harness: Option<String>,
    pub component_backend: bool,
    pub stub_imports: Option<StubImports>,
}

/// Mirrors `run::StubImports`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StubImports {
    Zero,
    Trap,
}

/// Always fails: this build has no runtime to execute the module.
pub fn run_module(options: &RunOptions<'_>) -> Result<()> {
    bail!(
        "cannot run {}: this witness was built without the `runtime` feature \
         (no embedded wasmtime). Run the module with a full witness build, or \
         with any runtime that writes the counters, then use `witness report`.",
        options.module.display()
    )
}
