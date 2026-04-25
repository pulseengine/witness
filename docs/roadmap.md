# witness — roadmap

> Generated 2026-04-25 from rivet artefacts and the v0.6 research-agent
> outputs. The provisional artefacts (`status: proposed`) for v0.6.1
> through v0.9 are auditable in `artifacts/*.yaml` from now —
> `rivet list --type requirement --format json` shows them.

## Where we are

| Version | Status | Capability |
|---|---|---|
| v0.1 | shipped 2026-04-24 | Branch coverage, instrument/run/report, exported counter globals |
| v0.2 | shipped 2026-04-25 | Manifest schema for decisions; per-target `br_table` counting; harness mode |
| v0.3 | shipped 2026-04-25 | rivet evidence + sigil predicate; quality bar (proptest, mutants, miri) |
| v0.4 | shipped 2026-04-25 | DWARF reconstruction algorithm; `witness diff`; release compliance bundle |
| v0.5 | shipped 2026-04-25 | Workspace split; `witness lcov` and `witness attest`; CI dogfood |
| **v0.6** | **shipped 2026-04-25** | **MC/DC consumer side: schema, reporter, verdict suite oracles, V-model graph, research roadmap. Runtime instrumentation deferred to v0.6.1.** |

## Forward path

| Version | Scope | Driving research | Status |
|---|---|---|---|
| **v0.6.1** | On-Wasm trace-buffer instrumentation per the v0.6 primitive recommendation | `docs/research/v06-instrumentation-primitive.md` | provisional artefacts: REQ-034, FEAT-015, DEC-013 (locked) |
| **v0.7** | Scaling MC/DC to httparse-class real applications (~1500 decisions) | `docs/research/v07-scaling-roadmap.md` | provisional: REQ-035, REQ-036, REQ-037, FEAT-016, DEC-019 |
| **v0.8** | Visualisation: Axum + maud + HTMX 2.x as wstd-axum wasm component, MCP transport on the same router, Playwright-tested | `docs/research/v08-visualisation-roadmap.md` | provisional: REQ-038, REQ-039, REQ-040, FEAT-017, DEC-020 |
| **v0.9** | State-of-the-art positioning + autonomous AI-agent gap-closing loop + signed agent attribution | `docs/research/v09-soa-and-agent-ux.md` | provisional: REQ-041, REQ-042, REQ-043, FEAT-018, DEC-021 |
| **v1.0** | Check-It pattern qualification — checkable attestation + small qualified checker | (no v1.0 brief yet; v0.9 close gates this) | REQ-011, FEAT-005 |

## How the four v0.6 research outputs compose

The roadmap was researched in parallel; the resulting recommendations
reinforce each other rather than collide:

- The **trace buffer** (v0.6.1, primitive B) preserves Rust's short-circuit
  semantics natively. Its sparse `evaluated` map per row is exactly the
  data shape the **v0.8 visualiser** renders directly to a truth-table
  cell grid — no transformation step, no impedance mismatch.
- The **v0.7 scaling work** stresses the trace buffer at httparse's
  ~1500-decision count and surfaces the bounded-memory + streaming
  encoding constraints that v0.7 closes.
- The **v0.9 MCP server** consumes the same `DecisionRow` records the
  v0.6.1 instrumentation produces and the v0.8 UI renders. The
  agent-native tool calls (`find_missing_witness`,
  `propose_test_row_to_close_gap`) are MC/DC-shaped queries against
  that one schema, not a separate API surface.
- **Signed agent attribution** (v0.9) plugs into the rivet V-model
  artefact graph that v0.6 has already built. New test rows authored
  by agents become rivet artefacts with author=agent-X and the
  closing-gap predicate is a DSSE-signed proof.

## V-model traceability

Every release ships:

- `artifacts/requirements.yaml`, `artifacts/features.yaml`,
  `artifacts/design-decisions.yaml` — rivet-validated artefact graph.
- `rivet list --format json` — current state, machine-readable.
- (v0.6.1+) `compliance/traceability-matrix.html` and `.json` —
  per-release matrix from `rivet trace` bundled into the GitHub
  release asset.

The v-model claim is provable, not asserted: opening a verdict folder
shows requirement → design decision → conditions → rows → signed
predicate in one page (`V-MODEL.md` plus `TRUTH-TABLE.md` plus the
release-pipeline-generated evidence bundle).

## Open competitive risks

(From `docs/research/v09-soa-and-agent-ux.md`.)

- **RapiCover** ships unbounded-condition MC/DC for C/C++/Ada with
  DO-178C qualification heritage. Witness cannot beat them on Ada DAL A
  avionics certification heritage. The v0.9 superiority claim narrows
  to *Rust-via-Wasm projects that want signed evidence and agent
  integration* — a real, underserved niche.
- **Parasoft** shipped a generic MCP server in March 2026. v0.9
  differentiation must stay MC/DC-specific (truth-table-shaped tool
  calls) and signed-attribution-specific (agent identity in the rivet
  V-model), not generic agent integration.

## Where to look for what

| If you need... | Look at |
|---|---|
| What v0.6.0 actually shipped | [CHANGELOG.md](../CHANGELOG.md) §0.6.0 |
| The MC/DC reporter implementation | `crates/witness-core/src/mcdc_report.rs` |
| The verdict suite | `verdicts/` (7 verdicts, each with V-MODEL.md + TRUTH-TABLE.md) |
| The instrumentation primitive choice | [v06-instrumentation-primitive.md](research/v06-instrumentation-primitive.md) |
| The scaling roadmap | [v07-scaling-roadmap.md](research/v07-scaling-roadmap.md) |
| The visualisation architecture | [v08-visualisation-roadmap.md](research/v08-visualisation-roadmap.md) |
| The competitive market scan | [v09-soa-and-agent-ux.md](research/v09-soa-and-agent-ux.md) |
| Provisional artefacts for next versions | `rivet list --status proposed --format json` |
