//! Build script: ensure assets/htmx.min.js is the real HTMX 2.x bundle,
//! not the placeholder shipped in the v0.9.0 source tree. Downloads via
//! system curl if needed; falls back to the placeholder with a warning
//! when offline. Runs only when the asset file is missing or stub-sized.

use std::path::Path;
use std::process::Command;

const HTMX_URL: &str = "https://unpkg.com/htmx.org@2.0.4/dist/htmx.min.js";
const PLACEHOLDER_THRESHOLD_BYTES: u64 = 30_000; // real bundle is ~50 KB

fn main() {
    let target = Path::new("assets/htmx.min.js");
    println!("cargo:rerun-if-changed=assets/htmx.min.js");
    println!("cargo:rerun-if-env-changed=WITNESS_VIZ_OFFLINE");

    let needs_download = match std::fs::metadata(target) {
        Ok(md) => md.len() < PLACEHOLDER_THRESHOLD_BYTES,
        Err(_) => true,
    };

    if !needs_download {
        return;
    }

    if std::env::var("WITNESS_VIZ_OFFLINE").is_ok() {
        println!("cargo:warning=WITNESS_VIZ_OFFLINE set; keeping placeholder htmx.min.js");
        return;
    }

    match Command::new("curl")
        .args(["-fsSL", "-o", "assets/htmx.min.js.tmp", HTMX_URL])
        .status()
    {
        Ok(s) if s.success() => {
            if let Err(e) = std::fs::rename("assets/htmx.min.js.tmp", target) {
                println!("cargo:warning=failed to install htmx bundle: {e}");
                let _ = std::fs::remove_file("assets/htmx.min.js.tmp");
                return;
            }
            println!("cargo:warning=downloaded htmx 2.x bundle from unpkg");
        }
        Ok(s) => {
            let _ = std::fs::remove_file("assets/htmx.min.js.tmp");
            println!("cargo:warning=curl exited {s}; keeping placeholder htmx.min.js");
        }
        Err(e) => {
            println!("cargo:warning=could not invoke curl ({e}); keeping placeholder htmx.min.js");
        }
    }
}
