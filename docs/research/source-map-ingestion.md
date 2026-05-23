# Design note: source-map ingestion in `witness-core`

**Status:** design draft, no code yet.
**Why now:** unblocks decision reconstruction for any wasm
toolchain that ships V3 source maps instead of DWARF. The
immediate trigger is Kotlin/Wasm — `witness instrument` now
succeeds (54 branches captured) but produces 0 decisions because
`decisions.rs` is DWARF-only.

## Problem

`witness-core`'s decision reconstruction in
`crates/witness-core/src/decisions.rs::reconstruct_decisions`
attributes each branch's `byte_offset` to a `(file, line)` tuple
via a `LineMap` built from DWARF `.debug_line` rows. Branches
sharing a `(function_index, source_file)` and within a configured
line span cluster into a `Decision`.

When a wasm module has no `.debug_*` custom sections —
`reconstruct_decisions` short-circuits to "no decisions" and the
manifest reports zero. Kotlin/Wasm, AssemblyScript (historically),
and any future source-map-only toolchain hit this.

V3 source maps are the JavaScript-ecosystem standard. Kotlin/Wasm
emits a `.wasm.map` sidecar referenced by a `sourceMappingURL`
custom section in the wasm binary, in the
[Source Map Revision 3 Proposal](https://sourcemaps.info/spec.html)
format. The browser/Node toolchain reads these for stack traces.
Witness can do the same for branch attribution.

## Goal

Add an alternative source-attribution path to `witness-core` so
that, when DWARF is absent but a V3 source map is available,
branches still attribute to `(file, line)` and clustering still
forms decisions. The existing decision clustering, chain-kind
detection, and reporter layers stay unchanged — only the
`LineMap` construction step gains a second backend.

## Non-goals

- **Inline-chain tracking from source maps.** V3 has no
  `DW_TAG_inlined_subroutine` equivalent. The "names" field is
  per-mapping, not per-call-site. `branch_inline_chains` stays
  empty for source-map-attributed branches; the manifest will
  reflect that honestly.
- **Address-range coverage.** V3 maps offsets → source positions,
  not ranges. The mcdc-v3 `DW_AT_ranges` view (v0.17+) does not
  apply.
- **Per-language tuning.** This isn't Kotlin-specific. Any wasm
  module with a V3 source map gets the same treatment.
- **Round-trip / re-emit.** Witness does not need to write source
  maps. Read-only.

## Background — DWARF vs V3 source maps, structural delta

| Feature | DWARF `.debug_line` | V3 source map |
|---|---|---|
| Map shape | Sparse list of `(addr, file, line, column, …)` rows | VLQ-packed mappings, grouped by output line |
| Address granularity | Byte offset into wasm code section | "Generated column" within an emitted line — wasm has no "emitted lines"; the convention is one mapping per byte offset |
| Inlining | `DW_TAG_inlined_subroutine` chains | Optional `name` field per mapping; no chain |
| File table | `file_names` array, indexable | `sources` array, indexable |
| Names | Per-symbol via DIE walk | Optional `name` field per mapping |
| Ranges | `DW_AT_low_pc` / `DW_AT_high_pc` / `DW_AT_ranges` | None |
| Embedded | Yes — `.debug_*` custom sections | Sidecar `.wasm.map` referenced by `sourceMappingURL` custom section |

The key practical difference for witness: source maps give us
`(byte_offset → file, line)` directly. That's exactly what
`LineMap` already abstracts. So the decisions-pass impact is
small — `LineMap` gets populated from a different reader.

## Design

### Layered structure

Today (DWARF path):
```
wasm bytes
  → extract_dwarf_sections()
  → build_line_map()         [LineMap = BTreeMap<u64 byte_offset, LineLocation>]
  → group_into_decisions()
```

After (DWARF-or-source-map):
```
wasm bytes  ────────────────────┐
  └─ DWARF custom sections present?
        yes → build_line_map() ─┤
        no  → sourceMappingURL? │
                yes → fetch .wasm.map, build_line_map_from_v3() ─┤
                no  → empty LineMap                              │
                                                                 │
  → group_into_decisions(...)  ◄────────────────────────────────┘
```

The DWARF path is unchanged. New code: a V3 parser that produces a
`LineMap` byte-for-byte indistinguishable from DWARF's output, so
the clustering stage doesn't notice the difference.

### Source-map discovery

V3 source maps live in a sidecar file `<wasm-basename>.wasm.map`.
The wasm binary references it via a `sourceMappingURL` wasm custom
section (single-section convention, payload is a UTF-8 URL or
relative path).

Two modes:

1. **Default — sidecar adjacent to the input wasm.** When witness
   reads the input from a path, check for `<input>.map` or
   `<input>.wasm.map`, and use it if present.
2. **Explicit — `--source-map <path>` CLI flag.** Override the
   default discovery for non-default layouts, embedded test
   fixtures, etc. The flag takes precedence over the sidecar.

Embedded source maps (data URL in `sourceMappingURL`) are out of
scope for the first pass — Kotlin/Wasm uses a relative path.

### V3 parser

The format is a JSON object:

```json
{
  "version": 3,
  "sources": ["src/wasmJsMain/kotlin/leap.kt", ...],
  "names":   ["leapYear", "y", ...],
  "mappings": "AACgB,SAASA,EAAU..."
}
```

`mappings` is a comma/semicolon-delimited string of base64-VLQ
fields per "generated position." Each segment carries 1, 4, or 5
fields:

| Fields | Meaning |
|---|---|
| 1 | Generated column |
| 4 | + source index, original line, original column |
| 5 | + name index |

For wasm, the "generated column" within a notional output line is
treated as the byte offset into the code section. Different wasm
toolchains agree on this convention; documentation lives in the
[wasm source map proposal](https://github.com/WebAssembly/source-map-spec)
(in progress) and Kotlin/Wasm's own emitter.

Two implementation options:

**Option A — pull in the `sourcemap` crate** (`sourcemap = "9"`,
MPL-2.0 licensed):
- Pros: battle-tested parser, handles edge cases, ~easy to drop in.
- Cons: adds a dep (~150 KB of compiled code), the MPL-2.0 license
  is workspace-compatible (witness is Apache/MIT) but worth a
  one-line CHANGELOG note for compliance reviewers.

**Option B — hand-roll a minimal V3 parser** (~250 LOC):
- Pros: no dep; we read only what we need (generated col, source
  idx, source line — name field can be ignored).
- Cons: yet another base64-VLQ implementation; small but real
  surface for bugs; needs its own test fixtures.

Recommendation: **start with Option A** to ship the feature
quickly, profile + revisit if the dep weight matters. The
`sourcemap` crate API is small enough that swapping later is
mechanical.

### LineMap population

The V3 mapping structure groups generated positions by line. For a
wasm binary, all mappings effectively share generated line 0; the
"generated column" carries the wasm code section byte offset. The
parser walks segments, and for each emits:

```rust
line_map.insert(
    byte_offset,
    LineLocation { file: sources[src_idx], line: orig_line },
);
```

This produces a `BTreeMap<u64, LineLocation>` identical in shape to
the DWARF path's output. `lookup_line()` (DWARF range semantics:
largest entry ≤ query) just works.

### Where it plugs in

`reconstruct_decisions(wasm_bytes, branches, opts)` grows an
optional `source_map` argument. The CLI's `instrument` /
`mcdc-report` paths thread the source map path through. The
existing call sites that pass `None` continue to work — DWARF-only
behavior is unchanged.

A new field in the manifest's `instrument_meta` (or similar)
records the attribution backend used:

```json
{
  "attribution_source": "dwarf" | "source-map-v3" | "none"
}
```

Reviewers reading an MC/DC report should know whether the
`(file, line)` labels came from DWARF (with ranges + inline
chains) or from a source map (line-only).

## Caveats — what Kotlin coverage can claim

Honest framing for the resulting MC/DC reports:

- ✅ **Branch counts** — same precision as DWARF (witness's
  instrumentation is identical; only attribution differs).
- ✅ **Decision clustering** — same `v0.19` IfThen+BrIf rules
  apply; clustering works once `(file, line)` is attributed.
- ✅ **`chain_kind` detection** — works (uses wasm bytecode
  shape, not DWARF).
- ⚠️ **Source labels** — only as accurate as the toolchain's
  source map. Kotlin's are typically good; quality varies by
  emitter.
- ❌ **Inline-chain depth** — always 1 for source-map paths.
  Multi-context `per_context` views collapse to the leaf.
- ❌ **Address-range views** — not available.

In the mcdc-v3 envelope, the `attribution_source: source-map-v3`
field signals these limitations to consumers. The structural
schema doesn't change.

## Implementation plan

Three commits, smallest first, each ships green:

**1. `LineMap` becomes attribution-source-aware** (refactor)
- Add `AttributionSource` enum (`Dwarf | SourceMapV3 | None`)
- Plumb through `reconstruct_decisions` / report metadata
- DWARF path tagged `Dwarf`; absent → `None`. No behaviour
  change yet — pure refactor.

**2. V3 source-map reader** (new module
   `crates/witness-core/src/sourcemap.rs`)
- `read_source_map(wasm_bytes, override_path) -> Option<SourceMap>`
- `build_line_map_from_v3(map) -> LineMap`
- Behind a `sourcemap` crate dep (per the recommendation above)
- Unit tests over a minimal hand-crafted V3 input

**3. Wire it into `reconstruct_decisions` + CLI**
- `reconstruct_decisions` checks for source map when DWARF is
  absent
- CLI `--source-map <path>` flag on `instrument` /
  `mcdc-report`
- Update Kotlin fixture README + cross-language matrix to
  show decisions count after wiring
- mcdc-v3 envelope gains `attribution_source` field (additive,
  back-compat for older consumers)

## Open questions

1. **mcdc-v3 schema bump?** Adding `attribution_source` is
   additive — older consumers ignore unknown fields. Either v3
   stays at v3 with a documented field addition (back-compat
   contract has been kind to this so far) or we cut a v4. Lean
   toward "add to v3" since the data interpretation doesn't
   change, only the provenance tag.
2. **Per-mapping `name` field — drop or surface?** The V3 `names`
   array contains symbol names per mapping. Kotlin populates this
   with the Kotlin function name. We don't need it for clustering,
   but could populate `BranchEntry.function_name` from it as a
   weak signal (witness already has function_name from the wasm
   name section, so this is duplicative — drop it).
3. **Discovery — sidecar vs `sourceMappingURL` custom section?**
   The wasm-side `sourceMappingURL` reference is the spec-correct
   discovery path. Sidecar by convention is the pragmatic
   shortcut. Implement both: prefer the wasm custom section,
   fall back to the conventional sidecar.

## Effort estimate

Best case 2 days, with proper testing 3. Bulk of the work is
plumbing + tests; the V3 parser via `sourcemap` is half a day.

## Out of scope for first PR

- AssemblyScript probe (next probe candidate after Kotlin lands)
- Embedded `data:` URL source maps
- Source-map round-trip / emission
- Source-map column resolution (we use line only)
- Hot-reload / sidecar file watching

These can follow as separate PRs if the use case shows up.
