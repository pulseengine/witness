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
/// How the source-line attribution that produced this result was
/// obtained. Populated by `reconstruct_decisions` so downstream
/// reporters (and reviewers reading manifests) can tell whether the
/// `(file, line)` labels come from DWARF — with its full structural
/// signal (`DW_AT_ranges`, `DW_TAG_inlined_subroutine` chains) — or
/// from a weaker source-map fallback, or are absent entirely.
///
/// `SourceMapV3` is reserved for an upcoming feature
/// (`docs/research/source-map-ingestion.md`) and is not yet produced
/// by any code path; this commit lays the type in place ahead of
/// the V3 reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttributionSource {
    /// No source-line attribution available — neither DWARF nor a
    /// source map was found. Decisions, if any, carry no
    /// `source_file` / `source_line`.
    #[default]
    None,
    /// Built from DWARF `.debug_line` rows. Includes inline-chain
    /// tracking (`DW_TAG_inlined_subroutine`) and range info
    /// (`DW_AT_ranges`).
    Dwarf,
    /// Built from a V3 source map. Line-only — no inline chains
    /// (V3 has no equivalent of `DW_TAG_inlined_subroutine`) and no
    /// address ranges.
    SourceMapV3,
}

/// What `reconstruct_decisions` returns: the clustered decisions
/// plus per-branch inline metadata, tagged with the attribution
/// source so reporters know what the source-line labels mean.
#[derive(Debug, Default, Clone)]
pub struct ReconstructionResult {
    pub decisions: Vec<Decision>,
    pub branch_inline_contexts: BTreeMap<u32, InlineContext>,
    pub branch_inline_chains: BTreeMap<u32, Vec<InlineContext>>,
    pub attribution_source: AttributionSource,
}

