# Changelog

All notable changes to witness are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [SemVer 2.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.10.0] — 2026-04-29

### Headline — signed evidence chain, end to end

v0.10.0 closes the gap three of four v0.9.10 evaluators converged on:
the predicate carried branch-coverage *summary*, not the MC/DC truth
tables. Now `witness predicate --kind mcdc` emits a Statement whose
payload is the per-decision truth table itself, content-bound by a
sha256 of the canonical JSON. The release tarballs themselves are
signed via cosign-OIDC. The vocabulary that confused fresh-eyes
evaluators (masking, unique-cause, polarity, ambiguous_rows) gets a
431-line `docs/concepts.md` page with worked examples.

### Added — `witness-mcdc/v1` predicate type

`witness predicate --kind mcdc` builds an in-toto Statement carrying:

- The full `McdcReport` (overall counts + per-decision verdicts +
  truth tables + condition pairs + interpretation strings).
- A `report_sha256` binding the envelope payload to a content hash,
  so signature verification implies truth-table integrity.
- Two subjects: the instrumented module + the original (pre-
  instrument) module, both with sha256 digests.
- Standard in-toto Statement frame.

Old `witness predicate` (now `--kind coverage`, the default) keeps
its `witness-coverage/v1` shape unchanged. Subjects: instrumented
plus original (when present).

### Added — `original_module` populated on every predicate

`witness instrument` now records the SHA-256 of the input
(pre-instrumentation) module in the manifest as
`original_module_sha256`. `witness predicate` reads it and emits a
second Statement subject automatically. Closes the long-standing
"original_module: null on every envelope" gap (E1 BUG-3).

`#[serde(default)]` on the new manifest field, so v0.9.x manifests
keep loading.

### Added — sigstore-OIDC keyless release signing

`.github/workflows/release.yml` adds a cosign step after the
flatten + before the GitHub-release create. Each asset gets a `.sig`
+ `.cert` pair. Verify downstream:

```sh
cosign verify-blob \
  --certificate-identity 'https://github.com/pulseengine/witness/.github/workflows/release.yml@refs/tags/v0.10.0' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  --signature witness-v0.10.0-x86_64-apple-darwin.tar.gz.sig \
  --certificate witness-v0.10.0-x86_64-apple-darwin.tar.gz.cert \
  witness-v0.10.0-x86_64-apple-darwin.tar.gz
```

No long-term key custody — signatures are bound to the workflow
identity via the GitHub OIDC token. Closes E1 blocker B3 / DOCS-2.

### Added — `interpretation_polarity` field on every MC/DC report

The truth-table `c0=T` columns record the **wasm br_if value**
(post-`i32.eqz; br_if` lowering), not the source-level condition
value. v0.9.x reports left this convention undocumented; reviewers
reading reports cold confused themselves. v0.10.0 adds an
`interpretation_polarity: "wasm-early-exit"` field on every
`McdcReport` so consumers can detect the convention. The full
explanation, with worked examples, lives at `docs/concepts.md` §4.

This is non-breaking: the on-the-wire semantics don't change, just
the documentation. v0.10.x may add an opt-in
`witness report --polarity source` that emits source-equivalent
outcomes (with the inversion table baked in) once the conversion
is fully tested across the verdict suite.

### Added — `docs/concepts.md` glossary + worked examples

431 lines closing E2's docs gap. Seven sections:

1. The 60-second pitch (Go/Python register, no Rust jargon).
2. **Vocabulary table** — 22 rows, plain-English definitions, examples,
   landing version cited from CHANGELOG.
3. The leap-year fixture walked row by row, sentence-by-sentence.
4. **The polarity inversion, named explicitly** with worked F-then-T
   example proving columns invert exactly.
5. The MC/DC criterion in plain language.
6. The signed evidence chain — what v0.9.x signs vs what v0.10.0
   signs.
7. DO-178C / post-preprocessor C lineage in three paragraphs.

### Added — `witness-mcdc-checker` extracted crate

The MC/DC pair-finder (`find_independent_effect_pair`, ~70 LoC) is
lifted into a no-deps crate auditors can read in isolation:

- `crates/witness-mcdc-checker/Cargo.toml` — pure `Vec` + `BTreeMap`
  + primitives, no runtime deps. Workspace member.
- `crates/witness-mcdc-checker/src/lib.rs` — `Row`, `Interpretation`,
  `find_independent_effect_pair` lifted verbatim.
- `crates/witness-mcdc-checker/tests/properties.rs` — 4 `proptest`
  property tests (256 cases each): outcome-differs, target-differs,
  non-target-compatibility, masking-vs-unique-cause interpretation.

`witness-core::mcdc_report` re-exports the function so callers
upstream don't need to migrate. Original tester Tier 2 #2.

### Added — Four published JSON Schemas at `docs/schemas/`

JSON Schema draft 2020-12 documents for every URL-stable schema:

- `witness-mcdc-v1.json` — McdcReport + nested types, includes the
  v0.9.7 `br-table-arm` interpretation extension and the v0.10.0
  `interpretation_polarity` field.
- `witness-coverage-v1.json` — in-toto Statement + Subject + Digests
  + Coverage predicate body.
- `witness-rivet-evidence-v1.json` — rivet evidence YAML schema
  (JSON-schema-validated after YAML→JSON conversion).
- `witness-trace-matrix-v1.json` — V-model traceability matrix.

`docs/schemas/README.md` lists each schema, its purpose, the URL,
and a `curl | jq` validation example. CI gains a (non-gating)
`schemas` job that validates each shipped fixture against the
matching schema. Closes original Tier 2 #4.

### Added — `SOURCE_DATE_EPOCH` honoured in predicate timestamps

When `SOURCE_DATE_EPOCH` is set in the environment, `witness
predicate` (both kinds) uses it as the `measured_at` timestamp.
Reviewers re-running the same instrumented module + harness against
the same epoch get a byte-identical predicate. Per
<https://reproducible-builds.org/docs/source-date-epoch/>.

Path stripping: absolute module paths get reduced to project-
relative when they fall under cwd, otherwise basename. Predicates
no longer leak machine-specific paths into the signed body.

### Changed — `TraceHealth.ambiguous_rows` → `trace_parser_active`

The old name was misleading: both fully-proved and fully-gap runs
set it to `true` whenever trace memory had data, which reviewers
read as an *error* indicator. New name: `trace_parser_active` —
"the trace-buffer parser produced per-iteration rows."

`#[serde(alias = "ambiguous_rows")]` on the field so v0.9.x
run.json files keep loading. The legacy alias sunsets in v0.11.

### Verified

- 100 tests pass across the workspace (8 + 0 + 8 + 8 + 60 + 10 + 4 +
  doc-tests). Includes 5 new predicate tests, 14 new
  `witness-mcdc-checker` tests (10 unit + 4 property), 2 new
  `SOURCE_DATE_EPOCH` tests.
- `cargo fmt --check` + `cargo clippy --all-targets --release -- -D warnings`
  clean across 5 crates (witness, witness-core, witness-mcdc-checker,
  witness-viz, witness-component).
- End-to-end live smoke verified: `witness new` → `./run.sh` →
  `verdict-evidence/` bundle → `witness predicate --kind mcdc` →
  output Statement carries 2 subjects + report + report_sha256 +
  `predicateType: https://pulseengine.eu/witness-mcdc/v1`.

### Notes for v0.10.x and beyond

The proposal at `docs/proposals/v0.10.0.md` carries 21 items across
must-ship / should-ship / nice-to-ship tiers. v0.10.0 ships the must-
ship tier in full plus several should-ship items. The remaining
backlog (rmcp migration, per-DWARF-inlined-context outcomes,
non-Rust frontend, full README rewrite) is tracked for v0.10.x and
v0.11.

## [0.9.12] — 2026-04-28

### Added — `witness quickstart` embedded subcommand

The 200-line `docs/quickstart.md` is now bundled in the binary via
`include_str!`. Users on a fresh machine without the repo can run:

```sh
witness quickstart | less
witness quickstart > my-notes.md
```

…and get the full 10-minute walkthrough — install, scaffold,
modify-and-rerun, sign, visualise, MCP smoke, LCOV. Same source of
truth as `docs/quickstart.md` and the GitHub-hosted copy.

### Added — `docs/proposals/v0.10.0.md`

The v0.10.0 release proposal lands in the repo. Synthesised from
four independent v0.9.10 evaluations (safety-critical Rust lens,
Go/Python fresh-user lens, curious side-project lens, rmcp
migration assessment). 7 sections, 21 items across must-ship /
should-ship / nice-to-ship tiers, 5-phase sequencing plan, risk
register, acceptance criteria.

Headline framing for v0.10.0: **"signed evidence chain, end to
end."** Closes three structural gaps three of the four evaluators
converged on:

1. Signed predicate carries branch coverage only; MC/DC truth tables
   sit unsigned next to it. v0.10.0 ships a `witness-mcdc/v1`
   predicate type carrying truth tables.
2. Truth-table polarity is wasm-level (br_if value) but reads as if
   source-level. Either normalise on emit or document the inversion
   table — proposal flags this as the design decision the user
   makes before Phase C starts.
3. Release tarballs themselves have no provenance. v0.10.0 wires
   sigstore-OIDC release signing for the witness binary.

The blog post for next week's pulseengine.eu drop ("witness ships
the truth table, not the percentage") is staged at
`pulseengine.eu/content/blog/2026-05-05-...md` (sibling repo,
draft=true). Adapted from the curious-side-project evaluator's
draft into maintainer's-voice build-in-public framing.

### Verified

- `witness quickstart` prints all 200 lines of `docs/quickstart.md`
  with no runtime cost (data is included at compile time).
- 50 unit + 8 integration + 7 mcp tests pass.
- workspace `cargo fmt --check` + `cargo clippy --all-targets -D warnings` clean.

### Notes for v0.10.0

The proposal is the next release's source of truth. v0.9.x is
otherwise scope-locked — feedback items from the four evaluations
that were not closed in v0.9.11 are explicitly deferred to v0.10.0.

## [0.9.11] — 2026-04-28

### Five tester-feedback items in one release

A second round of fresh-eyes evaluations on v0.9.10 (one
safety-critical Rust, one Go/Python new to wasm, one curious
side-project, one rmcp-migration assessor) surfaced three blockers
plus polish. v0.9.11 closes all five items the rmcp evaluator's
deferral recommendation kept in the v0.9.x line.

### Added — `witness new` scaffold writes the `verdict-evidence/` layout

Tester blocker #1: `witness new` produced `run.json` +
`instrumented.wasm.witness.json`, but `witness viz` wanted
`verdict-evidence/<name>/{report.json, manifest.json}`. No bridge.
Fresh users got stuck.

Now `run.sh` ends with:

```bash
EVIDENCE_DIR="verdict-evidence/<name>"
mkdir -p "$EVIDENCE_DIR"
witness report --input run.json --format mcdc-json > "$EVIDENCE_DIR/report.json"
cp instrumented.wasm.witness.json "$EVIDENCE_DIR/manifest.json"
echo "Bundle written under verdict-evidence/. Browse with:"
echo "  witness viz --reports-dir verdict-evidence"
```

Three commands from `witness new` to a running visualiser:

```sh
witness new my-fixture
cd my-fixture && ./run.sh
witness viz --reports-dir verdict-evidence
```

### Changed — `witness new` scaffolds the typed-args form by default

Tester blocker #3: the v0.9.10 scaffold used `core::hint::black_box`
inside zero-arg `run_row_*` exports. This poisoned DWARF line
attribution to `hint.rs:491` after any user edit (also propagating
to LCOV `BRDA` records). The v0.9.6 `--invoke-with-args` path does
NOT have this problem.

v0.9.11 scaffolds **one typed-arg export**:

```rust
#[unsafe(no_mangle)]
pub extern "C" fn is_leap(year: i32) -> i32 {
    is_leap_year(year as u32) as i32
}
```

Driven from `run.sh` via `--invoke-with-args 'is_leap:2001'` ×5.
Line attribution lands on the predicate's source line, not on
`hint.rs`.

### Added — MCP `initialize` handshake (and `notifications/initialized`)

Tester blocker #2: spec-compliant MCP clients (Claude Desktop,
Cursor, the official rmcp client) send `initialize` before any
other call. v0.9.10 returned `-32601 method not found`, breaking
every off-the-shelf client. The 17-line patch advertises tools
capability + serverInfo + protocolVersion echo, supporting
`2024-11-05`, `2025-03-26`, and `2025-06-18`:

```json
{
  "jsonrpc": "2.0", "id": 1,
  "result": {
    "protocolVersion": "2025-06-18",
    "capabilities": { "tools": { "listChanged": false } },
    "serverInfo": { "name": "witness-viz", "version": "0.9.11" }
  }
}
```

`notifications/initialized`, `notifications/cancelled`, and `ping`
are also handled. Hand-rolled rather than rmcp-based per the
migration evaluation (saved 6-8 hours, avoided 14+ transitive
crates, deferred to v0.10.0+ if a real MCP feature need surfaces).

### Changed — chatty success messages

Tester polish item: `instrument`, `run`, `predicate`, `attest` were
silent on success while `keygen`, `verify` were chatty. Same
asymmetry made evaluators re-run commands thinking they had failed.
Now all five print a `wrote <path> (<bytes> bytes)` line on
success. `attest` additionally hints the verify command.

### Added — `docs/quickstart.md`

Landed the QUICKSTART.md draft from the first-round evaluation as
the canonical 10-minute guide. 200+ lines covering: install,
scaffold, modify-and-rerun, sign + verify, visualise, MCP smoke,
LCOV. Updated for v0.9.11 specifics (typed-args default, auto-
bundle, chatty success, MCP initialize working).

### Verified

- 7/7 MCP tests pass including new `initialize_handshake_returns_server_info`.
- 50 unit + 8 integration tests pass.
- Scaffold end-to-end: `witness new` → `./run.sh` → `witness viz` works
  with no manual glue. Verified against the live binary.
- MCP initialize live-tested: `protocolVersion: "2025-06-18"`,
  `serverInfo.name: "witness-viz"` returned.
- Three independent fresh-eyes evaluations (safety-critical / Go-Python /
  curious side-project) ran against v0.9.10 + the QUICKSTART.md
  draft. All three deliverables landed friction logs that informed
  the v0.9.12 / v0.10.0 backlog.

### Notes for v0.9.12 / v0.10.0

Substantive findings from the safety-critical evaluator that
deferred (need design):

- **Signed predicate carries branch coverage, not MC/DC truth tables.**
  `witness predicate` builds the `witness-coverage/v1` Statement
  from the branch report, leaving the MC/DC report unsigned. The
  "signed evidence chain for MC/DC" pitch isn't fully delivered by
  the artefact. v0.10.0 should add a `witness-mcdc/v1` predicate
  type carrying the truth tables.
- **Truth-table polarity needs documentation.** `c0=T` in the truth
  table is the wasm-level br_if value (taken/not-taken),
  not the source-level condition value. Reviewers reading the
  report cold can be confused. v0.9.12 doc fix.
- **Release-binary provenance.** `SHA256SUMS.txt` is unsigned. For a
  tool whose pitch is signed evidence, the tool's own release
  pipeline has no signed evidence. v0.10.0 — sigstore-OIDC.

Other smaller items go into v0.9.12.

## [0.9.10] — 2026-04-28

### Added — `witness new <fixture>` template scaffold

Tester review Tier 3 #1: `witness new` drops a working fixture
project (Cargo.toml + src/lib.rs + build.sh + run.sh + .gitignore)
into `<dir>/<name>/` so first-time users skip the fiddly setup that
trips everyone:

- `[lib] crate-type = ["cdylib"]` (not the rustc default)
- `[profile.release] debug = true` (DWARF is what witness uses to
  group br_ifs into decisions)
- `panic = "abort"` (no_std)
- `#![no_std]` + `#[panic_handler]` plumbing
- `core::hint::black_box` around inputs (else rustc constant-folds
  the predicate and the row records nothing)
- `wasm32-unknown-unknown` target (else the Component Model preflight
  introduced in v0.9.4 rejects the input)

```sh
$ witness new my-fixture
Created witness fixture at ./my-fixture

  cd ./my-fixture
  ./build.sh         # builds verdict_my_fixture.wasm
  ./run.sh           # instruments + runs + reports

Five rows drive the leap-year predicate. Expected MC/DC: 1/1
decisions full, 2 conditions proved (rustc fuses the third).
```

The scaffolded predicate is the textbook ISO leap-year rule. Modular
arithmetic blocks rustc's constant-fold-to-bitwise transformation
that `(a && b) || c` over plain bools triggers (the same hazard
that makes `range_overlap` and `mixed_or_and` zero-decision in our
verdict suite). End-to-end on the scaffolded fixture: 1 decision
reconstructed at `lib.rs:40`, both reconstructed conditions proved
under masking MC/DC.

CLI flags: `--dir <parent>` (default cwd), `--force` (overwrite an
existing target). Names must be ASCII alphanumeric / `-` / `_` so
they're valid crate names.

### Verified

- `witness new test-fixture --dir /tmp` → builds clean, runs end-to-
  end, reports 1/1 full MC/DC + 2 proved.
- 50 unit + 8 integration tests pass.
- `cargo fmt --check` + `cargo clippy --all-targets -D warnings`
  both clean.

### Notes for v0.9.x — what remains pending

After v0.9.10, the tester-review item list is largely closed for the
v0.9.x line. Open items:

