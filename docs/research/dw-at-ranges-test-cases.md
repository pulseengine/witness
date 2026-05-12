# DW_AT_ranges test-case design

**Status:** Design analysis for v0.15.x or v0.16. Implementation deferred.
**Author:** witness maintainers, 2026-05-12.

## Background

`crates/witness-core/src/decisions.rs::build_inline_map` (v0.14.0)
collects `DW_TAG_inlined_subroutine` entries via `DW_AT_low_pc`
+ `DW_AT_high_pc`. Inlined subroutines that use `DW_AT_ranges`
(an offset into `.debug_rnglists` pointing to a list of address
intervals) are silently skipped. This document analyses how to
test the missing `DW_AT_ranges` walker without committing a
binary fixture to the repo.

## When does rustc emit `DW_AT_ranges` for inlines?

LLVM emits `DW_AT_ranges` for an inlined-subroutine entry when
the inlined call spans **more than one contiguous instruction
range**. This happens in three known shapes:

### Shape A — Tail-merged code

Two call sites of the same inlined function get tail-merged
into shared exit code by LLVM's `simplifycfg` or `merge-functions`
passes. The inlined subroutine then covers ranges in both
callers' bodies, expressed as `DW_AT_ranges`.

Likeliness in real Rust: **high** at `opt-level = 3` with LTO,
**rare** at lower opt-levels.

### Shape B — Hot/cold splitting

LLVM splits inlined code along a probably-untaken branch
(common with `core::panicking` paths). The hot path stays
contiguous; cold path lives in a separate section. The inlined
subroutine entry covers both via `DW_AT_ranges`.

Likeliness: **very high** in panic-bearing inlines. Likely
present in `httparse` and `nom_numbers` already; we just don't
currently catch them.

### Shape C — Cross-crate inlining with monomorphization

When a generic inlined function is monomorphized at distinct
call sites across crates and LLVM-LTO merges them, the
resulting inlined-subroutine entries can carry multi-range
addresses.

Likeliness: **high** in workspaces using LTO and `rust-lld`.

## Test-case strategy

The walker change (~80 LoC) is mechanical. Testing it requires
exercising the gimli `RangeList` API against a known
ranges-bearing DWARF input. Three options exist, ordered by
preference.

### Option A — `gimli::write` builder (recommended)

Use gimli's writing API (`gimli::write::Dwarf`) to construct a
minimal compilation unit with a single
`DW_TAG_inlined_subroutine` whose address coverage is
expressed via `DW_AT_ranges` over a `RangeListsTable`.
Serialise; feed to `build_inline_map`; assert the returned
`InlineMap` carries one entry per range with the same
`(call_file, call_line)` chain.

**Pros:**
- No binary fixtures committed to the repo.
- Test runs fast (in-memory).
- Coverage explicit: each test case constructs exactly the
  shape under test.

**Cons:**
- `gimli::write` is non-trivial. ~200 LoC of test-only DWARF
  scaffolding per shape. Maintenance cost.
- Trusts that `gimli::write` round-trips through `gimli::read`
  cleanly (it does; gimli has its own round-trip tests, but
  any version skew between write and read APIs would surface
  here first).

**Concrete sketch:**

```rust
#[test]
fn build_inline_map_handles_dw_at_ranges() {
    use gimli::write::{
        Address, AttributeValue, DwarfUnit, EndianVec, LineProgram,
        RangeList, RangeListTable, Sections, Unit, UnitId,
    };
    let mut dwarf = DwarfUnit::new(gimli::write::Encoding {
        format: gimli::Format::Dwarf32,
        version: 5,
        address_size: 4,  // wasm
    });
    // Add a CU with a line program that maps addr 0x100 and 0x200
    // to lib.rs:42.
    // Add a top-level subprogram covering [0x100..0x300).
    // Add a child DW_TAG_inlined_subroutine with:
    //   DW_AT_ranges → list of [(0x100, 0x150), (0x200, 0x280)]
    //   DW_AT_call_file → "outer.rs"
    //   DW_AT_call_line → 7
    // Serialise to bytes; wrap in `.debug_*` sections; feed to
    // build_inline_map; assert two InlineMap entries.
    // ...
}
```

The harness function `build_dwarf` in `decisions.rs:188` already
accepts raw byte slices via `DwarfSections`; the test just needs
to populate those slices.

