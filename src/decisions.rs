//! DWARF-grounded reconstruction of source-level decisions from Wasm
//! `br_if` sequences.
//!
//! # v0.4 implementation (lifted from v0.2.1 plan)
//!
//! 1. Walk the input Wasm via `wasmparser` and extract every custom
//!    section starting with `.debug_` — these are the DWARF sections
//!    Rust embeds when compiled with debug info.
//! 2. Hand them to `gimli` to build a `(byte_offset_within_function_body)
//!    -> (file_path, line)` map per function.
//! 3. For each `br_if` `BranchEntry` whose `byte_offset` is populated,
//!    look up the source line.
//! 4. Group adjacent `br_if`s in the same function that share the same
//!    `(file, line)` into a [`Decision`].
//! 5. Strict per-`br_if` fallback when DWARF is absent or sparse:
//!    return an empty `Vec`, leaving the manifest's strict-per-branch
//!    semantics in effect.
//!
//! # What this implementation does NOT do (v0.5+ work)
//!
//! - Macro-expansion disambiguation. Two `br_if`s on the same source
//!   line that came from different macro expansions are grouped into
//!   one `Decision` here. Distinguishing requires `DW_AT_call_file` /
//!   `DW_AT_call_line` chasing through inlined-subroutine entries; v0.4
//!   uses the *innermost* line, which is correct for hand-written
//!   short-circuit chains and merges over-eagerly when the same line
//!   spawns multiple decisions via macro magic.
//! - Per-target `br_table` decision reconstruction. `BrTableTarget` /
//!   `BrTableDefault` entries are not grouped into Decisions in v0.4;
//!   they remain strict-per-target.
//! - Cross-function inlining. If a source decision is inlined from
//!   another function, we reconstruct it relative to the inlined
//!   location, not the call-site. v0.5 with `DW_TAG_inlined_subroutine`
//!   handling.

use crate::Result;
use crate::instrument::{BranchEntry, BranchKind, Decision};
use gimli::{EndianSlice, LittleEndian};
use std::collections::BTreeMap;

/// Reconstruct source-level decisions from a branch manifest using DWARF.
///
/// Returns an empty vector when the input has no DWARF custom sections,
/// when no `br_if` entries have a `byte_offset`, or when the
/// reconstruction phase yields no multi-condition groups (i.e. every
/// surviving group is a single condition — strict per-`br_if` is the
/// correct interpretation, no need to materialise a `Decision`).
pub fn reconstruct_decisions(wasm_bytes: &[u8], branches: &[BranchEntry]) -> Result<Vec<Decision>> {
    let dwarf_sections = match extract_dwarf_sections(wasm_bytes) {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };

    let line_map = match build_line_map(&dwarf_sections) {
        Ok(m) => m,
        Err(_) => return Ok(Vec::new()),
    };
    if line_map.is_empty() {
        return Ok(Vec::new());
    }

    Ok(group_into_decisions(branches, &line_map))
}

/// DWARF custom sections lifted out of a Wasm module.
#[derive(Default, Debug)]
struct DwarfSections<'a> {
    debug_abbrev: &'a [u8],
    debug_info: &'a [u8],
    debug_line: &'a [u8],
    debug_str: &'a [u8],
    debug_line_str: &'a [u8],
    debug_str_offsets: &'a [u8],
    debug_addr: &'a [u8],
    debug_rnglists: &'a [u8],
    debug_loclists: &'a [u8],
}

fn extract_dwarf_sections(wasm_bytes: &[u8]) -> Option<DwarfSections<'_>> {
    let parser = wasmparser::Parser::new(0);
    let mut out = DwarfSections::default();
    let mut found_any = false;

    for payload in parser.parse_all(wasm_bytes) {
        let payload = payload.ok()?;
        if let wasmparser::Payload::CustomSection(section) = payload {
            let name = section.name();
            let data = section.data();
            // SAFETY-REVIEW: matching only known DWARF section names; an
            // unknown debug_* name is silently ignored, which is
            // forward-compatible with future DWARF revisions.
            #[allow(clippy::wildcard_enum_match_arm)]
            match name {
                ".debug_abbrev" => {
                    out.debug_abbrev = data;
                    found_any = true;
                }
                ".debug_info" => {
                    out.debug_info = data;
                    found_any = true;
                }
                ".debug_line" => {
                    out.debug_line = data;
                    found_any = true;
                }
                ".debug_str" => {
                    out.debug_str = data;
                    found_any = true;
                }
                ".debug_line_str" => {
                    out.debug_line_str = data;
                    found_any = true;
                }
                ".debug_str_offsets" => {
                    out.debug_str_offsets = data;
                    found_any = true;
                }
                ".debug_addr" => {
                    out.debug_addr = data;
                    found_any = true;
                }
                ".debug_rnglists" => {
                    out.debug_rnglists = data;
                    found_any = true;
                }
                ".debug_loclists" => {
                    out.debug_loclists = data;
                    found_any = true;
                }
                _ => {}
            }
        }
    }

    if found_any { Some(out) } else { None }
}