- Tier 1 — **Per-DWARF-inlined-context outcome tracking**. Largest
  remaining functional gap; needs a design pass before implementation.
- Tier 2 — sigstore-OIDC release signing; qualifiable MC/DC checker
  crate extraction; differential testing against rustc-mcdc; JSON
  schemas in `docs/schemas/`.
- Tier 3 — README split into 5-min-pitch + concepts + cli +
  qualification; viz API completeness (`/openapi.json`).

These cluster naturally into v0.10.0 (qualification + schemas) and
v0.10.x (docs + viz polish).

## [0.9.9] — 2026-04-27

### Added — `pulseengine/witness/.github/actions/witness@v1`

Tester review Tier 3 #2: a reusable composite GitHub Action so any
Rust crate can adopt the witness pipeline in 8 lines of YAML.

```yaml
- uses: pulseengine/witness/.github/actions/witness@v1
  with:
    module: build/app.wasm
    invoke: |
      run_row_0
      run_row_1
      run_row_2
    upload-to-release: ${{ startsWith(github.ref, 'refs/tags/') }}
```

The action:
1. Downloads the latest (or pinned) `witness` + `witness-viz` release
   tarball for the runner's platform (linux x86_64/aarch64, macOS
   x86_64/aarch64). No witness compile cost in adopters' CI.
2. Runs `instrument → run → predicate → attest`.
3. On tag-push events, uploads predicate, signed DSSE envelope, and
   verifying public key to the matching GitHub release.

Inputs: `witness-version`, `module`, `invoke`, `invoke-with-args`
(v0.9.6+), `output-dir`, `predicate-name`, `sign`,
`upload-to-release`. Outputs paths to every artefact for downstream
steps to consume.

`.github/actions/witness/README.md` documents adoption + verification
on the consumer side (`witness verify` or `cosign verify-blob`).

### Added — `.github/ISSUE_TEMPLATE/`

Tester review Tier 3 #6: empty issue tracker hides feedback channels.
Three templates land:

- **`instrument-failure.md`** — for `witness instrument` errors,
  manifest-content surprises. Asks for witness version, rustc target,
  whether the input is a core module or a Component.
- **`mcdc-surprise.md`** — for unexpected MC/DC numbers (proved/gap/
  dead/full). Asks for the source-level decision, manifest excerpt,
  what the user already ruled out.
- **`harness-question.md`** — for questions about `--harness <cmd>`,
  v1/v2 schemas, and non-wasmtime runtimes. Includes schema-version
  checkboxes.

Plus `config.yml` redirecting open-ended questions to Discussions.

### Changed — release artefact rename

Tester review Tier 3 #5: the v0.7.0+ wasm release asset shipped as
`witness-component-vX.X.X-wasm32-wasip2.wasm` and got mistaken for an
instrumentable target. It's actually the WASI 0.2.9 reporter
component (used by sigil/loom plumbing tests, never as a `witness
instrument` input). Renamed to:

```
witness-reporter-component-vX.X.X-wasm32-wasip2.wasm
```

Workflow `::notice::` annotation now spells out the purpose so
release-notes readers see the intent at a glance.

### Notes for v0.9.x — Tier 1+ still pending

- **Per-DWARF-inlined-context outcome tracking** — the original
  v0.8.3 fold-in target. Needs design pass before implementation.
- **Per-arm `brval`/`brcnt` for `br_table`s** — multi-condition
  match-guard MC/DC.
- **`witness new <fixture>` template** — Tier 3 #1, would eliminate
  90% of new-user friction. Half-scaffolded by the action above.
- **JSON schemas in `docs/schemas/`** — Tier 2 #4. Lock the four
  cross-tool contracts (`witness-mcdc/v1`, `witness-coverage/v1`,
  `witness-rivet-evidence/v1`, `witness-trace-matrix/v1`).
- **Qualifiable MC/DC checker crate extraction** — Tier 2 #2. Lift
  `find_independent_effect_pair` (~70 LoC) into `witness-mcdc-checker`.

### Verified

- `cargo test --workspace --release` — **50 unit + 8 integration
  tests pass**.
- `cargo clippy --all-targets -D warnings` clean.
- The action.yml structure mirrors the `.github/actions/compliance`
  pattern already in use; both are composite shell-step actions.

## [0.9.8] — 2026-04-27

### Added — `WITNESS_TRACE_PAGES` env override at instrument time

Tester review Tier 1 #5: the `TRACE_DEFAULT_PAGES = 16` constant
comment promised the env override; the code didn't honour it. v0.9.8
fixes the broken promise.

`witness instrument` now reads `WITNESS_TRACE_PAGES` and uses that
value (1..=65536, the wasm32 memory cap) for the trace memory's
declared size + the capacity field that gets baked into the trace
header at reset. Out-of-range, non-numeric, or unset values fall
back to `TRACE_DEFAULT_PAGES` (16 = 1 MiB = ~262K records).

```sh
# Default — 16 pages = 1 MiB trace memory
witness instrument app.wasm -o app.inst.wasm

# Big workloads (e.g. fuzz harnesses) — bump to 64 pages = 4 MiB
WITNESS_TRACE_PAGES=64 witness instrument app.wasm -o app.inst.wasm

# Memory-constrained embedded targets — drop to 4 pages = 256 KiB
WITNESS_TRACE_PAGES=4 witness instrument app.wasm -o app.inst.wasm
```

The setting is per-instrumented-module (baked into the wasm itself);
the runner reads back `pages_allocated` at run time and stores it in
`TraceHealth` so reviewers know what was provisioned without rerunning
`witness instrument`.

### Added — `TraceHealth.bytes_used` + `TraceHealth.pages_allocated`

`run_record.rs::TraceHealth` gains two new fields:

```rust
pub struct TraceHealth {
    pub overflow: bool,
    pub rows: u64,
    pub ambiguous_rows: bool,
    /// v0.9.8 — total trace memory bytes consumed across all rows.
    pub bytes_used: u64,
    /// v0.9.8 — pages of trace memory the instrumented module
    /// allocates (post-`WITNESS_TRACE_PAGES` resolution).
    pub pages_allocated: u32,
}
```

Both are `#[serde(default)]` so v0.9.7 records still deserialise.
The legacy `__witness_trace_bytes=N` note in the `invoked` list stays
for tooling that hasn't upgraded.

A run on `triangle` reports cleanly:

```json
"trace_health": {
  "overflow": false, "rows": 4, "ambiguous_rows": true,
  "bytes_used": 44, "pages_allocated": 16
}
```

Reviewers can now eyeball the headroom: a 1 MiB trace memory used 44
bytes — plenty of room — versus a hypothetical 700 KiB used out of
1 MiB which signals "consider `WITNESS_TRACE_PAGES=32` next time".

### Verified

- `trace_pages_env_override_round_trip` test: 32 → 32, 0 → default,
  garbage → default, 65537 → default, unset → default.
- End-to-end smoke: `WITNESS_TRACE_PAGES=8 witness instrument` →
  `pages_allocated=8` in run output; default → `pages_allocated=16`.
- `cargo test --workspace --release` — **50 unit tests + 8
  integration tests pass**.
- `cargo clippy --all-targets -D warnings` clean.

### Notes for v0.9.x — Tier 1 still pending

- **Per-DWARF-inlined-context outcome tracking** (original v0.8.3
  fold-in target). The httparse `7/67` Boolean MC/DC ratio still
  reflects this: when one source-level decision is inlined into many
  call sites, our reporter aggregates rows per (function, source-
  line) but can't distinguish per-call iterations of an inlined
  decision. Big architectural change; needs design pass.
- **Per-arm `brval`/`brcnt` for `br_table`s** — would let MC/DC
  pair-finding apply to multi-condition `match` guards
  (`match (x, y) { (T, T) => ... }`). Decision-shape generalization.

## [0.9.7] — 2026-04-27

### Added — per-target br_table decision reconstruction

Tester review Tier 1 #4: `BranchKind::BrTableTarget` and
`BranchKind::BrTableDefault` exist in the manifest, and per-arm
counters were already emitted, but `decisions.rs:25` had:

> Per-target br_table decision reconstruction. BrTableTarget /
> BrTableDefault entries are not grouped into Decisions in v0.4

— so every br_table arm stayed as a strict-per-target counter, not a
groupable decision. This bit `httparse` and `json_lite` hardest:
both compile their dispatch tables to br_tables (172 + 45 br_table
arms across the two), and reviewers couldn't see arm-coverage at the
decision level.

v0.9.7 closes that:

1. **Reconstruction pass** — `group_into_decisions` gains a second
   bucket. Entries are partitioned by kind: BrIf entries go through
   the existing source-line-cluster grouping, BrTable* entries are
   grouped by `(function_index, source_file, source_line)` (every
   target of one Wasm `br_table` instruction shares those three).
   Each group with `>= 2` entries becomes a Decision.

