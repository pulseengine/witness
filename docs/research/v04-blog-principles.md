# v0.4 Blog Principles Brief

Source survey of every published post on the pulseengine.eu blog, extracting
principles witness's v0.4 release must adopt or align with.

Survey method: posts read from `pulseengine.eu/content/blog/*.md` (markdown
source). The local Hugo server at `127.0.0.1:1024` rejected all `curl`
invocations beyond a single index probe in this sandbox, so two posts
listed at `/blog/` — `mythos-slop-hunt` and `three-patterns-colliding` —
could not be fetched and are not surveyed below. The principles those two
posts contribute that are load-bearing for v0.4 (Mythos = oracle-gated
slop hunt; three-patterns = collision of SDD + MBSE + verification) are
already echoed in `spec-driven-development-is-half-the-loop` and
`overdoing-the-verification-chain`, both surveyed.

---

## 1. Executive summary

Five sentences.

The pulseengine voice is *additive, honest, paywall-aware*: techniques
layer instead of compete, every claim names what it does and does not
cover, and standards quotes are bounded by what the author can actually
read. v0.4 must adopt three principles witness already half-practices —
the Check-It pattern (untrusted prover + tiny qualified checker), the
honest-assessment traffic-light table, and the rivet-as-living-artifact
discipline — and must add three witness has not yet shown publicly: the
oracle-gated parallel-agent scaffold for v0.4's own development, the
six-domain regulatory mapping table, and a CHANGELOG that names what
shipped vs. what is stubbed at the same heading level. Witness already
embodies the post-preprocessor-C / post-rustc-Wasm precedent move, the
"adopt all of them" stance toward Rust-level coverage tools, and the
Conventional-Commits + rivet-trailers discipline. The visible gaps are:
no Check-It checker yet, no signed witness predicate consumed end-to-end
by sigil, no per-domain assessor-fit matrix in README, and no
self-traceability claim ("witness uses witness"). v0.4 should close at
least the README matrix and the public predicate-consumed-by-sigil loop.

---

## 2. Per-post principles table

One row per post surveyed. Slug = path under `/blog/`. Status: ✅ surveyed,
❌ not fetchable in this run.