/// `address` here is the raw address from DWARF's line program. For Wasm
/// modules compiled by rustc + LLVM, DWARF addresses are byte offsets
/// into the *Code section* of the Wasm module — not absolute file
/// offsets, not offsets relative to a function body's start.
type LineMap = BTreeMap<u64, LineLocation>;

#[derive(Debug, Clone)]
struct LineLocation {
    file: String,
    line: u32,
}

fn build_line_map(sections: &DwarfSections<'_>) -> std::result::Result<LineMap, gimli::Error> {
    let dwarf = build_dwarf(sections);
    let mut units = dwarf.units();
    let mut out: LineMap = BTreeMap::new();

    while let Some(header) = units.next()? {
        let unit = dwarf.unit(header)?;
        let unit_ref = unit.unit_ref(&dwarf);
        let lp = match unit_ref.line_program.clone() {
            Some(p) => p,
            None => continue,
        };
        let mut rows = lp.rows();
        while let Some((header, row)) = rows.next_row()? {
            if row.end_sequence() {
                continue;
            }
            let line = match row.line() {
                Some(n) => u32::try_from(n.get()).unwrap_or(u32::MAX),
                None => continue,
            };
            let file = match row.file(header) {
                Some(entry) => unit_ref
                    .attr_string(entry.path_name())
                    .ok()
                    .and_then(|s| s.to_string().ok().map(str::to_owned))
                    .unwrap_or_default(),
                None => String::new(),
            };
            out.insert(row.address(), LineLocation { file, line });
        }
    }
    Ok(out)
}

fn build_dwarf<'a>(s: &DwarfSections<'a>) -> gimli::Dwarf<EndianSlice<'a, LittleEndian>> {
    let endian = LittleEndian;
    gimli::Dwarf {
        debug_abbrev: gimli::DebugAbbrev::new(s.debug_abbrev, endian),
        debug_addr: gimli::DebugAddr::from(EndianSlice::new(s.debug_addr, endian)),
        debug_aranges: gimli::DebugAranges::new(&[], endian),
        debug_info: gimli::DebugInfo::new(s.debug_info, endian),
        debug_line: gimli::DebugLine::new(s.debug_line, endian),
        debug_line_str: gimli::DebugLineStr::from(EndianSlice::new(s.debug_line_str, endian)),
        debug_str: gimli::DebugStr::new(s.debug_str, endian),
        debug_str_offsets: gimli::DebugStrOffsets::from(EndianSlice::new(
            s.debug_str_offsets,
            endian,
        )),
        debug_types: gimli::DebugTypes::new(&[], endian),
        locations: gimli::LocationLists::new(
            gimli::DebugLoc::new(&[], endian),
            gimli::DebugLocLists::new(s.debug_loclists, endian),
        ),
        ranges: gimli::RangeLists::new(
            gimli::DebugRanges::new(&[], endian),
            gimli::DebugRngLists::new(s.debug_rnglists, endian),
        ),
        file_type: gimli::DwarfFileType::Main,
        sup: None,
        abbreviations_cache: gimli::AbbreviationsCache::new(),
    }
}

/// Walk a manifest's branches and group adjacent `BrIf` entries that
/// share a `(function_index, file, line)` key into one `Decision`. Other
/// kinds (`IfThen` / `IfElse` / `BrTableTarget` / `BrTableDefault`) are
/// not grouped — they have natural per-arm semantics that don't compose
/// into MC/DC decisions the same way `br_if` chains do.
fn group_into_decisions(branches: &[BranchEntry], line_map: &LineMap) -> Vec<Decision> {
    let mut out: Vec<Decision> = Vec::new();
    let mut next_decision_id: u32 = 0;

    let mut groups: BTreeMap<(u32, String, u32), Vec<&BranchEntry>> = BTreeMap::new();
    for entry in branches {
        if entry.kind != BranchKind::BrIf {
            continue;
        }
        let Some(byte_offset) = entry.byte_offset else {
            continue;
        };
        let Some(loc) = lookup_line(line_map, u64::from(byte_offset)) else {
            continue;
        };
        groups
            .entry((entry.function_index, loc.file.clone(), loc.line))
            .or_default()
            .push(entry);
    }

    for ((_func, file, line), members) in groups {
        if members.len() < 2 {
            // Single-condition decisions are not worth materialising —
            // strict-per-br_if already covers them, and emitting a
            // one-element Decision adds no information.
            continue;
        }
        let conditions: Vec<u32> = members.iter().map(|e| e.id).collect();
        out.push(Decision {
            id: next_decision_id,
            conditions,
            source_file: if file.is_empty() { None } else { Some(file) },
            source_line: Some(line),
        });
        next_decision_id = next_decision_id.saturating_add(1);
    }
    out
}