2. **Reporter awareness** — `analyse_decision` detects br_table-shape
   decisions (every condition's branch entry is `BrTableTarget` or
   `BrTableDefault`) and uses **per-arm counters** for status:
   - `BranchHit.hits > 0` → `ConditionStatus::Proved` with
     `interpretation = "br-table-arm"`
   - `BranchHit.hits == 0` → `ConditionStatus::Dead`

   This is honest per-arm coverage, not Boolean MC/DC. The truth-
   table view stays empty for these decisions (no per-row brval/brcnt
   data — that's ARM-014 territory), but the conditions list reads
   true.

3. **Headline-ratio preservation** — br_table-shape decisions DO NOT
   bump `decisions_total` / `decisions_full_mcdc` in the overall
   counts. The MC/DC ratio is reserved for Boolean decisions where
   independent-effect proofs apply. Br_table arms count in
   `conditions_proved` / `conditions_gap` / `conditions_dead`, since
   those reflect arm-hit truth.

### Headline numbers move

Re-running the existing 40-row httparse harness:

```
                Before v0.9.7         After v0.9.7
br_if decs       67                    67  (unchanged)
br_table decs    0  (skipped)          19  (newly reconstructed)
proved           28                    54   ← +26 from arm coverage
gap              46                    46  (unchanged)
dead             108                   274  (br_table 0-hit arms)
total conditions 181                   374
full MC/DC ratio 7/67                  7/67  (Boolean ratio preserved)
```

`json_lite` similarly: 26 proved → 36 proved (+10 from br_table arm
coverage). The proved count is now an **honest reflection of which
match arms the test corpus exercises**, not silently zero.

### Notes for v0.9.x — Tier 1 still pending

- **Trace-buffer overflow telemetry + `WITNESS_TRACE_PAGES`** —
  comment promises the env override; code doesn't honour it yet.
- **Per-DWARF-inlined-context outcome tracking** — original v0.8.3
  fold-in target. Would lift the `7/67` Boolean MC/DC ratio for
  inlined-decision verdicts (httparse particularly).
- **Per-arm brval/brcnt for br_tables** — ARM-014. Truth-table view
  for match dispatches; would let MC/DC pair-finding apply to
  multi-condition `match` guards (`match (x, y) { (T, T) => ...}`).

### Verified

- workspace `cargo test --release` passes (49 unit + 8 integration).
- `cargo clippy --all-targets --release -- -D warnings` clean.
- httparse + json_lite re-run end-to-end against the suite; numbers
  reproduce as documented above.

## [0.9.6] — 2026-04-27

### Added — `--invoke-with-args 'name:val[,val...]'`

Tester review Tier 1 #2: embedded mode required zero-arg exports, so
users had to wrap inputs in `core::hint::black_box` to stop rustc
constant-folding the row entry. New flag accepts positional values
parsed against `func.ty()`:

```
witness run app.wasm \
  --invoke-with-args 'is_leap:2024' \
  --invoke-with-args 'parse_request:0,12345,3.14'
```

The export's Wasm signature drives type coercion. `i32` / `i64` /
`f32` / `f64` parameters are all supported; `v128` and reference
types remain wrapper-territory because they have no obvious CLI
encoding (the error explains that).

The flag composes with `--invoke` — no-arg entries process first,
then typed entries — so existing `run_row_*` workflows keep
working unchanged.

```rust
// New error paths:
//
// --invoke-with-args spec must be 'name:val[,val...]', got 'foo'
// spec 'two_args:42' has 1 values but the export declares 2 params
// spec 'is_leap:abc' param 0: cannot parse 'abc' as i32 (...)
```

Adds clarity to the no-arg restriction in the docstring while keeping
backward compat: `--invoke` continues to require zero-arg exports
(simple, fast path), and users only reach for `--invoke-with-args`
when their function takes parameters they want to vary.

### Verified

- `invoke_with_args_positional_typed_call`: an `if (param i32) ...
  if/else ...` export gets called with `i32=1`; the then-branch
  counter increments; the invoked-list shows the export name without
  the spec.
- `invoke_with_args_arity_mismatch_errors`: spec with 1 value vs
  2-param export errors with both counts named.
- `cargo test --workspace --release` — **49 unit tests + 8
  integration tests + 0 failures**.
- `cargo clippy --all-targets -D warnings` clean.

### Notes for v0.9.x — Tier 1 still pending

- **Per-target br_table decision reconstruction** — half-implemented;
  finishing it lifts httparse and json_lite numbers without API
  changes. Next.
- **Trace-buffer overflow telemetry + `WITNESS_TRACE_PAGES`** —
  comment promises the env override; code doesn't honour it yet.
- **Per-DWARF-inlined-context outcome tracking** — the original
  v0.8.3 fold-in target.

## [0.9.5] — 2026-04-27

### Added — `witness-harness-v2`: MC/DC-capable subprocess protocol

The biggest functional gap from v0.9.2's tester review (Tier 1 #1):
harness mode shipped only counters, so MC/DC reconstruction silently
degraded to branch coverage in subprocess mode. v2 closes that gap.

The new schema extends `HarnessSnapshot` with optional `rows`, each
carrying everything embedded wasmtime mode reads after each
invocation:

```json
{
  "schema": "witness-harness-v2",
  "counters": { "0": 7, "1": 3 },
  "rows": [
    {
      "name": "run_row_0",
      "outcome": 1,
      "brvals": { "0": 1, "1": 0 },
      "brcnts": { "0": 1, "1": 1 },
      "trace_b64": "AAAA..."
    },
    { "name": "run_row_1", "outcome": 0, "brvals": { ... }, "brcnts": { ... }, "trace_b64": "..." }
  ]
}
```

The harness must call `__witness_trace_reset` and
`__witness_row_reset` between rows so each entry carries isolated
post-invocation state. `trace_b64` is base64-standard-encoded raw
`__witness_trace` memory (with the 16-byte header). Witness then
parses each row's trace via the **same** `parse_trace_records`
function the embedded path uses, producing per-iteration
`DecisionRow`s identical to what wasmtime would have written.

Backward compatibility:
- `witness-harness-v1` still works. Schema dispatch picks the
  counters-only path verbatim — no behaviour change for existing
  Node WASI / kiln / hardware-board harnesses.
- Unknown schema strings (e.g. `witness-harness-vfuture`) now error
  with both supported versions named in the message.

```rust
// New error path covered by tests:
//
// harness snapshot schema unsupported: expected `witness-harness-v1`
// or `witness-harness-v2`, got `witness-harness-vfuture`
```

### Added — Node WASI v2 reference example

The README's harness-mode section gains a 25-line v2 implementation:
loop over `rowNames`, reset between rows, snapshot each row's
`__witness_brval_*` / `__witness_brcnt_*` globals + the trace memory
buffer, encode base64, ship JSON. Drops in as a CI-runnable
replacement for v1 with no API surface change beyond the schema
field.

### Verified

- New `harness_v2_full_mcdc_round_trip` integration test: synthesises
  a v2 snapshot via `cat <<EOF`, feeds it through `--harness`,
  asserts counter ingestion, decision-row population, invoked-list
  preservation. Passes.
- New `harness_unknown_schema_is_rejected` test: validates the new
  schema-mismatch error message names both v1 and v2.
- Existing `harness_subprocess_round_trip` (v1) passes unchanged.
- Workspace `cargo fmt + clippy --all-targets -D warnings` clean;
  47 + 2 = **49 unit tests pass**, 0 failures.

### Notes for v0.9.x — Tier 1 still pending

Per the senior tester review, three Tier 1 items remain after v2:
- **`--invoke-with-args 'name:i32=42'`** — eliminate the
  `core::hint::black_box` wrapper-export pattern users currently
  reach for. Medium work, medium-high impact. Next.
- **Per-target br_table decision reconstruction** — finishes the
  half-implemented BranchKind::BrTableTarget path; would lift
  httparse and json_lite numbers without API changes.
- **Trace-buffer overflow telemetry + `WITNESS_TRACE_PAGES`** — the
  comment promises the env override; the code doesn't honour it yet.

## [0.9.4] — 2026-04-27

### Tier 0 — tester-review feedback addressed

A senior tester drove every advertised surface of v0.9.2 and produced
a thorough maturity assessment. v0.9.4 is the "fix-in-an-afternoon,
huge UX impact" tier — five concrete items, all shipped.

#### 1. `witness-viz` is now in release tarballs

The most embarrassing miss from v0.9.0: the visualiser was pitched as
the headline feature, but `release.yml` only built the `witness` and
`witness-component` matrix. Tester reproduced: every release tarball
shipped without `witness-viz`, so `witness viz` errored out of the
box on a fresh install.

Fixed by extending the existing build-binaries matrix to also build
`crates/witness-viz/` (its own standalone workspace) per target and
copy both binaries into the same staging directory before packaging.
Tarballs from v0.9.4 onward contain `witness` AND `witness-viz` for
all five platforms (linux x86_64, linux aarch64, macos x86_64, macos
aarch64, windows x86_64).

#### 2. Component preflight in `witness instrument`

Today walrus reports an opaque `not supported yet` when a Wasm
Component is fed to `instrument`. Tester (and any first-time user
who points witness at a `wasm32-wasip2` build) can't tell whether
witness is broken, the file is corrupt, or they need a different
target.

`instrument_file()` now peeks at bytes 0..8: `\\0asm\\01\\00\\00\\00`
is a core module (passes through), `\\0asm\\0d\\00\\01\\00` is a
Component (returns `Error::InputIsComponent`):

```
$ witness instrument app.component.wasm -o app.inst.wasm
Error: input 'app.component.wasm' is a Wasm Component, not a core module.
  witness instruments core modules only. Either:
    (a) compile your crate to wasm32-unknown-unknown or wasm32-wasip1
        (instead of wasm32-wasip2 / Component-Model targets), or
    (b) extract the inner core module:
        wasm-tools component unbundle 'app.component.wasm' --module-out core.wasm
        witness instrument core.wasm
```

Two new unit tests cover the preflight: synthetic component header
returns the new error variant; core module passes through. Existing
8 instrument-tests still pass.

#### 3. Harness-mode protocol documented

The `witness-harness-v1` schema (`HarnessSnapshot { schema, counters }`)
was hidden in `run_record.rs:125` — tester recovered it by `strings`-
grepping the binary. Now there's a full "Harness-mode protocol"
section in the README with a 10-line Node WASI reference
implementation. Includes the **caveat** that v1 transports counters
only, so MC/DC reconstruction degrades to branch coverage in
subprocess mode — when you want truth tables, use embedded.

#### 4. `Error::RequirementMap` separates schema from runtime errors

Tester saw rivet-evidence report YAML schema errors as
`wasm runtime error: ...` because `RequirementMap::load` funnelled
parse failures through `Error::Runtime(anyhow!(..))`. Added a
dedicated variant:

```rust
#[error("requirement-map config malformed at {path}")]
RequirementMap {
    path: PathBuf,
    #[source]
    source: anyhow::Error,
}
```

Plus a third variant `Error::InputIsComponent { path }` for the
preflight check above. Two error-tagging issues, one PR.

#### 5. Walrus name-section warning silenced by default

`walrus::module: in name section: function index 0 is out of bounds
for local` fires on every well-formed cdylib produced by stable
rustc. Cosmetic, but it makes good output look broken.

`init_tracing()` now applies `walrus=error` filter at default
verbosity (no `-v`). At `-v` or higher the filter is lifted so the
warning is available when you actually want it. Tester's
specific complaint quoted in the comment for posterity.

### Bonus polish

- New `GET /healthz` route on witness-viz (200 + JSON with version
  + service name). Container-deployment friendly per tester suggestion.
- New `GET /api/v1/verdicts/{name}` route (plural alias for the
  existing singular form) — tester naturally typed plural after
  hitting `/api/v1/verdicts`. Both forms now work.

### Notes for v0.9.x

Tier 1 still pending (per tester review):

- **Harness mode v2** (carrying brvals/brcnts/trace) so subprocess
  mode produces MC/DC, not just branch coverage. Largest single
  functional gap remaining.
- **`--invoke-with-args 'name:42,2024'`** — eliminate the
  `core::hint::black_box` wrapper-export pattern.
- **Per-target br_table decision reconstruction** —
  `BranchKind::BrTableTarget` exists in the manifest but
  `decisions.rs` doesn't group them; finishing this lifts httparse
  and json_lite numbers without API changes.
- **Trace-buffer overflow telemetry** — `WITNESS_TRACE_PAGES` env
  override (the constant comment promises it; the code doesn't
  honour it yet) plus per-decision iteration counts in `TraceHealth`.

Tier 2/3 (qualification posture, DX) tracked for v0.9.x and v1.0.

### Implements / Verifies

- `cargo test --workspace --release` passes (49 tests, 2 new).
- `witness instrument <component>` returns the friendly preflight
  error; the synthetic-component unit test locks the behaviour.
- README's harness-mode section covers the v1 wire format with a
  runnable Node example.
- `gh release download v0.9.4` will (once published) contain both
  `witness` and `witness-viz` binaries.

## [0.9.3] — 2026-04-27

### Fixed — `json_lite` build under `-D warnings` (Linux CI)

The CI workflow runs with `RUSTFLAGS="-D warnings"`, which promotes
every Rust lint to an error. `json_lite` carried a single
`unused_mut` warning at `verdicts/json_lite/src/lib.rs:237` that's
been silently failing the verdict-suite step on every release tag
since v0.7.4 — the Release workflow does not set `-D warnings` so
release artefacts kept publishing, but CI was failing for the same
build reason.

```diff
-    let mut i = skip_ws(buf, 0);
+    let i = skip_ws(buf, 0);
```

Trivial change, rebound effect: the verdict suite now passes on
Linux CI for every release line going forward, so the green/red CI
badge tracks reality again.

### Verified — full verdict suite under `-D warnings`

Built every verdict (`leap_year`, `range_overlap`, `triangle`,
`state_guard`, `mixed_or_and`, `safety_envelope`, `parser_dispatch`,
`httparse`, `nom_numbers`, `state_machine`, `json_lite`,
`base64_decode`) with `RUSTFLAGS="-D warnings"` after a force-clean.
All 12 build clean. Workspace `cargo clippy --all-targets --release`
passes with `-D warnings` too.

### Implements / Verifies

- 47 unit + 7 integration tests pass; 30 Playwright tests pass.
- All 12 verdict fixtures build clean under the strictest lint
  config CI uses.
- Tags v0.8.2, v0.9.0, v0.9.1, v0.9.2 pushed in this session;
  Release runs queued for each.

## [0.9.2] — 2026-04-27

### Added — stacked coverage bars on the dashboard

The verdict scoreboard now ships an inline horizontal bar per row
showing the proved / gap / dead split visually. The TOTAL row gets
the same treatment plus a top-border to set it apart. CSS uses
gradient fills (light vs dark mode) — looks good in both color
schemes, no JS, no images.

```
verdict          branches  decisions  full MC/DC  coverage           proved  gap  dead
httparse         473       67         7/67        [██░░░░░░░░░]      28      46   108
nom_numbers      20        3          3/3         [██████████]       6       0    0
state_machine    14        5          4/5         [████████░░]       11      1    0
```

The bar is `width: 160px; height: 14px;` — fits in a table cell
without distorting the row height. Hover for `title` tooltip with
exact counts. The "full MC/DC" cell is now a fraction (`7/67`) so
the ratio is one glance away.

### Added — `base64_decode` verdict (12th fixture)

A new real-application verdict at `verdicts/base64_decode/`. Drives
the well-known `base64` v0.22 crate (with `default-features = false`,
`#![no_std]`) through 24 rows covering:

- STANDARD encoding (padded + unpadded)
- URL_SAFE alphabet (with the `-_` substitutions)
- Malformed input (invalid chars, misplaced padding, garbage)
- Edge cases (empty, single-char, all-pad)

Witness reconstructs **36 decisions** across the engine + alphabet
code paths. First run shows 11 proved / 19 gap / 70 dead conditions
— the gap and dead numbers reflect the same inlined-context issue
v0.9.3 will address (the `Engine::decode_slice` function inlines into
many call sites). Documented in the v0.9.x scoreboard.

Verdict count: **11 → 12**. Real-application fixtures: 4 → 5.

### Added — visual emphasis on the TOTAL row

`tr.total-row` now carries a 2px accent border-top + bg-alt
background, so the bottom row of the scoreboard reads as a summary
rather than another verdict.

### Notes for v0.9.x

- v0.9.3 — per-DWARF-inlined-context outcome tracking. The httparse
  7/67 and base64_decode 0/36 numbers both reflect this limit; the
  fix is the next single-number-bumping change.
- v0.9.x — visualiser self-witnessing CI step + gate ratchet
  (REQ-040). Foundations in place via the Playwright suite; the CI
  step needs a wasm32-wasip2 build of witness-viz first.
- v0.9.x — search box on the dashboard (`/search?q=`). Markup is
  HTMX-ready; design pending.

### Implements / Verifies

- 30 Playwright tests pass against the polished dashboard. New
  `tr.total-row` and `cov-bar` markup verified by Playwright (the
  existing `verdicts count` test now uses
  `a[href^="/verdict/"]` which is unambiguous).
- New verdict builds clean (658 KB wasm, 36 decisions reconstructed).

## [0.9.1] — 2026-04-27

### Added — gap drill-down view (`/gap/{verdict}/{decision}/{condition}`)

The reviewer-facing twin of MCP's `find_missing_witness`. For every
condition that isn't `proved`, witness-viz now renders a tutorial-
style page with three modes:

- **Gap conditions** — explains "to prove condition `cN` independently
  affects the decision, you need a row where `cN = X` and the outcome
  differs from row `R` (where `cN = !X`)". Renders the full required
  condition vector + a copy-paste Rust test stub:

  ```rust
  #[test]
  fn closes_gap_d2_c2() {
      // Verdict: parser_dispatch
      // Source: memchr.rs:40
      // Branch: 2
      //
      // TODO: drive the function so condition c2 evaluates to F and
      // the resulting decision outcome differs from existing pair row.
      todo!("witness viz: gap drill-down for d#2/c2");
  }
  ```

- **Proved conditions** — early-out: "Already proved by rows X and Y
  ({interpretation}). No action needed." Stops agents from churning
  on satisfied gaps.

- **Dead conditions** — reachability hint: "the runtime never reached
  this branch under any test row. The compiler may have folded the
  predicate, or the call-path is unreachable from the harness."

The condition list on `/decision/{verdict}/{id}` now renders a
`view gap →` link next to every non-proved condition. One click takes
the reviewer from the truth table to the tutorial — that's the
v0.9.0-brief promise of "truth-table-first PR review" delivered.

### Added — real HTMX 2.0.4 bundle

`crates/witness-viz/build.rs` ensures `assets/htmx.min.js` is the real
htmx 2.0.4 bundle (~50 KB) on every clean build. Downloaded via
system `curl` from `unpkg.com` if missing or stub-sized; falls back
to the placeholder with a `cargo:warning` when offline (set
`WITNESS_VIZ_OFFLINE=1` to force-skip the download).

`/assets/htmx.min.js` now serves the real bundle. Visualiser pages
support `hx-*` attributes for in-place swaps; existing full-page
`<a href>` navigation continues to work as the no-JS fallback.

### Added — Playwright @smoke subset + gap.spec.ts

- New `gap.spec.ts` — 5 tests verifying gap-view rendering for proved,
  gap, and dead conditions plus the `/decision → /gap` link path.
- `@smoke` tag on the most representative test in each spec file
  (5 tests total, runnable via `npm run test:smoke`).
- All 30 Playwright tests pass against the live witness-viz binary
  serving the v0.8.1 evidence bundle (`/tmp/v081-suite/`).

### Fixed — `verdicts count matches dashboard table` test

The v0.9.0 test counted `<tr>` elements in the dashboard table
expecting an explicit `.total-row` filter. The dashboard doesn't
render a TOTAL row (one is in `SUMMARY.txt` instead), so the count
included the thead row and tripped. Now counts `a[href^="/verdict/"]`
which is unambiguous.

### Notes for v0.9.x

- Per-DWARF-inlined-context outcome tracking (the v0.8.3 fold-in)
  remains v0.9.2's substance work. Architectural change to instrument-
  ation; deserves its own release.
- Visualiser self-witnessing (instrument witness-viz, drive via
  Playwright, report self-coverage on every release) — foundations
  in place; CI step + ratchet still pending.
- HTMX-powered in-place swaps on the gap-link click — the markup is
  ready (`hx-get` attributes), the styling for the swap target needs
  one more pass.

### Implements / Verifies

- **REQ-038** — interactive truth-table visualiser, *gap drill-down
  delivered* in addition to v0.9.0's truth table view.
- **REQ-043** — superior PR-review experience: the
  truth-table → tutorial-stub flow is the headline differentiator vs
  LDRA / VectorCAST / RapiCover / cargo-llvm-cov. None of those tools
  render a missing-witness row + needed condition vector.
- 30 Playwright tests pass; `cargo test --workspace --release` shows
  47 unit + 7 integration + 0 failures.

## [0.9.0] — 2026-04-27

### Headline — the agent UX chapter begins

v0.9.0 is the chapter where witness stops being "a great MC/DC tool"
and starts being "the only MC/DC tool with a truth-table-first
reviewer experience and an agent contract." Three new surfaces ship
together: the **witness-viz** Axum visualiser, the **MCP server**
mounted on it, and the **Playwright self-coverage** suite.

### Added — `witness-viz` (the visualiser)

A native Axum 0.8 web server at `crates/witness-viz/` that loads a
compliance bundle (`verdict-evidence/`) and serves it over HTTP. New
binary `witness-viz`, plus a new `witness viz` subcommand on the main
CLI that spawns it. **The reviewer experience is the truth table, not
the percentage.**

```
$ witness viz --reports-dir compliance/verdict-evidence/ --port 3037
witness-viz listening on http://127.0.0.1:3037
```

- **Routes (HTML)**:
  - `GET /` — dashboard with headline cards (decisions, full MC/DC,
    proved, gap, dead) plus the verdict scoreboard table.
  - `GET /verdict/{name}` — single-verdict drill-down listing every
    decision with status and source location.
  - `GET /decision/{verdict}/{id}` — **the hero view**: full truth
    table for one decision, condition columns with branch ids, gap
    rows highlighted, independent-effect pairs listed with proved /
    gap / dead status, drill-back to the parent verdict.
- **Routes (JSON)**:
  - `GET /api/v1/summary` — aggregate scoreboard.
  - `GET /api/v1/verdicts` — array of per-verdict summaries.
  - `GET /api/v1/verdict/{name}` — full `McdcReport` JSON.
  - `GET /api/v1/decision/{verdict}/{id}` — single-decision detail.
- **No template engine** — HTML rendered via `format!()` with helper
  functions, mirroring rivet's `serve/` pattern. Inline CSS at
  `/assets/styles.css` (~265 lines, dark/light mode via
  `prefers-color-scheme`). HTMX placeholder at `/assets/htmx.min.js`
  — full bundle deferred to v0.9.x; visualiser falls back cleanly to
  full-page navigation in the meantime (every link works).
- **Standalone workspace** — `crates/witness-viz/` has its own
  `[workspace]` declaration so axum/tokio don't pull into parent
  workspace builds. Same pattern as `crates/witness-component`.
- **Integration test** — `tests/integration.rs` writes a fake
  two-decision bundle, spawns `axum::serve` on `127.0.0.1:0`, hits
  every route, asserts 200 + key strings + 404s. Passes.

### Added — MCP server on `/mcp`

A pragmatic MCP-over-HTTP endpoint mounted on the witness-viz Axum
router. Pure JSON-RPC 2.0, no `rmcp` dependency — the surface is small
enough that hand-rolled is cleaner. Three tools that close the
agent loop:

- **`get_decision_truth_table`** — input `{ verdict, decision_id }`,
  returns the full `DecisionReport` (truth table, conditions, pairs,
  status). The agent's "what does this decision look like?" query.
- **`find_missing_witness`** — input
  `{ verdict, decision_id, condition_index }`, returns the needed row
  vector + pairing, plus a tutorial rationale string. The agent's
  "what test do I need to close this gap?" query. For already-proved
  conditions, returns `"Already proved by rows X and Y"` so the agent
  doesn't churn on satisfied gaps.
- **`list_uncovered_conditions`** — optional `{ verdict }` filter,
  returns an array of every gap/dead condition in the bundle with
  source file, line, branch id. The agent's "where's my work?" query.

```
$ curl -X POST http://127.0.0.1:3037/mcp \
    -H content-type:application/json \
    -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
{"jsonrpc":"2.0","id":1,"result":{"tools":[
  {"name":"get_decision_truth_table", ...},
  {"name":"find_missing_witness", ...},
  {"name":"list_uncovered_conditions", ...}
]}}
```

The MCP surface is a strict **subset** of the HTTP surface — no
tool returns information that an HTTP request couldn't. This is
DEC-027 from the v0.9 architecture decisions. It means humans
reviewing a PR see exactly what the agent saw.

### Added — Playwright self-coverage suite

A new `tests/playwright/` directory mirrors rivet's pattern. Five
spec files (`dashboard`, `verdict`, `decision`, `api`, plus the
shared `helpers.ts`) drive witness-viz against a live evidence
bundle and assert the truth-table widget renders correctly, gaps
get the red border, status indicators match, JSON API matches the
HTML scoreboard.

```
$ cd tests/playwright && npm install && npm run install-browsers && npm test
```

The `webServer` config boots
`crates/witness-viz/target/release/witness-viz --reports-dir $WITNESS_VIZ_FIXTURE
--port 3037`; default fixture is `/tmp/v081-suite/` (set the env var to
override). Workers=1, single chromium project, 60s per test.
TypeScript strict mode passes (`tsc --noEmit` clean).

This suite is the foundation for the **self-witnessing CI gate**
(REQ-040): v0.9.x will instrument witness-viz itself, drive it via
Playwright, and ratchet a ≥70% MC/DC release gate so the
visualiser proves its own correctness on every release.

### Added — rivet artifact unseal

REQ-038 through REQ-043 (visualiser, MCP server, dogfood,
high-complexity-app dogfood, agent loop, PR review) flipped from
`proposed` to `approved`. Five new FEATs (FEAT-019..023) and five
new DECs (DEC-023..027) document the v0.9 architecture choices:

- **DEC-023** — Native Axum first, wasm32-wasip2 deferred. The brief
  recommended `wstd-axum` on the wasi-http proxy world; rivet's
  de-risked native pattern ships v0.9.0 faster. The wasm32-wasip2
  build remains the v0.9.x stretch.
- **DEC-024** — `format!()` HTML, no template engine. Matches rivet.
- **DEC-025** — HTMX bundled inline, placeholder for v0.9.0.
- **DEC-026** — witness-viz as standalone workspace.
- **DEC-027** — MCP mounted on Axum router, strict subset of HTTP.

`rivet validate` passes with zero warnings.

### Workspace

- Workspace version bumped to **0.9.0** (witness, witness-core).
- `witness-viz` is `0.9.0` in its own workspace.

### Notes for v0.9.x

The v0.9.0 release is the visualiser + agent contract. Three things
stay deferred to v0.9.x patches:

1. **Per-DWARF-inlined-context outcome tracking** (v0.8.3 fold-in
   target). httparse 7/67 stays honest in the meantime; the
   inlined-context fix is the next single-number-bumping change.
2. **Real HTMX 2.x bundle** — currently a placeholder. Full-page
   navigation works in the meantime so no functionality is lost,
   only swap-without-reload UX.
3. **wasm32-wasip2 build of witness-viz** — `wstd-axum` integration,
   `wasmtime serve` distribution. The brief's headline architecture
   stays planned; the native binary is the v0.9.0 release.

Visualiser self-witnessing (the dogfood loop where witness instruments
witness-viz and the Playwright suite drives it) is REQ-040's v0.9.x
gate. Foundations are in place; the CI step + ratchet land in v0.9.1.

### Implements / Verifies

- **REQ-038** Interactive visualisation of MC/DC truth tables — first
  shipped form.
- **REQ-039** MCP server for MC/DC truth-table queries.
- **REQ-040** Visualiser self-witnessed (foundations: Playwright suite
  + integration test; dogfood loop is v0.9.1).
- **REQ-043** Superior PR-review experience vs incumbents — the
  truth-table-first view is the differentiator vs LDRA, VectorCAST,
  RapiCover, gcov+gcovr.
- **FEAT-019..023** all delivered or scaffolded.
- **DEC-023..027** documented in `artifacts/design-decisions.yaml`.

## [0.8.2] — 2026-04-26

### What v0.8.2 adds

- **`suite-index.html` in the compliance bundle.** The release pipeline
  now writes a self-contained HTML scoreboard at
  `verdict-evidence/suite-index.html`. Open it locally — no server
  required — to get:
  - **Headline cards** up front: conditions proved, full MC/DC count,
    decisions total, br_ifs instrumented, verdict count.
  - **Verify command** front-and-center, copy-paste ready:
    `witness verify --envelope httparse/signed.dsse.json --public-key
    verifying-key.pub`.
  - **Scoreboard** matching `SUMMARY.txt` (branches / decisions / full
    MC/DC / proved / gap / dead) with a TOTAL row, plus per-row links
    to `report.txt`, `report.json`, `lcov.info`.
  - **Per-verdict drill-down**: each row expands into a `<details>`
    section that inlines the first ~6 KB of `report.txt` — truth
    tables and gap-closure citations visible without leaving the page.
  - **Light/dark mode** via `color-scheme: light dark` + a
    `prefers-color-scheme: dark` media query.
- **Wired into the compliance action.** The `compliance/action.yml`
  step `Generate suite index HTML` runs `suite-index.py` after
  `run-suite.sh` lands the verdict evidence and the V-model matrix is
  generated. Best-effort (`|| true`) — a malformed report will not
  fail a release.

### Why a static HTML index

This release is about **review experience**. The compliance bundle
already had everything a reviewer needed (`SUMMARY.txt`,
`traceability-matrix.html`, per-verdict `report.txt` and signed
envelopes), but a reviewer arriving cold would have to know to open
`SUMMARY.txt` first, scroll to the TOTAL row, then dig into
individual subdirectories to read truth tables. `suite-index.html`
collapses that flow to one click: open `verdict-evidence/index.html`,
see the headline numbers, expand the verdicts you want to inspect.

The page is pure HTML + inline CSS — no JS, no external deps. It
works offline, on any browser, and renders identically whether
served, opened locally, or extracted from the tarball.

### Implements / Verifies

- New file: `.github/actions/compliance/suite-index.py` (Python 3
  stdlib only — no PyYAML or other deps).
- New step in `.github/actions/compliance/action.yml`: `Generate suite
  index HTML (v0.8.2+)`, gated on `verdict-evidence/` existing.
- README listing in the bundle's `README.txt` updated to mention
  `suite-index.html`.

No instrumentation, runtime, reporter, or schema changes; this is a
pure presentation-layer release on top of v0.8.0's substance.

## [0.8.1] — 2026-04-26

### What v0.8.1 adds

- **`SUMMARY.txt` gains per-condition columns + a TOTAL row.** The
  compliance bundle's top-level scoreboard now reads:
  ```
  verdict              branches  decisions full      proved  gap   dead   rows
  leap_year            2         1         1/1       2       0     0      4
  ...
  json_lite            165       29        2/29      26      31    33     28
  TOTAL                716       115       21/115    90      91    146
  ```
  One look gives the reviewer the headline numbers: 716 branches,
  115 reconstructed decisions across 11 verdicts, 21 full-MC/DC,
  **90 conditions proved**, 91 gap, 146 dead. The TOTAL row is the
  load-bearing demo number.
- **README scoreboard table** showing the v0.8.0 numbers up front,
  not buried in the version-history table.
- **Updated `Show me the proof` quickstart** uses v0.8.0 paths and
  references httparse instead of leap_year as the verify-target —
  picks the most-substantive evidence to demonstrate.

### Implements / Verifies

- This is a polish release on top of v0.8.0's substantive work. No
  instrumentation or reporter changes; CI gates are unchanged.

## [0.8.0] — 2026-04-26

### Two substantive items: chain-direction outcome derivation + 3 new
### real-application verdicts.

This is a minor-version bump because (a) the manifest schema gains a
new field (`Decision::chain_kind`), and (b) the verdict suite triples
in real-application coverage with three new fixtures.

### Added — chain-direction analysis (substance)

The instrumenter now classifies each `BrIf` site by inspecting the
preceding instruction:

- `i32.eqz; br_if N` → `BranchHint::BranchesOnFalse` (the standard
  short-circuit `&&` lowering — branch when condition is FALSE)
- bare `local.get x; br_if N` → `BranchHint::BranchesOnTrue` (the
  short-circuit `||` lowering — branch when condition is TRUE)

Per-decision aggregation yields a new `ChainKind` enum in
`witness-core::instrument`:

```rust
pub enum ChainKind { And, Or, Mixed, Unknown }
```

The runner uses `chain_kind` to derive per-iteration outcomes from
condition values: under Rust short-circuit semantics, the iteration's
outcome equals the LAST evaluated condition's value (F means
short-circuit-F for `&&`; T means short-circuit-T for `||`). For
`Mixed` / `Unknown`, falls back to the per-call kind=2 outcome from
v0.7.4 or the row-level function-return.