| # | Title | Slug | Status | Applicable-to-witness principles | Voice / style notes |
|---|---|---|---|---|---|
| 1 | Hello, World | `hello-world` | ✅ | Pipeline diagram is the org's master mnemonic (`meld → loom → synth → kiln`, `sigil` attests). witness must show its own diagram with the same mermaid `flowchart LR` shape, and place itself adjacent to where it actually plugs in (between `loom` and `sigil` on coverage; reading `meld`/`loom` Wasm output). | Single-sentence org thesis ("formally verified WebAssembly pipeline for safety-critical embedded systems — automotive, aerospace, medical"). Subscribe-to-feed close. No hype adverbs. |
| 2 | meld v0.1.0: static component fusion for WebAssembly | `meld-v0-1-0` | ✅ | Release-post template witness should mirror: insight callout → "What X does" → "Try it" with copy-paste curl/cargo commands → table-of-counts before/after → "Why does …" honest-question section → "What is next" bullets. CHANGELOG numbers (e.g. "4 core modules and 23 instantiation steps collapsed into 1 flat module") are load-bearing — witness's v0.4 post must lead with concrete counts. | mermaid graphs for module wiring; mermaid `graph LR` for pipeline; tables to carry size/count comparisons; final paragraph is "Code at github.com/pulseengine/<tool>". |
| 3 | The Component Model as a zero-cost abstraction (series intro) | `zero-cost-component-model` | ✅ | The "What X eliminates / What X preserves by design / What does not fit" three-bucket honesty pattern. Witness's announcement must explicitly state what coverage at the Wasm level *does not* address (e.g. unsafe-region UB — that's Miri/sanitizers; functional correctness — that's Verus/Kani). The "structural overhead is zero-cost / behavioral overhead persists where it must" framing transfers directly: structural coverage is what ships, but it is not the proof. | Series link table at the bottom. `{% note(kind="warning") %}` block to bound any forward-looking claim ("This is the goal, not the current state"). |
| 4 | meld: from intra-component to cross-component composition | `meld-component-fusion` | ✅ | "What X does today / Where X is heading" two-section spine. Witness's v0.4 post needs the same spine: ship-state vs. roadmap, with the warning block separating them. The "role of meld in the pipeline" section — one bullet per downstream consumer — maps directly to a "role of witness in the pipeline" subsection (rivet, sigil, loom, meld, kiln, spar). | Numbered short bullets per downstream consumer; warning block before any not-yet-implemented claim. |
| 5 | loom: post-fusion optimization | `loom-post-fusion-optimization` | ✅ | The "this is not a competition" stance toward wasm-opt: name the prior art, name what it does well, name where the new tool goes further. Witness must do the same toward `wasmcov` / `minicov` / Clang source-MC/DC / rustc-mcdc. The Z3 translation-validation framing — "every optimization X applies should be verifiable" — is the model for witness's "every counter X writes should be re-checkable from the manifest". | "Where loom goes further" as a section title; warning block on translation-validation limitations; phased pipeline list (1-9) for what loom does in order. |
| 6 | synth + kiln: from Wasm to firmware | `synth-kiln-wasm-to-firmware` | ✅ | "The landscape" section — name every prior tool with a github link and a one-line characterisation. Witness already does this in README "Related work"; v0.4 announcement must repeat it. The "build-time vs runtime" dichotomy ("DLR's DASC 2025 paper notes that JIT and AOT at *runtime* generate executable object code, 'a behavior which is not foreseen by current regulations'") is a regulatory-precedent move witness should mirror with the post-preprocessor-C 1992 precedent. | "The last mile" framing for the final transformation; warning block on early-state items; "Where X fits" as section title. |
| 7 | Proving the pipeline | `proving-the-pipeline` | ✅ | The "qualified once, inherited per project" economic argument — witness's v0.4 must claim the same shape: instrumented modules carry their own coverage, evidence is reusable across projects that consume the pipeline. The "build the tools → add verification → demonstrate the pipeline → pursue qualification" four-step long-game phrasing. | Numbered list for the long game; warning block: "None of this exists today in qualified form … Claiming otherwise would be dishonest." This sentence is the voice anchor for v0.4's status section. |
| 8 | Hermetic toolchain (Bazel + Nix + sigil) | `hermetic-toolchain` | ✅ | Reproducibility is regulatory ("ISO 26262 demands traceability from requirements to deployed artifact. DO-178C requires configuration management that can reproduce any released build"). Witness's v0.4 must produce reproducible coverage runs — same instrumented module + same harness invocation = bit-identical run JSON. The sigil attestation chain framing ("Verification says: 'meld's fusion algorithm is correct.' Reproducibility says: 'this build environment matches the qualified configuration.' sigil says: 'this specific ELF was produced by …'") is the template for witness's three-line claim: instrumentation says X, the run says Y, the report says Z. | Inline blockquote with three "X says" lines; warning block listing exactly what is partial; closing "If you are working on … we would like to hear from you" CTA. |
| 9 | Formal verification just became practical | `formal-verification-ai-agents` | ✅ | "What is still open" section is the single most important rhetorical move: long, named, granular. Witness must keep its CHANGELOG's stubbed-vs-shipped distinction at the same heading level — the v0.3.0 CHANGELOG already does this, but v0.4 must promote it to the README's status section. The "old economics / new economics" two-paragraph contrast is reusable. The "proof is not in a paper. It is in the CI log" closing line is the org's voice signature. | First-person singular throughout; bold leading phrase per paragraph in the practical section; literal code blocks for `inv()` and assert breadcrumbs; closing single-sentence aphorism. |
| 10 | rivet v0.1.0: because AI agents don't remember why | `rivet-v0-1-0` | ✅ | "Keep the artifacts next to the code" is the organising principle witness must echo: witness's coverage manifest, run JSON, predicate, and rivet-evidence files all live in the repo, all validated on every commit. The "rivet manages itself" / "tracks its own 344 artifacts, 71 features, 100% traceability coverage" self-application is the move witness should make at v0.4 — witness must publish "witness coverage of witness". The Conventional-Commits + rivet-trailers ("Implements: REQ-…", "Verifies: REQ-…") pattern is already in witness's CHANGELOG; v0.4 must keep it. The closing aphorism *"Meld fuses. Loom weaves. Synth transpiles. Kiln fires. Sigil seals. Rivet binds."* is the canonical org-mnemonic; witness's v0.4 announcement must add a verb for witness ("Witness measures." or similar) at the end of the line, not displace the existing six. | Short literal YAML block to show the artifact shape; literal CLI block to show the validation output; one-line per-tool aphorism. |
| 11 | temper: automated governance | `temper-governance` | ✅ | Every repo in the pulseengine org adheres to the same standards automatically. Witness must inherit temper's branch-protection + signed-commits + Dependabot configuration and document that inheritance in README. "A formally verified compiler that runs in an ungoverned development environment is still a compliance gap" — witness's coverage tool running in an ungoverned environment is the same gap. | Tabular ChatOps command listing; warning block bounds the production claim to one organisation; clear scope statement ("not a general-purpose governance tool — it is purpose-built for PulseEngine's specific requirements"). |
| 12 | What comes after test suites | `what-comes-after-test-suites` | ✅ | The seven-tool layer cake (Verus / Rocq / Lean / Kani / proptest / fuzz / Miri / differential) is the single best chart for witness to position against. Witness adds an eighth layer — *structural coverage of the deployed artifact* — that the cake explicitly does not have. The "Honestly" section ("This stack is not complete. I want to be clear about that.") is the voice anchor for any v0.4 status section. The "infrastructure has to exist before the velocity makes it impossible to retrofit" closing line is the organising urgency-claim witness must echo. | Lists of tools as bulleted list with bold leading verb; "Where he is right" / "Where he is partly wrong" two-section honest-disagreement structure; first-person; no rhetorical questions left unanswered. |
| 13 | Overdoing the verification chain | `overdoing-the-verification-chain` | ✅ | The 10-layer chain table is the canonical chain. Witness must publish a "witness fills the structural-coverage row" cell in this table, not propose a new table. The 6-standard credit matrix is the canonical matrix; witness must produce a single-row "witness" addition. The Check-It pattern (untrusted prover → certificate → tiny trusted checker → DO-330 qualified) is *the* v1.0 design decision: witness instrumentation = untrusted prover; witness manifest + run JSON = certificate; future witness-checker (TBD) = the qualified small piece. The four-gate pipeline (pre-commit / `bazel test //...` / GitHub Actions / `cargo-kiln verify-matrix`) is the operational template. | Heavy use of mermaid `flowchart`; traffic-light table with explicit legend; per-domain bullet list; named footnotes with "Primary" / "Secondary" tier annotations; final "Where we go from here" with three named missing pieces. |
| 14 | Spec-driven development is half the loop | `spec-driven-development-is-half-the-loop` | ✅ | Oracle-gated parallel agents pattern — *minimal prompt + strong mechanical oracle + parallel agents + fresh-session validator*. Witness must be a mechanical oracle: a coverage report is either complete-against-the-manifest or it is not. Witness's own development at v0.4 should adopt the four-file `rank.md` / `discover.md` / `validate.md` / `emit.md` shape (the post links concrete examples in `pulseengine/sigil`). Clay Nelson's *"You cannot attest to what you did not observe"* is the organising attestation principle: witness produces the observation; sigil signs it. The "MBSE, mandatory now" argument applies: witness's manifest is the model the build depends on, not an audit-only sidecar. The "Limits, migration, and where to start" section is the template for any "Limits" section v0.4 ships. | "Take-away" closing bullet list; per-pipeline table; mermaid `flowchart LR` for both the pattern and the V-model walk; first-person; named-precedent footnotes with tier annotation. |
| 15 | mythos-slop-hunt | `mythos-slop-hunt` | ❌ | Not fetchable in this run. From the surveyed posts and witness's existing AGENTS.md and CHANGELOG, the load-bearing claim is: *use the oracle-gated scaffold to find unused exports, dead branches, dishonest CHANGELOG entries, and stubbed-but-unmarked code paths*. Witness's v0.4 must run a slop-hunt against its own repo (per AGENTS.md "delete unused exports" stance) and document the result. | Inferred: same voice as the spec-driven post (which references the same scaffold). |
| 16 | three-patterns-colliding | `three-patterns-colliding` | ❌ | Not fetchable in this run. The load-bearing claim — based on the title and the spec-driven post's "MBSE mandatory now" section — is that SDD + oracle-gated agents + MBSE collide into a single audit-trail loop. Witness must position itself inside that loop: structural coverage is what closes the gap between "agent claims test passes" and "deployed Wasm executed every branch the manifest says exists". | Inferred. |