/// DWARF line-table addresses are sparse — a row at address X applies
/// until the next row at address Y. So a query for an arbitrary byte
/// offset uses the largest row address less than or equal to the query.
fn lookup_line(map: &LineMap, addr: u64) -> Option<&LineLocation> {
    map.range(..=addr).next_back().map(|(_, v)| v)
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
    use crate::instrument::BranchKind;

    fn entry_with_offset(id: u32, byte_offset: u32) -> BranchEntry {
        BranchEntry {
            id,
            function_index: 0,
            function_name: None,
            kind: BranchKind::BrIf,
            instr_index: id,
            target_index: None,
            byte_offset: Some(byte_offset),
            seq_debug: format!("Id {{ idx: {id} }}"),
        }
    }

    #[test]
    fn empty_module_yields_no_decisions() {
        let entries = vec![entry_with_offset(0, 0), entry_with_offset(1, 4)];
        let decisions = reconstruct_decisions(b"\x00asm\x01\x00\x00\x00", &entries).unwrap();
        assert!(
            decisions.is_empty(),
            "no DWARF sections → strict-per-br_if fallback (empty Decision list)"
        );
    }

    #[test]
    fn extract_dwarf_returns_none_on_dwarf_free_module() {
        let result = extract_dwarf_sections(b"\x00asm\x01\x00\x00\x00");
        assert!(
            result.is_none(),
            "minimal valid Wasm with no custom sections → no DWARF"
        );
    }

    /// Group two artificial entries with the same (file, line) into a
    /// single Decision. Direct test of `group_into_decisions` so we don't
    /// require an actual DWARF input here — that's covered by an
    /// integration test once the Rust→Wasm fixture builds with debug=2.
    #[test]
    fn group_into_decisions_merges_same_line() {
        let mut line_map = LineMap::new();
        line_map.insert(
            10,
            LineLocation {
                file: "lib.rs".to_string(),
                line: 42,
            },
        );
        line_map.insert(
            20,
            LineLocation {
                file: "lib.rs".to_string(),
                line: 42,
            },
        );
        let entries = vec![entry_with_offset(0, 10), entry_with_offset(1, 20)];
        let decisions = group_into_decisions(&entries, &line_map);
        assert_eq!(
            decisions.len(),
            1,
            "two conditions on line 42 → one decision"
        );
        assert_eq!(decisions[0].conditions, vec![0, 1]);
        assert_eq!(decisions[0].source_line, Some(42));
        assert_eq!(decisions[0].source_file.as_deref(), Some("lib.rs"));
    }

    #[test]
    fn group_into_decisions_skips_singletons() {
        let mut line_map = LineMap::new();
        line_map.insert(
            10,
            LineLocation {
                file: "lib.rs".to_string(),
                line: 42,
            },
        );
        line_map.insert(
            20,
            LineLocation {
                file: "lib.rs".to_string(),
                line: 99,
            },
        );
        let entries = vec![entry_with_offset(0, 10), entry_with_offset(1, 20)];
        let decisions = group_into_decisions(&entries, &line_map);
        assert!(
            decisions.is_empty(),
            "two singletons on different lines → no Decision (strict-per-br_if applies)"
        );
    }

    #[test]
    fn group_into_decisions_separates_by_function() {
        let mut line_map = LineMap::new();
        line_map.insert(
            10,
            LineLocation {
                file: "lib.rs".to_string(),
                line: 42,
            },
        );
        line_map.insert(
            20,
            LineLocation {
                file: "lib.rs".to_string(),
                line: 42,
            },
        );
        let mut a = entry_with_offset(0, 10);
        a.function_index = 0;
        let mut b = entry_with_offset(1, 20);
        b.function_index = 1;
        let decisions = group_into_decisions(&[a, b], &line_map);
        // Same line, different functions → no shared decision.
        assert!(
            decisions.is_empty(),
            "two functions sharing a (file, line) is a coincidence, not a decision"
        );
    }

    #[test]
    fn lookup_line_uses_largest_lt_eq() {
        let mut map = LineMap::new();
        map.insert(
            10,
            LineLocation {
                file: "a".to_string(),
                line: 1,
            },
        );
        map.insert(
            30,
            LineLocation {
                file: "b".to_string(),
                line: 2,
            },
        );
        // Below the first entry → None.
        assert!(lookup_line(&map, 5).is_none());
        // Exactly at an entry → that entry.
        assert_eq!(lookup_line(&map, 10).unwrap().line, 1);
        // Between entries → the earlier one (DWARF range semantics).
        assert_eq!(lookup_line(&map, 20).unwrap().line, 1);
        // After all entries → the last one.
        assert_eq!(lookup_line(&map, 100).unwrap().line, 2);
    }
}
