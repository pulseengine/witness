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
//! - ~~Per-target `br_table` decision reconstruction~~ — landed in
//!   v0.9.7. `BrTableTarget` + `BrTableDefault` entries that share a
//!   `(function, file, line)` key are now grouped into a single
//!   `Decision`. The shape isn't classical Boolean MC/DC (a br_table
//!   has N+1 mutually-exclusive arms, not a short-circuit chain), but
//!   the truth-table view + per-arm hit counts give reviewers the same
//!   "every arm must be exercised" picture for `match` expressions.
//! - Cross-function inlining. If a source decision is inlined from
//!   another function, we reconstruct it relative to the inlined
//!   location, not the call-site. v0.5 with `DW_TAG_inlined_subroutine`
//!   handling.

use crate::Result;
use crate::instrument::{BranchEntry, BranchKind, Decision, InlineContext};
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

    // v0.12.0 — inline-context map: byte_offset → InlineContext. Built
    // by walking the DIE tree for `DW_TAG_inlined_subroutine` entries
    // and recording each entry's `(call_file, call_line)` against its
    // address range. Empty map (no inlines or DWARF reader error) is
    // a no-op — `group_into_decisions` falls back to v0.11 keying.
    let inline_map = build_inline_map(&dwarf_sections).unwrap_or_default();

    Ok(group_into_decisions(branches, &line_map, &inline_map))
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

/// Maximum source-line span allowed within a single decision cluster.
///
/// v0.6.1 shipped same-line-only grouping which worked for predicates
/// rustc lowers onto a single line (e.g. `(a && b) || c` returned as a
/// one-line expression — leap_year). It missed multi-line short-circuit
/// chains where each `&&` operand is on its own source line — a common
/// shape for safety-critical guards (`state_guard`, `safety_envelope`).
///
/// v0.6.2 relaxes the criterion: br_ifs in the same `(function, file)`
/// whose source lines fall within `MAX_DECISION_LINE_SPAN` of each
/// other cluster into one Decision. The threshold is intentionally
/// permissive — the alternative (require strict adjacency) would miss
/// blank lines and inline comments inside a chain. False-grouping of
/// genuinely separate decisions is bounded because the cluster
/// terminates as soon as a br_if appears outside the line window.
pub const MAX_DECISION_LINE_SPAN: u32 = 10;

