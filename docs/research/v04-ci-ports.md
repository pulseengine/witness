# v0.4 CI ports — rivet-delta + compliance composite action

Adaptation of two rivet-side CI patterns to witness for v0.4.

Sub-agent investigation 2026-04-25; verified by main thread spot-checks
against the rivet source.

## Job 1 — `witness-delta.yml`

### What rivet-delta does (`rivet/.github/workflows/rivet-delta.yml`)

Triggers on PRs touching `artifacts/`, `schemas/`, `rivet.yaml`, or
`rivet-core/**` / `rivet-cli/**`. Single `delta` job that:

1. Checks out base (merge-base) and head (PR commit) into separate dirs.
2. Builds `rivet-cli` release.
3. Runs `rivet diff --base <dir> --head <dir> --format json`.
4. Runs `rivet impact --since <SHA> --depth 5 --format json`.
5. Exports an HTML dashboard via `rivet export --format html`.
6. Renders mermaid diagrams via `npx @mermaid-js/mermaid-cli`.
7. Pushes SVG to an orphan branch `rivet-delta-renders` (so PR comments
   can image-link without bloating the main repo).
8. Posts/updates a PR comment (idempotent via `<!-- rivet-delta-bot -->`
   marker).

All analysis steps are `continue-on-error: true` — the job never blocks
merge.

### Witness analogue

Witness has no artefact graph but does have **branch manifests** and
**run records**. The witness analogue is **coverage-set delta**:
branches added / removed / changed kind, instruction-index shifts,
function-name churn.

**Strategy:** compare manifests from base and head, output the delta as
JSON, post as a PR comment. Coverage-percentage delta requires a run
record on each side, which means CI would need to build and execute a
fixture on each side — fold that in if the delta is interesting on
its own; otherwise skip and ship manifest-only delta first.

### `witness diff` subcommand (v0.4 implementation work)

`witness-delta.yml` depends on a new CLI subcommand:

```
witness diff --base <manifest|run> --head <manifest|run> --format json
```

Outputs:
```json
{
  "schema": "https://pulseengine.eu/witness-delta/v1",
  "added_branches": [...],
  "removed_branches": [...],
  "changed_branches": [...],
  "coverage_delta": {"base_pct": 78.3, "head_pct": 81.5, "delta_pct": +3.2}
}
```

When inputs are manifests, `coverage_delta` is null. When inputs are
run records, all fields populate.

### Workflow file (drop into `.github/workflows/witness-delta.yml`)

See agent brief; trimmed below to the load-bearing bits.

```yaml
name: Coverage Delta

on:
  pull_request:
    paths:
      - "src/**"
      - "tests/**"
      - "Cargo.toml"

permissions:
  pull-requests: write

concurrency:
  group: witness-delta-${{ github.event.pull_request.number }}
  cancel-in-progress: true

jobs:
  coverage-delta:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
        with: { path: head, fetch-depth: 0 }
      - uses: actions/checkout@v5
        with:
          path: base
          ref: ${{ github.event.pull_request.base.sha }}
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Build witness
        working-directory: head
        run: cargo build --release
      - name: Compute manifest delta
        continue-on-error: true
        run: |
          ./head/target/release/witness diff \
            --base base/path/to/manifest.json \
            --head head/path/to/manifest.json \
            --format json > delta.json
      - uses: actions/upload-artifact@v4
        with:
          name: witness-delta-pr-${{ github.event.pull_request.number }}
          path: delta.json
          retention-days: 14
      - uses: peter-evans/create-or-update-comment@v4
        with:
          issue-number: ${{ github.event.pull_request.number }}
          body: |
            <!-- witness-delta-bot -->
            ## Coverage Delta
            See artifact for the full diff.
```

## Job 2 — Compliance composite action

### What rivet's compliance action does (`rivet/.github/actions/compliance/`)

The action.yml installs rivet from source or pre-built binary, runs
`rivet validate`, `rivet stats`, exports an HTML dashboard with:
`index.html` (overview), `requirements.html`, `documents.html`,
`matrix.html` (traceability), `coverage.html`, `validation.html`,
`config.js` (runtime nav), and tars the result.

Inputs: `report-label`, `homepage`, `theme`, `offline`, `rivet-version`,
`output`, `archive`, `archive-name`, `project-dir`.
Outputs: `report-dir`, `archive-path`, `artifact-count`, `validation-result`.

### Witness compliance bundle

What evidence witness must produce on release:
- Coverage report (JSON + text from `witness report`)
- Branch manifests (`.witness.json` files)
- In-toto predicates (unsigned, ready for sigil)
- Build metadata (witness version, cargo / rustc versions, build date)
- README.txt explaining what's in the bundle

### `witness compliance-bundle` subcommand (v0.4 implementation, optional)

The action can do this orchestration in shell, but a single
`witness compliance-bundle <run.json> <output-dir>` keeps the action
thin. Decision: ship the action as shell-orchestrating `witness
report` + `witness predicate` for v0.4; add the convenience
subcommand later if the action grows.

### Composite action (drop into `.github/actions/compliance/action.yml`)

Full YAML — agent brief had the complete file. Highlights:

- Inputs: `report-label`, `run-json`, `modules` (JSON array of module
  paths), `include-manifests`, `output`, `archive`, `archive-name`,
  `offline`.
- Outputs: `report-dir`, `archive-path`, `predicates`.
- Steps: build witness → determine label → generate coverage report →
  generate predicates per module → bundle manifests → write README →
  create archive.

### Wiring into `release.yml`

Add a step after artefact download, before `gh release create`:

```yaml
      - uses: ./.github/actions/compliance
        id: evidence
        with:
          report-label: ${{ github.ref_name }}
          run-json: ''           # set if the release pipeline produces a run
          modules: '[]'          # set per-module if predicates desired

      - name: Flatten artifacts
        run: |
          mkdir -p release-assets
          find artifacts -type f \( -name "*.tar.gz" -o -name "*.zip" \) \
            -exec cp {} release-assets/ \;
          [ -f "${{ steps.evidence.outputs.archive-path }}" ] && \
            cp "${{ steps.evidence.outputs.archive-path }}" release-assets/
          cd release-assets && sha256sum * > SHA256SUMS.txt
```

## Summary

**`witness diff`** is the v0.4 CLI feature blocking the delta workflow.
**`witness predicate`** already exists; the compliance action just
orchestrates `witness report` + `witness predicate` + tar.

**New v0.4 CLI features identified:**
1. `witness diff --base ... --head ... --format json` — the delta
   workflow's input.
2. (Optional) `witness compliance-bundle <run.json> <output-dir>` —
   convenience wrapper. Skip for v0.4; ship action as shell.

## Cited rivet source

| Path | Lines | What's there |
|---|---|---|
| `rivet/.github/workflows/rivet-delta.yml` | 1-259 | full workflow |
| `rivet/.github/actions/compliance/action.yml` | 1-210 | composite action |
| `rivet/.github/actions/compliance/README.md` | — | usage notes |

Sub-agent's full agent.yml drafts saved verbatim in this file's
predecessor; trimmed here for brevity but the structure above is
faithful to what shipped.
