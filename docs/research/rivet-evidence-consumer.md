# Rivet evidence-consumer brief (for witness v0.3)

Investigation of how `rivet` at `/Users/r/git/pulseengine/rivet` consumes
external evidence today, conducted 2026-04-25 to inform witness v0.3
design. Findings verified against the rivet source after the
sub-agent's initial pass.

## Executive summary

**Rivet has no coverage-from-tests evidence consumer today.**

- The only external-evidence consumer is `rivet-core/src/results.rs`'s
  `ResultStore`, which loads YAML test-result files (pass / fail / skip /
  error / blocked per artifact).
- `rivet-core/src/coverage.rs` exists, but its `CoverageReport` /
  `CoverageEntry` model **traceability-rule** coverage (which artifacts
  have which links per schema rule) — *not* test-coverage fractions
  attached to requirements. It computes internally; it does not ingest
  external evidence.

This is the gating fact for v0.3: witness can emit anything we want, but
rivet can't *consume* coverage evidence as evidence-for-a-requirement
without an upstream change to rivet.

## What rivet's evidence consumer actually looks like today

### `ResultStore` (test-results consumer)

`rivet-core/src/results.rs:143` declares the consumer entry:

```rust
pub struct ResultStore { ... }
```

Backing struct (`results.rs:82`):

```rust
pub struct TestResult {
    pub artifact: String,         // e.g. "REQ-001"
    pub status: TestStatus,        // pass | fail | skip | error | blocked
    pub duration: Option<f64>,
    pub message: Option<String>,
    // …
}
```

YAML on-disk format (per `results.rs:107` `TestRunFile`):

```yaml
run:
  id: ...
  timestamp: ...
  source: ...
  environment: ...
  commit: ...
results:
  - artifact: "REQ-001"
    status: "pass"
    duration: 0.42
```

Rivet auto-discovers result files in a directory, indexes them by
artifact id, and exposes `latest_for(req_id)` / `history_for(req_id)`
to downstream consumers.

### `CoverageReport` (NOT an external-evidence consumer)

`rivet-core/src/coverage.rs:53` declares:

```rust
pub struct CoverageEntry {
    pub rule_name: String,
    pub description: String,
    pub source_type: String,
    pub link_type: String,
    pub direction: CoverageDirection,    // Forward | Backward
    pub target_types: Vec<String>,
    pub covered: usize,                   // artifacts satisfying the rule
    pub total: usize,                     // total artifacts of source_type
    pub uncovered_ids: Vec<String>,
}
```

This is a **traceability-rule coverage** model: "of all REQ artifacts,
how many have a `verified-by` link to a TEST artifact?" Computed
internally from the artifact graph. There is no field for hit counts,
percentages from test runs, or per-branch data. There is no path to
ingest a JSON/YAML coverage document from an external tool.

### Coverage-specific gaps

| Gap | Status today | What it would mean to fill |
|---|---|---|
| Per-(requirement, test, coverage-percentage) evidence type | Not present | Add `rivet-core/src/coverage_evidence.rs` mirroring `results.rs` |
| Per-branch hit-count storage | Not present | Coverage-evidence schema needs a `branches: [...]` field |
| MC/DC condition decomposition | Not present | Coverage-evidence schema needs a `decisions: [...]` field |
| `rivet coverage import <file>` CLI | Not present | New subcommand or extension of `rivet results import` |
| In-toto / sigil bundle ingestion | Not present | Rivet currently does not unwrap sigil bundles to extract coverage predicates |

## Implication for witness v0.3

**Witness alone cannot integrate with rivet until rivet ships a
coverage-evidence consumer.** Three paths:

### Path A — Upstream rivet first (cross-repo work)

1. Design the rivet `CoverageStore` consumer type (mirroring
   `ResultStore`).
2. Implement it as a PR against `pulseengine/rivet`.
3. Ship a rivet release.
4. Witness v0.3 emits matching format.

Cost: roughly the same code as v0.3's witness-side work, but in a
separate repo with separate review and release. Probably 1.5x the time
budget.

Benefit: rivet integration *actually works* end-to-end on day one of
v0.3.

### Path B — Witness emits the format anyway (declared, not consumed)

1. Witness v0.3 emits coverage in the schema we *would* want rivet to
   consume.
2. The schema is documented in this brief and witness's DESIGN.md.
3. Rivet integration is "ready when rivet is" — witness output sits on
   disk waiting to be ingested.
4. v0.4 lands the rivet-side consumer; v0.4 of witness becomes a
   no-op compatibility check.

Cost: one round-trip when rivet's actual schema diverges from witness's
guess. Honest about the gap.

Benefit: v0.3 ships immediately. The output is useful as documented
"future evidence" and as a reference for the rivet upstream PR.

### Path C — Defer rivet integration entirely; v0.3 is sigil-only

1. Witness v0.3 ships only the sigil-side work (in-toto coverage
   predicate emission). See `sigil-predicate-format.md` — sigil works
   today with no upstream changes.
2. `witness merge` ships as the multi-run aggregation primitive (already
   implemented).
3. Rivet integration becomes v0.4 entirely.

Cost: v0.3 is smaller than originally planned (no rivet anything). The
"rivet integration" promise from DESIGN.md slips to v0.4.

Benefit: v0.3 delivers exactly what the ecosystem can absorb today.
No declared-but-not-consumed surfaces.

### Recommendation

**Path B**, with a footnoted "v0.4 closes the loop". Reasons:

1. Witness's output format is the v0.3 contribution; rivet's consumer is
   downstream of it. Designing the format first, then implementing the
   consumer to match, is the correct directionality for any
   producer-consumer pair.