pub fn reconstruct_decisions(
    wasm_bytes: &[u8],
    branches: &[BranchEntry],
) -> Result<ReconstructionResult> {
    let dwarf_sections = match extract_dwarf_sections(wasm_bytes) {
        Some(s) => s,
        None => return Ok(ReconstructionResult::default()),
    };

    let line_map = match build_line_map(&dwarf_sections) {
        Ok(m) => m,
        Err(_) => return Ok(ReconstructionResult::default()),
    };
    if line_map.is_empty() {
        return Ok(ReconstructionResult::default());
    }

    // v0.13.0 — Variant B: re-enable inline-map construction (v0.12.1
    // had gated this to an empty map after the v0.12.0 split-by-
    // context regression). v0.13 doesn't use the inline_map as a
    // bucket-key discriminator (that was Variant A's mistake);
    // instead it threads per-branch InlineContext through to the
    // manifest so the runner can stamp each row's inline_context tag
    // and the mcdc-v2 reporter can build per-context verdict views
    // *within* unified single-Decision clusters.
    //
    // v0.14.0 — also surfaces the full inline call CHAIN (Vec) per
    // branch, alongside the single-hop leaf context. The chain
    // captures multi-level inlines (e.g. `is_safe()` inlined into
    // `validate()` itself inlined into `audit()` produces a chain
    // of length 2). v3 mcdc envelopes ship the chain; v2 keeps the
    // single-hop view byte-identical.
    let inline_map = build_inline_map(&dwarf_sections).unwrap_or_default();

    let (decisions, branch_inline_contexts, branch_inline_chains) =
        group_into_decisions(branches, &line_map, &inline_map);
    Ok(ReconstructionResult {
        decisions,
        branch_inline_contexts,
        branch_inline_chains,
        attribution_source: AttributionSource::Dwarf,
    })
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
///
/// `pub(crate)` so the `sourcemap` module can build one from a V3
/// source map and feed it into the same clustering pass.
pub(crate) type LineMap = BTreeMap<u64, LineLocation>;

#[derive(Debug, Clone)]
pub(crate) struct LineLocation {
    pub(crate) file: String,
    pub(crate) line: u32,
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
/// Internal return shape of `group_into_decisions` — the three data
/// pieces the public `ReconstructionResult` wraps (plus an
/// `attribution_source` tag added by `reconstruct_decisions`).
type DecisionGroups = (
    Vec<Decision>,
    BTreeMap<u32, InlineContext>,
    BTreeMap<u32, Vec<InlineContext>>,
);

fn group_into_decisions(
    branches: &[BranchEntry],
    line_map: &LineMap,
    inline_map: &InlineMap,
) -> DecisionGroups {
    // Step 1: resolve each br_if's (function, file, line). Side-output
    // a per-branch `branch_id → InlineContext` map for everything
    // whose byte offset fell within an inlined-subroutine address
    // range — the runner consumes this map to stamp each row's
    // inline_context tag at execution time. v0.13's Variant B does
    // NOT use inline_context as a bucket-key discriminator (that was
    // v0.12.0's Variant A mistake); the conflated single-Decision
    // shape stays. Drop entries missing byte_offset or with no DWARF
    // mapping.
    type Resolved<'a> = (u32, String, u32, &'a BranchEntry);
    let mut resolved: Vec<Resolved<'_>> = Vec::new();
    let mut brtable_resolved: Vec<Resolved<'_>> = Vec::new();
    let mut branch_inline_contexts: BTreeMap<u32, InlineContext> = BTreeMap::new();
    // v0.14.0 — full call chain per branch, in addition to the
    // single-hop leaf context. Same keyset as branch_inline_contexts.
    let mut branch_inline_chains: BTreeMap<u32, Vec<InlineContext>> = BTreeMap::new();
    for entry in branches {
        let Some(byte_offset) = entry.byte_offset else {
            continue;
        };
        let addr = u64::from(byte_offset);
        let Some(loc) = lookup_line(line_map, addr) else {
            continue;
        };
        if let Some((ctx, chain)) = lookup_inline_with_chain(inline_map, addr) {
            branch_inline_contexts.insert(entry.id, ctx.clone());
            branch_inline_chains.insert(entry.id, chain.clone());
        }
        match entry.kind {
            // v0.19.0 — cluster `IfThen` alongside `BrIf` for
            // decision-key purposes. clang's lowering of `a && b`
            // emits `if/else` + 1 br_if per source decision (vs
            // rustc's br_if chain). The IfThen arm is the
            // "predicate was true" condition equivalent to a br_if.
            // IfElse stays excluded — it's the negation of IfThen
            // for the same branching site, double-counting it would
            // inflate the condition count.
            BranchKind::BrIf | BranchKind::IfThen => {
                resolved.push((entry.function_index, loc.file.clone(), loc.line, entry));
            }
            // v0.9.7 — br_table entries from the same match arm
            // grouped by (function, file, line). All targets of one
            // wasm br_table instruction share a source line in
            // hand-written code; macro-generated tables may merge with
            // adjacent code, which is the same trade-off `BrIf` makes.
            BranchKind::BrTableTarget | BranchKind::BrTableDefault => {
                brtable_resolved.push((entry.function_index, loc.file.clone(), loc.line, entry));
            }
            // IfElse stays out of the cluster pass; see IfThen note above.
            BranchKind::IfElse => {}
        }
    }

    // Step 2: bucket by (function, file). v0.13 reverted v0.12.0's
    // bucket-key extension — splitting by inline_context fragmented
    // v0.11's clusters into singletons that got dropped by the
    // `cluster.len() >= 2` gate. Variant B preserves the conflated
    // single-Decision shape and exposes per-context detail via the
    // row tag + per_context verdict view at the reporter layer.
    type BrIfBucketKey = (u32, String);
    type BrIfBucketEntry<'a> = Vec<(u32, &'a BranchEntry)>;
    let mut by_func_file: BTreeMap<BrIfBucketKey, BrIfBucketEntry<'_>> = BTreeMap::new();
    for (func, file, line, entry) in resolved {
        by_func_file
            .entry((func, file))
            .or_default()
            .push((line, entry));
    }

    let mut out: Vec<Decision> = Vec::new();
    let mut next_decision_id: u32 = 0;

    for ((_func, file), entries) in by_func_file {
        let mut cluster: Vec<&BranchEntry> = Vec::new();
        let mut cluster_min: u32 = u32::MAX;
        let mut cluster_max: u32 = 0;

        let flush = |cluster: &mut Vec<&BranchEntry>,
                     cluster_min: u32,
                     next_decision_id: &mut u32,
                     out: &mut Vec<Decision>,
                     file: &str,
                     branch_inline_contexts: &BTreeMap<u32, InlineContext>| {
            if cluster.len() >= 2 {
                let conditions: Vec<u32> = cluster.iter().map(|e| e.id).collect();
                // Decision.inline_context = modal context across the
                // cluster's branches. Used by reporters for the
                // headline label ("inlined from foo.rs:5") when the
                // cluster's rows are mostly one context. Mixed-
                // context clusters get `None` here; per_context
                // verdict views still surface every distinct context.
                let inline_context = modal_inline_context(&conditions, branch_inline_contexts);
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
                    inline_context,
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
                    &branch_inline_contexts,
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
            &branch_inline_contexts,
        );
    }

    // v0.9.7 — second pass: group BrTable* entries by (function,
    // file, line). v0.13 reverted v0.12.0's bucket-key extension
    // here too — same regression mode applies to br_table arms.
    // Single-arm tables (= unreachable default-only or 1-arm) are
    // emitted only when they contain >= 2 entries, mirroring the
    // BrIf threshold.
    let mut by_func_file_line: BTreeMap<(u32, String, u32), Vec<&BranchEntry>> = BTreeMap::new();
    for (func, file, line, entry) in brtable_resolved {
        by_func_file_line
            .entry((func, file, line))
            .or_default()
            .push(entry);
    }
    for ((_func, file, line), entries) in by_func_file_line {
        if entries.len() < 2 {
            continue;
        }
        let conditions: Vec<u32> = entries.iter().map(|e| e.id).collect();
        let inline_context = modal_inline_context(&conditions, &branch_inline_contexts);
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
            inline_context,
        });
        next_decision_id = next_decision_id.saturating_add(1);
    }

    (out, branch_inline_contexts, branch_inline_chains)
}

