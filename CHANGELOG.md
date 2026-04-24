# Changelog

All notable changes to witness are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [SemVer 2.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Bootstrap scaffolding: Cargo project, CLI skeleton with `instrument` /
  `run` / `report` subcommands, library module layout, rivet artifact
  seeds, AGENTS.md + CLAUDE.md.
- Design document capturing v0.1→v1.0 roadmap and the decision-granularity
  open research question.
- README with ecosystem relationship to rivet, sigil, loom, meld, kiln,
  spar, and explicit *complementary-not-competitive* framing against the
  Ferrous/DLR Rust-level MC/DC work.

### Target for v0.1

- `witness instrument <in.wasm> -o <out.wasm>` — walrus-based branch counter
  insertion at every `br_if` / `br_table` / `if`.
- `witness run --harness "<cmd>" --module <m>` — harness runner + counter
  collection.
- `witness report` — branch-level coverage summary (JSON and text).
