# witness JSON Schemas

Every schema-bearing JSON / YAML document witness emits names a schema
URL in its top-level `schema` (or `predicateType`) field. v0.10.0
publishes the matching JSON Schema documents at those exact URLs so
external consumers can validate witness output without depending on
the witness Rust types.

All schemas use **JSON Schema draft 2020-12**.

## Index

| Schema URL | File | Purpose |
|---|---|---|
| `https://pulseengine.eu/witness-mcdc/v1` | [`witness-mcdc-v1.json`](witness-mcdc-v1.json) | MC/DC report — per-decision truth tables, condition verdicts (proved / gap / dead), independent-effect pair interpretations (`unique-cause` / `masking` / `br-table-arm`), gap-closure recommendations, and `trace_health`. The signed `report.json` shipped with every verdict. |
| `https://pulseengine.eu/witness-coverage/v1` | [`witness-coverage-v1.json`](witness-coverage-v1.json) | in-toto Statement v1.0 carrying a witness branch-coverage predicate. Emitted by `witness predicate`; sigil signs it as a DSSE envelope. Subject 0 is the instrumented Wasm; subject 1 (when present) is the pre-instrumentation module. |
| `https://pulseengine.eu/witness-rivet-evidence/v1` | [`witness-rivet-evidence-v1.json`](witness-rivet-evidence-v1.json) | Rivet-shape coverage evidence. Emitted by `witness rivet-evidence` when a requirement-mapping YAML is supplied; rivet's `CoverageStore` consumer ingests it directly. |
| `https://pulseengine.eu/witness-trace-matrix/v1` | [`witness-trace-matrix-v1.json`](witness-trace-matrix-v1.json) | V-model traceability matrix linking requirements through features, design decisions, and verdict suites. Emitted by `.github/actions/compliance/trace-matrix.py` for the release compliance bundle. |

## Validating witness output

Each schema validates the corresponding witness artefact byte-for-byte.

### MC/DC report (`report.json`)

```sh
curl -fsSL https://pulseengine.eu/witness-mcdc/v1 -o /tmp/witness-mcdc-v1.json && \
  python3 -m jsonschema -i build/verdict-evidence/leap_year/report.json /tmp/witness-mcdc-v1.json
```

### Coverage predicate (`predicate.json`)

```sh
curl -fsSL https://pulseengine.eu/witness-coverage/v1 -o /tmp/witness-coverage-v1.json && \
  python3 -m jsonschema -i build/verdict-evidence/leap_year/predicate.json /tmp/witness-coverage-v1.json
```

### Rivet evidence (`evidence.yaml`, after JSON conversion)

```sh
curl -fsSL https://pulseengine.eu/witness-rivet-evidence/v1 -o /tmp/witness-rivet-evidence-v1.json && \
  python3 -c "import sys,yaml,json; print(json.dumps(yaml.safe_load(open('evidence.yaml'))))" \
    | python3 -m jsonschema /tmp/witness-rivet-evidence-v1.json -i /dev/stdin
```

### Traceability matrix (`traceability-matrix.json`)

```sh
curl -fsSL https://pulseengine.eu/witness-trace-matrix/v1 -o /tmp/witness-trace-matrix-v1.json && \
  python3 -m jsonschema -i compliance-evidence/traceability-matrix.json /tmp/witness-trace-matrix-v1.json
```

## Hosting

The `pulseengine.eu/witness-*/v1` URLs are served via GitHub Pages from
this directory; pushing changes to `docs/schemas/` on `main` updates
the published documents. The schemas should be **versioned forward**
(new URL, e.g. `/v2`) on breaking changes rather than edited in place.

## Provenance

Schemas were derived from the Rust types they describe:

- `witness-mcdc-v1.json` — `crates/witness-core/src/mcdc_report.rs`
  (`McdcReport`, `McdcOverall`, `DecisionVerdict`, `DecisionStatus`,
  `ConditionVerdict`, `ConditionStatus`, `GapClosure`, `RowView`)
  plus `crates/witness-core/src/run_record.rs` (`TraceHealth`).
- `witness-coverage-v1.json` — `crates/witness-core/src/predicate.rs`
  (`Statement`, `Subject`, `Digests`, `CoveragePredicate`,
  `Measurement`, `OriginalModule`) plus
  `crates/witness-core/src/report.rs` (`Report`, `FunctionReport`,
  `UncoveredBranch`) and `crates/witness-core/src/instrument.rs`
  (`BranchKind`).
- `witness-rivet-evidence-v1.json` —
  `crates/witness-core/src/rivet_evidence.rs` (`EvidenceFile`,
  `RunMetadata`, `ModuleRef`, `CoverageEvidence`, `CoverageType`).
- `witness-trace-matrix-v1.json` —
  `.github/actions/compliance/trace-matrix.py` (the dict structure
  written to `traceability-matrix.json`).