---

## 3. Cross-cutting principles

Each sub-section ends with 2-4 quoted phrases from the posts. Quotes are
exact; voice is consistent across posts and witness must mirror it.

### 3.1 Honest assessment as a section convention

Every long-form post has a section that names exactly what does not yet
work. Witness has the discipline in CHANGELOG; v0.4 must promote it to a
top-level "Honest assessment" / "Status" section in README.

> "This is comprehensive. It is also not complete — there are open
> questions I have not solved yet." — formal-verification-ai-agents

> "I want to be honest about what this approach does not yet solve." —
> formal-verification-ai-agents §What is still open

> "None of this exists today in qualified form. … Claiming otherwise
> would be dishonest." — proving-the-pipeline

> "This stack is not complete. I want to be clear about that." —
> what-comes-after-test-suites §Honestly

### 3.2 Defense in depth ("overdo")

The chain layers techniques with non-overlapping blind spots. Witness
fills the *structural coverage at the deployed-artifact level* row that
the chain currently leaves implicit.

> "Proofs cover all inputs. Tests cover realistic inputs. Concurrency
> checkers cover every interleaving. Mutation testing covers the test
> suite itself. Sanitizers catch what unsafe code actually does at
> runtime. Each technique answers a different question." —
> overdoing-the-verification-chain

