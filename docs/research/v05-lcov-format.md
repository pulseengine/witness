# v0.5 — LCOV emission for Wasm-bytecode coverage

> Research brief for the `witness lcov` subcommand. Decides what bytes
> witness writes when codecov / coveralls expect an `lcov.info` upload.

## 1. Executive summary

`witness lcov <run.json> -o witness.lcov` emits an LCOV tracefile in the
[geninfo(1)][geninfo] format. Each `BranchHit` becomes a `BRDA` record;
each instrumented Wasm function becomes one `FN` / `FNDA` pair; per-file
totals are summed into `LF` / `LH` / `FNF` / `FNH` / `BRF` / `BRH`.
**The synthesis decision is option (c), the hybrid:** when DWARF correlation
exists (`Decision.source_file` + `source_line` populated), witness emits
LCOV records keyed to the real source path, so codecov can join the upload
to repo files and render line-level branch annotations side-by-side with
cargo-llvm-cov's Rust-source LCOV under a separate `wasm-bytecode` flag.
When DWARF is absent (strict-per-`br_if`, no `Decision`s), witness emits
**no LCOV** for those branches — codecov rejects synthetic paths via the
[Path Fixing][codecov-paths] mechanism ("all file paths in the coverage
report must match the git file structure") — and writes a sibling
`witness-overview.txt` summarising the un-correlated bytecode coverage
that the user can attach as a build artefact for human inspection.

## 2. LCOV directive cheatsheet

Source: [`geninfo(1)` man page, "TRACEFILE FORMAT" section][geninfo].
Line ordering inside a record is conventional but not strictly enforced
by lcov-1.x consumers; codecov's [LcovProcessor][codecov-formats] follows
the same convention.

| Directive | Meaning | Mandatory? | Witness uses? |
|---|---|---|---|
| `TN:<test name>` | Test name. Emitted once at the top of the file. | Optional | Yes — `TN:wasm-bytecode` |
| `SF:<absolute path>` | Begins a per-source-file section. | **Yes** | Yes (DWARF-correlated path) |
| `FN:<line>,<name>` | Function definition starts at `<line>`. | Optional | Yes — one per Wasm function with at least one DWARF-correlated branch |
| `FNDA:<count>,<name>` | Function execution count (sum of hits on its branches). | Optional | Yes |
| `FNF:<n>` | Total functions in this `SF` section. | Required if any `FN` emitted | Yes |
| `FNH:<n>` | Functions with `FNDA` > 0. | Required if any `FN` emitted | Yes |
| `BRDA:<line>,[e]<block>,<branch>,<taken>` | One branch arm. `<taken>` is `-` if the basic block was never executed, otherwise an integer hit count. `[e]` is the optional `e` exception flag. | Optional | **Yes — primary signal** |
| `BRF:<n>` | Total branches in section. | Required if any `BRDA` emitted | Yes |
| `BRH:<n>` | Branches with non-zero `<taken>`. | Required if any `BRDA` emitted | Yes |
| `DA:<line>,<count>[,<checksum>]` | Line execution count. | Optional | Yes — synthesised from BRDA so codecov shows a line as "covered" when at least one of its branches fired |
| `LF:<n>` | Total instrumented lines. | Required if any `DA` | Yes |
| `LH:<n>` | Lines with `count` > 0. | Required if any `DA` | Yes |
| `end_of_record` | Closes the section. | **Yes** | Yes |

### `BRDA` field semantics — the load-bearing one

From [geninfo(1)][geninfo] and the [linux-test-project/lcov#334][brda-issue]
discussion:

```
BRDA:<line_number>,[<exception>]<block>,<branch>,<taken>
```

- **`line_number`** — must be > 0. `coveragepy` issue [#1846][cov-1846]
  documents that line 0 makes downstream parsers reject the record.
- **`block`** — integer ≥ 0; "uniquely defines a particular edge in the
  expression tree on this line". For witness, **`block = function_index`**
  (so all branches in one function on one line share a block) and
  **`branch = id`** (witness's globally-unique branch id, decimal).
- **`branch`** — string identifier for the edge.
- **`taken`** — `-` for "never reached" (basic block dead) vs. integer
  for hit count. Witness emits the integer hit count straight from
  `BranchHit.hits`. Witness never emits `-` because every `BranchHit`
  comes from a counter that was *instantiated* (the basic block reached
  the counter increment) — the question is only whether the counter
  fired.

## 3. Codecov flag-upload mechanism

[Codecov supports][codecov-formats] LCOV via its `LcovProcessor` ("Graphical
version of Gcov"); both `.lcov` and `.info` extensions are auto-detected.

### Two-flag upload pattern

[codecov-action's `files` parameter][codecov-action] accepts `path:flag`
syntax:

```yaml
- uses: codecov/codecov-action@v4
  with:
    files: ./lcov.info:rust-source,./witness.lcov:wasm-bytecode
    fail_ci_if_error: true
```

Each upload becomes a separate logical report keyed by flag. The
[codecov.yml flag definition][codecov-flags] in this repo will need:

```yaml
flags:
  rust-source:
    paths:
      - src/
    carryforward: false
  wasm-bytecode:
    paths:
      - src/
    carryforward: true   # bytecode coverage runs less often than unit tests
```

### What the dashboard renders

With two flags pointed at the same `paths:`, codecov's PR comment renders
both percentages side-by-side (the `flags` token in `comment.layout`,
already enabled in [codecov.yml line 13][codecov.yml]). The line-level
overlay in the codecov UI shows whichever flag the user toggles. For
files where both flags emit `BRDA`, codecov picks the union (any flag
covers the line ⇒ line is green); the per-flag totals stay distinct.

### Free-tier blockers — none confirmed

Codecov's [Flags documentation][codecov-flags] does not gate flags on
plan tier. [codecov-action issues #50][action-50] and [#1522][action-1522]
discuss multi-flag uploads with no payment-related caveats. **No blocker
identified for free-tier multi-flag uploads.**

## 4. Strict-fallback synthesis decision: option (c) hybrid

The three options from the brief, evaluated:

### Option (a) — synthesise paths like `wasm/function_42.wat`

**Rejected.** [Codecov Path Fixing docs][codecov-paths] state explicitly:
> For Codecov to operate correctly, all file paths in the coverage
> report must match the git file structure.

Synthetic paths land in codecov's "files not in repo" bucket and are
silently dropped from coverage totals. The `fixes:` mapping helps when
a coverage tool emits `before/foo.rs` and the repo has `after/foo.rs`,
but cannot conjure a real file from `wasm/function_42.wat`. Cross-tool
diff (the whole point of LCOV) is broken because the Rust-source LCOV
references `src/lib.rs:42` while the synthetic LCOV references a path
that does not exist in the repo.

### Option (b) — require DWARF for LCOV (no DWARF → no LCOV)

**Almost.** Better than (a) because it never lies to codecov. But it
strands users who have a stripped Wasm module and want *some*
machine-readable summary. Witness already produces `RunRecord` JSON for
that case, but a sibling text overview is friendlier for CI artefact
attachment.

### Option (c) — hybrid: DWARF-correlated LCOV + non-correlated overview

**Selected.** Concretely:

1. `witness lcov` walks the `RunRecord`. For each `BranchHit` whose
   `(function_index, instr_index)` resolves through the manifest's
   `decisions[]` list to a `(source_file, source_line)`, emit a `BRDA`
   record under `SF:<source_file>`.
2. Branches that do **not** resolve to a source line are aggregated
   into `witness-overview.txt` (per-function totals + uncovered branch
   list, the same format `witness report --format text` produces today).
3. If *zero* branches correlate, `witness lcov` writes an empty LCOV
   file (just `TN:wasm-bytecode\n`) and exits with a warning to stderr.
   Empty-but-valid LCOV is what codecov-action expects when no tests
   ran — it does not fail the upload.

### Justification

- **Codecov tolerance:** confirmed via the path-fixing docs that
  synthesised paths are dropped silently. Hybrid keeps every emitted
  `BRDA` joinable to a real file.
- **JaCoCo precedent:** [JaCoCo's coverage counters doc][jacoco-counters]
  states "for class files compiled with debug information, coverage
  information for individual lines can be calculated, where a source
  line is considered executed when at least one instruction assigned to
  this line has been executed." JaCoCo emits *no* line coverage when
  debug info is stripped — only its instruction counter remains. Hybrid
  matches this: when DWARF is missing, the `BRDA`/line view degrades to
  the bytecode-only overview, never lies about source coverage.
- **Wasmcov precedent:** [wasmcov][wasmcov] uses LLVM IR (`--emit=llvm-ir`)
  to recover source mapping precisely *because* the raw `.wasm` lacks
  the `__llvm_covmap` section — it refuses to fake source paths and
  takes the cost of an extra build artefact. Witness's hybrid is the
  same posture without forcing every user to keep DWARF.
- **No regression of existing artefacts:** the `RunRecord` JSON and the
  `witness rivet-evidence` outputs already cover the strict-per-`br_if`
  case. `witness lcov` is purely additive for the codecov-facing UX.

## 5. DWARF-correlated emission — `Decision` to `BRDA`

Source: `src/decisions.rs` populates `Manifest.decisions: Vec<Decision>`
when DWARF is present. Each `Decision` has:

```rust
pub struct Decision {
    pub id: u32,
    pub conditions: Vec<u32>,        // BranchHit.id values
    pub source_file: Option<String>, // DWARF-derived
    pub source_line: Option<u32>,    // DWARF-derived
}
```

Mapping rule (one Decision → multiple `BRDA` rows, one per condition):

```text
For each Decision d where d.source_file.is_some() && d.source_line.is_some():
    let block_id = function_index_of(d.conditions[0])
    For each (i, branch_id) in d.conditions.iter().enumerate():
        let hit = run_record.branches[branch_id].hits
        emit:  BRDA:<d.source_line>,<block_id>,<i>,<hit>
```

For non-`BrIf` kinds (`IfThen` / `IfElse` / `BrTableTarget` /
`BrTableDefault`), v0.4's `decisions::group_into_decisions` does **not**
group them. v0.5 either:

- **(preferred for v0.5)** emit them as singleton-`BRDA` rows under
  `SF:<wasm-only marker>`, or
- **(scope-deferred to v0.6)** wait until DWARF reconstruction extends
  to `if/else` and `br_table` arms (requires `DW_AT_call_file` chasing
  per `decisions.rs` module-level docstring).

Recommendation: **scope-defer to v0.6**. v0.5's deliverable is the LCOV
emission machinery proven against `BrIf` chains, which is the path that
matters for short-circuited boolean MC/DC. `if/else` arms in v0.5 land
in the overview text, same as the strict-fallback case.

### Function-level records

Within an `SF` section, witness emits one `FN` / `FNDA` pair per Wasm
function that has at least one DWARF-correlated branch in this file:

- `FN:<min_line_in_function>,<function_name>` — line is the lowest
  `source_line` across the function's correlated decisions; name comes
  from `BranchHit.function_name` (the wasm name-section export name)
  with a `wasm:` prefix to disambiguate from Rust functions in the
  cargo-llvm-cov LCOV.
- `FNDA:<sum_of_hits>,<name>` — sum across the function's branches.

`DA` records are synthesised one per unique `source_line` referenced by
a `BRDA`: count = max of branch hits on that line. Codecov uses `DA`
for the file-list percentages and `BRDA` for the branch-detail overlay;
emitting both is required for both views to show data.

## 6. Worked example

### Input — small `RunRecord` (5 branches, simplified)

Pretend witness measured a Rust function compiled to Wasm, with DWARF
correlating every `br_if` to `src/predicate.rs`. The short-circuit chain
`a && b && c` produced three `br_if`s on line 18; an `if/else` on line
22 produced two more branches (un-correlated in v0.5 — they land in the
overview file).

```json
{
  "schema_version": "2",
  "module_path": "target/wasm32-wasip1/release/example.wasm",
  "branches": [
    {"id": 0, "function_index": 7, "function_name": "evaluate",
     "kind": "br_if", "instr_index": 4, "hits": 12},
    {"id": 1, "function_index": 7, "function_name": "evaluate",
     "kind": "br_if", "instr_index": 9, "hits": 7},
    {"id": 2, "function_index": 7, "function_name": "evaluate",
     "kind": "br_if", "instr_index": 14, "hits": 3},
    {"id": 3, "function_index": 7, "function_name": "evaluate",
     "kind": "if_then", "instr_index": 22, "hits": 2},
    {"id": 4, "function_index": 7, "function_name": "evaluate",
     "kind": "if_else", "instr_index": 27, "hits": 1}
  ]
}
```

Manifest's `decisions[0]` groups branches `[0, 1, 2]` at
`src/predicate.rs:18`. Branches 3 and 4 have no Decision (v0.5 limitation).

### Output — `witness.lcov`

```
TN:wasm-bytecode
SF:src/predicate.rs
FN:18,wasm:evaluate
FNDA:22,wasm:evaluate
FNF:1
FNH:1
BRDA:18,7,0,12
BRDA:18,7,1,7
BRDA:18,7,2,3
BRF:3
BRH:3
DA:18,12
LF:1
LH:1
end_of_record
```

(`FNDA` value 22 = sum of hits 12+7+3 across the correlated branches —
function-level totals only count what we emit. `DA:18,12` uses the
max-hits-on-line synthesis rule from §5.)

### Output — `witness-overview.txt` (sibling, for un-correlated branches)

```
witness 0.5 wasm-bytecode coverage overview
module: target/wasm32-wasip1/release/example.wasm
correlated to source: 3/5 branches (60.0%)
non-correlated: 2 branches

fn 7 (evaluate) — non-correlated:
  instr +22 [IfThen]      id=3  hits=2
  instr +27 [IfElse]      id=4  hits=1
```

### codecov-action invocation

```yaml
- run: cargo llvm-cov --lcov --output-path lcov.info
- run: witness lcov target/run.json -o witness.lcov
- uses: codecov/codecov-action@v4
  with:
    files: ./lcov.info:rust-source,./witness.lcov:wasm-bytecode
    fail_ci_if_error: true
- uses: actions/upload-artifact@v4
  with:
    name: witness-overview
    path: witness-overview.txt
```

## 7. Implementation sketch

New module: `src/lcov.rs`. Pure-stdlib (no new deps); takes
`&RunRecord` + `&Manifest`, returns `(String, String)` for the LCOV
content and the overview text.

```rust
// src/lcov.rs (sketch — not committed)
use crate::instrument::{Decision, Manifest};
use crate::run::{BranchHit, RunRecord};
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub struct LcovOutputs {
    pub lcov: String,
    pub overview: String,
    pub correlated: usize,
    pub non_correlated: usize,
}

pub fn emit(record: &RunRecord, manifest: &Manifest) -> LcovOutputs {
    // 1. Index hits by branch id.
    let hits: BTreeMap<u32, &BranchHit> =
        record.branches.iter().map(|b| (b.id, b)).collect();

    // 2. Bucket Decisions by source file.
    let mut by_file: BTreeMap<String, Vec<&Decision>> = BTreeMap::new();
    let mut correlated_ids: std::collections::HashSet<u32> =
        std::collections::HashSet::new();
    for d in &manifest.decisions {
        let (Some(file), Some(_line)) = (&d.source_file, d.source_line)
            else { continue };
        for &c in &d.conditions { correlated_ids.insert(c); }
        by_file.entry(file.clone()).or_default().push(d);
    }

    // 3. Emit LCOV.
    let mut lcov = String::from("TN:wasm-bytecode\n");
    for (file, decisions) in &by_file {
        write!(lcov, "SF:{file}\n").unwrap();
        emit_section(&mut lcov, decisions, &hits);
        lcov.push_str("end_of_record\n");
    }

    // 4. Emit overview for non-correlated branches.
    let overview = build_overview(record, &correlated_ids);

    LcovOutputs {
        lcov,
        overview,
        correlated: correlated_ids.len(),
        non_correlated: record.branches.len() - correlated_ids.len(),
    }
}
```

`emit_section` walks each `Decision`, writes `FN`/`FNDA`/`BRDA`/`DA`
lines per the rules in §5, and totals `FNF`/`FNH`/`BRF`/`BRH`/`LF`/`LH`.

### CLI surface

```
witness lcov <run.json> [-o <path>] [--manifest <path>]
                        [--overview <path>]
                        [--testname <name>]
```

Defaults:
- `-o` defaults to `witness.lcov` next to `<run.json>`.
- `--manifest` defaults to `<module>.witness.json` (same heuristic as
  `witness run`).
- `--overview` defaults to `witness-overview.txt`.
- `--testname` defaults to `wasm-bytecode` (used as the `TN:` value).

### Determinism

Already required by AGENTS.md invariant 3 ("reports are deterministic").
The `BTreeMap` keying in the sketch above keeps file iteration order
stable; within a file, decisions are sorted by `(source_line, id)` and
conditions emit in their stored vector order (which `decisions.rs`
sorts by branch id during reconstruction).

### Tests

- Unit: empty manifest → empty LCOV (`TN:` only). Single Decision with
  3 conditions → exact `BRDA` triplet from §6. Mixed correlated +
  un-correlated → correlated lines in LCOV, un-correlated lines in
  overview.
- Property (proptest): for any randomly constructed `RunRecord` +
  `Manifest`, `BRF` always equals the count of emitted `BRDA` lines and
  `BRH` always equals the count of those with non-zero `taken`.
- Golden (insta): the §6 input as a checked-in fixture, snapshot the
  exact LCOV string. Catches accidental whitespace / ordering drift.

### What this does NOT do (deferred)

- Per-target `br_table` LCOV — needs DWARF-grouped Decisions over
  `BrTableTarget` arms (`decisions.rs` v0.4 module docstring marks this
  as v0.5+ work).
- Macro-expansion disambiguation — multiple decisions on one source
  line currently merge into one `BRDA` block. Same limitation as the
  current v0.4 reconstruction.
- `cobertura` XML emission — codecov accepts both, but every additional
  emitter is another spec to maintain. Defer to user-demand signal.

## Sources

- [geninfo(1) man page — Debian unstable][geninfo] — primary LCOV
  tracefile-format spec; `BRDA` syntax, `end_of_record`, optional
  exception flag.
- [linux-test-project/lcov issue #334 — BRDA branch coverage][brda-issue]
  — clarifies the `block`/`branch` semantics in the absence of formal
  prose in the man page.
- [coveragepy issue #1846][cov-1846] — line-number-zero in `BRDA`
  rejection caveat.
- [Codecov Supported Coverage Report Formats][codecov-formats] — LCOV
  is supported via `LcovProcessor` (.lcov / .info auto-detect).
- [Codecov Path Fixing][codecov-paths] — "all file paths in the coverage
  report must match the git file structure"; basis for rejecting
  option (a).
- [Codecov Flags][codecov-flags] — flag definition, multi-flag uploads,
  carryforward.
- [codecov-action README][codecov-action] — `files: ./a.xml:flag1,./b.xml:flag2`
  syntax for multi-flag CI uploads.
- [JaCoCo Coverage Counters][jacoco-counters] — "source line is
  considered executed when at least one instruction assigned to this
  line has been executed"; precedent for the hybrid-degradation
  posture when debug info is absent.
- [wasmcov General docs][wasmcov] — uses LLVM IR (`--emit=llvm-ir`) to
  recover source mapping for Wasm; precedent for refusing synthetic
  paths.
- [LLVM `llvm-cov` command guide][llvm-cov] — `--lcov` mode definition
  for cross-checking the format expectations of the Rust-source upload.

[geninfo]: https://manpages.debian.org/unstable/lcov/geninfo.1.en.html
[brda-issue]: https://github.com/linux-test-project/lcov/issues/334
[cov-1846]: https://github.com/coveragepy/coveragepy/issues/1846
[codecov-formats]: https://docs.codecov.com/docs/supported-report-formats
[codecov-paths]: https://docs.codecov.com/docs/fixing-paths
[codecov-flags]: https://docs.codecov.com/docs/flags
[codecov-action]: https://github.com/codecov/codecov-action
[action-50]: https://github.com/codecov/codecov-action/issues/50
[action-1522]: https://github.com/codecov/codecov-action/issues/1522
[jacoco-counters]: https://www.jacoco.org/jacoco/trunk/doc/counters.html
[wasmcov]: https://hknio.github.io/wasmcov/docs/General
[llvm-cov]: https://llvm.org/docs/CommandGuide/llvm-cov.html
[codecov.yml]: ../../codecov.yml