httparse classification breakdown: 44 Or, 20 Mixed, 3 And across 67
decisions.

### Added — three new verdict fixtures (breadth)

The verdict suite grows from 8 verdicts to **11**, adding three real-
application fixtures designed by a research sub-agent and built into
the existing run-suite.sh / compliance-bundle pipeline.

| Verdict | Source | Rows | Result on v0.8.0 |
|---|---|---:|---|
| **`nom_numbers`** | nom 7 (no_std) integer parser combinators | 28 | **3/3 full MC/DC** ✨ |
| **`state_machine`** | TLS 1.3 handshake transition guard (8-conjunct compound) | 27 | **4/5 full MC/DC** |
| **`json_lite`** | hand-rolled subset JSON parser (whitespace, escapes, structure) | 28 | **2/29 full MC/DC** (29 decisions, rich gap-analysis surface) |

`nom_numbers` and `state_machine` exercise compound boolean logic in
shapes the instrumenter now classifies cleanly. `json_lite` is more
parser-shaped — many sub-decisions, fewer per-decision conditions —
and gives the report's gap-recommendation logic a substantial
canvas.

### Suite scoreboard (v0.8.0 final)

```
verdict              branches   decisions    full-mcdc
leap_year            2          1            1/1
range_overlap        0          0            0/0   (optimised to bitwise)
triangle             2          1            1/1
state_guard          3          1            1/1
mixed_or_and         0          0            0/0   (optimised to bitwise)
safety_envelope      4          1            1/1
parser_dispatch      33         7            1/7
httparse             473        67           7/67
nom_numbers          20         3            3/3
state_machine        14         5            4/5
json_lite            165        29           2/29
                     ----       ----         -----
                     716        115          21
```

**21 full-MC/DC decisions across 115 reconstructed decisions in real
Rust code.** Up from v0.6.x's seven hand-derived canonical examples.

### Implementation notes

- `walk_collect` extended to compute hints alongside branch sites
  during the same IR walk — no second pass.
- `instrument_module` stashes the hints in a thread-local
  (`LAST_CHAIN_HINTS`) that `instrument_file` reads after DWARF
  decision reconstruction. Single-threaded by design (witness's
  instrument step is not multi-threaded).
- `apply_chain_kinds` aggregates per-decision: `(0, 0) → Unknown`,
  `(_, 0) → And`, `(0, _) → Or`, `(_, _) → Mixed`.
- `parse_trace_records` now takes a `chain_kinds` map and calls
  `derive_outcome` for every iteration that has at least one
  evaluated condition. The derived outcome wins over the row-level
  outcome when chain_kind is And/Or; Mixed/Unknown fall through.

### Updated CI gate

`verdict-suite` job's regression gate now checks all 8 should-have-
decisions verdicts: `leap_year triangle state_guard safety_envelope
parser_dispatch nom_numbers state_machine json_lite`. `range_overlap`
and `mixed_or_and` remain excluded (rustc fully optimises them to
bitwise ops with zero branches surviving).

### Notes for v0.8.x / v0.9

- httparse's score moved modestly (6/67 → 7/67) because most of its
  decisions are inlined into a single function and our chain-derived
  outcomes match the function-return outcomes in those cases. The
  next-larger gain there would be DWARF-inlined-context attribution
  to derive outcomes per source-level inlined-from function.
- json_lite at 2/29 has the most gap-analysis material in the suite.
  The reporter's row-closure recommendations should drive concrete
  test-row additions in v0.8.1.
- Manifest schema is `"2"` still. The new `chain_kind` field is
  serialised with `#[serde(skip_serializing_if = "ChainKind::is_unknown")]`
  so v0.7-and-earlier readers see a cleaner manifest when chain_kind
  isn't determined. v0.9 will bump the manifest schema to "3" if
  more fields land.

### Implements / Verifies

- Implements: REQ-027 (substance — chain_kind closes the per-iteration
  outcome story for `&&` / `||` chains).
- Implements: REQ-030 (breadth — verdict suite now has four real-
  application fixtures: httparse, nom_numbers, state_machine,
  json_lite).
- Verifies: full suite runs end-to-end, all 11 verdicts produce
  evidence, 21 decisions full-MC/DC across 115 total.

## [0.7.5] — 2026-04-26

### What v0.7.5 closes

httparse's MC/DC numbers were bottlenecked by **insufficient test
coverage** — only 15 rows, mostly happy-path requests. v0.7.5 adds
25 more rows targeting edge cases (truncated requests, malformed
methods, non-canonical line endings, large header counts, UTF-8
in paths, WebDAV methods, all status code classes, multi-word
reason phrases). With 40 rows, the witness pipeline finds
substantially more proving pairs.

### Result on httparse

| Metric | v0.7.4 (15 rows) | **v0.7.5 (40 rows)** |
|---|---:|---:|
| Full MC/DC | 1/70 | **6/67** |
| Conditions proved | 12 | **28** |
| Gap | 52 | 46 |
| Dead | 122 | 108 |

Best improvements:

- **`swar.rs`** (SIMD byte search) went from 0/2 full to **2/4
  full MC/DC** — the new test rows with longer inputs and
  byte-pattern variations exercised the SWAR loop's branches.
- **`lib.rs`** went from 0/18 to **2/14 full MC/DC** (decision
  count drops slightly because some decision shapes now group
  differently with broader DWARF coverage).
- **`result.rs`**: 1/2 full MC/DC, both conditions proved.
- **`iter.rs`** (iterator helpers): 1 full MC/DC, 6 proved.

### Added test rows (rows 15-39)