> "Adding a second, independent tool at the same layer is often cheaper
> than tightening the first one beyond diminishing returns." —
> overdoing-the-verification-chain

> "The cost of overdoing is CI budget. The cost of undercommitting is a
> certification campaign that stalls because one technique the assessor
> expected is missing." — overdoing-the-verification-chain

### 3.3 Check-It / certificate-checker discipline

Untrusted prover emits a checkable certificate; tiny trusted checker
validates it; only the checker is qualified. Witness's v1.0 must follow
this; v0.4 should at least name the checker as a tracked future
component.

> "untrusted prover emits a checkable proof certificate, tiny trusted
> checker validates it, only the checker is qualified under DO-330" —
> overdoing-the-verification-chain

> "Building certificate emitters and independent checkers collapses the
> DO-330 problem from 'qualify Z3' (infeasible) to 'qualify a small
> checker' (tractable)." — overdoing-the-verification-chain

### 3.4 Cannot attest to what you did not observe

The whole pipeline rests on producing instruments that emit observations,
then signing them. Witness *is* an instrument; v0.4 must close the loop
where sigil consumes the witness predicate.

> "You cannot attest to what you did not observe." — Clay Nelson, quoted
> in spec-driven-development-is-half-the-loop

> "The mechanical oracle is the instrument that produces the observation.
> A QA-lens agent reading a spec is a second opinion, not an instrument."
> — spec-driven-development-is-half-the-loop

> "*Meld fuses. Loom weaves. Synth transpiles. Kiln fires. Sigil seals.
> Rivet binds.*" — rivet-v0-1-0

### 3.5 Self-application ("X uses X")

rivet tracks its own artifacts; gale tracks its own. Witness must run
witness on witness for v0.4 and publish the coverage number alongside
test-count and clippy-clean claims.

> "rivet tracks its own 344 artifacts, 71 features, 100% traceability
> coverage" — rivet-v0-1-0

> "gale (our Zephyr RTOS kernel port) tracks 268 ASPICE artifacts at 97%
> coverage" — rivet-v0-1-0

> "The repository is the best example — it tracks its own development
> with the same schemas downstream projects will use." — rivet-v0-1-0

### 3.6 Concrete numbers, not adjectives

Every release post leads with counts. Witness's v0.4 announcement must
lead with: branch count instrumented, manifest size, run JSON size,
runtime overhead percentage, fixture branch coverage achieved.

> "4 core modules and 23 instantiation steps collapsed into 1 flat
> module" — meld-v0-1-0

> "402 tests. Cross-repository linking. Incremental validation. Kani
> proofs and Verus specs on core data structures." — rivet-v0-1-0

> "447 artifacts across 19 types, 100% coverage, zero warnings." —
> what-comes-after-test-suites

### 3.7 Paywall-aware sourcing

Standards quotes are bounded by what the author can read; secondary
vendor interpretations are named and tier-annotated. Witness's
docs/paper draft and any v0.4 regulatory mapping must follow.

> "Most safety-critical software standards … are published by standards
> bodies … behind per-copy paywalls. I cannot quote them verbatim. Claims
> about those standards above are backed either by freely available
> primary sources … or by named secondary interpretations from qualified
> vendors and consultants … Each footnote below flags its tier inline."
> — overdoing-the-verification-chain §Sources and caveats

