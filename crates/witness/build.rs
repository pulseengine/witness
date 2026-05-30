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
//!
//! Panic-free by design (matches crates/witness-viz/build.rs and the
//! workspace `unwrap_used` / `expect_used` lints): missing cargo env
//! or a write error degrades with a `cargo:warning` rather than
//! panicking.

use std::{env, fs, path::Path};

const FALLBACK: &str = "witness — 10-minute quickstart\n\n\
     The full quickstart ships with the release tarballs and is published at:\n\n    \
     https://github.com/pulseengine/witness/blob/main/docs/quickstart.md\n\n\
     (You're seeing this shorter pointer because this build came from the\n\
     crates.io package, which does not bundle the repo-root docs/ tree.\n\
     Install a release tarball for the embedded full walkthrough.)\n";

fn main() {
    // Rebuild when the canonical doc changes (in-workspace). Harmless
    // when the path is absent (published tarball).
    println!("cargo:rerun-if-changed=../../docs/quickstart.md");
    println!("cargo:rerun-if-changed=build.rs");

    let Ok(out_dir) = env::var("OUT_DIR") else {
        println!("cargo:warning=OUT_DIR unset; skipping quickstart staging");
        return;
    };
    let dest = Path::new(&out_dir).join("quickstart.md");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let canonical = Path::new(&manifest_dir).join("../../docs/quickstart.md");

    // Canonical doc in-workspace; FALLBACK in the published tarball
    // (docs/ lives outside the crate, so it isn't packaged).
    let text = fs::read_to_string(&canonical).unwrap_or_else(|_| FALLBACK.to_string());

    if let Err(e) = fs::write(&dest, &text) {
        // A write failure surfaces later as a clear include_str! "file
        // not found" pointing at OUT_DIR/quickstart.md — debuggable.
        println!("cargo:warning=failed to stage quickstart.md: {e}");
    }
}
