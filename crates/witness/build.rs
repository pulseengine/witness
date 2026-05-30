//! Stage the canonical quickstart into OUT_DIR so `witness quickstart`
//! can embed it via `include_str!` — without the source path escaping
//! the crate directory (which breaks `cargo publish`, since the
//! package tarball contains only the crate's own files).
//!
//! Source of truth is the repo-root `docs/quickstart.md`, two levels
//! up from this crate. In-workspace builds (and the release tarball
//! build, which runs in-workspace) embed the real text. The crates.io
//! published tarball does NOT contain `docs/` — it lives outside the
//! crate — so there the file is absent and we fall back to a pointer
//! to the online copy. That keeps a single canonical quickstart while
//! letting the crate publish cleanly.

use std::{env, fs, path::Path};

fn main() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR set by cargo");
    let dest = Path::new(&out_dir).join("quickstart.md");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set by cargo");
    let canonical = Path::new(&manifest_dir).join("../../docs/quickstart.md");

    let text = fs::read_to_string(&canonical).unwrap_or_else(|_| {
        // Published-tarball path: docs/ isn't part of the package.
        "witness — 10-minute quickstart\n\n\
         The full quickstart ships with the release tarballs and is published at:\n\n    \
         https://github.com/pulseengine/witness/blob/main/docs/quickstart.md\n\n\
         (You're seeing this shorter pointer because this build came from the\n\
         crates.io package, which does not bundle the repo-root docs/ tree.\n\
         Install a release tarball for the embedded full walkthrough.)\n"
            .to_string()
    });

    fs::write(&dest, text).expect("write quickstart.md into OUT_DIR");

    // Rebuild when the canonical doc changes (in-workspace). Harmless
    // when the path is absent (published tarball) — cargo just treats
    // it as always-changed for this tiny script.
    println!("cargo:rerun-if-changed=../../docs/quickstart.md");
    println!("cargo:rerun-if-changed=build.rs");
}