### Option B — Compile a known-shape Rust fixture under LTO

Add a verdict `verdicts/dw_at_ranges/` whose `Cargo.toml`
specifies `lto = "fat"` + a deliberately-large inlined
predicate that LLVM is known to split. Verify at build time
that the resulting wasm contains a `DW_TAG_inlined_subroutine`
with `DW_AT_ranges` (via `wasm-tools` or a pre-build assertion).
Use the witness binary itself as the test rig.

**Pros:**
- Real-world signal: confirms the feature works on actual
  rustc output, not just synthetic gimli.

**Cons:**
- LLVM's decision to use `DW_AT_ranges` is heuristic-driven
  and version-sensitive. A future rustc/LLVM might change the
  threshold and break the fixture. Tests become flaky on
  toolchain drift.
- Build time costs (LTO is slow).
- The verdict's `branches` count + decision shape might also
  shift across LLVM versions, complicating the suite's
  regression gate.

### Option C — Commit a precompiled `.wasm` blob

Build the LTO fixture once, commit the resulting `.wasm` as a
test resource. Test reads the blob, parses DWARF, asserts.

**Pros:**
- Tests deterministic across rustc versions.
- No build-time DWARF dependency.

**Cons:**
- Binary blobs in the repo (~5-20 KiB per fixture). Bad form
  for a tool that purports to make evidence auditable.
- Updating the fixture requires recompiling with a specific
  rustc + capturing the blob. Easy to drift from CI's rustc.
- A reviewer can't read a precompiled blob to verify it
  exercises the intended code path.

## Recommendation

**Go with Option A (gimli::write).** The cost is once-off (the
scaffolding can be a `tests/dwarf_synth.rs` helper used by
multiple tests, not one per case). The signal is clean: the
test isolates the walker's response to a known-shape DWARF
input. Option B can be added later as a soak-style integration
check; Option C should be avoided.

## Implementation outline

1. Add `tests/dwarf_synth.rs` with helper functions:
   - `build_unit_with_inline(ranges: &[(u64, u64)], call: (&str, u32)) -> DwarfSections<'static>`
   - Returns owned byte buffers wrapped in `DwarfSections` for
     `build_inline_map` to consume.
2. Extend `build_inline_map` (~80 LoC):
   - In the per-DIE attribute loop, also match
     `gimli::constants::DW_AT_ranges`.
   - On hit, resolve the offset via `unit_ref.ranges(offset)`
     (gimli `RngListIter`) and collect intervals.
   - Emit one `InlineEntry` per resolved interval, each
     carrying the SAME chain (the parent stack at this DIE's
     depth).
3. Add tests:
   - `build_inline_map_handles_dw_at_ranges_two_ranges` —
     asserts two `InlineMap` entries, same chain.
   - `build_inline_map_handles_dw_at_ranges_with_nested_inline`
     — outer inline uses `DW_AT_ranges`; inner inline (nested)
     uses `low_pc/high_pc` inside one of the outer ranges.
     Asserts chain ordering preserved.
   - `build_inline_map_skips_invalid_ranges` — empty range
     list or all-zero entries silently skipped.

## Risks

1. **gimli API surface drift.** `gimli::write` API has had
   breaking changes between gimli minor versions. Test-only
   dependency on `gimli` write-side wants pinning.
2. **DWARF version sensitivity.** v5 uses `DW_FORM_rnglistx` +
   `.debug_rnglists`; v4 uses `DW_FORM_sec_offset` +
   `.debug_ranges`. Witness's current build_dwarf builds both
   sections. Tests should cover at least v5 (rustc default).
3. **wasm address size.** `wasm32` is 4-byte addresses, not 8.
   gimli's `Encoding { address_size: 4 }` must match.

## Estimated diff size

- `decisions.rs::build_inline_map` extension: ~80 LoC
- `tests/dwarf_synth.rs` scaffolding: ~200 LoC
- 3 unit tests: ~150 LoC
- **Total: ~430 LoC, all under `witness-core`.**

## Out of scope for this analysis

- Whether the verdict suite gains a real LTO fixture for
  end-to-end signal — that's a separate, less urgent question
  whose answer is "yes, eventually, as v0.15.x or later."
- Per-context chain rendering when a single Decision's row
  contributes inline frames via `DW_AT_ranges` — falls out
  for free once `build_inline_map` produces multi-range
  entries; no additional code path required.