2. Sigil already works (Path C), so v0.3 still ships meaningful
   integration regardless of what rivet does. The rivet half is bonus
   structure for v0.4.
3. The schema agreed in v0.3 becomes the rivet upstream PR's spec.
4. The honest-assessment column on the DESIGN.md table reads ◐ rather
   than ❌, and the gap is named in one sentence.

## Proposed witness v0.3 evidence schema

Mirroring `ResultStore`'s shape so that, when rivet's `CoverageStore`
lands, the file format on disk does not change.

```yaml
# witness-coverage-evidence.yaml
schema: "https://pulseengine.eu/witness-rivet-evidence/v1"
version: "1.0"
witness_version: "0.3.0"

run:
  id: "witness-2026-04-25T10:30:00Z"
  timestamp: "2026-04-25T10:30:00Z"
  source: "cargo test --release"
  environment: "x86_64-unknown-linux-gnu"
  commit: "deadbeef"

module:
  path: "app.instrumented.wasm"
  digest:
    sha256: "..."

# Per-requirement coverage. Witness consumes a user-supplied
# branch_id → requirement_id map (from CLI flag or rivet artefact links)
# and emits one entry per requirement that has matching branches.
evidence:
  - artifact: "REQ-001"
    coverage_type: "branch"
    total: 12
    covered: 10
    percentage: 83.3
    hits: [1, 0, 2, 0, 3, 1, 1, 2, 0, 1, 0, 4]
    uncovered_branch_ids: [1, 3, 8, 10]
  - artifact: "REQ-002"
    coverage_type: "branch"
    total: 4
    covered: 4
    percentage: 100.0
    hits: [5, 2, 7, 3]
    uncovered_branch_ids: []
```

### Key shape decisions

- **Top-level `schema:`** — predicate-type-style URL so the file is
  self-identifying (helps the v0.4 consumer dispatch). Mirrors the
  in-toto convention.
- **`run:` block** matches rivet's `RunMetadata` shape (`results.rs:94`).
- **`evidence: [...]`** is the array of per-requirement entries —
  mirrors rivet's `results: [...]` array shape.
- **`artifact:`** field name matches `TestResult.artifact` exactly so a
  v0.4 rivet consumer can deserialise with minimal divergence.
- **`coverage_type:`** — `"branch"` for v0.1+v0.2's per-`br_if`/per-arm
  output. Reserved values: `"mcdc"` (when v0.2.1's reconstruction
  populates `decisions`), `"line"` (out of scope; for wasmcov-style
  tools).
- **`hits: [...]`** is per-branch, indexed by branch id. Empty / absent
  fields are tolerated.
- **`uncovered_branch_ids: [...]`** is the deterministic complement of
  hit-set, sorted ascending.

## CLI surface

```bash
# v0.3
witness rivet-evidence \
  --run witness-run.json \
  --requirement-map witness-rivet-map.yaml \
  --output witness-coverage-evidence.yaml
```

The `--requirement-map` file:

```yaml
mappings:
  - branches: [0, 1, 2, 3]
    artifact: "REQ-001"
  - branches: [4, 5, 6, 7, 8, 9, 10, 11]
    artifact: "REQ-002"
```

Branches not listed in any mapping are ignored. Mappings that name
unknown branches produce an error.

## Worked example

Input `witness-run.json` (from `witness merge`):

```json
{
  "schema_version": "2",
  "witness_version": "0.3.0",
  "module_path": "app.instrumented.wasm",
  "branches": [
    {"id": 0, "kind": "if_then", "instr_index": 12, "hits": 1},
    {"id": 1, "kind": "if_else", "instr_index": 12, "hits": 0},
    {"id": 2, "kind": "br_if",   "instr_index": 24, "hits": 2}
  ]
}
```

Map:

```yaml
mappings:
  - branches: [0, 1, 2]
    artifact: "REQ-001"
```

Output:

```yaml
schema: "https://pulseengine.eu/witness-rivet-evidence/v1"
version: "1.0"
witness_version: "0.3.0"
run:
  id: "witness-..."
  timestamp: "2026-04-25T10:30:00Z"
  source: "witness rivet-evidence"
module:
  path: "app.instrumented.wasm"
evidence:
  - artifact: "REQ-001"
    coverage_type: "branch"
    total: 3
    covered: 2
    percentage: 66.7
    hits: [1, 0, 2]
    uncovered_branch_ids: [1]
```

## Blockers

- **Rivet has no coverage-evidence consumer.** v0.3 emits the format;
  v0.4 (or an upstream rivet PR coordinated with this work) lands the
  consumer.
- **No agreed branch-id → requirement-id mapping convention.** v0.3
  introduces a per-project mapping YAML; longer-term this can be
  generated from rivet artifact links (`verified-by` between TEST and
  REQ artifacts).

## Cited rivet source

| File | Lines | What's there |
|---|---|---|
| `rivet-core/src/results.rs` | 51–143 | `TestStatus` enum, `TestResult`, `RunMetadata`, `TestRunFile`, `TestRun`, `ResultSummary`, `ResultStore` |
| `rivet-core/src/results.rs` | 226–257 | `ResultStore::load_dir` — the directory-scanning evidence ingestion path |
| `rivet-core/src/coverage.rs` | 53–110 | `CoverageEntry`, `CoverageDirection`, `CoverageReport` — traceability-rule coverage, NOT external evidence |
| `rivet-core/src/coverage.rs` | 116–199 | Internal coverage computation from artifact graph |

Sub-agent's verbatim findings included file paths and lines verified by
the main thread by direct grep.
