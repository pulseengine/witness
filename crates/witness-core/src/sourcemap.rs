//! V3 source-map reader — fallback source-line attribution when a
//! wasm module has no DWARF.
//!
//! Design + caveats: `docs/research/source-map-ingestion.md`.
//!
//! Two pieces:
//! - [`extract_source_mapping_url`] — read the `sourceMappingURL`
//!   wasm custom section if present.
//! - [`build_line_map_from_v3`] — parse a V3 source-map JSON string
//!   into a [`crate::decisions::LineMap`] that the clustering pass
//!   in `decisions.rs` consumes identically to a DWARF-built one.
//!
//! The convention this module follows for the wasm <-> source-map
//! address mapping: the **generated column** of each source-map
//! mapping is treated as the byte offset into the wasm code
//! section. Kotlin/Wasm and other emitters agree on this. The
//! generated line is ignored — wasm has no concept of "emitted
//! lines."
//!
//! What is *not* recovered from V3 (structural limitations of the
//! format, not a witness gap):
//! - Inline-call chains (no `DW_TAG_inlined_subroutine` equivalent).
//! - Address ranges (V3 maps positions, not ranges).
//! - Demangled symbols (the V3 `names` field is per-mapping; witness
//!   already has function names from the wasm name section, so it's
//!   dropped).

use anyhow::{anyhow, Result};

use crate::decisions::{LineLocation, LineMap};

/// Read the `sourceMappingURL` custom section payload, if any. The
/// payload is conventionally a UTF-8 URL or relative path pointing
/// at the `.wasm.map` sidecar. Returned untouched apart from
/// trimming trailing NULs that some emitters include.
pub(crate) fn extract_source_mapping_url(wasm_bytes: &[u8]) -> Option<String> {
    let parser = wasmparser::Parser::new(0);
    for payload in parser.parse_all(wasm_bytes) {
        let payload = match payload {
            Ok(p) => p,
            Err(_) => return None,
        };
        if let wasmparser::Payload::CustomSection(section) = payload
            && section.name() == "sourceMappingURL"
        {
            return std::str::from_utf8(section.data())
                .ok()
                .map(|s| s.trim_matches(char::from(0)).trim().to_string());
        }
    }
    None
}

/// Parse a V3 source-map JSON string and build a [`LineMap`] keyed
/// by wasm code-section byte offset. Returns `Err` if the JSON
/// isn't a valid V3 source map.
///
/// V3 lines are 0-indexed in the source map; this function converts
/// to the 1-indexed convention `decisions.rs` shares with DWARF.
pub(crate) fn build_line_map_from_v3(map_json: &str) -> Result<LineMap> {
    let sm = sourcemap::SourceMap::from_reader(map_json.as_bytes())
        .map_err(|e| anyhow!("failed to parse V3 source map: {e}"))?;

    let mut line_map = LineMap::new();
    for token in sm.tokens() {
        // The generated column is the wasm byte offset by
        // convention (see module docs).
        let byte_offset = u64::from(token.get_dst_col());

        // Source file index → resolved source path. `get_source()`
        // returns None if no source index attached; treat as empty.
        let file = token.get_source().unwrap_or_default().to_string();

        // V3 lines are 0-indexed; DWARF lines (and downstream
        // reporters) are 1-indexed. saturating_add keeps the worst
        // case (line == u32::MAX) sane rather than overflowing.
        let line = token.get_src_line().saturating_add(1);

        line_map.insert(byte_offset, LineLocation { file, line });
    }
    Ok(line_map)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// Minimal V3 source map: one mapping `MAAA` =
    /// `[6, 0, 0, 0]` in base64-VLQ — generated column 6, source
    /// index 0, original line 0, original column 0. After the
    /// 0→1-base conversion this lands at byte_offset 6 →
    /// `("leap.kt", 1)`.
    #[test]
    fn build_line_map_from_v3_single_mapping() {
        let json = r#"{"version":3,"sources":["leap.kt"],"names":[],"mappings":"MAAA"}"#;
        let line_map = build_line_map_from_v3(json).unwrap();
        assert_eq!(line_map.len(), 1, "one mapping → one LineMap entry");
        let (offset, loc) = line_map.iter().next().unwrap();
        assert_eq!(*offset, 6, "VLQ M = signed +6 = byte_offset 6");
        assert_eq!(loc.file, "leap.kt");
        assert_eq!(loc.line, 1, "V3 line 0 → 1-indexed LineLocation.line");
    }

    /// Two mappings, deltas applied. `MAAA` puts us at col 6,
    /// `,MAAA` then adds delta 6 → col 12. Both should land at
    /// distinct LineMap keys.
    #[test]
    fn build_line_map_from_v3_multiple_mappings_same_line() {
        let json =
            r#"{"version":3,"sources":["a.kt"],"names":[],"mappings":"MAAA,MAAA"}"#;
        let line_map = build_line_map_from_v3(json).unwrap();
        assert_eq!(line_map.len(), 2);
        let offsets: Vec<u64> = line_map.keys().copied().collect();
        assert_eq!(offsets, vec![6, 12]);
    }

    /// Plainly non-JSON input is an error. The `sourcemap` crate
    /// is permissive about partial source maps (missing optional
    /// fields), so we only assert the hard-error case here.
    #[test]
    fn build_line_map_rejects_invalid_json() {
        assert!(build_line_map_from_v3("not json").is_err());
    }

    #[test]
    fn extract_source_mapping_url_absent_returns_none() {
        // Minimal valid wasm (4-byte magic + 4-byte version), no
        // custom sections.
        let wasm = b"\x00asm\x01\x00\x00\x00";
        assert_eq!(extract_source_mapping_url(wasm), None);
    }

    #[test]
    fn extract_source_mapping_url_reads_custom_section() {
        // Hand-build a minimal wasm with a `sourceMappingURL`
        // custom section carrying "leap.wasm.map".
        let url = b"leap.wasm.map";
        let name = b"sourceMappingURL";

        // Custom-section payload: name length (LEB128) + name bytes
        // + url bytes.
        let mut payload = Vec::new();
        payload.push(u8::try_from(name.len()).unwrap()); // single-byte LEB for len < 128
        payload.extend_from_slice(name);
        payload.extend_from_slice(url);

        let mut wasm = Vec::new();
        wasm.extend_from_slice(b"\x00asm\x01\x00\x00\x00");
        wasm.push(0); // custom section id
        wasm.push(u8::try_from(payload.len()).unwrap()); // single-byte LEB section size
        wasm.extend_from_slice(&payload);

        assert_eq!(
            extract_source_mapping_url(&wasm).as_deref(),
            Some("leap.wasm.map")
        );
    }
}
