# Changelog

All notable changes to witness are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [SemVer 2.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.27.0] — 2026-05-29

Headline: **crates published to crates.io again** (under the
`witness-mcdc-*` namespace), and **per-condition provenance** in
the dashboard — each condition now shows which function its branch
lives in, the branch kind, and the inline call chain when one
exists.

### Added — per-condition provenance (PR #63, FEAT-032)

The Decision page shows, under each condition, the provenance
joined from `manifest.json`: the demangled **function** the branch
lives in, its **kind** (`br_if` / `br_table_target` /
`br_table_default`), and — when the branch was genuinely inlined —
its DWARF **inline call chain** (`a.rs:1 ← b.rs:2`).

This answers "what are these N conditions on one line?" The
motivating example (json_lite decision #29, 29 conditions) reads
as `verdict_json_lite::parse_primitive · br_table_target` ×29 —
i.e. the 29 arms of a `match` compiled to one `br_table`, not an
inlined call. (The investigation corrected an earlier assumption:
`branch_inline_chains` covers only genuinely-inlined branches — 2
of 165 in json_lite — so `function + kind` is the primary signal,
the chain a bonus layer. See REQ-052 / DEC-035.) Viz-only: parses
the existing manifest, no witness-core change, no schema bump.
Degrades to pre-v0.27 output when no manifest is present. Adds
`rustc-demangle`.

### Changed — crates renamed to `witness-mcdc-*` (PR #63, FEAT-031)

To sidestep the crates.io name conflict that blocked publishing
since v0.23 (an unrelated prior registration owns `witness` /
`witness-core`):

| crates.io package | was |
|---|---|
| `witness-mcdc` | `witness` |
| `witness-mcdc-core` | `witness-core` |
| `witness-mcdc-viz` | `witness-viz` |
| `witness-mcdc-checker` | (unchanged) |

**Package-name change only (DEC-034).** Every `[lib] name` and
`[[bin]] name` is unchanged, so `use witness` / `use witness_core`
compile untouched and `cargo install witness-mcdc` still installs
a `witness` command. CI, the verdict-suite runner, and the rivet
`verification.yaml` oracle were updated to the new `-p` specs.

### Published to crates.io

`witness-mcdc-checker`, `witness-mcdc-core`, `witness-mcdc`,
`witness-mcdc-viz` — the first full-set publish since v0.23. The
installed CLI is still `witness` (and `witness-viz`).

New rivet artifacts: REQ-051/052, FEAT-031/032, DEC-034/035
(approved).

### Known limitations (carried)

- 4 of 13 verdicts show no source on the dashboard (decisions
  attribute to `~/.cargo` dependency files, not vendored under
  `verdicts/`).
- Kotlin/Wasm still produces 0 decisions (kotlinc `if`/`else`).

## [0.26.0] — 2026-05-29

Headline: **the published dashboard now hosts the last 3 releases
side by side** with a cross-version MC/DC summary, instead of only
the latest. Plus the inline source snippet is now
syntax-highlighted and correctly single-spaced.

### Added — multi-version Pages + `pages-index` (PR #61)

The release `publish-pages` job auto-detects the last 3 release
tags and builds a multi-version site:

- Current tag renders under `/<VERSION>/` with `--source-root
  verdicts` (source snippets + full files).
- The 2 most-recent prior published releases are downloaded
  (their `compliance-evidence` asset) and rendered under
  `/<tag>/` without source (their decisions would mis-point
  against the moved-on `verdicts/` tree).
- A new `witness-viz pages-index --site-dir <dir>` (and `witness
  viz-pages-index` passthrough) scans the `vX.Y.Z/` subdirs, reads
  each `summary.json`, semver-sorts newest-first, and writes
  `<dir>/index.html` — a cross-version table (verdicts /
  decisions / full-MC/DC / proved / gap / dead / source files)
  with Δ-vs-next-older annotations and links into each versioned
  dashboard.

`summary.json` gains the MC/DC aggregates
(`decisions_full_mcdc`, `conditions_proved/gap/dead`) so the index
builds from summary.json alone; `#[serde(default)]` keeps
pre-v0.26 summaries loading.

**Live-URL change:** `https://pulseengine.github.io/witness/` now
shows the cross-version landing page; the latest dashboard moves
to `/<VERSION>/`.

New rivet artifacts: REQ-050, FEAT-030, DEC-033 (approved).

### Fixed — inline snippet rendering (PR #60)

The Decision/Gap inline source snippet was (1) double-spaced (a
`writeln!` trailing `\n` *and* `display:block` both broke the
line) and (2) plain text while the full-file page was
syntax-highlighted. Extracted `highlight_source_lines` shared by
both (highlights the whole file so multi-line constructs carry
state, then the snippet slices its window); the snippet is now
syntect-highlighted, single-spaced, with a `.ln` gutter matching
the full-file page.

### Known limitations (carried)

- The inline **call chain** for inlined decisions (e.g. json_lite
  decision #29's 29 conditions, all branches of an inlined
  `parse_string`) is computed and present in `manifest.json`
  (`branch_inline_chains`) but not yet rendered in the dashboard —
  a reviewer sees *that* a line has N conditions but not *where*
  each lives. Queued as a viz-only follow-up.
- crates.io publish for `witness-core` / `witness` still blocked
  by the `flammafex/Sibyl` name conflict.
- 4 of 13 verdicts show no source (decisions attribute to
  `~/.cargo` dependency files, not vendored under `verdicts/`).

## [0.25.0] — 2026-05-28

Headline: **`witness viz-pr-comment`** — the PR-comment leg of the
Codecov-parity story. The trio is now complete: live HTMX
dashboard (`witness viz`), static source-visible export (`witness
viz-export`), and an inline MC/DC delta comment for pull requests
(`witness viz-pr-comment`).

### Added — PR-time MC/DC delta comment (PR #58)

`witness viz-pr-comment --base <dir-or-report> --head <dir-or-report>`
emits Markdown to stdout (pipe into `gh pr comment --body-file -`)
or `--out FILE`:

```
## MC/DC coverage delta
| verdict | decisions | full MC/DC | proved | gap | dead |
| `v` | 1 | 1 → 0 (-1) | 2 → 1 (-1) | 0 → 1 (+1) | 0 |
| **TOTAL** | … |
### ⚠️ Regressions (proved → gap/dead)
- `v` decision #0 c1 (`lib.rs:10`): proved → gap
```

- A per-verdict + TOTAL table with `before → after (±N)` cells for
  decisions / full-MC/DC / proved / gap / dead.
- "Regressions" (proved → gap/dead), "Improvements" (gap/dead →
  proved), and "Other transitions" (gap ↔ dead) sections naming
  each per-condition change by verdict / decision / condition
  coordinate.
- Either `--base` / `--head` may be a verdict-evidence directory or
  a single `report.json` (auto-detected via `data::load_report_set`).

Design (DEC-032): match base↔head **verdicts by name, decisions by
`id`, conditions by `index`** — stable identity keys, O(n),
deterministic, no fuzzy alignment (which would mis-pair decisions
across a refactor and emit false regressions). A
verdict/decision/condition on only one side is reported
added/removed (🆕 / ❌removed table markers), never diffed.

New rivet artifacts: REQ-049, FEAT-029, DEC-032 (all `approved`).

### Known limitations (carried)

- crates.io publish for `witness-core` / `witness` still blocked by
  the `flammafex/Sibyl` name conflict.
- 4 of 13 canonical verdicts show no source on the dashboard
  (decisions attribute to `~/.cargo` dependency files, not vendored
  under `verdicts/`) — vendoring measured deps is deferred.
- Kotlin/Wasm still produces 0 decisions (kotlinc `if`/`else`, not
  `br_if`).

## [0.24.1] — 2026-05-28

Fix: **source visibility now actually populates the published
dashboard**. v0.24.0 shipped the feature but the auto-published
dashboard showed `source_files: 0` because the canonical reports
carry basename-only `source_file` values (DWARF `DW_AT_name`,
e.g. `lib.rs`) and `--source-root .` resolved them against the
repo root — which has no `lib.rs`. The witness fixtures live at
`verdicts/<name>/src/<basename>`, so resolution must be
verdict-scoped.

### Fixed

- New `resolve_source_path(source_root, verdict, source_file)` tries,
  in order: `<root>/<verdict>/<source_file>`,
  `<root>/<verdict>/src/<basename>` (the canonical fixture layout),
  `<root>/<source_file>`, `<root>/<basename>`. Both the inline
  snippet (`render_source_snippet_for`) and the full-file page
  emission share it.
- Full-file source pages are now **verdict-scoped**:
  `out/source/<verdict>/<source_file>.html`. v0.24.0 wrote
  `out/source/<source_file>.html`, which would have **collided**
  across verdicts (two verdicts each with a `lib.rs` clobbered one
  another). `link_to_source` and the "view full file →" link match
  the new path.
- release.yml `publish-pages` now passes `--source-root verdicts`
  (was `--source-root .`).

After this fix, the canonical dashboard resolves **9 of 13**
verdicts' own `lib.rs` (the 4 misses are verdicts whose decisions
attribute only to dependency files under `~/.cargo`, not vendored
under `verdicts/`). `source_files: 0 → 9`.

### Known limitation

Decisions attributed to dependency-crate files (e.g. json_lite's
`accum.rs` from the measured JSON crate) still show no source —
those files aren't under `verdicts/<name>/`. Vendoring measured
dependency sources into the evidence bundle is deferred future work.

## [0.24.0] — 2026-05-27

Headline: **source visibility lands in the static MC/DC
dashboard**. Each Decision and Gap page now shows a `±5 lines`
inline snippet around the recorded `source_file:source_line` plus
a "view full file →" link to a syntect-highlighted full source
page with `#L<n>` anchors. Closes the "what code am I looking
at?" gap v0.23.0's first cut left open. Plus a ~35% page-count
reduction on the canonical fixture set, retrospective rivet
artifact backfill, and the two follow-up chores from v0.23.0.

### Added — source visibility in MC/DC reports (PR #55)

New `witness viz-export --source-root <repo-root>` flag (and on
`witness-viz` directly). When set:

- **Inline snippet** on Decision and Gap pages: ±5 lines around
  `source_file:source_line`, target line highlighted with a left
  border, gutter, and `>` marker. Plain `<pre>` — ~300-500 bytes
  per page added.
- **Full-file pages** at `out/source/<path>.html` — one page per
  unique source file across all Decisions, syntax-highlighted via
  `syntect` (Rust, C, C++, Swift, Kotlin, Zig, Go), `#L<n>`
  anchors for deep links from snippets, `.marked` class on lines
  that carry a Decision so they stand out.
- **"view full file →"** link below each snippet, depth-aware so
  the relative URL is correct from any page level.
- Path safety: refuses `/...` absolute or `..` traversal in
  `source_file` paths; missing files degrade gracefully (snippet
  suppressed, rest of page renders unchanged).
- Manifest gains `source_files` counter.

Release workflow's `publish-pages` job now passes `--source-root .`
so the auto-published dashboard at
https://pulseengine.github.io/witness/ shows source for every
canonical-fixture Decision.

### Changed — skip proved-condition gap pages (PR #55)

The gap drill-down for `proved` conditions rendered "already
proved, no action needed" — accurate but not actionable. They
typically dominate (~80% of conditions) the canonical fixtures.
Skipping them in export mode cuts the canonical 13-verdict dist
from ~875 → ~290 pages (~35% reduction). Dead conditions KEEP
their drill-down because the "compiler folded / unreachable from
harness" investigation is still actionable for the reviewer. The
Decision page still surfaces `proved` status via the badge.

### Added — rivet artifact backfill for v0.23 + v0.24 (PR #54)

The forward-looking artifact set had stopped at FEAT-023 (v0.9
agent-gap loop), leaving v0.23.0's five shipped items with zero
typed traceability. This release closes that retrospective gap:

- v0.23 (approved): REQ-044 + FEAT-024 (viz-export), REQ-046 +
  FEAT-026 (release-artefact standardisation), REQ-047 + FEAT-027
  (cargo-witness all), REQ-048 + FEAT-028 (V3 source-map
  fallback), DEC-028 (pure-render seam), DEC-029 (Pages publish),
  DEC-030 (walrus 0.26.3 unpin).