/// v0.13.0 — pick the most-common `InlineContext` across the given
/// branches' tags. `None` (top-level / no inlining) competes alongside
/// `Some(ctx)` values; whichever count wins becomes the cluster's
/// `Decision.inline_context`. Ties (including `None` ties with a
/// `Some(ctx)`) resolve to `None` so the headline label stays
/// conservative — per_context verdict views at the reporter layer
/// surface every distinct context regardless.
fn modal_inline_context(
    branches: &[u32],
    branch_inline_contexts: &BTreeMap<u32, InlineContext>,
) -> Option<InlineContext> {
    if branches.is_empty() {
        return None;
    }
    let mut counts: BTreeMap<Option<InlineContext>, u32> = BTreeMap::new();
    for &b in branches {
        let key = branch_inline_contexts.get(&b).cloned();
        let entry = counts.entry(key).or_insert(0);
        *entry = entry.saturating_add(1);
    }
    let max_count = counts.values().copied().max().unwrap_or(0);
    let winners: Vec<&Option<InlineContext>> = counts
        .iter()
        .filter(|(_, c)| **c == max_count)
        .map(|(k, _)| k)
        .collect();
    if winners.len() == 1 {
        winners.first().cloned().cloned().unwrap_or(None)
    } else {
        // Tie → conservative: no headline label.
        None
    }
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
    /// v0.14.0 — full call chain from outermost to innermost inlined
    /// frame (inclusive of `context` at the tail). When `is_safe()`
    /// is inlined inside `validate()` which is itself inlined into
    /// `audit()`, the chain at the innermost address is
    /// `[validate-call-site-in-audit, is_safe-call-site-in-validate]`.
    /// `chain.last() == Some(&context)` is an invariant: the last
    /// frame is always this entry's own call site.
    chain: Vec<InlineContext>,
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
#[allow(dead_code)] // v0.12.1 — call disabled in `reconstruct_decisions`; v0.13's Variant B reuses this.
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

        // v0.14.0 — track the parent inlined-subroutine chain via a
        // depth-keyed stack. Each entry on the stack is an outer
        // inlined call site that hasn't closed yet (the DFS cursor
        // is still inside its DIE subtree). When the cursor ascends
        // past an inline parent's depth, we pop it. When we encounter
        // a new inlined-subroutine DIE, we push it AFTER recording
        // its chain (= parents-on-stack + self).
        let mut inline_parents: Vec<(isize, InlineContext)> = Vec::new();
        let mut current_depth: isize = 0;

        let mut entries = unit.entries();
        while let Some((delta, entry)) = entries.next_dfs()? {
            current_depth = current_depth.saturating_add(delta);
            // Pop inline parents that closed (their DIE subtree is
            // behind the cursor now).
            while let Some(&(parent_depth, _)) = inline_parents.last() {
                if parent_depth >= current_depth {
                    inline_parents.pop();
                } else {
                    break;
                }
            }
            if entry.tag() != gimli::constants::DW_TAG_inlined_subroutine {
                continue;
            }

            // v0.17.0 — extract only the call-site attributes here
            // (`DW_AT_call_file` + `DW_AT_call_line`). Address
            // ranges resolve via gimli's `die_ranges`, which
            // handles BOTH the `DW_AT_low_pc + DW_AT_high_pc`
            // contiguous form AND the `DW_AT_ranges` scattered
            // form uniformly. Scattered inlines (LLVM hot/cold
            // splits, tail-merged code, cross-crate LTO) now
            // produce one `InlineEntry` per range, all sharing
            // the same chain.
            let mut call_file_idx: Option<u64> = None;
            let mut call_line: Option<u64> = None;

            let mut attrs = entry.attrs();
            while let Some(attr) = attrs.next()? {
                match attr.name() {
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

            let Some(line) = call_line else {
                continue;
            };
            let line_u32 = u32::try_from(line).unwrap_or(u32::MAX);
            let call_file = call_file_idx
                .and_then(|idx| usize::try_from(idx).ok())
                .and_then(|idx| unit_files.get(idx).cloned());

            let context = InlineContext {
                call_file,
                call_line: line_u32,
            };
            // v0.14.0 — chain = parents-on-stack + self, in
            // outermost-to-innermost order. Last frame is always
            // this entry's own context.
            let chain: Vec<InlineContext> = inline_parents
                .iter()
                .map(|(_, c)| c.clone())
                .chain(std::iter::once(context.clone()))
                .collect();

            // v0.17.0 — resolve address ranges via gimli's unified
            // API. Yields one or more `gimli::Range { begin, end }`
            // tuples. Each becomes an independent `InlineEntry`
            // sharing the same chain. Empty ranges + degenerate
            // entries (begin >= end) are skipped.
            let Ok(mut ranges) = unit_ref.die_ranges(entry) else {
                continue;
            };
            while let Some(gimli::Range { begin, end }) = ranges.next()? {
                if end <= begin {
                    continue;
                }
                // BTreeMap keyed on low_pc; later inlines at the
                // same low_pc overwrite earlier ones (rare).
                out.insert(
                    begin,
                    InlineEntry {
                        high_pc_exclusive: end,
                        context: context.clone(),
                        chain: chain.clone(),
                    },
                );
            }

            // Push self onto the parent stack so any nested inlines
            // visited deeper in the DFS see this frame as their parent.
            inline_parents.push((current_depth, context));
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // v0.12.1 — kept for v0.13's Variant B inline walker.
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
#[allow(dead_code)] // v0.12.1 — only called from build_inline_map; both kept for v0.13.
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
#[allow(dead_code)] // v0.14.0 — superseded by lookup_inline_with_chain in the prod path; kept for tests + future selectors.
fn lookup_inline(map: &InlineMap, addr: u64) -> Option<&InlineContext> {
    let (_, entry) = map.range(..=addr).next_back()?;
    if addr < entry.high_pc_exclusive {
        Some(&entry.context)
    } else {
        None
    }
}

/// v0.14.0 — chain-aware lookup. Same `addr` resolution as
/// `lookup_inline`; returns both the innermost frame (for v2 back-
/// compat) and the full chain from outermost to innermost. Callers
/// in v3-aware code paths consume the chain; v2 code paths consume
/// only the leaf via `lookup_inline`.
#[allow(dead_code)] // wired into group_into_decisions; called via the per-branch path.
fn lookup_inline_with_chain(
    map: &InlineMap,
    addr: u64,
) -> Option<(&InlineContext, &Vec<InlineContext>)> {
    let (_, entry) = map.range(..=addr).next_back()?;
    if addr < entry.high_pc_exclusive {
        Some((&entry.context, &entry.chain))
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

    fn entry_with_kind(id: u32, byte_offset: u32, kind: BranchKind) -> BranchEntry {
        BranchEntry {
            id,
            function_index: 0,
            function_name: None,
            kind,
            instr_index: id,
            target_index: None,
            byte_offset: Some(byte_offset),
            seq_debug: format!("Id {{ idx: {id} }}"),
        }
    }

    #[test]
    fn empty_module_yields_no_decisions() {
        let entries = vec![entry_with_offset(0, 0), entry_with_offset(1, 4)];
        let result =
            reconstruct_decisions(b"\x00asm\x01\x00\x00\x00", &entries).unwrap();
        assert!(
            result.decisions.is_empty(),
            "no DWARF sections → strict-per-br_if fallback (empty Decision list)"
        );
        assert_eq!(
            result.attribution_source,
            AttributionSource::None,
            "no DWARF, no source map → attribution source is None"
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
        let (decisions, _, _) = group_into_decisions(&entries, &line_map, &InlineMap::new());
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
        let (decisions, _, _) = group_into_decisions(&entries, &line_map, &InlineMap::new());
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
        let (decisions, _, _) = group_into_decisions(&[a, b], &line_map, &InlineMap::new());
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
        let (decisions, _, _) = group_into_decisions(&entries, &line_map, &InlineMap::new());
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
        let (decisions, _, _) = group_into_decisions(&entries, &line_map, &InlineMap::new());
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

    /// v0.13 (Variant B) — same input as v0.12.0's split test, but
    /// with the v0.13 expectation flipped. Two pairs of br_ifs at
    /// the same source line with distinct inline_contexts now
    /// produce **one** unified Decision (cluster preserved → no
    /// fragmentation, no singleton drops), while the per-branch
    /// `branch_inline_contexts` map carries the call-site detail
    /// the runner needs to stamp each row's `inline_context` tag.
    /// This is the load-bearing assertion for Variant B.
    #[test]
    fn cluster_preserved_with_split_inline_contexts() {
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
                chain: Vec::new(),
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
                chain: Vec::new(),
            },
        );
        let entries = vec![
            entry_with_offset(0, 10),
            entry_with_offset(1, 20),
            entry_with_offset(2, 30),
            entry_with_offset(3, 40),
        ];
        let (decisions, branch_inline_contexts, _) =
            group_into_decisions(&entries, &line_map, &inline_map);
        assert_eq!(
            decisions.len(),
            1,
            "v0.13 preserves the cluster (no v0.12 fragmentation): got {decisions:?}"
        );
        // The unified Decision spans all four br_ifs at lib.rs:42.
        assert_eq!(decisions[0].conditions, vec![0, 1, 2, 3]);
        assert_eq!(decisions[0].source_line, Some(42));
        // 2 branches in ctx call_line=5 vs 2 branches in ctx call_line=10
        // → tied modal → conservative `None` headline label. Per-row
        // detail surfaces via `branch_inline_contexts`.
        assert!(
            decisions[0].inline_context.is_none(),
            "tied 2-vs-2 modal → conservative None headline; got {:?}",
            decisions[0].inline_context
        );
        // Per-branch context map is populated for every br_if whose
        // address falls inside an inline range.
        assert_eq!(branch_inline_contexts.get(&0).map(|c| c.call_line), Some(5));
        assert_eq!(branch_inline_contexts.get(&1).map(|c| c.call_line), Some(5));
        assert_eq!(
            branch_inline_contexts.get(&2).map(|c| c.call_line),
            Some(10)
        );
        assert_eq!(
            branch_inline_contexts.get(&3).map(|c| c.call_line),
            Some(10)
        );
    }

    /// v0.13 — modal headline label: when one inline context wins
    /// the count outright, `Decision.inline_context` reflects it.
    /// 3 branches in ctx_a, 1 in ctx_b → ctx_a is the headline.
    #[test]
    fn decision_inline_context_is_modal() {
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
        let mut inline_map = InlineMap::new();
        // 3 branches in ctx_a (call_line 5)
        inline_map.insert(
            10,
            InlineEntry {
                high_pc_exclusive: 35,
                context: InlineContext {
                    call_file: Some("caller.rs".to_string()),
                    call_line: 5,
                },
                chain: Vec::new(),
            },
        );
        // 1 branch in ctx_b (call_line 10)
        inline_map.insert(
            35,
            InlineEntry {
                high_pc_exclusive: 45,
                context: InlineContext {
                    call_file: Some("caller.rs".to_string()),
                    call_line: 10,
                },
                chain: Vec::new(),
            },
        );
        let entries = vec![
            entry_with_offset(0, 10),
            entry_with_offset(1, 20),
            entry_with_offset(2, 30),
            entry_with_offset(3, 40),
        ];
        let (decisions, _, _) = group_into_decisions(&entries, &line_map, &inline_map);
        assert_eq!(decisions.len(), 1);
        let label = decisions[0]
            .inline_context
            .as_ref()
            .expect("modal context should win");
        assert_eq!(label.call_line, 5, "ctx_a wins 3-to-1 → headline is ctx_a");
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
        let (decisions, _, _) = group_into_decisions(&entries, &line_map, &InlineMap::new());
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

    /// v0.19.0 — `IfThen` clusters alongside `BrIf` so clang-shaped
    /// short-circuit chains (one `if/else` + one `br_if` per source
    /// decision) produce a Decision. `IfElse` is excluded — it's the
    /// negation of the same site and counting it would inflate the
    /// condition count.
    #[test]
    fn group_into_decisions_clusters_if_then_with_br_if() {
        let mut line_map = LineMap::new();
        line_map.insert(
            100,
            LineLocation {
                file: "leap.c".to_string(),
                line: 6,
            },
        );
        line_map.insert(
            104,
            LineLocation {
                file: "leap.c".to_string(),
                line: 6,
            },
        );
        let entries = vec![
            entry_with_kind(0, 100, BranchKind::IfThen),
            entry_with_kind(1, 100, BranchKind::IfElse),
            entry_with_kind(2, 104, BranchKind::BrIf),
        ];
        let (decisions, _, _) = group_into_decisions(&entries, &line_map, &InlineMap::new());
        assert_eq!(
            decisions.len(),
            1,
            "IfThen + BrIf on same line → one Decision (IfElse stays out)"
        );
        assert_eq!(
            decisions[0].conditions,
            vec![0, 2],
            "Decision conditions must include the IfThen id and the BrIf id, NOT the IfElse"
        );
        assert_eq!(decisions[0].source_line, Some(6));
    }

    /// v0.19.0 — a lone `IfThen` (no BrIf companion, no second IfThen)
    /// stays below the cluster threshold and is dropped, mirroring the
    /// pre-v0.19 singleton handling for BrIf. Prevents the cluster
    /// rule from emitting spurious 1-condition decisions.
    #[test]
    fn group_into_decisions_drops_lone_if_then() {
        let mut line_map = LineMap::new();
        line_map.insert(
            50,
            LineLocation {
                file: "leap.c".to_string(),
                line: 6,
            },
        );
        let entries = vec![entry_with_kind(0, 50, BranchKind::IfThen)];
        let (decisions, _, _) = group_into_decisions(&entries, &line_map, &InlineMap::new());
        assert!(
            decisions.is_empty(),
            "singleton IfThen → no Decision (cluster.len() >= 2 gate holds)"
        );
    }
}