### 3.8 Adopt-not-replace stance toward prior art

Prior tools are characterised by what they do well, then the new tool's
narrow differentiator is named. Witness already practices this in the
seven-row "Related work" table and must keep it.

> "This is not a competition. wasm-opt is production-grade,
> battle-tested … For standard Wasm optimization … wasm-opt is the
> ecosystem standard and categorically more comprehensive than loom's
> general-purpose passes." — loom-post-fusion-optimization

> "wasm2c … is pragmatically powerful — you get GCC, Clang, or a
> qualified safety-critical compiler … and for safety-critical work, the
> qualification burden shifts to the C compiler — which may already be
> qualified for the target domain." — synth-kiln-wasm-to-firmware

### 3.9 Pipeline-diagram-first

Every architectural post leads with a mermaid `flowchart` or `graph LR`.
Witness's v0.4 README must include a diagram showing where its
coverage-counter writes plug into the existing pipeline mermaid.

> mermaid `graph LR` with `meld → loom → synth → kiln` and `sigil`
> dotted-line attestations — hello-world, zero-cost-component-model,
> meld-v0-1-0

### 3.10 Voice mechanics

- Short sentences, no hype adverbs.
- Traffic-light symbols ✅ ◐ ❌ ○ ● with explicit legend per table.
- First-person singular ("I", not "we") in technical assertions; "we"
  reserved for org-collective claims and CTAs.
- Named footnotes with tier annotation ("Primary (open access)" /
  "Secondary").
- mermaid for diagrams; tables for regulatory mapping; bulleted lists
  for action items.