- v0.24 (proposed → approved on landing this release): REQ-045 +
  FEAT-025 + DEC-031 (source visibility).

`rivet validate` passes; no orphans, no broken cross-refs.

### Housekeeping

- PR #52 — release.yml carries a comment on the github-pages
  tag-policy gotcha (one-shot `gh api` for `v*` tag policy).
- PR #53 — intra-workspace path-deps now carry version specs
  (`version = "0.24.0"`) so `cargo publish --dry-run` passes
  without `--allow-dirty` fixup.

### New dependency

`syntect = "5"` with `default-fancy` features. ~5 MB embedded
syntax/theme assets in the binary; enables Rust, C, C++, Swift,
Kotlin, Zig, Go highlighting with zero runtime config.

### Known limitations (unchanged from v0.23)

- **crates.io publish** for `witness-core` and `witness` is
  blocked by a name conflict with `flammafex/Sibyl` (legitimate
  prior registration, not squatting). `witness-mcdc-checker
  0.23.0` is on crates.io; `witness-viz 0.24.0` may follow.
  Namespace decision (rename vs skip permanently) deferred.
- **Kotlin/Wasm** still produces 0 decisions on the leap-year
  fixture due to kotlinc-wasm's `if`/`else` shape (orthogonal
  to source-map ingestion).

## [0.23.0] — 2026-05-26

