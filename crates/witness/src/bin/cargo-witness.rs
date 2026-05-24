//! `cargo witness` subcommand shim.
//!
//! Cargo's convention: a binary named `cargo-<name>` placed on PATH
//! is invoked as `cargo <name> ...`. So this binary, when installed
//! into `~/.cargo/bin/`, makes `cargo witness instrument foo.wasm
//! -o inst.wasm` work as a thin alias for `witness instrument
//! foo.wasm -o inst.wasm`.
//!
//! Cargo invokes the subcommand binary with the subcommand name as
//! `argv[1]` (so we receive `cargo-witness witness instrument …`).
//! This shim strips that leading `witness` pseudo-arg and re-execs
//! the sibling `witness` binary in the same install directory.
//!
//! Why a re-exec rather than calling the CLI directly: the `witness`
//! CLI logic lives in `crates/witness/src/main.rs` as a private
//! module, not the library. Refactoring that into a public
//! `run_cli(args)` entry point is a larger surface change than
//! warranted for a Level-A subcommand alias. The re-exec adds one
//! `Command::new + status()` indirection — negligible cost, and
//! crucially preserves stdout/stderr/exit-code byte-for-byte
//! identical to a direct `witness` invocation.
//!
//! Level B (a `cargo witness all` wrapper that runs `cargo build`,
//! `witness instrument`, `witness run`, and `witness report`
//! end-to-end with sensible defaults) is tracked as a separate
//! piece of work; this shim is just the naming alias.

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let mut args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();

    // Strip the `witness` subcommand-name arg that cargo prepends.
    // Don't strip if we were invoked directly (e.g. for testing), in
    // which case `argv[1]` is the user's first actual arg.
    if args.first().map(|a| a == "witness").unwrap_or(false) {
        args.remove(0);
    }

    let witness_exe = match locate_sibling_witness() {
        Some(p) => p,
        None => {
            // SAFETY-REVIEW: user-facing CLI failure message; print
            // is the intended channel.
            #[allow(clippy::print_stderr)]
            {
                eprintln!(
                    "cargo-witness: could not locate the sibling `witness` binary. \
                     Both binaries should be installed in the same directory \
                     (e.g. ~/.cargo/bin/). Try `cargo install witness` or \
                     download the release tarball."
                );
            }
            return ExitCode::from(127);
        }
    };

    match Command::new(&witness_exe).args(&args).status() {
        Ok(status) => {
            // Map the child's exit code 1:1. On signal-termination
            // (Unix), Rust returns None — surface as code 1. Exit
            // codes outside [0, 255] are clamped via try_from; this
            // matches std::process::ExitCode's u8 contract.
            let code = status.code().unwrap_or(1);
            ExitCode::from(u8::try_from(code & 0xFF).unwrap_or(1))
        }
        Err(e) => {
            // SAFETY-REVIEW: as above.
            #[allow(clippy::print_stderr)]
            {
                eprintln!(
                    "cargo-witness: failed to spawn {}: {e}",
                    witness_exe.display()
                );
            }
            ExitCode::from(127)
        }
    }
}

fn locate_sibling_witness() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let name = if cfg!(windows) {
        "witness.exe"
    } else {
        "witness"
    };
    let candidate = dir.join(name);
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}