/// Walk a manifest's branches and group `BrIf` entries that share a
/// `(function_index, file)` and have source lines within
/// `MAX_DECISION_LINE_SPAN` into a `Decision`. Other kinds (`IfThen` /
/// `IfElse` / `BrTableTarget` / `BrTableDefault`) are not grouped —
/// they have natural per-arm semantics that don't compose into MC/DC
/// decisions the same way `br_if` chains do.
///
/// Clustering walks branches in branch-id order (= source-walk
/// emission order from `walk_collect`), which preserves the
/// short-circuit chain's natural sequence. A new cluster starts when
/// the next br_if's line falls outside the current cluster's span.
fn group_into_decisions(
    branches: &[BranchEntry],
    line_map: &LineMap,
    inline_map: &InlineMap,
) -> Vec<Decision> {
    // Step 1: resolve each br_if's (function, file, line, inline_context).
    // v0.12.0 — inline_context discriminates Decisions whose source
    // location matches but whose calling site differs (e.g. a single
    // `is_safe()` predicate inlined twice in `validate()` at different
    // lines). Drop entries missing byte_offset or with no DWARF
    // mapping; pass `None` for inline_context when not in any
    // inlined range (top-level br_if).
    type Resolved<'a> = (u32, String, u32, Option<InlineContext>, &'a BranchEntry);
    let mut resolved: Vec<Resolved<'_>> = Vec::new();
    let mut brtable_resolved: Vec<Resolved<'_>> = Vec::new();
    for entry in branches {
        let Some(byte_offset) = entry.byte_offset else {
            continue;
        };
        let addr = u64::from(byte_offset);
        let Some(loc) = lookup_line(line_map, addr) else {
            continue;
        };
        let inline_ctx = lookup_inline(inline_map, addr).cloned();
        match entry.kind {
            BranchKind::BrIf => {
                resolved.push((
                    entry.function_index,
                    loc.file.clone(),
                    loc.line,
                    inline_ctx,
                    entry,
                ));
            }
            // v0.9.7 — br_table entries from the same match arm
            // grouped by (function, file, line). All targets of one
            // wasm br_table instruction share a source line in
            // hand-written code; macro-generated tables may merge with
            // adjacent code, which is the same trade-off `BrIf` makes.
            BranchKind::BrTableTarget | BranchKind::BrTableDefault => {
                brtable_resolved.push((
                    entry.function_index,
                    loc.file.clone(),
                    loc.line,
                    inline_ctx,
                    entry,
                ));
            }
            // IfThen / IfElse counters are emitted by the if/else
            // lowering and don't represent independent conditions in
            // the source-MC/DC sense. They're already consumed by the
            // BrIf path's per-decision aggregation when present.
            BranchKind::IfThen | BranchKind::IfElse => {}
        }
    }

    // Step 2: bucket by (function, file, inline_context). v0.12.0 —
    // adding inline_context here is the load-bearing change. Two
    // sets of br_ifs from distinct inlined call sites no longer
    // collapse into one Decision; each call site gets its own
    // bucket and its own pair-finding scope. Within each bucket,
    // br_ifs are in branch-id order (= source-walk order from
    // `walk_collect`).
    type BrIfBucketKey = (u32, String, Option<InlineContext>);
    type BrIfBucketEntry<'a> = Vec<(u32, &'a BranchEntry)>;
    let mut by_func_file: BTreeMap<BrIfBucketKey, BrIfBucketEntry<'_>> = BTreeMap::new();
    for (func, file, line, inline_ctx, entry) in resolved {
        by_func_file
            .entry((func, file, inline_ctx))
            .or_default()
            .push((line, entry));
    }

    let mut out: Vec<Decision> = Vec::new();
    let mut next_decision_id: u32 = 0;

    for ((_func, file, inline_ctx), entries) in by_func_file {
        let mut cluster: Vec<&BranchEntry> = Vec::new();
        let mut cluster_min: u32 = u32::MAX;
        let mut cluster_max: u32 = 0;

        let flush = |cluster: &mut Vec<&BranchEntry>,
                     cluster_min: u32,
                     next_decision_id: &mut u32,
                     out: &mut Vec<Decision>,
                     file: &str,
                     inline_ctx: &Option<InlineContext>| {
            if cluster.len() >= 2 {
                let conditions: Vec<u32> = cluster.iter().map(|e| e.id).collect();
                out.push(Decision {
                    id: *next_decision_id,
                    conditions,
                    source_file: if file.is_empty() {
                        None
                    } else {
                        Some(file.to_string())
                    },
                    source_line: Some(cluster_min),
                    // v0.8: chain_kind is filled in by a later pass that
                    // inspects the wasm at each br_if site. Default
                    // ChainKind::Unknown here keeps decisions.rs free of
                    // walrus dependencies.
                    chain_kind: crate::instrument::ChainKind::default(),
                    inline_context: inline_ctx.clone(),
                });
                *next_decision_id = next_decision_id.saturating_add(1);
            }
            cluster.clear();
        };

        for (line, entry) in entries {
            if cluster.is_empty() {
                cluster.push(entry);
                cluster_min = line;
                cluster_max = line;
                continue;
            }
            let candidate_min = cluster_min.min(line);
            let candidate_max = cluster_max.max(line);
            if candidate_max.saturating_sub(candidate_min) <= MAX_DECISION_LINE_SPAN {
                cluster.push(entry);
                cluster_min = candidate_min;
                cluster_max = candidate_max;
            } else {
                flush(
                    &mut cluster,
                    cluster_min,
                    &mut next_decision_id,
                    &mut out,
                    &file,
                    &inline_ctx,
                );
                cluster.push(entry);
                cluster_min = line;
                cluster_max = line;
            }
        }
        flush(
            &mut cluster,
            cluster_min,
            &mut next_decision_id,
            &mut out,
            &file,
            &inline_ctx,
        );
    }

    // v0.9.7 — second pass: group BrTable* entries by (function, file,
    // line). v0.12.0 — inline_context joins the key so the same
    // `match` expression inlined at multiple call sites within one
    // caller produces multiple Decisions (one per call site), each
    // with its own per-arm hit counts. Single-arm tables (= unreachable
    // default-only or 1-arm) are emitted only when they contain >= 2
    // entries, mirroring the BrIf threshold.
    let mut by_func_file_line: BTreeMap<
        (u32, String, u32, Option<InlineContext>),
        Vec<&BranchEntry>,
    > = BTreeMap::new();
    for (func, file, line, inline_ctx, entry) in brtable_resolved {
        by_func_file_line
            .entry((func, file, line, inline_ctx))
            .or_default()
            .push(entry);
    }
    for ((_func, file, line, inline_ctx), entries) in by_func_file_line {
        if entries.len() < 2 {
            continue;
        }
        let conditions: Vec<u32> = entries.iter().map(|e| e.id).collect();
        out.push(Decision {
            id: next_decision_id,
            conditions,
            source_file: if file.is_empty() { None } else { Some(file) },
            source_line: Some(line),
            // br_table arms aren't a short-circuit chain — set chain_kind
            // to `Unknown` so downstream reporters skip the
            // derive-outcome-from-conditions path. Per-arm counters are
            // the truth-table view here.
            chain_kind: crate::instrument::ChainKind::default(),
            inline_context: inline_ctx,
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

/// v0.12.0 — interval-style map of inlined-subroutine address ranges
/// to their call site. Keyed by `low_pc` (the start of the inlined
/// subroutine's address range); the value carries `high_pc` (the
/// exclusive end) plus the call site `(call_file, call_line)`. A
/// `BTreeMap` keyed on `low_pc` lets us range-query the largest entry
/// whose `low_pc <= addr`, then test `addr < high_pc` for membership.
type InlineMap = BTreeMap<u64, InlineEntry>;

#[derive(Debug, Clone)]
struct InlineEntry {
    high_pc_exclusive: u64,
    context: InlineContext,
}

/// Walk the DIE tree of every compilation unit and collect every
/// `DW_TAG_inlined_subroutine` entry's address range + call site.
/// Returns the empty map on any DWARF reader error (back-compat —
/// the caller treats it as "no inlines detected").
///
/// v0.12.0 supports the simple form: `DW_AT_low_pc` + `DW_AT_high_pc`
/// (with high_pc as offset relative to low_pc, the usual rustc
/// emission). Inlined entries that use `DW_AT_ranges` (multi-range
/// scattered inlines, less common) are skipped silently — those
/// fall back to the v0.11 conflated-decision behaviour, no
/// regression. v0.13's per-context row tagging (Variant B) will
/// pick those up via DW_AT_ranges traversal.
fn build_inline_map(sections: &DwarfSections<'_>) -> std::result::Result<InlineMap, gimli::Error> {
    let dwarf = build_dwarf(sections);
    let mut units = dwarf.units();
    let mut out: InlineMap = BTreeMap::new();

    while let Some(header) = units.next()? {
        let unit = dwarf.unit(header)?;
        let unit_ref = unit.unit_ref(&dwarf);

        // Build the unit-local file table once per unit so DW_AT_call_file
        // (a line-program file index) can be resolved to a path string.
        let unit_files = collect_unit_files(&unit_ref);

        let mut entries = unit.entries();
        while let Some((_depth, entry)) = entries.next_dfs()? {
            if entry.tag() != gimli::constants::DW_TAG_inlined_subroutine {
                continue;
            }

            let mut low_pc: Option<u64> = None;
            let mut high_pc_form: Option<HighPcForm> = None;
            let mut call_file_idx: Option<u64> = None;
            let mut call_line: Option<u64> = None;

            let mut attrs = entry.attrs();
            while let Some(attr) = attrs.next()? {
                match attr.name() {
                    gimli::constants::DW_AT_low_pc => {
                        if let gimli::AttributeValue::Addr(a) = attr.value() {
                            low_pc = Some(a);
                        }
                    }
                    gimli::constants::DW_AT_high_pc => {
                        // Two encodings only: absolute address or
                        // offset-from-low_pc. Other DWARF attribute
                        // forms aren't legal for DW_AT_high_pc per
                        // the standard, so silently ignore them.
                        #[allow(clippy::wildcard_enum_match_arm)]
                        match attr.value() {
                            gimli::AttributeValue::Addr(a) => {
                                high_pc_form = Some(HighPcForm::Addr(a));
                            }
                            gimli::AttributeValue::Udata(d) => {
                                high_pc_form = Some(HighPcForm::Offset(d));
                            }
                            _ => {}
                        }
                    }
                    gimli::constants::DW_AT_call_file => {
                        if let Some(idx) = attr.udata_value() {
                            call_file_idx = Some(idx);
                        }
                    }
                    gimli::constants::DW_AT_call_line => {
                        if let Some(n) = attr.udata_value() {
                            call_line = Some(n);
                        }
                    }
                    _ => {}
                }
            }

            // Need both endpoints + a call line to be useful for
            // decision-key splitting. Without `call_line` the
            // inlined entry can't act as a discriminator.
            let (Some(lo), Some(hpc), Some(line)) = (low_pc, high_pc_form, call_line) else {
                continue;
            };
            let hi_excl = match hpc {
                HighPcForm::Addr(a) => a,
                HighPcForm::Offset(off) => lo.saturating_add(off),
            };
            if hi_excl <= lo {
                continue;
            }
            let line_u32 = u32::try_from(line).unwrap_or(u32::MAX);
            let call_file = call_file_idx
                .and_then(|idx| usize::try_from(idx).ok())
                .and_then(|idx| unit_files.get(idx).cloned());

            // BTreeMap keyed on low_pc; later inlines at the same
            // low_pc overwrite earlier ones. Rare; if it happens
            // both ranges cover the same code so either is fine.
            // For nested inlines, the DIE walker visits children
            // after parents and OUR inner-most entry wins — which
            // matches Variant A's semantics ("innermost call site
            // discriminates").
            out.insert(
                lo,
                InlineEntry {
                    high_pc_exclusive: hi_excl,
                    context: InlineContext {
                        call_file,
                        call_line: line_u32,
                    },
                },
            );
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy)]
enum HighPcForm {
    Addr(u64),
    Offset(u64),
}

/// Collect a unit's line-program file table once, so DW_AT_call_file
/// indices on `DW_TAG_inlined_subroutine` entries can be resolved to
/// path strings. Index 0 is the unit's primary compilation file in
/// DWARF v5; v4 uses 1-based indexing. Returns a flat Vec where
/// `vec[idx]` is the path at file index `idx` (or empty string when
/// resolution fails).
fn collect_unit_files(unit_ref: &gimli::UnitRef<'_, EndianSlice<'_, LittleEndian>>) -> Vec<String> {
    let mut files: Vec<String> = Vec::new();
    let Some(lp) = unit_ref.line_program.as_ref() else {
        return files;
    };
    let header = lp.header();
    // DWARF v5 file table is 0-based; v4 is 1-based with index 0
    // representing the unit's primary file. Push entries verbatim
    // and let callers index by what they read from DW_AT_call_file.
    let file_count = header.file_names().len();
    for i in 0..=file_count {
        let path = header
            .file(u64::try_from(i).unwrap_or(0))
            .and_then(|f| {
                unit_ref
                    .attr_string(f.path_name())
                    .ok()
                    .and_then(|s| s.to_string().ok().map(str::to_owned))
            })
            .unwrap_or_default();
        files.push(path);
    }
    files
}

/// Look up the inline context for a given byte offset. Returns the
/// innermost inlined-subroutine entry whose address range covers
/// `addr`, or `None` if the address is at top-level or outside any
/// known inlined range. Pre-v0.12 behaviour (no inlines tracked)
/// falls out of an empty `InlineMap`.
fn lookup_inline(map: &InlineMap, addr: u64) -> Option<&InlineContext> {
    let (_, entry) = map.range(..=addr).next_back()?;
    if addr < entry.high_pc_exclusive {
        Some(&entry.context)
    } else {
        None
    }
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
        let decisions = group_into_decisions(&entries, &line_map, &InlineMap::new());
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
        let decisions = group_into_decisions(&entries, &line_map, &InlineMap::new());
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
        let decisions = group_into_decisions(&[a, b], &line_map, &InlineMap::new());
        // Same line, different functions → no shared decision.
        assert!(
            decisions.is_empty(),
            "two functions sharing a (file, line) is a coincidence, not a decision"
        );
    }

    /// v0.6.2 — `state_guard`-style decision: 4-cond `&&` chain whose
    /// operands rustc attributes to consecutive source lines. The
    /// adjacent-line clustering must group all four into one Decision.
    #[test]
    fn group_into_decisions_clusters_adjacent_lines() {
        let mut line_map = LineMap::new();
        line_map.insert(
            10,
            LineLocation {
                file: "lib.rs".to_string(),
                line: 23,
            },
        );
        line_map.insert(
            20,
            LineLocation {
                file: "lib.rs".to_string(),
                line: 24,
            },
        );
        line_map.insert(
            30,
            LineLocation {
                file: "lib.rs".to_string(),
                line: 25,
            },
        );
        line_map.insert(
            40,
            LineLocation {
                file: "lib.rs".to_string(),
                line: 26,
            },
        );
        let entries = vec![
            entry_with_offset(0, 10),
            entry_with_offset(1, 20),
            entry_with_offset(2, 30),
            entry_with_offset(3, 40),
        ];
        let decisions = group_into_decisions(&entries, &line_map, &InlineMap::new());
        assert_eq!(
            decisions.len(),
            1,
            "four br_ifs on lines 23-26 → one decision"
        );
        assert_eq!(decisions[0].conditions, vec![0, 1, 2, 3]);
        assert_eq!(decisions[0].source_line, Some(23));
    }

    /// v0.6.2 — gap larger than `MAX_DECISION_LINE_SPAN` separates
    /// clusters. Two `&&` chains in the same function (e.g. two
    /// independent guard expressions) must be reported as two
    /// Decisions, not merged.
    #[test]
    fn group_into_decisions_splits_on_large_gap() {
        let mut line_map = LineMap::new();
        line_map.insert(
            10,
            LineLocation {
                file: "lib.rs".to_string(),
                line: 10,
            },
        );
        line_map.insert(
            20,
            LineLocation {
                file: "lib.rs".to_string(),
                line: 11,
            },
        );
        // 50-line gap between two clusters.
        line_map.insert(
            30,
            LineLocation {
                file: "lib.rs".to_string(),
                line: 60,
            },
        );
        line_map.insert(
            40,
            LineLocation {
                file: "lib.rs".to_string(),
                line: 61,
            },
        );
        let entries = vec![
            entry_with_offset(0, 10),
            entry_with_offset(1, 20),
            entry_with_offset(2, 30),
            entry_with_offset(3, 40),
        ];
        let decisions = group_into_decisions(&entries, &line_map, &InlineMap::new());
        assert_eq!(
            decisions.len(),
            2,
            "two clusters separated by a 49-line gap"
        );
        assert_eq!(decisions[0].conditions, vec![0, 1]);
        assert_eq!(decisions[1].conditions, vec![2, 3]);
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

    /// v0.12.0 — the load-bearing test: two pairs of br_ifs whose
    /// source location is identical (same file, same line, same
    /// function) but whose inline_context differs (different
    /// `(call_file, call_line)`) must produce TWO Decisions, not
    /// one. This is the exact shape that motivates the v0.12.0
    /// rework: a single source predicate inlined twice in one caller.
    #[test]
    fn group_into_decisions_splits_by_inline_context() {
        // Source predicate at lib.rs:42, br_ifs at offsets {10, 20}
        // for call site A, {30, 40} for call site B.
        let mut line_map = LineMap::new();
        for off in [10u64, 20, 30, 40] {
            line_map.insert(
                off,
                LineLocation {
                    file: "lib.rs".to_string(),
                    line: 42,
                },
            );
        }
        // Inline map: offsets [10, 20) inlined from caller.rs:5;
        // offsets [30, 40) inlined from caller.rs:10. Note ranges
        // are right-open (high_pc_exclusive).
        let mut inline_map = InlineMap::new();
        inline_map.insert(
            10,
            InlineEntry {
                high_pc_exclusive: 25,
                context: InlineContext {
                    call_file: Some("caller.rs".to_string()),
                    call_line: 5,
                },
            },
        );
        inline_map.insert(
            30,
            InlineEntry {
                high_pc_exclusive: 45,
                context: InlineContext {
                    call_file: Some("caller.rs".to_string()),
                    call_line: 10,
                },
            },
        );
        let entries = vec![
            entry_with_offset(0, 10),
            entry_with_offset(1, 20),
            entry_with_offset(2, 30),
            entry_with_offset(3, 40),
        ];
        let decisions = group_into_decisions(&entries, &line_map, &inline_map);
        assert_eq!(
            decisions.len(),
            2,
            "two distinct inline contexts → two Decisions, not one (got {decisions:?})"
        );
        // Both Decisions land at the same source line (the inlined
        // predicate's line) but carry distinct inline_context values.
        let lines: Vec<u32> = decisions.iter().filter_map(|d| d.source_line).collect();
        assert_eq!(lines, vec![42, 42], "both at lib.rs:42");
        let call_lines: Vec<u32> = decisions
            .iter()
            .filter_map(|d| d.inline_context.as_ref().map(|ic| ic.call_line))
            .collect();
        // BTreeMap sort order: call_line 5 < call_line 10.
        assert_eq!(
            call_lines,
            vec![5, 10],
            "decisions discriminated by their inline call_line"
        );
        // Each Decision contains exactly the two br_ifs from one
        // inlined call site.
        assert_eq!(decisions[0].conditions, vec![0, 1]);
        assert_eq!(decisions[1].conditions, vec![2, 3]);
    }

    /// Negative control: when the same predicate is reached from
    /// the SAME inline context (or top-level / no inlines), the
    /// existing v0.11 conflation behaviour holds — one Decision
    /// per cluster. This protects against the v0.12.0 split being
    /// over-eager and breaking back-compat.
    #[test]
    fn group_into_decisions_keeps_single_when_no_inline_context() {
        let mut line_map = LineMap::new();
        for off in [10u64, 20, 30, 40] {
            line_map.insert(
                off,
                LineLocation {
                    file: "lib.rs".to_string(),
                    line: 42,
                },
            );
        }
        // No inlines at all — InlineMap empty.
        let entries = vec![
            entry_with_offset(0, 10),
            entry_with_offset(1, 20),
            entry_with_offset(2, 30),
            entry_with_offset(3, 40),
        ];
        let decisions = group_into_decisions(&entries, &line_map, &InlineMap::new());
        assert_eq!(
            decisions.len(),
            1,
            "no inline context → all conditions cluster (v0.11 behaviour preserved)"
        );
        assert_eq!(decisions[0].conditions, vec![0, 1, 2, 3]);
        assert!(
            decisions[0].inline_context.is_none(),
            "Decision.inline_context absent when no inlines detected"
        );
    }
}
