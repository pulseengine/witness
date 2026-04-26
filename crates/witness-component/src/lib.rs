//! witness-component — MC/DC reporter as a Wasm Component.
//!
//! Implements the `pulseengine:witness@0.7.0/component` world defined
//! in `wit/world.wit`. Compiled to `wasm32-wasip2` via `cargo build
//! --target wasm32-wasip2 -p witness-component`, the resulting
//! `witness_component.wasm` is a real Component-Model component that
//! exposes the witness-core MC/DC reporter API to any consumer that
//! can speak Component Model (wasmtime serve, wac plug, etc.).
//!
//! Replaces v0.5–v0.6 release pipeline's `witness_core.wasm`, which
//! was a 13 KB build-smoke artefact with no callable exports because
//! witness-core has no `extern "C"` interface and no WIT bindings.
//! v0.7's witness-component closes that gap.

// SCRC clippy lints are enforced workspace-wide on hand-written code,
// but wit-bindgen's generated FFI bindings legitimately use patterns
// (mem::forget on Box for ownership transfer to the host, indexing
// into the canonical-ABI lift/lower paths) that those lints flag.
// Suppress at the crate level so the generated code compiles; the
// hand-written impl below is small and reviewed by hand.
#![allow(
    clippy::mem_forget,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::print_stderr,
    clippy::expect_used,
    clippy::unwrap_used
)]

wit_bindgen::generate!({
    world: "component",
    path: "wit",
    pub_export_macro: false,
});

use exports::pulseengine::witness::reporter::Guest;
use witness_core::attest::verify_envelope;
use witness_core::mcdc_report::McdcReport;
use witness_core::run_record::RunRecord;

struct Component;

impl Guest for Component {
    fn report_text(run_json: String) -> Result<String, String> {
        let record: RunRecord = serde_json::from_str(&run_json).map_err(|e| e.to_string())?;
        Ok(McdcReport::from_record(&record).to_text())
    }

    fn report_json(run_json: String) -> Result<String, String> {
        let record: RunRecord = serde_json::from_str(&run_json).map_err(|e| e.to_string())?;
        let report = McdcReport::from_record(&record);
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())
    }

    fn verify_envelope(envelope_json: String, public_key: Vec<u8>) -> Result<String, String> {
        let stmt = verify_envelope(envelope_json.as_bytes(), &public_key)
            .map_err(|e| e.to_string())?;
        Ok(format!(
            "OK — schema {}, {} subjects",
            stmt.predicate_type,
            stmt.subject.len()
        ))
    }
}

export!(Component);