- Closing single-line aphorism ("The proof is not in a paper. It is in
  the CI log." / "Meld fuses. Loom weaves. …").

---

## 4. v0.4 adoption checklist

Twenty concrete actions. Each maps to a specific section / artifact /
diff. Status column: ✅ already in repo, ◐ partial, ❌ missing.

| # | Action | Status | Source post |
|---|---|---|---|
| 1 | Add an `## Honest assessment` (or `## Status — what ships, what is stubbed`) section to README at top level, separating CHANGELOG-quality "shipped" claims from "stubbed in v0.X" claims. | ◐ | formal-verification-ai-agents, what-comes-after-test-suites |
| 2 | Add a six-domain assessor-fit row for witness to README — one row per ISO 26262 / DO-178C+DO-333 / IEC 61508 / EN 50128 / IEC 62304 / ECSS-Q-ST-80C, traffic-light cell per. Cross-reference the overdo blog table by name. | ❌ | overdoing-the-verification-chain |
| 3 | Add an "Honest assessment" sub-section to the v0.4 release post that explicitly names the three checker-side gaps: no Check-It checker yet; no qualified DO-330 dossier; no third-party assessor evaluation. | ❌ | proving-the-pipeline, formal-verification-ai-agents |
| 4 | Lead the v0.4 release post with concrete numbers: branches instrumented in the fixture, manifest size, run JSON size, runtime overhead percentage, end-to-end coverage on the rivet-evidence loop. | ❌ | meld-v0-1-0, rivet-v0-1-0 |
| 5 | Run `cargo-mutants` on the witness library and publish the score in CHANGELOG / README. The slop-hunt principle: if mutating the code does not fail a test, the test was not a real oracle. | ◐ (job exists, score not surfaced) | spec-driven-development-is-half-the-loop, overdoing-the-verification-chain |
| 6 | Add a self-application claim: "witness measures witness". Run witness on witness's own integration test build, publish the coverage number. | ❌ | rivet-v0-1-0 |
| 7 | Add a one-verb extension to the canonical org aphorism in the v0.4 announcement (e.g. "Meld fuses. Loom weaves. Synth transpiles. Kiln fires. Sigil seals. Rivet binds. Witness measures.") — at the **end** of the line; do not displace any of the existing six. | ❌ | rivet-v0-1-0 |
| 8 | Wire the witness predicate (`https://pulseengine.eu/witness-coverage/v1`) into a sigil end-to-end test demonstrating signed-coverage attestation through to verify. v0.3 ships the predicate emitter; v0.4 must close the consumed loop. | ◐ (emitter shipped, sigil-side consumer not pushed) | hermetic-toolchain, spec-driven-development-is-half-the-loop |
| 9 | Push the rivet-side `feat/witness-coverage-evidence-consumer` branch and cut a rivet release that consumes witness output. The CHANGELOG already flags this as left-local; v0.4 must not. | ❌ | rivet-v0-1-0 |
| 10 | Document the Check-It target in README's "Roadmap" section: name it as the v1.0 milestone, characterise the checker's expected size in LOC, name what it would be qualified under (DO-330 TQL-1 candidate). | ◐ (mentioned in AGENTS.md, not in public README) | overdoing-the-verification-chain |
| 11 | Add a `## Where witness fits` mermaid diagram in README placing witness in the org pipeline alongside meld → loom → synth → kiln, with sigil's dotted-line attestation extended to the witness predicate. | ◐ (table exists, diagram does not) | hello-world, zero-cost-component-model |
| 12 | Use `{% note(kind="warning") %}` (or the equivalent Markdown blockquote) before any forward-looking v0.4 claim, and explicitly state the limit, the ETA, and the tracking issue. | ◐ | every series post |
| 13 | Convert the README's "Related work" table into the v0.4 announcement's "The landscape" section, adding a one-paragraph "Where witness goes further" framing that names the post-rustc Wasm gap explicitly. | ◐ (table is there, prose framing is not) | loom-post-fusion-optimization, synth-kiln-wasm-to-firmware |
| 14 | Adopt the four-file oracle-gated agent pipeline (`rank.md` / `discover.md` / `validate.md` / `emit.md`) under `scripts/` for v0.4's own development work — at minimum, a `vmodel`-style pipeline that uses `rivet validate` as the oracle for traceability gaps in witness's own artifact set. | ❌ | spec-driven-development-is-half-the-loop |
| 15 | Add a paywall-aware Sources section to `docs/paper/v0.2-mcdc-wasm.md` (and any v0.4 paper update) with footnotes tagged "Primary (open access)" / "Secondary (vendor interpretation)". The six TODOs flagged in the CHANGELOG must be either resolved or footnote-tagged. | ◐ (TODOs exist, tier annotation does not yet) | overdoing-the-verification-chain §Sources |
| 16 | Verify every CHANGELOG entry distinguishes "shipped" vs. "stubbed" at the same heading level (the v0.2.0 entry already has a `### Stubbed (lands in v0.2.1)` header; v0.4 must keep this style). Every "stubbed" item must name the tracked landing milestone. | ✅ (v0.3 / v0.2 both do this) | formal-verification-ai-agents, what-comes-after-test-suites |
| 17 | Inherit temper's branch protection / signed commits / Dependabot — and document that inheritance in README under "Governance". | ◐ (inherited operationally; not documented) | temper-governance |
| 18 | Reproducibility check: same instrumented Wasm + same harness + same wasmtime version = bit-identical run JSON. Add a CI job that asserts this and document it in README. | ❌ | hermetic-toolchain |
| 19 | Add a `## Limits` section to v0.4 release post (the spec-driven post's template): the oracle has to exist; the oracle can be wrong; signed is not the same as safe; brownfield does not mean stop-the-world; if you start with one tool, start with rivet (witness's spin: if you start with one coverage point, start with the source-level Rust MC/DC — witness adds the post-rustc Wasm row on top). | ❌ | spec-driven-development-is-half-the-loop |
| 20 | Final paragraph of the v0.4 release post: "If you are working on … we would like to hear from you. Everything is at github.com/pulseengine." Mirrors the closing of every series post. | ❌ | hermetic-toolchain, synth-kiln-wasm-to-firmware, proving-the-pipeline |

Twenty actions. Status totals: ✅ 1, ◐ 9, ❌ 10.

---

## 5. What witness already does vs. what's missing

| What witness already does (✅) | What's missing for v0.4 (❌) |
|---|---|
| README cites the overdo blog and the spec-driven blog by name (lines 11-22). | Does not cite hermetic-toolchain or proving-the-pipeline despite using their patterns. |
| `docs/research/overdo-alignment.md` extracts design constraints C1–C7 from the overdo blog. | No equivalent alignment doc for spec-driven-development or proving-the-pipeline. |
| CHANGELOG separates "Added / Changed / Stubbed" at heading level (v0.2.0). | The README's "Status" section does not surface the stubbed list to top-level visibility. |
| Conventional Commits + rivet trailers ("Implements: REQ-…", "Verifies: REQ-…") in CHANGELOG. | Public v0.3 CHANGELOG flags the rivet-side branch as "left local for review — not pushed to origin". v0.4 must close this. |
| README "Related work" table (seven rows) mirrors the synth-kiln "landscape" section pattern. | No prose "Where witness goes further" paragraph that frames the post-rustc Wasm gap. |
| The `--invoke` / `--harness` two-mode runner already documented; subprocess harness mode shipped in v0.2. | No reproducibility-pinning CI job; no `hermetic` claim wired through. |
| `witness predicate` subcommand emits the in-toto Statement v1.0 for sigil consumption (v0.3). | sigil does not yet read the predicate end-to-end in CI; the loop is uncosted on the consumer side. |
| `witness rivet-evidence` subcommand emits the rivet-evidence JSON (v0.3). | Rivet-side `feat/witness-coverage-evidence-consumer` branch is left local; not pushed, not released. |
| Adopt-not-replace stance on Rust-level MC/DC tools (Clang, rustc, Ferrous/DLR) — README closes with "Resistance is futile". | No traffic-light per-domain assessor-fit row for witness in the overdo matrix. |
| Self-traceability via rivet (project uses rivet for its own artifacts; AGENTS.md shows the managed section). | No public "witness measures witness" coverage figure; AGENTS.md says only "2 artifacts across 2 types". |
| Use of mermaid diagrams in DESIGN.md and the v0.2 paper draft. | No mermaid diagram in README placing witness inside the org pipeline. |
| The post-preprocessor-C / post-rustc-Wasm precedent move is in README §line 17–22. | Not yet quoted in the v0.4 announcement template; not yet wrapped with a named footnote and tier annotation. |
| Aligned tool-of-the-quarter: cargo-mutants job exists (informational). | Mutation score not surfaced; no slop-hunt of unused exports per the mythos pattern; AGENTS.md has the stance, no public artifact yet. |
| Six-domain regulatory framing already in `docs/research/overdo-alignment.md` §2. | The framing has not migrated into README or any public-facing doc. |
| The v0.2 paper draft `docs/paper/v0.2-mcdc-wasm.md` has 8.2k words, six sourcing TODOs. | TODOs not yet "Primary / Secondary" tier-annotated as the overdo blog does. |

---

## 6. References

Every post linked. Slug at the path under `https://pulseengine.eu/blog/`.

| # | Title | Slug |
|---|---|---|
| 1 | Hello, World | `hello-world` |
| 2 | meld v0.1.0: static component fusion for WebAssembly | `meld-v0-1-0` |
| 3 | The Component Model as a zero-cost abstraction for safety-critical systems | `zero-cost-component-model` |
| 4 | meld: from intra-component fusion to cross-component composition | `meld-component-fusion` |
| 5 | loom: why optimizing after fusion is not the same as wasm-opt | `loom-post-fusion-optimization` |
| 6 | synth + kiln: from Wasm to firmware | `synth-kiln-wasm-to-firmware` |
| 7 | Proving the pipeline: verification from component to binary | `proving-the-pipeline` |
| 8 | The toolchain: hermetic builds and supply chain attestation | `hermetic-toolchain` |
| 9 | Formal verification just became practical — AI agents changed the economics | `formal-verification-ai-agents` |
| 10 | rivet: because AI agents don't remember why | `rivet-v0-1-0` |
| 11 | temper: automated governance for a safety-critical toolchain | `temper-governance` |
| 12 | What comes after test suites | `what-comes-after-test-suites` |
| 13 | Overdoing the verification chain — and mapping it to six safety domains | `overdoing-the-verification-chain` |
| 14 | Spec-driven development is half the loop | `spec-driven-development-is-half-the-loop` |
| 15 | mythos-slop-hunt | `mythos-slop-hunt` *(not fetchable in this run; principles inferred from neighbouring posts)* |
| 16 | three-patterns-colliding | `three-patterns-colliding` *(not fetchable in this run; principles inferred from neighbouring posts)* |

Index URL: `http://127.0.0.1:1024/blog/` (Hugo dev server) — confirmed
returning 200 in this run; only the index probe was permitted, so the
fifteen posts above were sourced from
`/Users/r/git/pulseengine/pulseengine.eu/content/blog/*.md` per the
brief's fallback clause. The two posts marked with footnotes
(`mythos-slop-hunt`, `three-patterns-colliding`) are listed at the
index but not present on disk and could not be fetched; the principles
attributed to them in §2 are inferred from the surveyed posts that
reference them.