| Row | Targets |
|---|---|
| 15 | PUT method |
| 16 | DELETE method |
| 17 | Many short headers (8 distinct) |
| 18 | Body bytes after header terminator |
| 19 | HTTP/1.0 (version branch) |
| 20 | Bad version number |
| 21 | Lowercase method (case-sensitivity) |
| 22 | Empty header value |
| 23 | No space after colon |
| 24 | Header with embedded colons |
| 25 | LF instead of CRLF |
| 26 | Multi-word reason phrase (418 I'm a teapot) |
| 27 | 3xx redirect |
| 28 | 2xx with no headers (204) |
| 29 | 4xx with detailed reason (422) |
| 30 | Single-byte truncated request |
| 31 | Method only, no URI |
| 32 | Boundary byte after \r\n\r\n |
| 33 | 15 headers near 16-slot cap |
| 34 | Numeric path |
| 35 | UTF-8 byte in path |
| 36 | Long method (PROPFIND, WebDAV) |
| 37 | CR alone (malformed line ending) |
| 38 | Status without reason phrase |
| 39 | 1xx informational + custom header |

### Updated CI gate

`verdict-suite` job now also asserts `httparse.conditions_proved >= 20`
(in addition to the existing `decisions >= 30` floor). Below either
threshold is a regression in instrumentation, decisions
reconstruction, or pair-finding.

### Notes for v0.7.6 / v0.8

- The 108 dead conditions are still a substantial chunk. A v0.7.6
  could add even more rows targeting specific dead conditions
  (looking at the gap-closure recommendations the report emits).
- v0.8 chain-direction analysis (per-decision outcome derivation
  from condition values) remains the larger architectural fix that
  would meaningfully move the needle on inlined code without needing
  ever-more-test-rows.

### Implements / Verifies

- Implements: REQ-030 (verdict suite as canonical evidence — bigger
  is better evidence).
- Verifies: 6× improvement in full-MC/DC count, 2.3× improvement in
  proved-condition count, with no instrumentation or reporter
  changes — purely from richer test coverage.

## [0.7.4] — 2026-04-26

### What v0.7.4 closes (the architecture)

v0.7.3 made per-iteration condition vectors visible but reused the
top-level row's function-return as the outcome for every decision.
For decisions in *separately-compiled* called functions, that's
wrong — those functions have their own return values. v0.7.4
adds per-function-call outcome capture: each instrumented function
emits a `kind=2` trace record at every return point carrying
`(function_index, return_value)`.

### Added — `__witness_trace_outcome` helper + return-point instrumentation

A new internal helper function `__witness_trace_outcome(function_idx,
value)` writes a 4-byte record with `kind=2` to the trace memory.

For each local function f satisfying both:
1. f contains at least one `BrIf` decision in its body.
2. f's signature has exactly one i32 result.

The instrumenter walks f's body and:
- Replaces each `Return` instruction with `local.tee tmp; const
  f_idx; local.get tmp; call helper; local.get tmp; return` — captures
  the return value, records it, restores it for the actual return.
- Appends `local.tee tmp; const f_idx; local.get tmp; call helper`
  to the end of the entry block — for the implicit fall-through
  return. The tee leaves the value on the stack as Wasm's implicit
  return semantics expect.

### Updated — runner parser handles `kind=2` records

`parse_trace_records` now treats `kind=2` records distinctly: when
one arrives with function_index F, every in-flight iteration of
every decision belonging to F is finalised with the outcome value
from the record. Decisions whose function never wrote a kind=2
record (because the function had a non-i32 return type, or
trapped, or never reached its return) fall back to the row-level
function-return outcome.

The runner builds two lookups: `branch_to_decision` (for kind=0
records) and `function_to_decisions` (for kind=2 records).

### Result on httparse — same numbers, different reason

| Version | full MC/DC | proved | gap | dead | trace bytes |
|---|---:|---:|---:|---:|---:|
| v0.7.3 (per-row outcomes) | 1/70 | 12 | 52 | 122 | 6328 |
| **v0.7.4 (per-call outcomes)** | **1/70** | **12** | 52 | 122 | **6380** |

The score is unchanged because **rustc inlines aggressively**: most
"interesting" memchr / iter / SWAR decisions are inlined into
`parse_request`, so the wasm-level `function_index` for those
inlined br_ifs is parse_request's index even though the manifest
records `source_file: "memchr.rs"`. Per-wasm-function outcome
capture for the inlined case is the same as the row-level outcome.

The 52 extra trace bytes (= 13 outcome records ÷ ~15 rows ≈ 1
outcome per row) is exactly parse_request's outcome being captured
on every call.

### Why ship anyway

v0.7.4 is structurally correct — for separately-compiled functions
(common in less-optimised builds, or in CI runs with `opt-level = 0`,
or when `#[inline(never)]` is applied to the predicates in the call
graph), per-call outcomes are now captured. The architecture lays
the foundation for v0.8's per-DWARF-inlined-context outcome
tracking, which is the proper fix for aggressively-inlined code
like httparse. The work doesn't compose into something later;
it's the layer below.

### Notes for v0.7.5 / v0.8

- **Per-DWARF-inlined-context outcome tracking** is the next track.
  `function_index` in the manifest is the wasm-level function;
  v0.8 needs to also track the DWARF inlined-subroutine chain so
  decisions inlined from memchr into parse_request get attributed
  back to memchr's logical "outcome" (which doesn't exist as a
  real return value because there is no real call — but can be
  derived from the chain's terminating br_if direction).
- **Multi-result function support**. Currently only single-i32-
  result functions get outcome instrumentation. Rust's
  `Option<usize>` and similar lower to multi-result on
  wasm32-wasip2 sometimes; v0.7.5 could extend the filter.

### Implements / Verifies

- Implements: REQ-034 (architecture for per-call outcomes — the
  v0.6 trace-buffer plan now has all four record kinds wired:
  conditions, row-markers, and outcomes).
- Verifies: leap_year unchanged at 1/1 + 2 proved; httparse
  unchanged at 1/70 + 12 proved (with 52 extra trace bytes
  documenting the captured outcomes).

## [0.7.3] — 2026-04-26

### What v0.7.3 closes (read side)

v0.7.2 shipped the trace-buffer write side. v0.7.3 ships the
runner-side parser that converts the 4-byte trace records into
per-iteration `DecisionRow` entries. The MC/DC reporter then
finds proving pairs across iterations that the per-row-globals
collapse hid.

### Added — `parse_trace_records` in the runner

Reads the trace memory bytes after each row, walks records in
order. For each condition record (kind=0):

1. Looks up `branch_id` in `branch_to_decision` (built from
   manifest `Decision::conditions`).
2. Appends `(condition_index, value)` to the decision's "current
   iteration" map.
3. When a duplicate condition_index appears for the same
   decision (= the loop iterated), finalises the current
   iteration and starts fresh.
4. Trailing in-progress iterations flushed at the end.

The runner now generates one `DecisionRow` per iteration, each
with a fresh `row_id`. Outcome is the function's return value
(per-decision outcome capture is a separate v0.7.x track).

When the trace memory is empty (e.g. a v0.6 fixture predating
v0.7.2 instrumentation), the runner falls back to the per-row-
globals path. So existing verdicts stay backward-compatible.

### Result on httparse

| Version | full MC/DC | proved | gap | dead |
|---|---:|---:|---:|---:|
| v0.7.0 (per-row globals) | 0/70 | 9 | 55 | 122 |
| **v0.7.3 (trace parser)** | **1/70** | **12** | 52 | 122 |

Modest but real. `mod.rs` gained a fully-proved decision; lib.rs and
macros.rs each gained 1-2 proved conditions. The remaining gap is
because outcomes are still uniform per row (function return value);
per-decision outcome capture is the next track.

### Implementation notes

- Iteration boundary detection is conservative — "duplicate
  condition_index" may incorrectly join two semantically distinct
  iterations if the second iteration short-circuits at the same
  condition that fired last. v0.7.x will switch to row-marker-based
  boundaries (the `__witness_trace_row_marker` helper exists already
  but isn't yet emitted between iterations).
- Records other than `kind=0` (row-marker, decision-outcome) are
  reserved and skipped by this pass. Becomes load-bearing in the
  next iteration of the iteration-boundary detection.

### Notes for v0.7.4+

- **Per-decision outcome capture** is the largest remaining
  unlock. Currently every decision's outcome is the row's
  function-return value, which is correct only for the top-level
  predicate decision. Sub-decisions (memchr's compound predicates,
  iter.rs's bounds checks) have actual outcomes the trace buffer
  doesn't yet record.
- **Row-marker-based iteration boundaries** would be more accurate
  than "duplicate condition_index". The instrumenter already exports
  `__witness_trace_row_marker(u32)`; v0.7.4 wires the runner to call
  it between rows, and the parser splits on those markers instead.
- **Trace buffer overflow handling** at v0.9-scale workloads —
  currently the writer sets `overflow_flag` and silently drops, the
  reporter refuses MC/DC verdicts on overflow. v0.9 might add a
  host-callback flush.

### Implements / Verifies

- Implements: REQ-034 (substance — trace-buffer primitive end-to-end).
- Verifies: leap_year unchanged (1/1 full MC/DC, 2 proved); httparse
  improved from 0/70 + 9 proved to 1/70 + 12 proved with no other
  changes.

## [0.7.2] — 2026-04-26

### What v0.7.2 closes (write side)

v0.7.0 hit the limitation Agent A's research warned about: per-row
globals can capture only the last value per condition per row, so
loop-bearing programs (httparse) end up with `0/N full MC/DC`
because every iteration's evaluated map collapses into the last
iteration's. v0.7.2 ships the **write side** of the linear-memory
trace buffer that lifts this limitation.

The runner-side parser that converts trace records into per-iteration
`DecisionRow` entries is the v0.7.3 follow-up.

### Added — trace memory, helper exports

Each instrumented module now exports a 16-page (1 MiB) trace memory
plus three helper functions:

- `__witness_trace`: 16-page exported memory. Header at offsets 0-15
  (cursor, capacity, overflow_flag, reserved); records starting at
  offset 16.
- `__witness_trace_reset()`: zeros cursor + overflow_flag, sets
  capacity. The runner calls this between row invocations.
- `__witness_trace_row_marker(row_id: i32)`: writes a row-marker
  record. Reserved for v0.7.3+ when iteration boundaries get
  emitted at row transitions.
- `__witness_trace_record(branch_id: i32, value: i32)`: internal
  helper called by per-br_if instrumentation. Writes a 4-byte
  record `(branch_id u16, value u8, kind=0 u8)` to trace memory at
  cursor, advances cursor.

### Added — per-br_if trace-record writes

`rewrite_brif` was extended to emit `i32.const branch_id;
local.get tmp; call __witness_trace_record` after the brval/brcnt
sequence. Stack-neutral (consumes 2, pushes 0); the v0.5 invariant
that the tee'd condition stays on the stack for the if-counter-inc
that follows is preserved.

### Added — runner reads the trace watermark

`witness run` now reads the trace memory header after each row
invocation and reports the bytes-used watermark in
`RunRecord.invoked` as `__witness_trace_bytes=N`. Sets
`trace_health.ambiguous_rows = true` when any trace activity is
seen — flag for the reporter that v0.7.3's per-iteration parser
should be applied to this run.

### Verified end-to-end

```
$ witness instrument verdicts/leap_year/verdict_leap_year.wasm -o lyt.wasm
$ witness run lyt.wasm --invoke run_row_0..3 -o lyt.run.json
trace_health: {'overflow': False, 'rows': 4, 'ambiguous_rows': True}
trace: __witness_trace_bytes=28      → 7 records / 4 rows / 2 br_ifs each ≈ 1.75 r/row

$ witness instrument verdicts/httparse/verdict_httparse.wasm -o hp.wasm
$ witness run hp.wasm --invoke run_row_0..14 -o hp.run.json
trace_health: {'overflow': False, 'rows': 15, 'ambiguous_rows': True}
trace: __witness_trace_bytes=6328     → 1582 records across 15 rows
```

httparse's **1582 records** across 15 rows is exactly the
per-iteration data per-row globals could not capture. v0.7.3 parses
this into per-iteration DecisionRow entries; the MC/DC reporter
then finds proving pairs that the per-row-globals collapse hid.

### Implementation notes

- Helper-function approach (not inline). Each per-br_if site is
  3 instructions (const + local.get + call) instead of ~15 inline
  instructions. Trade-off: function-call overhead per branch in
  exchange for smaller module size + simpler stack-typing review.
  v0.7.x can switch to inline if profiling shows the call dominates.
- Module file size growth ≈ 100 bytes for the trace infrastructure
  (memory + 3 helpers) + 12 bytes per `BrIf` site (3 instructions
  ≈ 4 bytes each). httparse's 481 br_ifs add ~5.7 KB of
  instrumentation; the bulk of `verdict_httparse.wasm`'s 677 KB
  is httparse itself.
- Multiple-memory support required at the runtime side. wasmtime
  42 supports it natively (the wasi-preview2 import set already
  uses multiple memories). Older runtimes that don't support
  `multi_memory` will reject witness-instrumented modules; the
  runtime check is upstream of any witness-specific failure.

### Notes for v0.7.3

- Runner-side trace parser: read the trace memory after each row,
  walk the records, group by decision (via manifest's branch→
  decision map), split into iterations (next condition_idx
  appearing equal-or-less-than the previous starts a new
  iteration), emit one DecisionRow per iteration. Outcome stays the
  function return value (per-decision outcome capture is a
  separate v0.7.x track).
- MC/DC reporter changes: handle the case where one row produces
  multiple iterations of the same decision. Each iteration is a
  candidate for pair-finding.

### Implements / Verifies

- Implements: REQ-034 (substance — first half: trace-buffer
  primitive on the write side).
- Verifies: leap_year produces 7 records, httparse produces 1582
  records — trace memory is being written by per-br_if
  instrumentation as designed.

## [0.7.1] — 2026-04-26

### What v0.7.1 closes

v0.7.0's httparse demo proved witness scales to a real Rust crate
(70 decisions, 481 br_ifs) but the per-decision report at that
size — 1519 lines — is unreadable. v0.7.1 adds module-rollup
report mode: 13 lines instead of 1519, per-file decisions/
conditions table sorted by decision count.

### Added — `witness report --format mcdc-rollup` and `mcdc-rollup-json`

```
$ witness report --input httparse.run.json --format mcdc-rollup
module: httparse.wasm
overall: 0/70 full MC/DC; conditions: 9 proved, 55 gap, 122 dead (186 total)

source file                               decisions  full mcdc     proved        gap       dead
---------------------------------------- ---------- ---------- ---------- ---------- ----------
lib.rs                                           18          0          1         18         25
iter.rs                                          16          0          1         16         28
macros.rs                                        10          0          3         13         15
count.rs                                          6          0          0          0         12
mod.rs                                            6          0          1          1         16
num.rs                                            3          0          0          0         10
validations.rs                                    3          0          2          3          4
const_ptr.rs                                      2          0          0          2          2
swar.rs                                           2          0          1          2          2
converts.rs                                       1          0          0          0          2
index.rs                                          1          0          0          0          2
panic.rs                                          1          0          0          0          2
result.rs                                         1          0          0          0          2
```

The user reads this as: "lib.rs has 18 decisions; 1 condition is
independently witnessed, 18 have gaps a new test row would close,
25 are dead (never reached by the suite)". One-line-per-file, the
most-decision-rich files at the top.

JSON variant emits the same structure for tooling consumption,
schema URL `https://pulseengine.eu/witness-mcdc/v1/rollup`.

### Implementation

- New `McdcRollup` and `FileRollup` structs in
  `witness-core::mcdc_report`.
- `McdcRollup::from_report(&McdcReport)` walks decisions, buckets by
  `source_file`, sums per-bucket condition tallies. Sorts by
  decision-count descending.
- New `mcdc_report::rollup_from_run_file(path)` convenience.
- CLI: two new variants in the `ReportFormat` enum.
- Long file paths (e.g. inlined-stdlib paths in httparse) get
  `…suffix` truncation in the text output to keep the table
  column-aligned at 40 chars.

### Notes for v0.7.2

- The per-row-globals limitation that caps httparse's full-MC/DC
  count at 0/70 is still in effect. v0.7.2 plans the trace-buffer
  primitive switch.

### Implements / Verifies

- Implements: REQ-029-substance — the report is now actually
  reviewable at scale, where v0.7.0's per-decision text was not.

## [0.7.0] — 2026-04-26

### What v0.7.0 closes

Two substantive items: (a) the v0.5–v0.6 release pipeline shipped a
13 KB `witness-core.wasm` build smoke-test as if it were a usable
component — it had no `extern "C"` exports and no WIT interface, so
linker dead-code-elimination threw away the entire library; (b) the
v0.6.x verdict suite covered seven canonical compound-decision
shapes but no real-application demo. v0.7.0 closes both.

### Added — `crates/witness-component`

A new workspace member that builds witness-core's MC/DC reporter as
a **real Component Model component** for `wasm32-wasip2`. WIT
interface at `crates/witness-component/wit/world.wit`:

```wit
package pulseengine:witness@0.7.0;

interface reporter {
    report-text: func(run-json: string) -> result<string, string>;
    report-json: func(run-json: string) -> result<string, string>;
    verify-envelope: func(envelope-json: string, public-key: list<u8>)
        -> result<string, string>;
}

world component { export reporter; }
```

`wasm-tools component wit` confirms the exports land
(`pulseengine:witness/reporter@0.7.0`). The emitted artefact is
**~400 KB** vs the v0.5–v0.6 13 KB stub — a 30× size jump that's
witness-core's actual code being kept by the linker because the
WIT interface gives it reachable entry points.

### Replaced — release asset name

- v0.5 / v0.6: `witness-core-vX.Y.Z-wasm32-wasip2.wasm` (13 KB stub).
- **v0.7.0+**: `witness-component-vX.Y.Z-wasm32-wasip2.wasm` (~400 KB
  real component).

Asset-name change makes the format change visible to downstream
consumers tracking release artefacts. The release notes call it out
explicitly.

### Added — `verdicts/httparse` real-application fixture

A new verdict crate that depends on the `httparse` crate (RFC 7230
HTTP/1.x parser) and exposes 15 `run_row_<n>` exports driving the
parser with representative request and response shapes. Witness
reconstructs **~70 decisions across 481 br_ifs** spanning
`httparse/lib.rs`, `httparse/iter.rs`, `httparse/macros.rs`, and
inlined stdlib code (`core/src/iter`, `core/str`, `swar.rs`, etc.).

This is "witness on a real Rust crate" — the verdict suite's 7
existing canonical shapes are still useful for verifying the
reporter's correctness, but httparse demonstrates witness on
something with a real test surface.

#### What httparse reveals — and the per-row-globals limitation

Running the suite on httparse reveals that **0 of 70 decisions
achieve full MC/DC** under our 15 test rows. The cause is a known
limitation of the v0.6.1 per-row-globals primitive:

- httparse's parsing loops hit each `br_if` *multiple times per
  test row* (once per iteration).
- Our `__witness_brval_<id>` global captures only the LAST value
  per row.
- So the recorded `evaluated[i]` for each condition is the value at
  loop termination, identical across rows that traverse similar
  loop paths.
- Pair-finding can't find independent-effect proofs because the
  toggling-condition rows produce identical `evaluated` maps.

**This is exactly the loop-case Agent A's research warned about**
and recommended the linear-memory trace buffer as the long-term
fix (`docs/research/v06-instrumentation-primitive.md` §2.5, §7.1).
v0.7.0 documents the limitation clearly; v0.7.x or v0.8 ships the
trace-buffer primitive that lifts it.

For now, the httparse report is still useful as a **reachability /
dead-condition picture**: which decisions in the parser are
exercised by which test rows, which conditions never fire across
the suite, which gaps a new test row would close.

### Updated — verdict-suite regression gate

CI's `verdict-suite` job adds httparse to the assertion list:
expected ≥ 30 decisions reconstructed (the count varies by rustc
version; 30 is a generous floor). leap_year + triangle + state_guard
+ safety_envelope + parser_dispatch each still asserted ≥ 1.

### Implements / Verifies

- Closes the witness-core wasm overclaim from v0.5/v0.6 by shipping
  a real Component Model component (REQ-031 substance, not just
  packaging).
- Adds httparse as the v0.9 destination workload anchor (per
  `docs/research/v07-scaling-roadmap.md` recommendation).
- Verifies: `wasm-tools validate` + `wasm-tools component wit` on
  the emitted `witness_component.wasm`; verdict suite with httparse
  produces 70 decisions / 481 branches / 1050 row records.

### Notes for v0.7.x

- **Module-rollup MC/DC report mode** for usability at httparse
  scale (1519-line per-decision report → per-file roll-up). v0.7.1.
- **Trace-buffer instrumentation primitive** to lift the per-row-
  globals limitation on loop-bearing code. v0.7.x or v0.8.
- **Per-decision outcome capture** (currently the function return
  value is applied uniformly to every decision; a sub-decision's
  actual outcome may differ from the top-level return). Likely
  v0.7.x alongside the trace buffer.
- **Component verification harness** — wasmtime-driven integration
  test that loads `witness_component.wasm`, calls `reporter:report-
  text`, asserts the output. v0.7.1.

## [0.6.9] — 2026-04-26

### What v0.6.9 closes

The v0.6.4–v0.6.8 work shipped a complete signed-evidence pipeline,
but the threat model — what a witness signature proves, what it
*doesn't*, why ephemeral keys — was scattered across CHANGELOG
narratives. v0.6.9 consolidates it into a `SECURITY.md` that
adopters can cite when scoping witness's role in their qualification
chain.

### Added — `SECURITY.md`

Five sections:

1. **What a signature proves** — predicate body integrity, signer
   key identity, instrumented-module digest binding.
2. **What a signature does NOT prove** — no long-term key
   continuity, no source-binding, no test-suite representativeness,
   no LLVM lowering soundness (the v0.2 paper's coverage-lifting
   open problem, deferred to v1.0's Check-It pattern).
3. **The ephemeral-key approach** — pros and cons, when adopters
   should layer their own signing chain on top (sigstore Fulcio,
   HSM/KMS).
4. **Key sizes and algorithms** — Ed25519, raw 64+32 byte format,
   PEM/DER deferred to v0.7.
5. **Reporting security issues** — `security@pulseengine.eu`,
   security-relevant code paths.

### Why this matters for v0.6.x adopters

Safety-critical adopters need to scope witness in their
qualification chain. The signature's claims are precise but
narrow: "this evidence was produced by the release pipeline that
wrote this verifying-key.pub". Anything broader requires
composition with surrounding tools (build provenance, sigstore,
HSM-backed signing). SECURITY.md states that boundary explicitly
so adopters don't over-claim.

### Closing the v0.6.x ratchet

This is likely the **last v0.6.x sub-version**. The series
ratcheted from v0.6.0's "consumer side only" through v0.6.8's
"release self-verifies its own bundle". The v0.7 work — scaling to
real applications, BrTable per-row MC/DC, visualisation, sigstore
integration — is substantively different in shape from v0.6.x
ratchets and warrants its own planning cycle (the existing v07
research brief at `docs/research/v07-scaling-roadmap.md` is the
starting point).

Future v0.6.x patch releases remain available for fixes.

### Implements / Verifies

- Documents: the v0.6.4 sign + v0.6.7 verify + v0.6.8 self-verify
  loop's threat model and scope.

## [0.6.8] — 2026-04-26

### What v0.6.8 closes

v0.6.4 added DSSE signing; v0.6.7 documented how to verify a release.
v0.6.8 closes the loop in the other direction: every release pipeline
now self-verifies the compliance archive it just built. If the
signing path regresses (key not bundled, envelope corrupted, verify
command broken), the release fails before publication rather than
shipping broken evidence to users.

### Added — self-verify step in `.github/actions/compliance/action.yml`

After the V-model trace matrix is written and before the README, the
action runs `witness verify` against one signed envelope from the
just-built bundle, against the bundled `verifying-key.pub`. The
specific envelope tried is `leap_year/signed.dsse.json` — the
canonical demo, expected to produce a verdict on every release.
Falls back to triangle / state_guard / safety_envelope /
parser_dispatch if leap_year is missing for some reason.

The step is conditionally a no-op if signing was skipped (no
`verifying-key.pub` in the bundle) — keeps the action backward-
compatible with the v0.5 dev-mode invocation that doesn't run the
verdict suite.

### Why this matters

It's defence in depth in the strictest sense: the failure mode being
prevented is "release ships, downstream consumer downloads, runs
witness verify, gets failure, files a bug — but we already shipped".
Self-verify makes that scenario impossible because the release
pipeline is the first downstream consumer.

It also doubles as a smoke test for `witness verify` itself: the
verify command runs in the release pipeline against real production
evidence on every release. If a future change to the attest / verify
code breaks compatibility with already-produced bundles, the next
release fails closed.

### Implements / Verifies

- Verifies: the v0.6.4 sign + v0.6.7 verify loop self-attests on
  every release, no manual intervention required.

## [0.6.7] — 2026-04-26

### What v0.6.7 closes

The v0.6.x series shipped a complete signed-evidence pipeline across
six versions, but the README was still pinned to v0.1.0 — visitors
landed on the repo and saw none of it. v0.6.7 fixes that: the README
features a "Show me the proof — verify a release in 60 seconds"
recipe walking through download → extract → verify → see truth
tables.

### Updated — README.md

- **Status** section replaced with a v0.6.x ratchet table summarising
  what each sub-release added.
- **Show me the proof** section before the usage block. Walks through
  `gh release download`, extract, `cat SUMMARY.txt`, `witness verify`.
  Includes a directory tree of the bundle so users know what each
  file is for.
- **Usage** section updated to demonstrate the v0.6 commands:
  `witness report --format mcdc`, `keygen`, `attest`, `verify`,
  `lcov` — replacing the v0.1.0-era examples.

### Verified end-to-end against the production v0.6.4 release

Downloaded v0.6.4's compliance archive from GitHub, extracted, and
ran `witness verify` against `leap_year/signed.dsse.json` and
`parser_dispatch/signed.dsse.json` against the bundled
`verifying-key.pub`. Both verify with `OK`. The published release
ships exactly what the README documents — no broken promises.

### Notes for v0.6.8 / v0.7

- The `Related work` section in the README still doesn't mention
  RapiCover (the closest commercial competitor identified by the
  v0.9 research agent). Worth adding for v0.6.8.
- The `Where it fits` table predates the v0.6.x ecosystem reality —
  loom and meld don't yet emit the offset-translation maps witness
  needs for post-loom / post-meld coverage; that's v0.7+ work
  pending the upstream issues from v0.5.

### Implements / Verifies

- Ratifies the v0.6.x ladder for repo visitors. No code changes —
  this release exists to put the showable proof on the front page.

## [0.6.6] — 2026-04-26

### What v0.6.6 closes

The v0.6.3 release added a verdict-suite regression gate to CI on
main, but a PR-time view of "did this PR regress the suite?" was
missing. v0.6.5's verdict-suite-delta note flagged this as a
v0.6.6 candidate; v0.6.6 ships it.

### Added — `verdict-suite-delta` job in `.github/workflows/witness-delta.yml`

Triggers on PRs touching `crates/`, `verdicts/`, `Cargo.toml`, or
either of the two delta-related files. Steps:

1. Checks out the base (main) and head (PR) revisions side by side.
2. Builds witness in release mode against each.
3. Runs `verdicts/run-suite.sh` against both with `SIGN=0` (PR
   delta does not need signed envelopes — that's the release
   pipeline's job).
4. Walks both `delta-head/` and `delta-base/` directory trees,
   reads each verdict's `report.json`, builds a per-verdict
   comparison table with columns:
   - base full/total decisions
   - head full/total decisions
   - head conditions (proved / gap / dead)
   - status: improvement / unchanged / regression / new
5. Posts the table as a PR comment, replacing any prior comment
   tagged with the `<!-- witness-verdict-delta -->` marker.
6. Uploads the delta directory as an artefact for inspection.

The job runs `continue-on-error: true` so a verdict-suite failure
doesn't block PR merging — the comment is the signal, not the
status check. A regression is flagged in **bold** in the comment
body so reviewers can't miss it.

### Updated — paths-filter on the delta workflow

The original filter only triggered on `src/`, `tests/`, and
`Cargo.toml`. v0.6.6 expands to include `crates/`, `verdicts/`, and
the action workflows themselves. PRs that touch the verdict suite
or the compliance pipeline now correctly fire the delta job.

### Notes for v0.7

- The same comparison logic could fire weekly against `main` to
  catch *upstream* regressions (e.g. a rustc upgrade that changes
  optimiser behaviour and silently moves a verdict from full-MC/DC
  to partial). That's a v0.7 ergonomics item alongside the
  scaling-roadmap work.
- The signing path (v0.6.4) and the trace matrix (v0.6.5) are
  release-time only. A v0.7 candidate is to surface the trace
  matrix on PR delta too — a single HTML artefact a reviewer can
  open to see the cross-references for the PR's branch state.

### Implements / Verifies

- Implements: REQ-022 (coverage-delta PR workflow) — extended from
  v0.4's manifest-only delta to include the verdict-suite roll-up.
- Verifies: comment-bot logic uses the existing
  `actions/github-script@v7` pattern; comment marker dedupes on
  re-runs.

## [0.6.5] — 2026-04-26

### What v0.6.5 closes

REQ-032 — every release ships a JSON + HTML traceability matrix
generated from `artifacts/*.yaml` at release time, bundled into the
compliance archive. v0.6.0 declared the requirement; v0.6.5 ships
the implementation.

Plus: parser_dispatch's `TRUTH-TABLE.md` gains a "Post-rustc
Wasm-level reality" section documenting why the report shows 7
decisions instead of the source author's intended 1 — the v0.2
paper's coverage-lifting argument in concrete form.

### Added — `.github/actions/compliance/trace-matrix.py`

Pure-Python (PyYAML only) script that reads the rivet artefact
graph and the verdict-evidence directory and emits two files:

- `traceability-matrix.json` (schema URL
  `https://pulseengine.eu/witness-trace-matrix/v1`) carrying
  totals + per-requirement satisfied-by-feature / supporting-
  decision lists + per-verdict MC/DC roll-up + signed-envelope
  flag.
- `traceability-matrix.html` styled for human review with a
  verdict-suite table at the top and a requirements table below.

Composite-action wiring installs PyYAML via `apt python3-yaml`
preferentially, falling back to `pip --break-system-packages` for
runners that lack the apt package.

### Added — Wasm-level-reality section in `parser_dispatch/TRUTH-TABLE.md`

The source-level table at the top of the file documents the
predicate as the author wrote it (5 conditions, hand-derived rows,
under-masking pair structure). The new section at the bottom
documents what witness's report actually finds:

- 7 decisions (1 in `lib.rs`, 5 across `memchr.rs`, 1 split via
  inlining of the byte-search exit structure)
- 33 br_ifs total
- 5 dead conditions because `memchr`'s SIMD path requires inputs
  longer than our 6 test rows provide

The discrepancy is the v0.2 paper's coverage-lifting argument made
concrete: post-rustc Wasm coverage measures what the optimizer
left, including stdlib internals invoked by user code. The user is
responsible for scoping which decisions are part of their MC/DC
claim.

### Compliance bundle now contains

| File | Purpose |
|---|---|
| `verdict-evidence/<name>/*` | Per-verdict instrument-run-report-predicate-signed chain (v0.6.3+, v0.6.4 added signing) |
| `verdict-evidence/SUMMARY.txt` | One-line-per-verdict status table |
| `verdict-evidence/verifying-key.pub` | Ed25519 public key (v0.6.4+) |
| `verdict-evidence/VERIFY.md` | Verification walkthrough (v0.6.4+) |
| `traceability-matrix.json` | V-model matrix machine-readable (v0.6.5+) |
| `traceability-matrix.html` | V-model matrix human-readable (v0.6.5+) |
| `predicates/`, `manifests/` | Legacy v0.5 directories (still present for compatibility) |
| `coverage-report.{json,txt}` | Top-level coverage report (when run-json input is provided) |

### Implements / Verifies

- Implements: REQ-032 (V-model traceability matrix in every release).
- Verifies: matrix renders against the actual v0.6.4 verdict-evidence
  bundle locally — 39 requirements, 17 features, 22 design-decisions,
  7 verdicts (5 with non-zero decisions), all with signed envelopes.

## [0.6.4] — 2026-04-26

### What v0.6.4 closes

v0.6.3 populated the compliance bundle with real per-verdict
evidence — manifest, run record, MC/DC report, unwrapped in-toto
predicate per verdict. v0.6.4 adds the **signature** layer: each
verdict's predicate is wrapped in a DSSE envelope and signed with an
ephemeral Ed25519 keypair generated fresh for the release. The
verifying public key ships in the bundle. Tampering with the
predicate body, the envelope, or the key fails verification.

### Added — `witness keygen` and `witness verify` CLI commands

Two new subcommands close the signing loop:

- `witness keygen --secret SK --public PK` — generate a fresh
  Ed25519 keypair (raw 64-byte secret + 32-byte public). Used by the
  verdict-suite signing path; also available for users who want to
  sign their own predicates.
- `witness verify --envelope E --public-key PK` — validate a DSSE
  envelope against an Ed25519 public key. Exits zero with `OK` on
  match, non-zero (with a clear error) on mismatch. Standards-
  compliant DSSE means `cosign verify-blob` works equivalently.

### Added — `witness-core::attest::generate_keypair_files`

Public library API for keypair generation. Mirrors the existing
`sign_predicate_file` and `verify_envelope_file` shapes (file in /
file out / `Result<()>`). Lets downstream tooling embed the same
ephemeral-key flow.

### Added — `witness-core::attest::verify_envelope_file`

File-IO wrapper around the existing `verify_envelope` byte-slice API.
Reads the envelope and public key from disk, returns the inner
in-toto Statement on success.

### Updated — `verdicts/run-suite.sh`

When `SIGN=1` (default), the script:

1. Generates an ephemeral Ed25519 keypair via `witness keygen`.
2. Writes the public key to `<bundle>/verifying-key.pub`.
3. For each verdict's `predicate.json`, runs `witness attest` to
   produce `<verdict>/signed.dsse.json` with `key_id` =
   `witness-suite/<verdict>`.
4. Discards the secret key on exit (in a `mktemp` directory cleaned
   up by a `trap`).
5. Writes a `<bundle>/VERIFY.md` documenting the verification
   command for both `witness verify` and `cosign verify-blob`.

Setting `SIGN=0` skips the signing path — useful for fast local
iteration.

### Verifies — round-trip end-to-end

Local sign-verify proven on the leap_year and parser_dispatch
envelopes:

```
$ witness verify \
    --envelope leap_year/signed.dsse.json \
    --public-key verifying-key.pub
OK — DSSE envelope leap_year/signed.dsse.json verifies against verifying-key.pub
  predicate type: https://pulseengine.eu/witness-coverage/v1
  subjects: 1
```

Tampering the public key (XOR first byte) correctly fails:

```
Error: wasm runtime error: DSSE verify failed: VerificationFailed
exit=1
```

### Why ephemeral keys

Per-release ephemeral keys avoid long-term key custody. The verifying
key is shipped in the compliance bundle. The secret key is generated
fresh in CI, used to sign, then discarded. A signature thus proves
"this evidence was produced by the release pipeline that wrote this
verifying-key.pub" — exactly the V-model claim. Long-term key
management (rotation, attestation chains, sigstore Fulcio integration)
is v0.7+ work.

### Implements / Verifies

- Implements: REQ-031 (witness-core.wasm signed release asset — the
  pattern now applies uniformly to verdict predicates too).
- Verifies: round-trip sign + verify works for every verdict that
  produces a non-empty predicate; tampered key correctly rejected.

## [0.6.3] — 2026-04-26

### What v0.6.3 closes

v0.6.0 promised real per-verdict signed evidence in the release archive
(REQ-033) but shipped a structural placeholder: empty `predicates/`
and `manifests/` directories. v0.6.1 made the per-row instrumentation
work end-to-end; v0.6.2 made 5 of 7 verdicts produce reports. v0.6.3
finally **populates the compliance bundle** with that evidence and
adds a CI regression gate so the suite stays green across rustc
upgrades.

### Added — `verdicts/run-suite.sh`

End-to-end driver script. Invoked locally (`./verdicts/run-suite.sh
some-out-dir`) and from the compliance action. For each of the seven
verdicts:

1. Builds with `wasm32-unknown-unknown` (core module — walrus can
   rewrite; `wasm32-wasip2` produces Components walrus doesn't yet
   handle).
2. Instruments with the v0.6.1 per-row primitive.
3. Runs every `run_row_<n>` export.
4. Emits text + JSON MC/DC reports.
5. Builds the unwrapped in-toto Statement (signing is v0.6.4 once
   release-key management is wired in).
6. Emits LCOV + sibling overview when DWARF surfaces decisions.

A `SUMMARY.txt` rolls up branches / decisions / full-MC/DC counts:

```
verdict              branches   decisions    full-mcdc
-------              --------   ---------    ---------
leap_year            2          1            1/1
range_overlap        0          0            0/0
triangle             2          1            1/1
state_guard          3          1            1/1
mixed_or_and         0          0            0/0
safety_envelope      4          1            1/1
parser_dispatch      33         7            1/7
```

### Added — populated compliance bundle

`.github/actions/compliance` now invokes `verdicts/run-suite.sh` and
nests the output under `compliance/verdict-evidence/<name>/`. Each
release's compliance archive contains:

- The seven verdict directories with their full instrument-run-report
  chain.
- `SUMMARY.txt` at the top of the bundle.
- The original (now non-empty) `predicates/` and `manifests/`
  directories.

Closes REQ-033 ("compliance bundle populated with real evidence") in
substance, not just structurally.

### Added — `verdict-suite` CI regression gate

New `verdict-suite` job in `ci.yml`:

- Builds witness in release mode.
- Adds `wasm32-unknown-unknown` target.
- Runs the suite script.
- **Asserts** that `leap_year`, `triangle`, `state_guard`,
  `safety_envelope`, and `parser_dispatch` each produce >= 1
  reconstructed decision. A regression (e.g. a future rustc that
  fully optimises one of these verdicts to bitwise) fails CI on main.
- Uploads `verdict-evidence/` as an artefact.

`range_overlap` and `mixed_or_and` are deliberately excluded from the
gate — their pure-boolean conditions are expected to be fully
optimised to `i32.and` and produce zero branches at the Wasm level.

### Notes for v0.6.4

- DSSE-sign each verdict's predicate with a release-time key. Pulls
  `wsc-attestation` into the action, manages the key via GitHub
  Secrets.
- Add a `verdict-suite-delta` PR-level CI job that diffs the
  `decisions / conditions / full-mcdc` counts vs `main` and posts the
  delta to the PR conversation. Useful for catching subtle optimiser
  regressions earlier than the regression gate fires.

### Implements / Verifies

- Implements: REQ-033 (compliance bundle populated with real
  per-verdict evidence).
- Verifies: the verdict-suite CI gate exists and fails closed when
  any of the five "should-have-decisions" verdicts regresses to zero.

## [0.6.2] — 2026-04-26

### What v0.6.2 closes

v0.6.1 made the per-row instrumentation work end-to-end on the
`leap_year` verdict, where rustc happens to attribute all surviving
`br_if`s to the same source line. Verdicts whose conditions span
multiple source lines (`state_guard`, `triangle`, `safety_envelope`,
`parser_dispatch`) had surviving `br_if`s in their manifests but
**zero reconstructed decisions** — `decisions::group_into_decisions`
required strict same-line equality for the grouping criterion, which
short-circuit chains formatted across multiple lines do not satisfy.

v0.6.2 relaxes the criterion: br_ifs in the same `(function, file)`
whose source lines fall within `MAX_DECISION_LINE_SPAN = 10` cluster
into one Decision. Walks branches in branch-id order (= source-walk
emission order), starts a new cluster when the next br_if is outside
the line window. False-grouping is bounded — adjacent decisions
separated by a > 10-line gap stay separate.

### Result

| Verdict | branches | decisions | full MC/DC | notes |
|---|---:|---:|---|---|
| `leap_year` | 2 | 1 | 1 | unchanged from v0.6.1 |
| `state_guard` | 3 | 1 | 1 | **new in v0.6.2** |
| `triangle` | 2 | 1 | 1 | **new in v0.6.2** |
| `safety_envelope` | 4 | 1 | 1 (3 conds) | **new in v0.6.2** |
| `parser_dispatch` | 33 | **7** | 1 | **new in v0.6.2** — finds decisions in `memchr` library calls automatically |
| `range_overlap` | 0 | 0 | n/a | optimised to `i32.and` (bitwise), nothing to measure |
| `mixed_or_and` | 0 | 0 | n/a | optimised to bitwise; nothing to measure |

### parser_dispatch is the standout

The `parser_dispatch` verdict's `s.contains(b'@')` call lowered into
the `memchr` library's byte-search loops, which themselves contain
compound boolean conditions. `decisions::reconstruct_decisions` picks
them up automatically:

```
$ witness report --input parser_dispatch.run.json --format mcdc
decisions: 1/7 full MC/DC; conditions: 7 proved, 15 gap, 5 dead

decision #0 lib.rs:37: Partial
  c0 (branch 3): proved via rows 1+4 (masking)
  c1 (branch 10): DEAD — never evaluated in any row
  c2 (branch 11): DEAD — never evaluated in any row
  c3 (branch 17): GAP — try a row {c0=T, c3=T} (outcome must differ from row 4)
decision #1 lib.rs:58: FullMcdc
  c0 (branch 18): proved via rows 2+4 (masking)
  c1 (branch 19): proved via rows 3+5 (unique-cause)
decision #2 memchr.rs:40: Partial
  c0 (branch 0): proved via rows 0+4 (masking)
  c1 (branch 1): proved via rows 1+5 (masking)
  c2 (branch 2): GAP — try a row {c0=T, c1=T, c2=F} (outcome must differ from row 4)
...
```

This is "witness on real code, not toys" — the predicate is six
test rows of 4-condition URL-authority validation, but the underlying
implementation drags in the standard library's compound predicates,
and witness reports MC/DC on all of them with cited row pairs and
closure recommendations.

### Implementation

- `decisions::MAX_DECISION_LINE_SPAN: u32 = 10` — public constant so
  consumers can document the threshold in V-model briefs.
- `group_into_decisions` rewritten as a two-pass algorithm: resolve
  br_if entries to `(function, file, line)`, then bucket by
  `(function, file)` and cluster within each bucket using the
  adjacent-line span.
- Two new unit tests: `group_into_decisions_clusters_adjacent_lines`
  (4 br_ifs on lines 23-26 → one Decision) and
  `group_into_decisions_splits_on_large_gap` (two clusters separated
  by a 49-line gap stay separate).

### Implements / Verifies

- Implements: REQ-027, REQ-028, REQ-029 — extends the v0.6.0 schema
  + reporter to cover the broader range of compound-decision
  lowerings rustc emits.
- Verifies: 5 verdicts (leap_year, state_guard, triangle,
  safety_envelope, parser_dispatch) produce non-empty MC/DC reports
  with cited row pairs.

### Notes for v0.6.3 / v0.7

- `range_overlap` and `mixed_or_and` produce zero branches because
  rustc fully optimises their pure-boolean conditions to bitwise
  arithmetic. v0.7's "compiler hint" work could ask rustc to emit
  branches for these patterns when an opt-in attribute is present —
  per the v0.2 paper's "witness-and-checker" stance. Out of scope
  for v0.6.x.
- `parser_dispatch` shows 5 dead conditions across 6 test rows; the
  verdict's `TRUTH-TABLE.md` should be revised to align expected
  rows with what the post-rustc lowering actually exposes (rather
  than the source-level decisions originally documented).

## [0.6.1] — 2026-04-26

### What v0.6.1 closes

v0.6.0 shipped the consumer side (schema, reporter, verdict-suite oracles)
and explicitly deferred the runtime instrumentation to v0.6.1. v0.6.1 is
that runtime path: real per-row condition capture during `witness run`,
real `RunRecord.decisions` populated from execution rather than hand-
curated, real end-to-end demonstrable MC/DC on the canonical leap_year
verdict.

### Added — per-row instrumentation

- **Per-condition exported globals**: each `BrIf` / `IfThen` / `IfElse`
  branch now allocates two additional globals alongside its existing
  `__witness_counter_<id>`:
  - `__witness_brval_<id>` (i32) — the condition's evaluated value
    (0 or 1) when reached this row, or `1` for fired arms.
  - `__witness_brcnt_<id>` (i32) — count of evaluations this row;
    non-zero means evaluated, zero means short-circuited (absent
    from `DecisionRow.evaluated`). `BrTable*` branches keep
    counter-only instrumentation per DEC-015.
- **`__witness_row_reset` exported function**: emitted by every
  instrumentation pass. Zeros all `brval` / `brcnt` globals so the
  next row's captures don't leak prior state.

### Added — runner row-by-row capture

- `witness run` (embedded wasmtime path) now, for each `--invoke`:
  1. Calls `__witness_row_reset` to clear per-row state.
  2. Invokes the export, capturing the return value as the row's
     decision outcome (when the export returns an `i32`).
  3. Reads the per-row `brval` / `brcnt` globals.
  4. For each `Decision` in the manifest, builds a `DecisionRow`
     populated with the per-condition values evaluated this row.
- `RunRecord.decisions` is now populated from execution; the
  `mcdc_report` reporter consumes it directly with no manual curation.

### End-to-end demonstrable on `verdicts/leap_year`

Building the leap_year verdict, instrumenting it, running all 4 row
exports, and asking for the MC/DC report produces:

```
$ witness instrument verdicts/leap_year/verdict_leap_year.wasm -o leap.wasm
$ witness run leap.wasm --invoke run_row_0 ... --invoke run_row_3 -o run.json
$ witness report --input run.json --format mcdc
module: leap.wasm
decisions: 1/1 full MC/DC; conditions: 2 proved, 0 gap, 0 dead

decision #0 lib.rs:46: FullMcdc
  truth table:
    row 0: {c0=T} -> F
    row 1: {c0=F, c1=T} -> T
    row 2: {c0=F, c1=F} -> F
    row 3: {c0=F, c1=F} -> T
  conditions:
    c0 (branch 0): proved via rows 0+1 (masking)
    c1 (branch 1): proved via rows 1+2 (unique-cause)
```

Two conditions, both proved with cited row pairs, full MC/DC at the
Wasm bytecode level.

### Why some verdicts report zero decisions

The leap_year decision `(year%4==0 && year%100!=0) || year%400==0`
lowered to two `br_if` instructions plus inline arithmetic for the
third condition. That's why the report shows two conditions rather
than three: the third was elided by rustc's optimizer into the
fall-through computation. This is exactly the v0.2 paper's coverage-
lifting thesis — post-rustc Wasm coverage measures *what the
optimizer left as branches*, not the source-level condition count.

For verdicts whose conditions are all side-effect-free comparisons
(e.g. `a && b` over pure booleans), rustc may lower `&&` to a single
`i32.and` instruction and eliminate branches entirely. `range_overlap`
and similar verdicts produce zero `BrIf` entries in the manifest as a
result. The reporter correctly reports zero decisions — that is the
honest measurement at this point. Source-level MC/DC for these
predicates is the rustc-mcdc tool's territory; witness covers what
survives lowering. The "overdo stance" (DEC-005) — adopt both, do
not pick one.

The remaining verdicts (`triangle`, `state_guard`, `mixed_or_and`,
`safety_envelope`, `parser_dispatch`) have varying numbers of
surviving branches depending on rustc's lowering choices for their
specific shapes. Their `TRUTH-TABLE.md` files document the
hand-derived source-level MC/DC; the witness report shows the
Wasm-level MC/DC. The discrepancy between the two is itself
evidence of how aggressive the optimizer's elision is — useful
data for the v0.7 work on inlined-subroutine handling and decision
reconstruction extension.

### Implements / Verifies

- Implements: REQ-034 (on-Wasm trace-buffer instrumentation; v0.6.1
  uses per-row globals as the simplest correct primitive instead of
  the linear-memory trace buffer recommended by Agent A — both
  satisfy the requirement, the per-row globals are simpler when each
  row invokes the predicate exactly once).
- Implements: FEAT-015 (the runtime side of the v0.6 redo).
- Verifies: leap_year verdict end-to-end pipeline produces the
  expected Wasm-level MC/DC report (1 decision, 2 conditions, full
  MC/DC under masking).

### Notes for v0.6.2

- Investigate why state_guard / triangle / mixed_or_and decisions
  don't always reconstruct under DWARF-based grouping despite having
  surviving br_ifs. Likely fix: relax the `(function, source_file,
  source_line)` grouping criterion to handle inlined-subroutine line
  attribution.
- Consider whether the per-row-globals primitive should evolve toward
  the linear-memory trace buffer (Agent A's recommendation) once
  v0.7's scaling work surfaces hot-loop overflow patterns.
- Per-target br_table MC/DC reconstruction (DEC-015 deferral).

## [0.6.0] — 2026-04-25

### What v0.6 is — and what it is not

v0.5.0 shipped DWARF-grouped branch coverage but the report layer
computed *per-branch hit counts*, not MC/DC truth tables. The CHANGELOG
described it as MC/DC; that was an overclaim. v0.6 is the redo: the
schema, the reporter, the verdict suite, and the V-model artefact graph
that real MC/DC requires. The on-Wasm instrumentation that captures
per-row condition vectors lands as a v0.6.1 follow-up — see "Deferred
to v0.6.1" below.

### Added — schema and reporter

- **`RunRecord` schema v3**: new `decisions: Vec<DecisionRecord>` and
  `trace_health: TraceHealth` fields (REQ-027, FEAT-012, DEC-013).
  `DecisionRecord` carries per-decision `rows: Vec<DecisionRow>`; each
  `DecisionRow` has a sparse `evaluated: BTreeMap<u32, bool>` so
  short-circuited conditions are first-class evidence (DEC-014). v0.5
  records (schema "2") still load — both new fields default to empty.
- **`witness-core::mcdc_report` module**: per-decision truth tables,
  independent-effect citations under masking MC/DC (DO-178C accepted
  variant), gap analysis with row-closure recommendations (REQ-028,
  REQ-029). 6 unit tests covering all canonical decision shapes pass.
- **`witness report --format mcdc`** and **`--format mcdc-json`**:
  CLI surface for the new reporter. Schema URL
  `https://pulseengine.eu/witness-mcdc/v1`.

### Added — verdict suite (REQ-030, FEAT-012, DEC-016)

The `verdicts/` directory contains seven canonical compound-decision
verdicts, each as a self-contained Rust crate that compiles to
`wasm32-wasip2`. Each verdict ships:

- `Cargo.toml` — standalone, opts out of the witness workspace.
- `src/lib.rs` — the predicate plus `run_row_<n>` exports, one per
  test row.
- `TRUTH-TABLE.md` — the **expected** MC/DC analysis, hand-derived,
  with a machine-readable JSON block. Acts as the verification oracle
  for the `mcdc_report` reporter.
- `V-MODEL.md` — one-page traceability: REQ → DEC → conditions → rows
  → evidence.
- `build.sh` — standalone build to `wasm32-wasip2`.

The seven verdicts and their shapes:

| Verdict | Decision | Conds | Rows |
|---|---|---|---|
| `leap_year` | `(y%4==0 && y%100!=0) \|\| y%400==0` | 3 | 4 |
| `range_overlap` | `a.start <= b.end && b.start <= a.end` | 2 | 3 |
| `triangle` | Myers-paper "not a triangle" check (3-cond OR) | 3 | 4 |
| `state_guard` | TLS handshake guard (4-cond AND chain) | 4 | 5 |
| `mixed_or_and` | `(a\|\|b) && (c\|\|d)` | 4 | 5 |
| `safety_envelope` | 5-cond automotive envelope (beyond LLVM 6-cap) | 5 | 6 |
| `parser_dispatch` | RFC 3986 URL authority validator (real-world anchor) | 5 | 6 |

The reporter's correctness has been verified by reproducing each
verdict's hand-derived `TRUTH-TABLE.md` from a hand-curated
`DecisionRecord` in unit tests.

### Added — V-model artefact graph (REQ-032, FEAT-014, DEC-017)

- 7 new requirements (REQ-027..033)
- 3 new features (FEAT-012..014)
- 6 new design decisions (DEC-013..018), with DEC-013 documenting the
  trace-buffer instrumentation primitive recommendation from the
  `v06-instrumentation-primitive` research brief.
- `rivet validate` PASS across the workspace.

### Added — research roadmap (4 parallel agent docs, ~19k words total)

- `docs/research/v06-instrumentation-primitive.md` — chooses linear-
  memory trace buffer with row markers as the v0.6.1 instrumentation
  primitive. Wasm-side rewrite sketch, schema diff, short-circuit
  semantics policy, BrTable v0.7 deferral, prior-art citations,
  implementation risk register.
- `docs/research/v07-scaling-roadmap.md` — destination workload pick:
  `seanmonstar/httparse` (~1500 decisions, clean wasm32-wasip2 build).
  v0.7 capability list (streaming counter encoding, i64 saturating
  counters, inlined-subroutine DWARF, auto-generated synthetic
  requirements, module-rollup default report). Top scaling risk:
  DWARF parsing memory at scale.
- `docs/research/v08-visualisation-roadmap.md` — architecture call:
  `wstd-axum` + `maud` + HTMX 2.x, runnable as `wasmtime serve` or
  composed via `wac plug`. AI-agent surface = REST+JSON content
  negotiation plus `rmcp` MCP transport mounted on the same Axum
  router. Playwright tests reuse rivet's pattern; visualiser
  visualises its own coverage (the v0.8 demo screenshot).
- `docs/research/v09-soa-and-agent-ux.md` — competitive scan
  (LDRA, VectorCAST, Cantata, BullseyeCoverage, Squore, gcov+gcovr).
  v0.9 positioning: first MC/DC tool with end-to-end signed evidence
  and agent-native MCP API. Top 3 superiority features identified.
  Biggest competitive risk: RapiCover already has unbounded
  conditions plus DO-178C heritage for C/C++/Ada.

### Deferred to v0.6.1

- **On-Wasm instrumentation that captures per-row data.** The
  trace-buffer rewrite from `v06-instrumentation-primitive.md` is
  scoped for v0.6.1. v0.6.0 ships the consumer side (schema +
  reporter + verdict suite oracles + CLI). The `witness instrument`
  subcommand still emits v0.5-style per-counter instrumentation;
  v0.6.1 extends it with the trace primitive so `witness run`
  produces populated `RunRecord.decisions`.
- **End-to-end verdict execution.** Each verdict's `src/lib.rs`,
  `TRUTH-TABLE.md`, and `V-MODEL.md` are in place; `cargo build
  --target wasm32-wasip2` against each verdict crate produces a
  `.wasm`. The reporter's correctness has been verified against the
  hand-derived truth tables in unit tests, and the verdicts' V-MODEL
  evidence chains will be populated by `compliance` when v0.6.1's
  instrumentation lands.

### Why ship the foundation as 0.6.0

The schema, reporter, verdict-suite oracles, and V-model artefact
graph are independent of the instrumentation runtime path. Shipping
them as v0.6.0 lets downstream consumers (rivet, sigil, agent
integrations) build against the v3 schema and the
`witness-mcdc/v1` predicate type now, while the instrumentation
work continues in the v0.6.1 release. The verdicts' `TRUTH-TABLE.md`
files are the verification oracles v0.6.1 will reproduce.

### Implements / Verifies

- Implements: REQ-027 (truth-table emission), REQ-028 (independent-
  effect citations), REQ-029 (gap-closure recommendations), REQ-030
  (verdict suite — scaffolded), REQ-032 (V-model traceability —
  artefact graph), REQ-033 (compliance bundle structure).
- Implements: FEAT-012 (real MC/DC reporter — consumer side),
  FEAT-014 (V-model artefact graph).
- Verifies: 6 mcdc_report unit tests reproduce each canonical verdict
  shape's expected truth table and pair-finding outcomes.

## [0.5.0] — 2026-04-25

### Added

- **Workspace split.** Single-crate `witness` becomes a workspace with
  `crates/witness-core` (pure-data algorithms; `wasm32-wasip2`-buildable)
  and `crates/witness` (CLI binary plus the wasmtime-using runner).
  All algorithm modules — instrument, decisions, diff, predicate,
  report, rivet_evidence, run_record, lcov, attest — live in
  witness-core. Only main.rs + run.rs (wasmtime embedder) stay in the
  binary crate.
- **`witness lcov`** subcommand (REQ-023). Emits LCOV from a
  `RunRecord` per the
  [v0.5 LCOV format brief](docs/research/v05-lcov-format.md). Hybrid
  emission: DWARF-correlated `Decision`s become standard `BRDA`
  records keyed to real source files; uncorrelated branches go in a
  sibling overview text. Codecov-ingestible as `flag: wasm-bytecode`.
- **`witness attest`** subcommand (REQ-024). Wraps an unwrapped
  in-toto Statement (from `witness predicate`) in a DSSE envelope
  signed with an Ed25519 secret key. Compatible with sigil's
  `wsc verify`, sigstore cosign, and any in-toto-attestation
  consumer. Implementation depends on the workspace `wsc-attestation`
  path-dep into `pulseengine/sigil`.
- **Wasm-target artefact.** `cargo build -p witness-core --target
  wasm32-wasip2` produces `target/wasm32-wasip2/release/witness_core.wasm`,
  uploaded as a CI artefact and (in release builds) attached to the
  GitHub release. The full Component Model build with WIT bindings
  is the v0.6 stretch goal.
- **CI dogfood loop.** New `dogfood` job builds the
  `sample-rust-crate` fixture, instruments it with the freshly-built
  witness, runs every `run_*` export, and emits LCOV. Uploads to
  codecov with `flag: wasm-bytecode` for side-by-side comparison
  with the existing `flag: rust-source` LCOV (cargo-llvm-cov).
- **`witness-core` Wasm-target CI job.** Verifies witness-core
  compiles to `wasm32-wasip2` on every push to main; uploads the
  resulting `.wasm` artefact.
- **Loom + meld upstream issue drafts** at
  `docs/research/v05-loom-meld-upstream.md` ready for the maintainer
  to file. Both ask for DWARF preservation plus a byte-offset
  translation map so witness v0.6 can correlate post-loom / post-meld
  Wasm to source-level decisions.

### Research output

- `docs/research/v05-blog-principles.md` (placeholder; previously
  `v04-blog-principles.md` covers the same corpus).
- `docs/research/v05-lcov-format.md` — codecov-flag-compatible LCOV
  emission; recommends hybrid C strategy (BRDA for correlated, text
  overview for uncorrelated).
- `docs/research/v05-wsc-integration.md` — wsc-attestation API
  surface, Cargo dep model, witness-attest subcommand sketch. Confirmed
  wasm32 compatibility under the `signing` feature.
- `docs/research/v05-component-witness.md` — component-model build
  path; confirms cargo-component, wac, wit-bindgen all installed
  locally; identifies wasmtime as the only host-only dep.
- `docs/research/v05-loom-meld-upstream.md` — issue drafts for the
  upstream tools.

### Changed

- The `coverage` CI job now uploads with `flag: rust-source` so the
  new bytecode-coverage upload (`flag: wasm-bytecode`) renders
  side-by-side in codecov.
- Workspace pulls `wsc-attestation` from a sibling
  `pulseengine/sigil` checkout (path dep). Will become a regular
  crates.io dep when wsc-attestation publishes.
- Direct `ed25519-compact` dep added to witness-core for keypair
  generation in tests and direct use by `attest.rs`.

### Implements / Verifies

- Implements: REQ-023 (witness lcov), REQ-024 (witness attest), plus
  the v0.5 workspace-split and dogfood-CI requirements (REQ-025,
  REQ-026 in the artefact set).

### Deferred to v0.6

- DWARF preservation through loom optimisation (gated on the upstream
  loom issue).
- DWARF preservation through meld fusion (gated on the upstream meld
  issue).
- Full Component Model build with WIT interface and `wac`-based
  composition with sigil's wsc component for in-process signing.

## [0.4.0] — 2026-04-25

### Added

- **DWARF-grounded MC/DC reconstruction** (FEAT-011, REQ-005, REQ-006,
  REQ-016). `decisions::reconstruct_decisions` now parses Wasm DWARF
  custom sections via `gimli` and `wasmparser`, builds a
  `(byte_offset → file, line)` map per compilation unit, and groups
  `BrIf` `BranchEntry`s sharing a `(function, file, line)` key into
  source-level `Decision`s. Strict per-`br_if` fallback when DWARF is
  absent. Lifted from v0.2.1 plan; v0.2.1 is therefore not released as
  a separate version.
- **`witness diff` subcommand** (REQ-020). Computes added / removed /
  changed branches and (when both inputs are runs) coverage-percentage
  delta. Schema URL `https://pulseengine.eu/witness-delta/v1`. Both
  JSON and text output. Required by the v0.4 PR delta workflow.
- **`witness-delta.yml` PR workflow** (REQ-022). Triggers on every PR
  touching `src/` / `tests/` / `Cargo.toml`. Checks out base + head,
  builds the head witness, runs `witness diff` on whatever manifests
  the fixture pipeline emits, attaches the delta JSON+text as a PR
  artefact. `continue-on-error` throughout — never blocks merge.
- **`actions/compliance` composite action** (REQ-021). Mirrors rivet's
  equivalent. Generates a tar.gz evidence bundle on release containing
  coverage report, in-toto predicates per module, branch manifests,
  and a README. Wired into `release.yml` between `build-binaries` and
  `create-github-release` as a new `compliance` job; the resulting
  archive is attached to the GitHub release alongside the binaries.

### Research output

- `docs/research/v04-blog-principles.md` — survey of every published
  pulseengine.eu post and the principles witness must adopt; 4756
  words across 14/16 posts; 20-item adoption checklist; voice
  mechanics catalogued.
- `docs/research/v04-ci-ports.md` — adaptation brief for
  rivet-delta.yml and the rivet compliance composite action; full
  YAML drafts for both witness-side workflow files.
- `docs/research/v04-compiler-qualification-reduction.md` — 451-line
  brief: ISO 26262-8 §11.4.5 substitution argument for ASIL B
  (works), DAL B (weaker), DAL A (broken). Most surprising finding:
  the TCL framework explicitly yields TCL 1 — "no qualification
  required" — when TI 1 *or* TD 1 holds; the work is in establishing
  TD 1, not in carving an exception.
- `docs/research/v04-mythos-slop-audit.md` — quick-pass slop audit
  using the methodology from
  <http://127.0.0.1:1024/blog/mythos-slop-hunt/>. Two P1 findings
  applied (deleted `report::save_json`; removed direct `tracing` dep).
  Two P2 findings kept as consumer-facing constants. Twelve P3
  findings documented as intentional defensiveness.

### Removed

- `report::save_json` — orphan-slop, no callers (P1 slop-hunt finding).
- `tracing = "0.1"` direct dependency — only `tracing-subscriber` is
  actively used (P1 slop-hunt finding).

### Deferred to v0.5

- Component-model coverage (was nominal v0.4; needs walrus or wac
  component support).
- Post-cfg / post-meld / post-loom measurement points (depends on
  loom's translation-validation evidence shape, which is itself
  evolving).
- A Wasm Component Model fixture for end-to-end testing (folded with
  the above).

### Implements / Verifies

- Implements: REQ-005, REQ-006, REQ-016, REQ-020, REQ-021, REQ-022
- Implements: FEAT-011 (v0.4 feature wrapper)

## [0.3.0] — 2026-04-25

### Added

- **`witness merge`** subcommand. Aggregates per-branch counters across
  multiple `witness run` outputs (one per test binary or harness
  invocation). Validates that all inputs share the same instrumented
  module and branch list before summing. Five new tests + four proptest
  properties (commutativity, monotonicity, sum-preservation,
  single-record identity).
- **`witness predicate`** subcommand. Emits an unwrapped in-toto
  Statement v1.0 carrying the coverage report as a
  `https://pulseengine.eu/witness-coverage/v1` predicate. Subject is
  the instrumented module (sha256); the original module's digest goes
  in the predicate body. Sigil reads the predicate type opaquely (no
  registry, no schema validation per type — see
  `docs/research/sigil-predicate-format.md`), so witness emits today
  with no sigil-side change. 5 unit tests including known-vector
  SHA-256 and RFC 3339 timestamp calibration.
- **`witness rivet-evidence`** subcommand. Emits coverage in the
  `https://pulseengine.eu/witness-rivet-evidence/v1` schema, partitioned
  by a user-supplied `branch_id → artefact_id` mapping YAML. The
  schema mirrors rivet's existing `ResultStore` shape so the consumer
  can be a near-drop-in copy. 4 unit tests + 2 proptest properties on
  RequirementMap flattening.
- **rivet upstream consumer** on the
  `feat/witness-coverage-evidence-consumer` branch in
  `pulseengine/rivet`. Adds `rivet-core::coverage_evidence::CoverageStore`
  mirroring `ResultStore`, plus 9 unit tests, plus the new
  `Error::CoverageEvidence` variant. 491 LOC. 780 rivet-core tests
  pass; clippy/fmt/deny clean. Branch is **left local for review** —
  not pushed to origin.
- **`docs/research/rivet-evidence-consumer.md`** and
  **`docs/research/sigil-predicate-format.md`** — evidence-of-design
  briefs that established the schemas before the code was written.
- **Rust→Wasm test fixture** under `tests/fixtures/sample-rust-crate/`.
  A minimal `no_std` Rust crate that compiles to Wasm and exercises
  every instrumentation pattern (`br_if`, `if/else`, `br_table`).
  Eight integration tests in `tests/integration_e2e.rs` runtime-skip
  if the fixture isn't built; `./tests/fixtures/sample-rust-crate/build.sh`
  is the one-shot builder for CI.

### Quality bar (REQ-019, FEAT-010)

- **Property-based tests** via `proptest` (new dev-dependency). 8
  properties covering merge invariants, serde round-trip of `Manifest`
  / `RunRecord`, and `RequirementMap::flatten` semantics. CI's
  `proptest-extended` job on main runs with `PROPTEST_CASES=2048`.
- **Mutation testing** via `cargo-mutants`. New CI job `mutants` runs
  on main as informational (continue-on-error: true). Configuration
  in `.cargo/mutants.toml` constrains mutation to the witness library
  and skips test modules.
- **Miri** CI job runs nightly miri with `-Zmiri-tree-borrows` over the
  pure-Rust modules (`report::*`, `decisions::*`, predicate's SHA
  vector + RFC 3339 path). The walrus / wasmtime FFI surface is
  excluded — miri's foreign-call constraints make it more noise than
  signal there.
- **Coverage threshold raised** to 75% project / 80% patch
  (`codecov.yml`).

### Implements / Verifies (rivet trailers)

- Implements: REQ-007, REQ-008, REQ-017, REQ-018, REQ-019
- Implements: FEAT-003 (rivet/sigil integration), FEAT-010 (quality bar)

### Notes

- v0.2.1 (DWARF reconstruction algorithm body) remains an any-time
  release. The schema is forward-compatible — when v0.2.1 lands, the
  rivet-evidence and predicate emitters automatically populate
  `decisions: [...]` for hosts that consume MC/DC.
- rivet integration is end-to-end **once the rivet upstream branch is
  pushed and a rivet release cuts**. The witness output is correctly
  shaped today; the rivet consumer code is on a feature branch.

## [0.2.0] — 2026-04-25

### Added

- **Subprocess harness mode** (`witness run --harness <cmd>`). Spawns a
  user-supplied command via `sh -c` with `WITNESS_MODULE` /
  `WITNESS_MANIFEST` / `WITNESS_OUTPUT` env vars set; the harness writes
  a counter snapshot to `WITNESS_OUTPUT` before exiting. Witness merges
  the snapshot with the manifest to produce the final run JSON. Escape
  hatch for runtimes the embedded wasmtime cannot accommodate
  (browser-based tests, custom WASI capability profiles, etc.).
  Implements REQ-014 / FEAT-006 / DEC-009.
- **Per-target `br_table` counting** (REQ-013 / FEAT-007 / DEC-008). v0.1's
  single-counter "executed" instrumentation is replaced with one counter
  per target plus one for the default arm. A generated
  `__witness_brtable_<n>` helper function dispatches on the selector via
  i32.eq chain (or i32.ge_u for the default), increments the matching
  counter, and returns the selector unchanged for the original
  `br_table` to dispatch. `BranchKind::BrTable` is removed; replaced by
  `BrTableTarget` (with `target_index: u32`) and `BrTableDefault`.
- **Manifest schema v2** (`schema_version: "2"`). Adds:
  - `BranchEntry.byte_offset: Option<u32>` — original wasm bytecode
    offset from walrus's `InstrLocId`. Required for DWARF correlation.
  - `BranchEntry.target_index: Option<u32>` — for `BrTableTarget` only.
  - `Manifest.decisions: Vec<Decision>` — DWARF-grounded source-level
    decisions reconstructed from `br_if` sequences. Empty when DWARF is
    absent or the v0.2.0 stub is in effect.
- **No artificial condition-count cap** (REQ-015). Witness uses exported
  globals, not LLVM's bitmap encoding, and supports decisions of any
  size.
- **`docs/paper/v0.2-mcdc-wasm.md`** — 8.2k-word paper draft covering
  motivation, formal MC/DC at Wasm, the reconstruction algorithm, the
  coverage-lifting soundness argument (DEC-010), comparison with
  rustc-mcdc / Clang / wasmcov / Whamm, and regulatory framing. Six
  sourcing TODOs for closed-access primary references (DO-178C clause,
  Chilenski & Miller 1994, Vilkomir & Bowen, Pnueli et al., DWARF
  spec).
- **README**: new "Related work" section with seven-row comparison
  table; status updated to "v0.1.0 shipped 2026-04-24"; usage examples
  refreshed to show both `--invoke` and `--harness` modes.

### Changed

- **MSRV unchanged at 1.91** (matches wasmtime 42's transitive floor).
- `Module` is loaded via `from_buffer` rather than `from_file` so the
  original bytes are available to the (stubbed) DWARF reconstructor.

### Stubbed (lands in v0.2.1)

- **DWARF-grounded reconstruction algorithm body** (DEC-012). v0.2.0
  ships the schema and the fallback path; the algorithm itself
  (`src/decisions.rs::reconstruct_decisions`) currently returns an
  empty list, leaving hosts on the strict per-`br_if` interpretation.
  The algorithm is documented in `docs/paper/v0.2-mcdc-wasm.md`. The
  schema is forward-compatible; v0.2.1 will fill the stub without a
  schema bump.

### Implements / Verifies (rivet trailers)

- Implements: REQ-013, REQ-014, REQ-015, REQ-016
- Verifies: REQ-004 (semantic preservation; round-trip tests pass for
  br_if, if-else, br_table)

## [0.1.0] — 2026-04-24

### Added

- `witness instrument <in.wasm> -o <out.wasm>` — walrus-based branch-counter
  insertion at every `br_if`, `if-else`, and `br_table` in every local
  function. Counter values are exposed as exported mutable globals named
  `__witness_counter_<id>`. Emits a sidecar manifest JSON describing each
  branch's function index, instruction index within its sequence, and
  kind.
- `witness run <instrumented.wasm> --invoke <export>` — built-in wasmtime
  runner that instantiates the module, invokes the requested no-argument
  export(s), reads all counter globals, and writes a raw run JSON.
  WASI-preview1 is wired with `inherit_stdio`; `--call-start` runs the
  WASI `_start` command-style entry-point.
- `witness report --input <run.json>` — branch-coverage report in human
  text or JSON. Per-function aggregation, deterministic uncovered-branch
  ordering.
- Library crate `witness::{instrument, run, report, error}` for callers
  that want to drive the pipeline programmatically (rivet integration in
  v0.3 will use this entry-point).
- SCRC Phase 1 + 2 clippy lints enforced workspace-wide; `cargo clippy
  --all-targets -- -D warnings` is a hard CI gate.
- Cross-platform CI: fmt, clippy, test matrix (Linux/macOS/Windows),
  MSRV (1.85), cargo-deny, cargo-audit, coverage via cargo-llvm-cov +
  codecov.
- Release workflow: tag-triggered cross-compiles for five targets
  (x86_64/aarch64 Linux, x86_64/aarch64 macOS, x86_64 Windows) with
  SHA256 checksums and auto-generated release notes.

### Design notes

- **Counter mechanism.** v0.1 exposes counters as exported mutable
  globals rather than a `__witness_dump_counters` function that
  serialises to linear memory. The exported-global path removes any
  cooperation-protocol requirement on the module-under-test and makes
  the runtime-side extraction a two-line `instance.get_global` for every
  conformant Wasm host. A dump-function escape hatch can be added later
  if an embedder requires a single exit point.
- **`br_table` granularity.** v0.1 counts `br_table` as a single
  "executed" point, not per-target. Per-target counting is a v0.2
  concern alongside DWARF-informed decision reconstruction; counting
  each target requires reconstructing which arm was taken from the
  selector, which materially complicates the rewrite without
  information DWARF-in-Wasm will give us cheaply in v0.2.
- **Harness model.** v0.1 ships the wasmtime-embedded runner only;
  subprocess-harness mode (`--harness <cmd>`) is deferred to v0.2 for
  modules that need a richer runtime.

### Research briefings

- `docs/research/rivet-template-mapping.md` — mapping of rivet's CI,
  lint, and release patterns adapted to witness's single-crate scope.
- `docs/research/overdo-alignment.md` — alignment brief extracting
  design constraints C1–C7 from the *Overdoing the verification chain*
  blog post the project's AGENTS.md cites.