Headline: **PR-time MC/DC visualisation lands**. `witness
viz-export` produces a self-contained static HTML dashboard from a
verdict bundle, and every tagged release now auto-publishes the
canonical-fixture dashboard to GitHub Pages
(https://pulseengine.github.io/witness/). Plus end-to-end
ergonomics — `cargo witness all`, V3 source-map ingestion,
release-artifact standardization (SBOM + SLSA + cosign bundle),
and a docs reframe on the wasi target story.

### Added — `witness viz-export` static dashboard (PR #50)

`witness viz-export --reports-dir <…> --out <…>` walks every page
of the dashboard through the same renderer the live HTMX server
uses and writes self-contained HTML browseable from `file://`,
deployable to any static host. Output tree:

```
out/index.html
out/verdict/<name>.html
out/decision/<verdict>/<id>.html
out/gap/<verdict>/<id>/<cond>.html
out/_assets/styles.css
out/summary.json
```

No HTMX, no API endpoints in the dump. Links are depth-aware
relative (e.g. `../../verdict/foo.html` from a decision page) so
the output works from any URL prefix.

The release workflow now includes a `publish-pages` job: every
`v*` tag generates the canonical verdict-evidence dashboard and
deploys it to GitHub Pages via the official `actions/deploy-pages`
action. Site URL: https://pulseengine.github.io/witness/.

Requires Pages repo setting "Source → GitHub Actions" (already
configured for this repo).

### Changed — viz internals: pure-render seam (PRs #48, #49)

The four-page axum dashboard (overview / verdict / decision / gap)
was refactored into a pure `crate::render::render_*` core plus
thin axum handlers. The renderer takes a borrow-only
`RenderContext` whose `href_prefix` + `link_ext` knobs choose
between serve mode (`"/"`, `""` — byte-identical output with v0.22)
and export mode (depth-counted `"../"`, `".html"`). `views.rs`
shrank from ~565 lines to ~125 lines of axum plumbing only.

This is the design seam that made `witness viz-export` cheap to
build (~170 lines on top of the renderer; the renderer itself does
the heavy lifting).

### Changed — wasi target docs (PR #47)

`docs/quickstart.md` reframes the wasi target guidance: `p1` is
**today's smoothest path**, not "the recommended default", because
the ecosystem direction is `p2`/`p3`. Adjusts the tone of every
"recommended" claim without changing the practical advice for
v0.23.0 users (still use `p1` if you want a clean run today).

### Earlier in this window (commits since v0.22.0, pre-PR-stack)

These landed individually since the v0.22 tag and roll up here:

- **`witness all` / `cargo witness all`** (PR #44) — end-to-end
  pipeline subcommand. Instrument → run → report → predicate →
  attest → verify in one call, with sensible defaults the
  scaffolded fixtures use.
- **V3 source-map ingestion** (PR #41) — `witness-core` reads V3
  source maps as a fallback when DWARF is missing or incomplete;
  Kotlin/Wasm reconstruction now works for the leap-year fixture's
  Decisions even though kotlinc-wasm emits no DWARF.
- **walrus fork pin** (PR #37) — pinned to
  `pulseengine/walrus@ae623af` for the legacy try/catch panic fix
  (upstream PR #316 at wasm-bindgen/walrus, still in maintainer
  review). Unblocks Kotlin Tier B (PR #39). **In v0.23.0:
  unpinned** — upstream PR #316 merged 2026-05-26 and walrus 0.26.3
  shipped to crates.io the same day; witness now depends on the
  released `walrus = "0.26.3"`. crates.io publish unblocked.
- **Release-artifact standardization** (PR #46) — per the org-wide
  brief: CycloneDX 1.5 SBOM (`witness-X.Y.Z.cdx.json`), SLSA v1
  build provenance via `actions/attest-build-provenance`, sums-file
  cosign bundle (`SHA256SUMS.txt.cosign.bundle` + `.sig` + `.pem`
  triple), and `build-env.txt` forensics. Per-asset
  `.sig`/`.cert` carved out as a witness-specific certification
  evidence requirement.
- **Friction reductions from sigil adoption review** (PR #42) —
  five small ergonomic fixes from outside-the-team review.
- **cargo-witness subcommand alias** (PR #43) — Level-A integration
  so `cargo witness <subcmd>` resolves to `witness <subcmd>`.
- **Docs polish** — README MC/DC TL;DR (PR #36), Kotlin Tier
  D→Tier B update (PR #39).
- **wasmtime 44.0.1 → 44.0.2** (PR #38) — RUSTSEC-2026-0149.
- **walrus 0.24 → 0.26.1** (PR #34) — dependency keepup.

### Known limitations

- **Kotlin/Wasm** produces 0 decisions on the leap-year fixture
  because kotlinc-wasm emits `if`/`else` only (not `br_if`); the
  orthogonal clustering issue is a separate track from source-map
  ingestion.
- **First-release publish-pages** requires Pages source = "GitHub
  Actions" (handled via API for this repo).

## [0.22.0] — 2026-05-17

Three landed since v0.21: deeper C++ probes, Swift promoted to
Tier A, and a rivet-driven verification gate mirroring spar's
pattern. No witness-core code changes; CI infrastructure +
fixtures + schema extension.

### Added — C++ deeper probes (PR #30)

Three new sub-fixtures under `examples/languages/cpp/`:

- **virtual-dispatch/** — null-result probe: 627 branches, 107
  Decisions, but **zero** attributed to the Shape hierarchy.
  Doctrinal proof: `call_indirect` (vtable dispatch) is NOT an
  MC/DC Decision.
- **eh-table/** — br_table audit on a 4-arm switch (parse_token)
  + vfprintf's 57-arm format-specifier dispatch. v0.9.7's
  br_table clustering pass exercised on real-world tables.
  Found wasi-sdk 33 ships libcxx without `__cxa_throw` shims;
  build.sh has graceful `-fno-exceptions` fallback.
- **stl-shortcircuit/** — exposes the optimisation-vs-DWARF
  trade-off: at -O0, 26 of 105 inline chains hit depth 2 in
  printf_core; at -O1 the wasm-ld DWARF gap kicks in (0
  decisions). Real upstream limitation, not witness-side.

### Added — Swift promoted to Tier A (PR #32)

`examples/languages/swift/leap-year/` was Tier C (blocked on
swiftmodule version mismatch between Apple swift-6.3.2 and
SwiftWasm built against swift-6.3-RELEASE). Unblock 2026-05-16
via `swiftly install 6.3.0` (no sudo, ~/.swiftly install).

Result: **4,915 Decisions** — biggest single fixture yet.
Swift's standard library adds enormous decision surface
(Sequence, Optional, String, runtime metadata). The predicate
itself is visible by Swift's mangled name
`$s4leap0A4YearySbs6UInt32VF`, with `chain_kind = or / and /
mixed` all detected. Same wasm-ld cross-CU attribution caveat
as the wasi-sdk C/C++ fixtures.

Source change beyond the README: `leap.swift` uses
`nonisolated(unsafe) var yearInput: UInt32` instead of a
reference-typed holder — Swift 6 strict-concurrency rejects
unannotated global mutable state.

### Added — rivet-driven verification gate (PR #31)

Mirrors the [spar verification-gate pattern](https://github.com/pulseengine/spar/commit/ba329f3d44da4098f462f272fef17b6540f02a13).
Makes verification artifacts EXECUTABLE rather than purely
descriptive. New CI job iterates `type: test-case` artifacts,
runs each one's `fields.steps[].run` commands, and upserts a
sticky PR comment with pass/fail counts.

What landed:

- **`artifacts/verification.yaml`** — 9 `test-case` artifacts
  covering unit tests per crate (TEST-CORE / TEST-CLI /
  TEST-CHECKER), static analysis (TEST-CLIPPY / TEST-FORMAT),
  security (TEST-DENY — aligned with ci.yml's `cargo deny
  check bans licenses sources` invocation; advisories skipped
  while smithy ships cargo-audit < 0.22.1), end-to-end
  (TEST-VERDICT-SUITE, TEST-CROSS-LANG-RUST), schemas
  (TEST-SCHEMAS).

- **`schemas/witness-verification.yaml`** — defines the
  `test-case` artifact type with proper `method` (enum) +
  `steps` fields. Resolves the `rivet validate` INFO warnings
  that would have appeared if we'd overloaded the built-in
  `feature` type as spar does.

- **`tools/run_verification.py`** — pure stdlib Python that
  iterates matching test-cases, runs each step's `run` under
  bash, writes `verification-results.json`. Captures last 2 KB
  of stdout/stderr on failure so CI logs surface what
  actually broke (not just `failed: <cmd>`).

- **`tools/post_verification_comment.py`** — upserts a single
  marker-tagged PR comment from the results JSON. Pure stdlib
  urllib, no `gh` CLI dependency.

- **`.github/workflows/verification-gate.yml`** — 45-minute
  timeout, cached-rivet guard (skips ~6 min `cargo install`
  on warm runs), env-var-bound untrusted inputs (no `${{ ...
  }}` in `run:`).

Run locally:

```sh
tools/run_verification.py
tools/run_verification.py --filter '(and (= type "test-case") (has-tag "unit-tests"))'
```

Per-PR scope via PR body: `Verify-Filter: <sexp>`.

### Adjacent fix

- **`clippy.toml`** MSRV 1.91 → 1.92 — the v0.19 MSRV bump
  missed this file. Surfaced by the gate's TEST-CLIPPY step
  (with `-D warnings`, the mismatch escalates to a failure).

### No code changes

Pure additions under `examples/`, `tools/`, `schemas/`,
`artifacts/`, `.github/workflows/` + doc updates. No
witness-core / instrument / decisions changes.

## [0.21.0] — 2026-05-15

Cross-language sweep continued. Adds three more probes covering
C++ (success), Swift (toolchain-version blocked), and
Kotlin/Wasm (tool-side blocked on wasm-gc). The two blocked
fixtures still ship so future work can pick up from the
documented state without re-discovering the gaps.

### Added — fixtures

- **`examples/languages/cpp/leap-year/`** (Tier A) — wasi-sdk
  clang++ + `wasm32-wasip1 -std=c++20 -O0`. 79 Decisions
  (libc dominates) + 92 inline chains. **C++ specific signal**:
  template instantiation visible in `function_name` field:
  `bool leap_year<unsigned int>(unsigned int)`. The 2 br_ifs
  of the template instantiation cluster into a Decision with
  `chain_kind = or` detected. Same wasm-ld cross-CU
  attribution caveat as the C wasi-sdk fixture applies.

- **`examples/languages/swift/leap-year/`** (Tier C, blocked) —
  SwiftWasm 6.3-RELEASE SDK was built against `apple/swift
  6.3-RELEASE`; macOS ships swift-6.3.2 which is patch-level
  ahead. Swiftmodule binary format is patch-sensitive, so the
  SDK's Swift stdlib refuses to load. Fixture source is
  well-formed; unblock requires either a newer SwiftWasm
  release or installing the matching apple/swift snapshot
  toolchain. No witness-side change needed.

- **`examples/languages/kotlin/leap-year/`** (Tier D, blocked) —
  Kotlin Multiplatform 2.2 `wasmJs()` target builds cleanly
  via Gradle, but witness can't parse the output:
  `Error: gc proposal not supported (at offset 0x10)`.
  Walrus 0.24 (witness's wasm-rewriter) rejects the wasm-gc
  proposal Kotlin emits. Independently, Kotlin uses
  `.wasm.map` source maps rather than DWARF — even with
  walrus GC support, source attribution would need a new
  source-map ingestion path in witness.

### Cross-language docs

`docs/cross-language.md` updated:
- Tier A: 6 entries (Rust, C, C wasi-sdk, Zig, TinyGo, **C++**)
- Tier B: 2 entries (C `-O1`+, C wasi-sdk `-O1`+ — both
  blocked on the upstream wasm-ld DWARF gap)
- Tier C: 1 entry (Swift — toolchain mismatch, recoverable)
- Tier D: 3 entries (standard `go build`, AssemblyScript,
  **Kotlin/Wasm**)

### Findings

- **C++ adds template signal on top of C** — same structural
  decision count, but `function_name` carries the demangled
  template instantiation. Important for compliance reviewers
  doing per-monomorphisation audits.
- **wasm-gc is a real boundary for witness** — current walrus
  cannot parse modules using GC reference types. Until
  walrus ships GC support, Kotlin/Wasm + Dart-wasm + any
  future wasm-gc language stays in Tier D.
- **Source maps are a separate gap from DWARF** — Kotlin's
  decision to ship `.wasm.map` instead of `.debug_line`
  means even a hypothetical walrus-GC-aware witness still
  wouldn't have source attribution without a source-map
  ingestion design.

### No code changes

Pure additions under `examples/languages/` + doc updates.
Same shape as v0.20.0 — version bump signals the cross-
language matrix expanded.

## [0.20.0] — 2026-05-14

Cross-language sweep. Adds three new Tier A fixtures exercising
v0.19's IfThen+BrIf clustering against LLVM-frontend toolchains
beyond Rust/clang-unknown-unknown, plus one new Tier B fixture
documenting the upstream wasm-ld DWARF gap at `-O1`+.

### Added — fixtures

- **`examples/languages/zig/leap-year/`** — Zig 0.16 +
  `wasm32-freestanding -OReleaseSafe`. 1 Decision on
  `leap.zig:17` with `chain_kind = or` detected. Surprise:
  Zig lowers `or` to br_if chains (rustc-style), not clang's
  `if/else` shape. v0.19's IfThen clustering isn't load-
  bearing here; the existing BrIf clustering catches the
  pattern.

- **`examples/languages/go/leap-year/`** — TinyGo 0.41 +
  `wasm-unknown -opt 1`. 4 Decisions total: 2 in `leap.go:28`
  (TinyGo inlined `leapYear` into both call sites — exactly
  the multi-context scenario v0.14's chain tracker was built
  for), 2 in TinyGo's `float.go` runtime primitives.
  `chain_kind = or` detected, 2 inline chains populated.
  Strongest non-Rust DWARF probe.

- **`examples/languages/c/leap-year-wasi/`** — wasi-sdk 33 +
  `wasm32-wasip1 -O0`. **79 Decisions + 92 inline chains**
  across full libc (`vfprintf`, `fwrite`, `memchr`,
  `__stdio_exit`, `wcrtomb`, …) — the biggest witness
  coverage demo to date on non-Rust code. Source attribution
  is partly cross-contaminated by wasm-ld's missing DWARF
  address relocations (multi-CU line programs collapse to a
  flat address space); decision clusters themselves are
  structurally correct.

### Cross-language docs

`docs/cross-language.md` updated: three new Tier A entries
(Zig, Go-via-TinyGo, wasi-sdk-C-at-O0); one new Tier B entry
(wasi-sdk-C-at-O1 documenting the same upstream wasm-ld gap as
`wasm32-unknown-unknown`); Tier C trimmed by removing the
entries that just moved up.

### Findings — what we learned about each toolchain

- **Rust + Zig** — both lower `||` and `or` to br_if chains.
  IfThen clustering is structurally additive but not
  triggered by these.
- **clang + TinyGo** — both use LLVM frontends that lower
  `&&`/`||` to `if/else` blocks. IfThen clustering is
  load-bearing on these.
- **wasi-sdk DWARF is the best non-Rust toolchain** — line
  program survives linking, inlined-subroutine entries get
  populated, address relocations apply at `-O0`. At `-O1`+
  the LLVM optimiser folds the line program past wasm-ld's
  ability to re-link DWARF addresses — same upstream gap as
  `wasm32-unknown-unknown`.
- **TinyGo's inline-chain tracking is fully cross-language**
  — v0.14's chain implementation isn't Rust-specific.

### No code changes

Pure additions under `examples/languages/` + doc updates.
Version bump signals the cross-language story expanded; no
behaviour changes in the witness CLI, witness-core decision
reconstruction, or any schemas.

## [0.19.0] — 2026-05-13

Extends decision clustering to recognise clang/LLVM-frontend
short-circuit lowering shapes. v0.18.0's C probe revealed that
clang emits `if/else` + 1 `br_if` per `&&`/`||` source decision
rather than rustc's `br_if` chain, so the v0.17 clustering rule
(BrIf-only) couldn't form Decisions for clang-shaped wasm.

### Changed — `decisions.rs::group_into_decisions`

`IfThen` entries now cluster alongside `BrIf` for decision-key
purposes. The IfThen arm is the "predicate was true" outcome,
semantically equivalent to a `BrIf` for chain-shape decision
reconstruction. `IfElse` stays excluded — it's the negation of
the same site within a single source decision, so counting it
would inflate the condition count and double-bill the same
predicate. `BrTableTarget` / `BrTableDefault` clustering is
unchanged.

The cluster threshold (`cluster.len() >= 2`) holds. A lone
IfThen with no companion still drops out, mirroring the
pre-v0.19 behaviour for singleton BrIfs.

Two new regression tests in `crates/witness-core/src/decisions.rs`
pin this — `group_into_decisions_clusters_if_then_with_br_if`
(IfThen + BrIf on the same line → 1 Decision with IfElse
excluded from `conditions`) and `group_into_decisions_drops_lone_if_then`
(singleton IfThen → 0 Decisions). 15 / 15 decisions tests pass.

### Cross-language probe — C leap-year, the upstream gap

Updated `examples/languages/c/leap-year/README.md` with the
full diagnosis. v0.19 unblocks the clustering rule, but the
clang `-O1` + wasm-ld build path on the `wasm32-unknown-unknown`
target still hits an upstream wall: `wasm-ld` emits an empty
`.debug_line` program (40 bytes total, prologue-only, zero
rows) when the predicate function is force-inlined or when
target-specific DWARF relocation isn't applied. The fixture
verifies cleanly at `-O0` (1 Decision reconstructed), confirms
the v0.19 clustering rule, and documents wasi-sdk +
`wasm32-wasi` as the workaround for the line-program gap.

### Why ship v0.19 even when the C probe still hits the
upstream wall

The clustering change is load-bearing for every clang/LLVM-
frontend probe coming next — Zig, TinyGo, Swift, Kotlin/Wasm,
MoonBit. Once their builds clear wasm-ld's DWARF gap (most via
wasi-sdk by default), the cross-language matrix can extend
without further clustering work in witness.

### MSRV bump — 1.91 → 1.92

Picks up the rustc floor that wasmtime 44.0.1 + cranelift
0.131.x require (introduced in the v0.13 security bump for
RUSTSEC-2026-0114). The CI MSRV job was failing on main from
that bump forward; this commit aligns `rust-version` in both
Cargo.toml files and the `ci.yml` MSRV job to 1.92.0.

## [0.18.0] — 2026-05-12

Documents witness's cross-language story honestly. Ships the
first non-Rust probe (a C leap-year fixture via clang +
wasm-ld), `docs/cross-language.md` as the long-running language
matrix, and README framing of witness's position vs the
existing OSS MC/DC tooling.

### Reframing the OSS landscape

Earlier README framing implied OSS MC/DC tooling for non-Rust
languages didn't exist. That was wrong. The actual landscape:

- **GCC 14 (2024)** ships `-fcondition-coverage` covering
  C/C++/D/Rust at the source-level (frontend) chain layer.
- **[Coveron](https://coveron.github.io/)** is an OSS C/C++
  MC/DC tool aimed at automotive/aviation.
- **[linux-mcdc](https://github.com/xlab-uiuc/linux-mcdc)** is
  a 2025-published Linux-kernel MC/DC tool (DASC 2025 best
  paper) layered on GCC's `-fcondition-coverage`.
- **[GNATcoverage](https://github.com/AdaCore/gnatcoverage)**
  covers Ada and C at source + object level.

These all measure pre-codegen at the **source layer**. Witness
measures **post-codegen at the wasm bytecode layer** — same
DO-178C "post-preprocessor C" precedent the variant-pruning
blog post leans on. Different chain layer, additive evidence.

### Added — `examples/languages/c/leap-year/`

First C-language probe. `leap.c` compiles the canonical
leap-year predicate via `clang --target=wasm32-unknown-unknown
-g -O1` with `wasm-ld` linkage. Outcomes verified against
witness v0.17:

- ✅ Wasm produced with `.debug_info` sections.
- ✅ Witness instruments cleanly (1264 → 2178 bytes).
- ✅ DWARF byte-offset → source-line attribution resolves.
- ✅ 12 branches captured (4 funcs × 3 each).
- ❌ **0 decisions reconstructed** because clang's `&&` / `||`
  lowering emits `if/else` + 1 br_if per function instead of
  the br_if chain rustc produces. `decisions.rs::group_into_decisions`
  only clusters `BrIf`s today; `IfThen` / `IfElse` entries
  are counted but never form decisions.

The `examples/languages/c/leap-year/README.md` documents the
limitation, the two design choices that could fix it, and
witness's already-better-than-source claim even at the partial
v0.17 level (post-LLVM transformations like inlining are
visible in witness's branch view; source-level tools can't
see them by definition).

### Added — `docs/cross-language.md`

Long-running language matrix. Four tiers:

- **A — Verified**: Rust (full witness pipeline, 12 fixtures).
- **B — Partial**: C (as documented above).
- **C — Should work, untested**: C++ / Zig / Swift / TinyGo /
  Kotlin/Wasm. Each entry documents the toolchain, expected
  test cases (templates / comptime / optionals / channels /
  sealed classes), and the open probe question.
- **D — Won't work without compiler changes**: Go (no DWARF in
  standard go build), AssemblyScript (sourcemaps only),
  MoonBit (DWARF emission status unknown in current
  toolchain).

Plus a probe recipe for new languages: compile → confirm
DWARF → instrument → check `(branches, decisions,
branch_inline_contexts)` counts in the manifest. Distinguishes
"instrumentation works, clustering doesn't" (file an issue)
from "compiler isn't emitting DWARF" (different problem).

### README — Cross-language reach section

New section under "Cross-language reach" before "Where it
fits". Cites the four existing OSS tools (GCC 14, Coveron,
linux-mcdc, GNATcoverage) with hyperlinks; positions witness
as the post-codegen layer. References `docs/cross-language.md`
for the matrix.

### Notes for v0.19+

The C-fixture finding tracks the next planned increment:
extend `decisions.rs::group_into_decisions` to cluster
`IfThen` entries alongside `BrIf` when they share a
`(function_index, source_file)` key. Small code change;
unlocks C / C++ / Zig / TinyGo / Swift / Kotlin in one go.
Tracked as v0.19+.

## [0.17.0] — 2026-05-12

DW_AT_ranges scattered-inline support — closes the
v0.14.0-noted limitation where `DW_TAG_inlined_subroutine`
entries using `DW_AT_ranges` (multi-range scattered code) were
silently skipped by the DIE walker. The walker now resolves
both range forms uniformly and emits one `InlineEntry` per
range.

### Why this matters

LLVM emits `DW_AT_ranges` (rather than `DW_AT_low_pc` +
`DW_AT_high_pc`) when an inlined call's emitted code is not
contiguous. Three real-world shapes that trigger it:

- **Tail-merged code** — LLVM merges identical exit sequences
  across two call sites of the same inlined function.
- **Hot/cold splitting** — panic-bearing paths get pushed to
  cold sections. Common in stdlib paths.
- **Cross-crate inlining with LTO** — distinct
  monomorphisations merge after monomorphisation.

Pre-v0.17.0, witness silently produced no inline-context tags
for any branch landing inside such an inlined frame. v0.17.0
captures them.

### Refactor — walker now uses `gimli::UnitRef::die_ranges`

`crates/witness-core/src/decisions.rs::build_inline_map` was
parsing `DW_AT_low_pc` + `DW_AT_high_pc` attributes manually
to derive a single `(low_pc, high_pc)` interval per DIE. v0.17
delegates to `gimli`'s `UnitRef::die_ranges(entry)` which
handles both `DW_AT_low_pc + DW_AT_high_pc` AND `DW_AT_ranges`
forms uniformly, yielding a `RangeIter` over one-or-more
intervals. Each interval becomes its own `InlineEntry` sharing
the same chain (parents-on-stack at this DIE's depth + self).

Net change: contiguous-form inlines (the common case) produce
exactly one `InlineEntry` per DIE as before — bit-for-bit
identical output on the verdict suite. Scattered-form inlines
now produce N `InlineEntry`s (one per range) where v0.14-v0.16
produced zero.

### Verified

- Workspace tests pass (69 mcdc_report + others; 1 new test).
- Verdict suite at v0.17.0 vs v0.16.0 baseline: byte-identical
  (21/177 full-MC/DC, httparse 7/86, all counts match).
- No regression in `branch_inline_contexts` /
  `branch_inline_chains` counts on httparse (106 entries
  unchanged) — the corpus's inline DIEs all use contiguous
  form.

### Notes for v0.17.x and v0.18

- Synthetic `gimli::write` test for the DW_AT_ranges path —
  the implementation is gimli-API-delegated and the existing
  walker tests cover the contiguous form; targeted ranges-form
  tests are nice-to-have but not blocking. `docs/research/
  dw-at-ranges-test-cases.md` has the full sketch.
- Real-world LTO fixture for end-to-end ranges-form signal —
  could ship as `verdicts/lto_split/` when needed.
- macOS Developer ID signing + notarisation — waiting on user
  cert plumbing.

## [0.16.0] — 2026-05-12

Refines the per-context drill-down bucketing rule. Previously
buckets keyed only on the leaf inline_context — two rows with
the same leaf but different parent paths through the inline
hierarchy would collapse into one bucket and conceal the
distinction. v0.16.0 keys on the *full chain* (v0.14+ rows) or
falls back to the leaf (v0.13 rows). v0.13-only fields stay
bit-for-bit compatible.

### Changed — `derive_per_context` bucketing

`crates/witness-core/src/mcdc_report.rs::derive_per_context`
now buckets `DecisionRow`s by `inline_chain` (Vec) when present,
falling back to `[inline_context]` when chain is absent. Bucket
key is `Vec<InlineContext>` uniformly. Rows whose chains differ
in any frame land in distinct buckets even when their leaves
match — the case `is_safe()` inlined via two distinct wrapper
paths within the same function.

`PerContextVerdict.inline_context` continues to carry the leaf
(`chain.last()` semantics). `PerContextVerdict.inline_chain`
ships the full chain when chain length > 1; stays `None` for
single-frame buckets (legacy v0.13 leaf-only rows) so v3
envelopes from v0.13-vintage records stay byte-clean.

### Why this is a correctness fix, not a feature

v0.13's leaf-bucketing assumed rows with the same leaf
inline_context arrived through identical inline hierarchies.
For predicates inlined into a function from multiple distinct
parent inlines (the design's headline use case), rows DO share
the leaf but DIFFER in the chain. v0.13 grouped them; v0.16
splits them — same data, more honest grouping.

The verdict-suite `multi_context` fixture documented in v0.14.2
still doesn't populate per_context with > 1 bucket: that
fixture's stdlib-anchored br_ifs end up in `memchr` /
`<[u8]>::contains` function bodies (not in the user's wrapper),
so the chain captures stdlib's internal frames but not the
wrapper's call site. Resolving that fixture's empty per_context
needs runner-side multi-context row tagging (`Vec<InlineContext>`
per row) or a different fixture shape (dispatcher pattern with
inlined wrappers + invocation count emitting per-iteration
rows). Both are v0.16.x / v0.17 work.

### Tests

- `mcdc_report::tests::per_context_buckets_by_full_chain_not_just_leaf`
  — synthesises 5 rows with identical leaf inline_context but
  two distinct chains (`outer_a` vs `outer_b`). Asserts
  `per_context.len() == 2`; both buckets share the leaf; each
  bucket's `inline_chain.first()` matches its respective outer
  frame.

### Notes for v0.16.x / v0.17

- DW_AT_ranges scattered-inline support — implementation
  plan at `docs/research/dw-at-ranges-test-cases.md`; ~430 LoC.
- Runner-side multi-context row tagging (`Vec<InlineContext>`)
  for rows whose conditions span multiple call sites.
- macOS Developer ID signing + notarisation — waiting on user
  cert plumbing.

## [0.15.1] — 2026-05-12

Adds a Rekor-transparency-log binding for every predicate
envelope in the compliance evidence bundle. Each
`predicate.json` is now also signed with keyless cosign via the
release workflow's OIDC identity and the signing event is
logged to Sigstore's public Rekor instance. The Ed25519 DSSE
signature on `signed.dsse.json` stays — Rekor adds a second,
independent binding-proof layer rather than replacing it.

### Added — Rekor bolt-on in `release.yml::compliance`

`compliance` job in `.github/workflows/release.yml` now:

1. Runs the existing compliance Action to build the
   `compliance/` evidence directory (unchanged).
2. Loops over `compliance/verdict-evidence/*/predicate.json`,
   running keyless `cosign sign-blob` on each with
   `COSIGN_EXPERIMENTAL=1`. cosign uploads the signature event
   to Rekor automatically; the local `.cosign.sig` +
   `.cosign.cert` outputs land next to the predicate.
3. Re-archives the bundle so the uploaded artifact carries the
   new files. Downstream `cosign verify-blob` can re-verify
   each predicate's signature + Rekor inclusion proof offline
   from the bundle, OR query Rekor live by certificate
   identity.

### Documentation — SECURITY.md

New subsection "cosign verify-blob for predicate envelopes
(v0.15.1+)" under "Verifying a witness release" with the
exact verification command + a paragraph explaining that
Ed25519 DSSE and cosign are *independent* signatures of
overlapping evidence — defence in depth, not redundancy.

### Why this matters

Pre-v0.15.1, predicate envelopes were Ed25519-signed with an
ephemeral key generated per release. Verifying required
trusting the public key shipped alongside the bundle. v0.15.1
adds a public transparency-log proof: any third party can
query Rekor to confirm a predicate existed at release time
under this workflow's identity, without trusting any key
witness ships. For DO-178C / ISO 26262 evidence chains where
non-repudiation matters, Rekor inclusion is the
externally-verifiable anchor.

### Notes for v0.15.x and v0.16

- DW_AT_ranges scattered-inline support — see
  `docs/research/dw-at-ranges-test-cases.md`.
- macOS Developer ID signing + notarisation — waiting on user
  cert plumbing.

## [0.15.0] — 2026-05-11

Flips the `--mcdc-schema` default from `v2` to `v3`. Soak
validated: v0.14.0 + v0.14.1 + v0.14.2 v2-mode output remained
byte-identical to v0.13.1 baseline (21/177 full-MC/DC, httparse
7/86 across all releases); v3-mode adds the per-row inline
call chain (up to 8 levels deep on httparse) without regression.

### Changed — `--mcdc-schema` default v2 → v3

- `McdcSchemaVersion::default()` now returns `V3` (was `V2`).
- `witness predicate --kind mcdc` without an explicit
  `--mcdc-schema` flag now emits v3 envelopes by default.
  The wrapper's `predicateType` AND the embedded
  `predicate.report.schema` both ship the v3 URL.
- Pass `--mcdc-schema v2` to lock to the v2 shape, or `v1` to
  fall back to the pre-v0.13 single-Decision-per-cluster shape.

### Tests

- `predicate::tests::mcdc_predicate_round_trips_truth_tables_and_gaps`
  and `attest::tests::mcdc_predicate_sign_then_verify_round_trip`
  now assert against `MCDC_PREDICATE_TYPE_V3` (was
  `MCDC_PREDICATE_TYPE_V2`), matching the new default.

### Notes for v0.15.x and v0.16

- `DW_AT_ranges` scattered-inline support — currently skipped
  silently when DWARF reports inlined-subroutine address ranges
  via `DW_AT_ranges` instead of `DW_AT_low_pc + DW_AT_high_pc`.
  See `docs/research/dw-at-ranges-test-cases.md` for the test-
  case design analysis.
- Rekor binding for predicate envelopes — v0.15.1.
- macOS Developer ID signing + notarisation — waiting on user
  cert plumbing.

## [0.14.2] — 2026-05-11

Adds the first verdict-suite fixture written specifically to
exercise v0.13+ inline-context tagging and v0.14's chain
drill-down. Also picks up two README touch-ups: the
variant-pruning blog post is no longer "(draft)" — it's live at
pulseengine.eu — and a new Related Work subsection cites the
arXiv 2604.22673 input-side equivalence-class inference work as
the upstream complement to witness's downstream MC/DC
measurement.

### Added — `verdicts/multi_context/`

13th verdict-suite fixture. Predicate `is_valid(s: &[u8])` uses
stdlib slice helpers (`is_empty`, `contains`, `first`) so
rustc/LLVM reliably emit `DW_TAG_inlined_subroutine` DIEs across
stdlib + the user's two wrappers `check_first` / `check_second`.
Eight exported `run_row_*` no-arg functions split between the
two wrappers (even rows go via `check_first`, odd via
`check_second`).

End-to-end at v0.14:
- Manifest carries **16 entries** in `branch_inline_contexts`
  and `branch_inline_chains`.
- The decision at `memchr.rs:40` (stdlib `slice::contains`)
  carries inline chains **4 frames deep**:
  `[run-call-to-wrapper, wrapper-call-to-is_valid,
  is_valid-call-to-contains, contains-call-to-memchr]`.
- v3 mcdc envelope's `RowView.inline_chain` populates for every
  row in that decision.

### Known limitation documented (`verdicts/multi_context/TRUTH-TABLE.md`)

The fixture demonstrates v0.14 chain extraction, **not**
`per_context.len() == 2`. The runner's trace parser emits one
`DecisionRow` per function-return boundary; within a single
`run_row_*` invocation no boundary separates the wrapper from
the dispatcher (both inlined), so the row's modal inline-context
tag aggregates across the invocation. Multiple invocations each
carry a single context tag, but rows-with-the-same-context
group into one bucket → `per_context.len() == 1`.

Producing two buckets would require row-per-iteration trace
records firing inside a loop that alternates between wrappers,
or runner-side support for multi-context row tags
(`Vec<InlineContext>` rather than `Option<InlineContext>`).
Both are deferred to v0.14.x or v0.15.

### Documentation — README

- The blog-post bullet at the top of the file no longer says
  "(draft)" — it's now a live link to
  `https://pulseengine.eu/blog/variant-pruning-rust-mcdc/`.
- New subsection "Upstream — equivalence-class inference on
  legacy binaries" under "Related work" cites
  [arXiv:2604.22673](https://arxiv.org/abs/2604.22673) (De Luca,
  De Angelis, Amalfitano, Cimmino) as the input-side complement
  to witness's structural-coverage measurement. The citation is
  bibliographic; no pipeline integration.

## [0.14.1] — 2026-05-10

Closes the v0.14.0 deferral on `PerContextVerdict.inline_chain`
and adds a focused chain-propagation test.

### Added — `PerContextVerdict.inline_chain`

`Vec<InlineContext>` field on the per-context drill-down bucket
in mcdc-v3 envelopes. The bucket's `inline_context` (single-hop
leaf) is preserved as before; the new chain field tells reviewers
the *full* call path the bucket represents — useful when the same
leaf call site is reached from multiple parent contexts.

Stripped under v1 / v2 schemas via `from_record_with_schema`.
Pre-v0.14 envelopes deserialise unchanged via `#[serde(default)]`.

### Schema — v3 extended (additive)

`docs/schemas/witness-mcdc-v3.json::PerContextVerdict` gains
the optional `inline_chain` property. v3 envelopes from v0.14.0
without per-context entries (the verdict suite case) still
validate; v0.14.1 v3 envelopes WITH per-context entries also
validate cleanly.

### Tests

- `mcdc_report::tests::v3_chain_propagates_from_decision_row_to_row_view_and_per_context`
  — synthesises a Decision with two distinct chain tags across
  five rows; asserts (a) v3 schema populates chain on RowView
  and PerContextVerdict, (b) v2 schema strips both.

## [0.14.0] — 2026-05-10

DWARF inline-context CHAIN tracking — extends v0.13's single-hop
`InlineContext` to a full `Vec<InlineContext>` from outermost to
innermost frame. Real-world signal: httparse runs through inlined
chains up to **8 levels deep**; v0.13.x only saw the leaf.

### Added — chain tracking in the DIE walker

`crates/witness-core/src/decisions.rs::build_inline_map` now
maintains a depth-keyed parent stack across the DIE depth-first
walk. When the cursor enters a `DW_TAG_inlined_subroutine` DIE,
the entry's `(call_file, call_line)` plus all currently-active
parent inlined frames form the chain stored against the entry's
address range. When the cursor ascends past a parent's depth,
the parent is popped.

`InlineEntry` gains `chain: Vec<InlineContext>` (outermost →
innermost, inclusive of the entry's own context at the tail).
A new `lookup_inline_with_chain` helper returns both the leaf
and the full chain in one query.

### Added — `Manifest.branch_inline_chains`

`BTreeMap<u32, Vec<InlineContext>>` mirroring
`branch_inline_contexts` (same keyset; for any branch present
in both maps, `chain.last()` equals the leaf context). Pre-v0.14
manifests deserialise unchanged via `#[serde(default)]`.

### Added — `DecisionRow.inline_chain` + `RowView.inline_chain`

Both runners (embedded + harness) populate `DecisionRow.inline_chain`
via a new `row_modal_chain` helper that picks the modal *whole
chain* across the row's evaluated condition branches. Same
tie-resolves-to-`None` rule as `row_modal_context`. The reporter
copies `inline_chain` through into `RowView.inline_chain` for v3
envelopes; v1/v2 strip it via `from_record_with_schema`.

### Added — mcdc-v3 schema

New schema URL `https://pulseengine.eu/witness-mcdc/v3` at
`docs/schemas/witness-mcdc-v3.json`. Structurally a superset of
v2 with the `inline_chain` field on `RowView`.

In `mcdc_report`: `MCDC_SCHEMA_URL_V3` const +
`McdcSchemaVersion::V3` variant. In `predicate.rs`:
`MCDC_PREDICATE_TYPE_V3` const; the `build_mcdc_statement_*`
paths derive the in-toto `predicateType` from the report's
schema URL so v3 envelopes ship a v3 predicateType in BOTH
the wrapper and the embedded report.

### Added — `witness predicate --mcdc-schema v3`

CLI flag accepts `v3` alongside `v1` / `v2`. **Default stays v2**
for v0.14.0 (matches v0.13.1 default; v0.14.x will flip to v3
once the chain shape stabilises). Pass `--mcdc-schema v3` to
opt in.

### Verified — verdict suite delta

- v0.14.0 v2-mode output **byte-identical** to v0.13.1 v2-mode
  (verdict suite: 21/177 full-MC/DC, httparse 7/86; all conditions
  counts match). No regression.
- v0.14.0 v3-mode against the httparse fixture: 106 branches
  carry inline-tag entries (matches v0.13); **max chain depth
  observed is 8 frames**. v3 envelope validates against v3
  schema; v2 envelopes still validate against v2 schema.

### Notes for v0.14.x and v0.15

- v0.14.1 — extend the DIE walker to `DW_AT_ranges`-form inlined
  subroutines (multi-range scattered inlines). Currently skipped
  silently; not a blocker because the contiguous form covers the
  common case.
- v0.14.x or v0.15 — also surface chain on `PerContextVerdict`
  (per-bucket chain label so reviewers reading the drill-down see
  not just the leaf but the full path the bucket represents).
- v0.14.x — flip `--mcdc-schema` default v2 → v3 once the chain
  shape stabilises and downstream tooling consumes it.
- macOS Developer ID signing — waiting on user cert plumbing.
- Predicate Rekor-binding — deferred.

## [0.13.1] — 2026-05-10

Soak window for v0.13.0 closed. The verdict suite verified
v0.13.0's v1-mode output is byte-identical to v0.11.5
(21/177 full-MC/DC, httparse 7/86); v2-mode adds the per-row
`inline_context` tags and the `per_context` drill-down without
regression. v0.13.1 flips the `witness predicate
--mcdc-schema` default from `v1` to `v2`.

### Changed — `--mcdc-schema` default v1 → v2

- `McdcSchemaVersion::default()` now returns `V2` (was `V1`).
- `witness predicate --kind mcdc` (no explicit `--mcdc-schema`)
  now emits v2 envelopes by default. The wrapper's `predicateType`
  AND the embedded `predicate.report.schema` both ship the v2 URL.
- Existing consumers that schema-validate strictly against v1
  must pass `--mcdc-schema v1` to preserve byte-identical output.

### Tests

- `predicate::tests::mcdc_predicate_round_trips_truth_tables_and_gaps`
  and `attest::tests::mcdc_predicate_sign_then_verify_round_trip`
  now assert against `MCDC_PREDICATE_TYPE_V2` (was `MCDC_PREDICATE_TYPE`),
  matching the new default.

### Verified

- `cargo test --workspace`: all green (67 mcdc_report + others).
- Live sample at `/tmp/v0131-default.json` (httparse run): default
  emits v2 (`predicateType` and `report.schema` both
  `https://pulseengine.eu/witness-mcdc/v2`). Explicit
  `--mcdc-schema v1` still emits v1 envelopes byte-identical to
  the v0.13.0 v1-mode output.

### Notes for v0.14+

- v0.14 — `DW_AT_ranges` scattered-inline support + chain-depth
  tracking (row tag becomes `Vec<InlineFrame>`; mcdc-v3 schema).
- macOS Developer ID signing — waiting on user cert plumbing.
- Predicate Rekor-binding — deferred to v0.14+.

## [0.13.0] — 2026-05-10

Variant B — per-DWARF-inlined-context row tagging in mcdc-v2.
Successor to v0.12.0's failed Variant A. Verdict suite at v0.13.0
matches v0.11.5 baseline byte-for-byte (21/177 full-MC/DC,
httparse 7/86): no regression vs the pre-v0.12 shape, full
substrate in place for per-call-site drill-down via the new
mcdc-v2 schema.

### Why Variant B (and not another Variant A)

The v0.12.0 soak (2026-05-10) showed Variant A's premise was
wrong: real Rust→wasm code emits one br_if per inlined call
site. Splitting the Decision-key by inline context fragmented
v0.11's beneficial multi-condition clusters into singletons
that the existing `cluster.len() >= 2` gate dropped. Net loss
of 10 decisions, 7 conditions, 1 full-MC/DC verdict.

Variant B keeps v0.11's `(function, file)` keying — preserving
the unified single-Decision shape — and adds inline_context as
a **row-level tag** that the auditor filters by. The split-by-
context happens at the row-filter view at the reporter layer,
not at the Decision-key bucket layer. Cluster sizes are
preserved; nothing gets fragmented.

### Added — `Manifest.branch_inline_contexts`

`crates/witness-core/src/instrument.rs::Manifest` gains a
`BTreeMap<u32, InlineContext>` field that maps every br_if's
branch_id → the `(call_file, call_line)` of its enclosing
inlined-subroutine entry (when DWARF reports one). Populated
by `decisions::reconstruct_decisions` from the same DIE walk
v0.12.0 introduced; previously gated to an empty map in v0.12.1.

The map is the substrate the runner consumes to tag each
DecisionRow's executing context.

### Added — `DecisionRow.inline_context`

`crates/witness-core/src/run_record.rs::DecisionRow` gains an
optional `Option<InlineContext>` field. Populated by both the
embedded runner (`crates/witness/src/run.rs`) and the harness
runner via a `row_modal_context` helper that walks the row's
evaluated condition indices, looks up each branch_id in the
manifest's `branch_inline_contexts`, and stamps the modal
context (with ties resolving to `None` so reporters route the
row to the headline view). Pre-v0.13 records keep
deserialising via `#[serde(default)]`.

### Added — `DecisionVerdict.per_context` + `RowView.inline_context`

`crates/witness-core/src/mcdc_report.rs::DecisionVerdict` gains
a `Vec<PerContextVerdict>` (additive, `skip_serializing_if`
`Vec::is_empty`). For each distinct non-`None` inline_context
across the decision's rows that has at least 2 rows, the
analyse path runs the full MC/DC kernel against just that
context's rows. Reviewers can see "this Decision is FullMcdc
when inlined from `validate.rs:5` but Partial when inlined
from `audit.rs:10`" without the headline aggregating both into
a single Partial verdict.

`RowView.inline_context` ships per-row tags into the truth-
table view so reviewers reading individual rows can correlate
without cross-referencing.

### Added — mcdc-v2 schema

New schema URL `https://pulseengine.eu/witness-mcdc/v2` at
`docs/schemas/witness-mcdc-v2.json`. Structurally a superset
of v1 with the `PerContextVerdict` / `RowView.inline_context`
fields. Also new in `mcdc_report`:
`MCDC_SCHEMA_URL_V2`, `McdcSchemaVersion::{V1,V2}`,
`McdcReport::from_record_with_schema(record, version)`.

In `predicate.rs`: `MCDC_PREDICATE_TYPE_V2`. The
`build_mcdc_statement_*` paths derive the in-toto `predicateType`
from `report.schema` so v2 envelopes ship a v2 predicateType
in BOTH the wrapper and the embedded report.

### Added — `witness predicate --mcdc-schema {v1,v2}`

CLI flag selects the emitted schema. **Default stays v1 for
v0.13.0** (soak window — keep current Sigil consumers byte-
identical). v0.13.1 will flip the default to v2 once the
soak validates.

### Verified — verdict suite delta

Built witness at v0.13.0 + v0.11.5, ran the verdict suite at
both, diffed the SUMMARY.txts. v0.13.0 v1-mode output is
**byte-identical** to v0.11.5: 21/177 full-MC/DC, httparse
7/86, all conditions counts match. 106 branches in httparse
carry `branch_inline_contexts` entries; 22 Decisions get a
non-`None` `inline_context` headline label under v2 (matches
v0.12.0's count, but without the cluster-fragmentation regression).

`per_context` populates only when a Decision's rows span
multiple contexts. The verdict suite (12 fixtures, single-
invocation-per-row test corpus) doesn't have any Decision
hit by multiple contexts in one run, so per_context stays
empty there. Substrate ships ready for fixtures with intra-
export branching across inlined sites.

Synthetic v0.13.0 envelopes validate against the v2 schema;
synthetic v0.11.5 envelopes validate against v1 unchanged.

### Tests

- `decisions::tests::cluster_preserved_with_split_inline_contexts`
  — formerly v0.12.0's `splits_by_inline_context`: same input,
  flipped expectation. Now asserts unified Decision +
  populated `branch_inline_contexts` map.
- `decisions::tests::decision_inline_context_is_modal` — modal
  headline label when one context wins outright.
- `mcdc_report::tests::per_context_verdict_skipped_when_no_inline_tags`
- `mcdc_report::tests::per_context_verdict_skipped_when_all_rows_share_one_context`
- `mcdc_report::tests::per_context_verdict_splits_by_call_site`
  — load-bearing: 2 contexts → 2 per_context entries, ctx_a
  FullMcdc, ctx_b Partial.

### Notes for v0.13+

- v0.13.1 — flip the `--mcdc-schema` default from v1 to v2.
- v0.14 — extend the inline walker to `DW_AT_ranges` for
  scattered inlines + chain-depth tracking (`Vec<InlineContext>`
  on the row tag). Currently single-hop only.
- macOS Developer ID signing — waiting on user cert plumbing.
- Predicate Rekor-binding — deferred to v0.14+.

## [0.12.1] — 2026-05-10

Revert of v0.12.0's regressing keying change. The v0.12.0 soak
check on 2026-05-10 (verdict suite at v0.12.0 vs v0.11.5
baseline) showed:

- Total decisions: **21/177 → 20/167** (lost 10 decisions, lost
  1 full-MC/DC).
- httparse: **7/86 → 6/77** (the very fixture v0.12.0 was
  designed to lift; instead it lost 1 full-MC/DC and 4 proved
  conditions).
- parser_dispatch and base64_decode also regressed.
- 9 of 12 fixtures showed no change at all (the inline-context
  path didn't fire on them).

### Diagnosis

The Variant A design premise — that inlined-multi-br_if
predicates exist in real Rust→wasm output and benefit from
per-context Decision separation — was wrong. Macros and
inlined functions emit one branching site each. The split key
change `(function, file)` → `(function, file, inline_context)`
strictly fragmented v0.11's clusters. Where v0.11 grouped 2
br_ifs from distinct inline contexts into a useful 2-condition
Decision, v0.12.0 produced two singleton clusters that the
existing `cluster.len() >= 2` gate at `decisions.rs:303`
dropped. Net: both decisions and conditions lost.

### What v0.12.1 reverts

`crates/witness-core/src/decisions.rs::reconstruct_decisions`
now passes an empty `InlineMap` to `group_into_decisions`,
restoring v0.11's `(function, file)` keying behaviour.

### What v0.12.1 keeps (additive surface preserved)

The `InlineContext` type, the `inline_context: Option<InlineContext>`
fields on `Decision` / `DecisionRecord` / `DecisionVerdict`,
the mcdc-v1 schema's `InlineContext` `$def`, the
`build_inline_map` function, and the v0.12.0 tests all stay.
`build_inline_map` and helpers are marked `#[allow(dead_code)]`
in v0.12.1 with a comment that v0.13's Variant B reuses them.
Wire format is unchanged — v0.12.0 envelopes still validate,
consumers that read `inline_context` keep working (they just
never see a populated value in v0.12.1 output).

### Where Variant A goes

Variant A is shelved permanently. v0.13 ships Variant B
(per-context row tagging within unified Decisions, mcdc-v2
schema). Variant B preserves v0.11's beneficial clustering and
adds inline_context as a row-level tag the auditor can filter
by — the design that fits the actual corpus shape.

### Notes for v0.13+

Unchanged from v0.12.0. v0.13 = mcdc-v2 + Variant B (next).
macOS Developer ID signing waiting on user cert plumbing.
Predicate Rekor-binding deferred to v0.14+.

## [0.12.0] — 2026-05-03

Per-DWARF-inlined-context decision split — closes the v0.5
deferral first noted at `decisions.rs:35`. When a single source
predicate is inlined at multiple call sites within one wasm
function, witness now reconstructs one Decision *per call site*
instead of conflating rows from every site into one Decision.

The motivating shape: `is_safe()` inlined twice in `validate()`
at lines 5 and 10. Pre-v0.12 collapsed both call sites into one
Decision with rows from both — pair-finding then failed because
rows from distinct contexts got compared as if they were
alternatives within one chain. v0.12.0 splits them, so each
call site gets its own pair-finding scope and its own truth
table.

### Added — DWARF inlined-subroutine walking

`crates/witness-core/src/decisions.rs` gains `build_inline_map`
which walks each compilation unit's DIE tree, finds every
`DW_TAG_inlined_subroutine` entry, and records its
`(low_pc..high_pc, call_file, call_line)`. The map is then
queried per branch entry alongside the line map; together they
yield `(source_file, source_line, inline_context)` for every
br_if site.

Variant A semantics (per the design conversation): one level of
inlining only — the *innermost* enclosing inlined call site.
Deeply-nested inlines collapse to that one hop. Variant B
(full chain tracking via per-context row tagging) lands as
mcdc-v2 schema in v0.13.

Skipped silently:
- Inlined entries that use `DW_AT_ranges` (multi-range
  scattered inlines, less common). Rustc's typical wasm
  emission uses contiguous `low_pc + high_pc` ranges, so this
  covers the common case. v0.13 will pick up DW_AT_ranges as
  part of the chain walk.
- Modules without DWARF, or units that fail to parse — back-
  compat: empty inline_map → v0.11 keying behaviour.

### Added — `Decision.inline_context` (manifest)

`Decision` (in `instrument.rs`) gains `inline_context:
Option<InlineContext>` where `InlineContext { call_file,
call_line }` names the call site. Additive,
`#[serde(default)]` — pre-v0.12 manifests deserialise unchanged.

### Added — `DecisionRecord.inline_context` (run record)

`DecisionRecord` (in `run_record.rs`) gains the same field. The
runner propagates the manifest's `inline_context` into each
`DecisionRecord` so reporters can attribute split decisions to
their call sites without re-reading the manifest. Additive,
`#[serde(default)]`.

### Added — `DecisionVerdict.inline_context` (mcdc report)

`DecisionVerdict` (in `mcdc_report.rs`) gains the field for the
auditor view: reviewers see "this Decision is `is_safe`
inlined from `validate.rs:5`" vs "...from `validate.rs:10`",
disambiguating two Decisions that share `source_file` /
`source_line`.

### Schema — `witness-mcdc/v1` extended (additive)

`docs/schemas/witness-mcdc-v1.json` gains the `InlineContext`
`$def` and the optional `inline_context` property on
`DecisionVerdict`. v0.11.x envelopes still validate unchanged.
v0.12.0 envelopes with split decisions also validate.

### Tests

- `decisions::tests::group_into_decisions_splits_by_inline_context`
  — two pairs of br_ifs at the same source line but with
  different inline contexts produce two Decisions with distinct
  `call_line` values; each Decision contains exactly its own
  call site's br_ifs.
- `decisions::tests::group_into_decisions_keeps_single_when_no_inline_context`
  — back-compat negative control: empty inline map yields the
  same single conflated Decision the v0.11 path produced.

### Notes for v0.13+

- v0.13 — mcdc-v2 schema: per-context row tagging (Variant B).
  Recovers source-DRY view (one Decision per source location;
  rows tagged with their call chain). Includes DW_AT_ranges
  walk for scattered inlines and full chain depth.
- macOS Developer ID signing — waiting on user cert plumbing.
- Predicate Rekor-binding — deferred to v0.13+.

## [0.11.5] — 2026-05-03

`br_table` MC/DC with dual presentation. Closes the v0.11.4
notes item by shipping both halves: bytecode-level brval/brcnt
captures for `br_table` arms (so the discriminant integer is
recoverable per row), and a `BrTableAudit` block on every
`br_table`-shape decision that derives discriminant-bit
independent-effect proofs from the captured values.

The per-arm verdict (the headline reviewer view) is unchanged.
The new audit block sits next to it as a drill-down for DO-178C
objective 5.2 work that wants the textbook MC/DC math for
switch-shape decisions, not just per-arm coverage.

### Added — bytecode brval/brcnt for `br_table` arms

`crates/witness-core/src/instrument.rs` lifts the
`BrTableTarget | BrTableDefault => (None, None)` special case
at the global-allocation site to real allocations, and extends
`build_brtable_helper` to write per-arm `brval` (= arm index
when target arm fires; = actual discriminant value when default
arm fires) and increment per-arm `brcnt` inside the same
`if sel == i { ... }` blocks the existing counter-increment
already runs in.

For `BrTableDefault` the recorded `brval` is the load-bearing
capture: target arms imply `discriminant == arm_index`, but
default-arm rows could have any `discriminant ≥ N`. Without
the actual integer, the audit layer can't decompose
default-path discriminants into bits.

### Added — `DecisionRow.raw_brvals` (additive, serde-default)

`crates/witness-core/src/run_record.rs::DecisionRow` gains an
optional `raw_brvals: BTreeMap<u32, i32>` field (`#[serde(default,
skip_serializing_if = BTreeMap::is_empty)]`). The runner
populates it from the per-row brval globals; the audit layer
reads it. Pre-v0.11.5 run records keep deserialising — empty
map = audit layer no-ops.

### Added — `BrTableAudit` per-decision block

`crates/witness-core/src/mcdc_report.rs::DecisionVerdict` gains
`br_table_audit: Option<BrTableAudit>`. Populated on every
`br_table`-shape decision that has at least one row carrying
`raw_brvals`. Carries:

- `bit_width`: highest set bit across observed discriminants + 1
  (≤ 32; the wasm i32 ceiling).
- `bits[]`: per-bit verdict (`proved` / `gap` / `dead`) with the
  proving row pair when proved.
- `status`: aggregate (`proved` / `partial` / `gap` /
  `not_applicable`).

The pair criterion: row A and row B prove bit *i* when their
discriminants differ in bit *i* AND the firing arm differs.
Equivalent to MC/DC's "this condition independently affects the
outcome" applied to the discriminant-as-bit-vector.

### Schema — `witness-mcdc/v1` extended (additive)

`docs/schemas/witness-mcdc-v1.json` gains `BrTableAudit` and
`BrTableBitVerdict` `$defs`, plus the optional `br_table_audit`
property on `DecisionVerdict`. v0.11.4 envelopes still validate
unchanged — the new field is `additionalProperties: false`-
compliant and skipped when absent. Verified locally against a
synthesised v0.11.5 envelope.

### Tests

- `instrument::tests::br_table_arms_export_brval_and_brcnt_globals`
  — every br_table arm now exports its three globals.
- `run::tests::br_table_records_brval_and_brcnt_per_arm` —
  three-row drive of a 3-arm `br_table` produces correct per-arm
  hit counts.
- `mcdc_report::tests::br_table_audit_proves_each_observed_bit`
  — discriminants `{0, 1, 2, 7}` yield a `bit_width = 3`,
  `status = proved` audit with witness pairs for every bit.
- `mcdc_report::tests::br_table_audit_absent_when_pre_v0_11_5_run`
  — empty `raw_brvals` → `br_table_audit: None` (back-compat
  with v0.11.4 run records).

### Notes for v0.12+

Unchanged. Still deferred: per-DWARF-inlined-context decisions
(v0.12.0, Variant A — per-context decision split); mcdc-v2
schema with per-context row tagging (v0.13, Variant B); macOS
Developer ID signing; predicate Rekor-binding.

## [0.11.4] — 2026-05-03

Security patch — bumps wasmtime from 42 to 44 to close
RUSTSEC-2026-0114 ("Panic when allocating a table exceeding the
size of the host's address space"). Patched in wasmtime
`>= 43.0.2, < 44` and `>= 44.0.1`; v0.11.4 picks the latest
stable (44.x).

### Fixed — RUSTSEC-2026-0114

The Bytecode Alliance disclosed GHSA-p8xm-42r7-89xg on 2026-04-30,
affecting every wasmtime release from 30.0.0 through the
unpatched ranges. Witness uses wasmtime in two places:

- `witness run` (embedded mode) — instantiates user-provided
  modules, reads counter globals + trace memory.
- `witness-component` — the compliance-evidence consumer.

Both now build against wasmtime 44 with no API changes required —
`Engine::new`, `Module::from_binary`, `Linker`, `Store`, `Func`,
`Val`, and `ExternType::Func(_)` all unchanged across 42 → 44.

### Changed — `Measurement.toolchain.wasmtime_version` reports `"44"`

The compile-time wasmtime version constant in
`crates/witness-core/src/predicate.rs` (`WASMTIME_VERSION`) is
now `"44"`. v0.11.0..v0.11.3 envelopes that report `"42"` are
still valid — the field is provenance, not a correctness gate —
but reviewers should expect to see `"44"` on freshly-built v0.11.4
predicates.

### Notes for v0.11.x and v0.12

Unchanged. Deferred per the v0.11.4 planning conversation:

- v0.11.5 — `br_table` MC/DC variant. Designing a hybrid that
  surfaces both the textbook discriminant-bit math and the
  per-arm reviewer view side-by-side, instead of forcing a
  pick-one between rigour and readability.
- v0.12.0 — per-DWARF-inlined-context decisions (Variant A:
  per-context decision split). Bumps httparse 7/67 → much
  higher next week.
- v0.13 — mcdc-v2 schema with per-context row tagging
  (Variant B), with deprecation window.
- macOS Developer ID signing + notarisation — waiting on user
  cert plumbing.
- Predicate Rekor-binding — deferred to v0.13+.

## [0.11.3] — 2026-05-01

Closes the v0.11.0 deferral on `witness new --all-exports`
(proposal item 15) by shipping both halves: the alternate
"row-per-export" scaffold *and* the `witness run --invoke-all`
flag the scaffold's run.sh depends on.

### Added — `witness run --invoke-all` auto-discovery

Embedded-mode `witness run` gains a `--invoke-all` flag that
auto-invokes every no-arg, non-`__witness_*` export the module
exposes. Skips `_start`, `_initialize`, non-function exports
(memories, globals, tables), and any function whose signature
declares parameters (those need explicit `--invoke-with-args`
specs). Discovered exports are appended after explicit
`--invoke` / `--invoke-with-args` entries, in module-export
order, so the row sequence stays deterministic across reruns.

When `--invoke-all` is passed alone with nothing to discover,
witness now errors loudly rather than producing a zero-row run
record — same chatty-failure principle as the v0.11.0 Action
fixes.

Two unit tests cover the happy path (filtering keeps
`hit_then`/`hit_else`, drops the `(param i32)` export, never
leaks `__witness_*`) and the empty-discovery error.

### Added — `witness new --all-exports` scaffold

`witness new` accepts `--all-exports` to scaffold the row-per-
export fixture shape. The generated `src/lib.rs` exposes five
no-arg `run_row_0..4` exports, each calling the leap-year
predicate against a hardcoded year wrapped in
`core::hint::black_box`. The generated `run.sh` drives them via
`witness run --invoke-all` — no typed-args spec needed.

When to prefer `--all-exports` over the v0.9.11 default:
- One named test case per row (more obvious in CI logs).
- Integrating with a harness convention that calls every export
  it finds.

When to prefer the default `is_leap` shape:
- DWARF source attribution lands on the predicate's source line
  in `lib.rs` instead of `hint.rs:491` (typed-args lifts the
  input through a function parameter, no `black_box` needed).

### Verified

End-to-end smoke test of `witness new --all-exports leap-rows
&& cd leap-rows && ./build.sh && ./run.sh`: 1/1 decisions full
MC/DC, 2 conditions proved, 5 rows with stable export-name row
ids.

### Notes for v0.11.x and v0.12

Unchanged. Deferred: macOS Developer ID signing + notarisation;
predicate Rekor-binding (v0.12); differential testing against
rustc-mcdc; per-arm `brval`/`brcnt` for `br_table` decisions
(v0.11.4).

## [0.11.2] — 2026-05-01

Documentation patch: refreshes the published JSON Schemas at
`docs/schemas/witness-coverage-v1.json` and
`docs/schemas/witness-mcdc-v1.json` so they accept the v0.11.0
envelope shape. Pure docs change; no Rust API or wire-format
movement; v0.11.0 / v0.11.1 envelopes were already correct, the
schemas just rejected them.

### Fixed — schemas rejected v0.11 envelopes

v0.11.0 added `Measurement.toolchain` (rustc + wasmtime
provenance) and `Measurement.test_cases` (positional row→
invocation map) to the predicate body, plus
`McdcReport.interpretation_polarity` to MC/DC reports. The shipped
schemas had `additionalProperties: false` on `Measurement` and no
top-level polarity field, so any consumer running schema
validation against `https://pulseengine.eu/witness-coverage/v1` or
`https://pulseengine.eu/witness-mcdc/v1` rejected v0.11 envelopes
as invalid.

The CI `schemas` job (informational, `continue-on-error: true` —
see v0.10.0 item 5) caught this on every v0.11.x build; the
release was unblocked but the schemas didn't reflect what witness
actually emitted.

v0.11.2 adds:

- `Measurement.toolchain` (`#/$defs/Toolchain`) — `rust_version`
  and `wasmtime_version`, both optional best-effort lookups.
- `Measurement.test_cases` (array of `#/$defs/TestCase` with
  `row_id` + `invocation`) — positional row → invocation map.
- `McdcReport.interpretation_polarity` (top-level enum:
  `wasm-early-exit` | `source-equivalent`) — the polarity
  convention the truth-table cells use, defaulted to
  `wasm-early-exit` for v0.10.x reports that don't declare it.
- `TraceHealth.trace_parser_active` — current canonical name.
  `TraceHealth.ambiguous_rows` is kept as a deprecated alias
  (`"deprecated": true`) so v0.9.x reports still validate; this
  matches the runtime serde alias.

### Verified

Both schemas validate cleanly against:

- A synthetic v0.11.1 predicate (toolchain + test_cases populated)
  → accepted.
- A synthetic v0.10.4 predicate (no toolchain, no test_cases,
  `original_module: null`) → still accepted.

### Notes for v0.11.x and v0.12

Unchanged from v0.11.1. Still deferred: macOS Developer ID
signing + notarisation (waiting on user's cert plumbing);
predicate Rekor-binding (v0.12); differential testing against
rustc-mcdc (v0.11.x stretch); `witness new --all-exports`
auto-invoke (now scheduled for v0.11.3).

## [0.11.1] — 2026-05-01

### Fixed — `cargo fmt --check` failure on the v0.11.0 release

The v0.11.0 commit landed before `cargo fmt --all` ran; the new
`Verify` command's struct fields at `crates/witness/src/main.rs:617`
weren't formatted to rustfmt's preferred multi-line `[arg(...)] field`
shape. CI's Format job failed; Release ran clean (cosign signing
worked, every platform built).

Fix: ran `cargo fmt --all`. No semantic change.

The Schemas-informational job also failed — the v0.10-vintage
JSON-Schema validator caught the v0.11.0-shaped `Measurement` (now
carrying optional `toolchain` and `test_cases`) as an unknown
property. The schemas-job is `continue-on-error: true` so it didn't
gate the release; v0.11.x will refresh the published `witness-mcdc-v1.json`
and `witness-coverage-v1.json` to permit the new fields.

## [0.11.0] — 2026-04-30

### Headline — audit-grade evidence

v0.11 closes the round-3 evaluator deferrals: P2 (avionics
architect) wanted toolchain provenance + test-case-to-row map in
the predicate; P3 (DevOps) wanted the Action to fail loudly on
empty invocations + survive upload failures; P5 (security) wanted
SECURITY.md rewritten for the v0.10 cosign-OIDC chain. All shipped.

Plus a new `witness verify --check-content` flag that re-derives
the canonical-JSON sha256 of the embedded report and compares to
the predicate's `report_sha256`. The signature already protects
that field (it's inside the signed payload); `--check-content`
gives auditors a separately-cite-able binding-evidence line.

### Added — predicate `measurement.toolchain` + `measurement.test_cases`

E1/P2 finding: a DO-178C auditor wants to know *which Rust + which
wasmtime* produced the verdict, not just which witness version
reported it. `Measurement` gains:

```rust
pub toolchain: Option<Toolchain>,            // {rust_version, wasmtime_version}
pub test_cases: Vec<TestCase>,               // [{row_id, invocation}, ...]
```

`witness predicate` now populates both:

- `toolchain.rust_version` from `rustc --version` at predicate-build
  time. `None` when rustc isn't on PATH (downloaded-binary use cases).
- `toolchain.wasmtime_version` from a compile-time constant matching
  the workspace dep pin (currently `"42"`).
- `test_cases` from the run record's `invoked` list — preserves the
  full typed-args spec so `row_id: 2` maps back to `is_leap:2100`,
  not just `is_leap`.

Both fields are `#[serde(default)]` so v0.10.x envelopes deserialise
into the v0.11 type without panic. v0.10.x consumers see new fields
they don't recognise but don't fail.

### Added — `witness verify --check-content`

For `witness-mcdc/v1` envelopes the predicate body carries
`report_sha256`. v0.11.0 adds a `--check-content` flag that:

1. Verifies the DSSE signature (existing behaviour).
2. Re-derives the canonical-JSON sha256 of `predicate.report` and
   compares to the stored `report_sha256`. Mismatch → exit non-zero
   with `content check failed: stored ... derived ...`.

The signature already protects `report_sha256`, so a mismatch means
the producer stored a wrong hash, not that the envelope was
tampered. Auditor logs gain a cite-able binding-evidence line:

```
OK — DSSE envelope env.json verifies against pk
  predicate type: https://pulseengine.eu/witness-mcdc/v1
  subject: instrumented.wasm sha256:1b4903fe...
  subject: verdict_my_fixture.wasm sha256:64c9375d...
  content: report sha256 matches stored value (da0ffcfa…)
```

No-op for `witness-coverage/v1` envelopes (they don't carry
`report_sha256`); the flag emits a one-liner explaining why.

### Fixed — canonical JSON ordering for `report_sha256`

v0.10.0's `report_sha256` was computed via `serde_json::to_vec(&report)`
— struct-field-declaration order. The verifier (`--check-content`,
SECURITY.md sample) recomputes from `stmt.predicate["report"]` whose
`Map<String, Value>` is `BTreeMap`-sorted. Equal contents, different
bytes, mismatched sha. v0.11 canonicalises producer-side via
`to_value()` first so both sides see the same sorted-keys form.
Existing v0.10.x envelopes now fail `--check-content`; sign a fresh
envelope under v0.11+ to use the new flag. Sigstore-style content
binding works going forward.

### Added — `RunRecord.invoked` preserves typed-args specs

Pre-v0.11 the runner pushed bare export names (`"is_leap"`) to
`invoked`. v0.11 pushes the full spec (`"is_leap:2024"`) for typed-
args invocations and the bare name for no-arg invocations.
Downstream `predicate.measurement.test_cases` inherits the change.

### Changed — composite Action: silent-no-op fixes

Three of P3's failure-mode-matrix items closed. New `Validate
invoke inputs` step fails loudly when both `invoke` and
`invoke-with-args` are empty (was: green action, zero rows in the
predicate, evidence shipped empty). New `Warn on non-tag
upload-to-release` emits a `::warning::` and falls back to
`actions/upload-artifact@v4` for the workflow-run artefacts panel.
The `Upload to GitHub Release` step gains `continue-on-error: true`
+ a fallback `actions/upload-artifact@v4` that fires `if: failure()`
so a failed `gh release upload` no longer orphans evidence on a
runner about to be reaped.

Version-compat check (P3 #4) deferred — bash version-comparator
adds ~25 lines for marginal value when `--invoke-with-args` is
v0.9.6+ and adopters generally pin recent versions.

### Rewritten — `SECURITY.md` for the v0.10 cosign-OIDC chain

P5 finding: the file documented v0.6.x ephemeral-Ed25519 only. v0.11
ships a 341-line rewrite covering both signing chains as
complementary signals, the threat model split into addressed /
not-addressed / deferred-to-v0.11+ buckets, and the verifier's
exact commands (no truncation). The combined recipe at the end
ties tarball provenance to predicate integrity in a single CI job
— the SLSA-reviewer-defensible posture P5 was looking for.

### Verified

- 100 tests pass; clippy + fmt clean.
- End-to-end: `witness new` → `./run.sh` → `witness predicate
  --kind mcdc` → `witness verify --check-content` produces:
  - `toolchain.rust_version: "rustc 1.95.0 ..."`
  - `toolchain.wasmtime_version: "42"`
  - 5 test_cases with full specs (`is_leap:2001` through
    `is_leap:1900`)
  - `content: report sha256 matches stored value`

### Notes for v0.11.x and v0.12

Deferred:
- Apple Developer ID signing + notarisation (waiting on user's
  cert plumbing per the previous walkthrough).
- Predicate Rekor-binding (cosign-attest for predicate envelopes,
  not just release tarballs) — substantial integration; v0.12.
- Differential testing against rustc-mcdc (proposal item 10) —
  needs nightly rustc + a comparison harness; v0.11.x stretch.
- `witness new --all-exports` auto-invoke (proposal item 15) —
  half-scoped in v0.11 thinking; v0.11.x.

## [0.10.4] — 2026-04-29

Bug fixes from the v0.10.3 round-3 evaluator pass (5 fresh-eyes
personas, all desk-only after Bash sandbox denials).

### Fixed — `SOURCE_DATE_EPOCH` expression in release.yml was inverted

P5 (security/SLSA evaluator) caught: the v0.10.0 expression
`${{ github.event.head_commit.timestamp && '' || '1700000000' }}`
returns `''` (empty) when a head_commit exists — i.e. always on
tag push — so SOURCE_DATE_EPOCH was effectively unset and
predicate.rs's wall-clock fallback ran. *Plus* `head_commit.timestamp`
is an ISO-8601 string, not the Unix-epoch integer
SOURCE_DATE_EPOCH expects.

v0.10.4 removes the broken workflow-level expression and adds a
per-job step that computes the epoch from the tag commit's Unix
timestamp via `git log -1 --pretty=%ct`, exporting it via
`$GITHUB_ENV`. End-user reproducibility (the actual claim) was
unaffected — predicate.rs honoured the env var when set.

### Fixed — composite Action header read "v0.9.9 — first cut"

P3 (DevOps evaluator) caught: `.github/actions/witness/action.yml`
header described its initial v0.9.9 cut despite v0.10.x changes.
Updated to reflect the v0.10.4 additions (sha256 verification of
the tarball download, real version number).

### Fixed — README + docs referenced a non-existent `@v1` tag

P3 caught: copy-pasting the README's example
`uses: pulseengine/witness/.github/actions/witness@v1` failed
because no `v1` tag existed. v0.10.4 updates docs to use a pinned
release tag (`@v0.10.4`) and documents the rolling `@v0.10` form
for adopters who want patch-level updates within a major.

A rolling `v1` tag may ship later as the GitHub-Actions-marketplace
convention; for now, **explicit version pinning is the documented
path** and matches the safety-critical adoption posture the
v0.10.x stability contract describes.

### Fixed — Action `curl`'d the tarball with no checksum

P3 caught: the action downloaded `witness-${VERSION}-${TARGET}.tar.gz`
and `chmod +x`'d its contents with no integrity check. v0.10.4
adds a SHA-256 verification step: download `SHA256SUMS.txt`
alongside the tarball, compute the local digest, fail with a clear
`::error::` if they don't match. The cosign-OIDC envelope on the
release is still the gold-standard proof; the SHA check catches
the much-cheaper "tarball was truncated mid-download" + "release
was published incomplete" cases without a cosign install.

### Fixed — README front-paragraph jargon

P1 (junior-dev evaluator) caught: the README's first paragraph
landed `MC/DC`, `WebAssembly components`, `in-toto coverage
predicate` before any glossary link, and the first reference to
`docs/concepts.md` was at line 50. v0.10.4 adds a "New here?"
callout in the first 12 lines pointing at quickstart + concepts,
plus an "Is this for you?" subsection that says when *not* to
reach for witness (line/statement coverage on plain Rust → use
cargo-llvm-cov or tarpaulin).

### Verified

- 100 tests pass; clippy + fmt clean.
- The action's sha256 verification step runs locally against a
  v0.10.3 release: SHA256SUMS.txt download succeeds, awk filter
  finds the platform-specific line, comparison passes.

### Notes for v0.10.x and v0.11

Documentation drift items deferred (need design pass):
- SECURITY.md describes v0.6.x ephemeral-Ed25519 chain, not the
  v0.10.0 cosign-OIDC chain (P5 finding).
- Predicate carries `witness_version` only — no Rust toolchain,
  wasmtime version, or test-case-ID-to-row map (P2 architect-lens
  finding).
- DO-330 Tool Qualification Plan + TQL classification + Tool
  Operational Requirements (P2): blocked on the v1.0 Check-It
  artefact design.

These are real gaps, but addressing them right requires a design
pass and (for SECURITY.md) coordinated reframing of the threat
model. v0.11 territory.

## [0.10.3] — 2026-04-29

Five quick-wins from the v0.10.0 proposal's should-ship + nice-to-ship
tiers. Each S effort, all low-risk.

### Fixed — DSSE error messages no longer wrap as "wasm runtime error"

E1 BUG-5 / BUG-6 / F5: pre-v0.10.3 `witness verify` reported
envelope corruption + signature mismatch + key-shape errors via
`Error::Runtime(anyhow!(...))`, which formatted as `wasm runtime
error: DSSE verify failed: ...`. Misleading — no wasm runtime is
involved.

v0.10.3 adds four dedicated `Error` variants:

```rust
EnvelopeMalformed(String)   // file isn't valid DSSE JSON
SignatureInvalid(String)    // verify failed against the supplied key
KeyMalformed(String)        // public key isn't 32 bytes Ed25519
PayloadDecode(String)       // signature ok but base64 decode failed
```

The shape is the same; reviewers reading `witness verify` errors
now see the right category at a glance.

### Fixed — `seq_debug` field emits stable integer string

E1 BUG-4: every manifest entry carried `seq_debug: "Id { idx: 1 }"`
— Rust `Debug` formatting leaked into the schema. v0.10.3 strips
the wrapper; manifests now emit `"seq_debug": "1"`. Diagnostic-only
field (no consumer parses it), so this is a docs-friendly cleanup
rather than a schema break. The helper handles future walrus
versions gracefully — falls back to the full Debug string if the
prefix changes.

### Fixed — compliance bundle no longer double-nests

E1 BUG-10 / F13: `tar -xzf witness-vX-compliance-evidence.tar.gz
-C compliance` previously produced `compliance/compliance/...` —
the `.github/actions/compliance/action.yml` packager included the
output dir's basename as a top-level entry. v0.10.3 packages flat
(`tar -czf bundle.tar.gz -C "$OUTPUT_DIR" .`) so extraction into
any target dir produces the contents directly with no extra
nesting.

### Added — `witness rivet-evidence` quickstart example

Item 17 from the proposal. Pre-v0.10.3 the `witness rivet-evidence`
command was discoverable only via `--help`. v0.10.3 adds §8 of
`docs/quickstart.md` with a 4-line `requirement-map.yaml` and the
single-command invocation. Schema lives at
`docs/schemas/witness-rivet-evidence-v1.json`.

### Added — GitHub Action quickstart pointer

Item 21 from the proposal. The composite Action shipped in v0.9.9
(`pulseengine/witness/.github/actions/witness@v1`) lets any Rust
crate adopt the witness pipeline in 8 lines of YAML, but it wasn't
in the quickstart. v0.10.3 adds §9 with a copy-paste workflow step
and a pointer to `.github/actions/witness/README.md`.

### Verified

- 100 tests pass; clippy + fmt clean.
- New error variants don't break existing callers (the `Error::Runtime`
  paths in non-DSSE code are untouched).
- `witness quickstart` (the embedded version) now carries §8 and §9.

## [0.10.2] — 2026-04-29

### Tester caveats from the v0.10.1 review — docs only

A second-round reviewer flagged four caveats. Three are fixable by
docs alone; the fourth (macOS Developer ID signing + notarisation)
needs Apple Developer Program plumbing and is parked pending the
maintainer wiring in the cert + API key. Three doc patches ship
here.

#### Caveat 1 — post-codegen view, named explicitly in the README

The leap-year fixture reports 2 conditions where the source has 3
because rustc fuses `% 400 == 0` into the same `br_if` chain as the
first two. Pre-v0.10.2 this was buried in `--help`, `lib.rs`
comments, and `docs/concepts.md` §3-§4. v0.10.2 lands a "What
witness measures" subsection at the top of the README's Status
section: rustc may fuse, eliminate, or constant-fold; we count what
the runtime actually executes; this is the post-preprocessor C
parallel from 1992. Readers who skim the front page no longer
need to discover the convention themselves.

#### Caveat 2 — harness mode lifted into `docs/quickstart.md` §7

Pre-v0.10.2 the `docs/quickstart.md` (which the binary embeds via
`witness quickstart`) handwaved harness mode in "What's missing
from this guide" and pointed at the README's full reference.
Readers who only have the quickstart hit a dead end. v0.10.2 lifts
both the v1 (counters only) and v2 (full MC/DC) wire formats into a
proper §7 with the 10-line Node WASI reference implementation
inline. `witness quickstart` now prints the harness reference too.

#### Caveat 3 (doc half) — Gatekeeper note made prominent

The macOS arm64 binaries are not yet Apple Developer ID-signed.
Pre-v0.10.2 the `xattr -d com.apple.quarantine` workaround was a
two-line aside in the install section. v0.10.2 expands it into a
"macOS Gatekeeper note (read me first if you're on macOS)"
subsection that explains both options (xattr and System Settings)
plus the `cosign verify-blob` command that proves provenance
regardless of Gatekeeper status. Apple Developer ID signing +
notarisation is the planned completion path; the workflow
plumbing is documented in the v0.10.x stability contract.

#### Caveat 4 — Stability contract section in README

The reviewer noted: "Project is young — first release April 2026,
0 stars, 1 open issue. Velocity is high but production adoption
should track v0.10.x stability." Fair. v0.10.2 lands a "Stability
contract — v0.10.x" table in the README naming what's stable from
v0.10 (schema URLs, CLI flags, the `witness-mcdc-checker` crate,
the JSON shapes — with serde aliases for v0.9.x field names) and
what's "use at your own risk until v1.0" (Rust public API). v1.0
is positioned as the Check-It qualification artifact.

### Caveat 3 (cert half) — deferred

macOS Developer ID signing + Apple notary submission needs:
- a Developer ID Application certificate exported as `.p12`
- App Store Connect API Key (`.p8` + key ID + issuer ID)
- five GitHub repo secrets

Maintainer has the Apple Developer Program membership; the cert is
installed locally. Plumbing wires up on the next signed-and-
notarised release once secrets are configured. The cosign-OIDC
chain (already shipped in v0.10.0) provides cryptographic
provenance regardless.

### Verified

- 100 tests pass; clippy + fmt clean.
- `witness quickstart` now embeds the harness mode reference.

## [0.10.1] — 2026-04-29

### Fixed — `Test (windows-latest)` failure on the v0.10.0 release

Two issues in the v0.10.0 predicate path-stripping landing surfaced
on the Windows CI runner:

1. **Windows path-rooting**. `Path::new("/foo/bar").is_absolute()`
   returns `false` on Windows because Windows requires a drive
   letter (`C:\...`) for "absolute". `strip_to_project_relative`
   would skip basename collapsing for synthetic Unix-rooted test
   paths. Fix: also accept paths starting with `/` or `\` as
   "rooted enough to strip" — covers the cross-platform fixture
   shape without breaking the relative-path passes-through case.

2. **SOURCE_DATE_EPOCH test race**. v0.10.0 added two standalone
   `now_rfc3339()` tests next to the predicate-level
   `source_date_epoch_pins_predicate_timestamp_and_strips_paths`
   test; all three mutated the same env var. Cargo test
   parallelism inside one binary made them race. Fix: drop the
   redundant standalone tests; the predicate-level test already
   covers `SOURCE_DATE_EPOCH=1700000000` → `2023-11-14T22:13:20Z`
   end to end.

100 tests pass on Linux, macOS, Windows. cargo fmt + clippy
--all-targets -D warnings clean.

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
