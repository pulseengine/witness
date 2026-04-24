# Overdo-alignment briefing for witness

Source: `pulseengine.eu/content/blog/2026-04-22-overdoing-the-verification-chain.md`
(draft, `draft = true` in front matter, dated 2026-04-22).

Purpose: keep witness's README, announcement post, and v0.1-1.0 design decisions
in voice-and-substance alignment with the "overdo" thesis that the witness
project already cites in its AGENTS.md. This is an alignment brief, not a
summary — only the load-bearing claims and quotes are captured.

---

## 1. The argument's core claim

Regulated software should layer independent verification techniques rather
than pick one and tune it; each technique has a different blind spot, and
combinations shrink the blind spot faster than tightening any single
technique. Overdoing the chain is the honest default when you work across
standards whose assessors will each demand a different subset of evidence.
The cost of overdoing is CI budget; the cost of undercommitting is a
certification campaign stalling because one technique the assessor expected
is missing.

## 2. The six safety domains

The blog actually names **seven** domains in its matrix — the AGENTS.md's
"six" count excludes nuclear (IEC 60880), which the blog explicitly says
"sits outside this table deliberately". For witness's purposes the six
in-scope domains with their verification-chain contribution are:

| Domain | Standard | What this domain demands of the chain |
|---|---|---|
| Avionics | DO-178C + DO-333 | Permits formal analysis in place of testing under soundness + completeness conditions; translation validation is the under-valued DO-333 asset; DO-330 tool-qualification dossier is the cost. |
| Automotive | ISO 26262 Part 6 (ASIL A–D) | Formal verification "highly recommended" at ASIL D; Tool Confidence Level qualification is the gap. |
| General functional safety | IEC 61508-3 (SIL 1–4) | FM "highly recommended" at SIL 3/4; theorem-proving lineage welcomed; proven-in-use or §7.4.4 argument is the gap. The baseline — if this works, most of the others follow. |
| Railway | EN 50128 (SIL 0–4) | Formal proof explicitly "highly recommended" at SIL 3/4; theorem-proving is the cultural norm (B-method heritage). |
| Medical | IEC 62304 + FDA CSA (Class A/B/C) | Standard silent on specific tools; accepts technique plus traceability. Traceability is the usable artifact. |
| Space | ECSS-Q-ST-80C Rev.2 (Cat A/B) | Recommended; no rigid qualification regime; assessor-checkable artifacts (proof terms, SMT certificates) are where realistic credit lives. |

Nuclear (IEC 60880) is the seventh domain in the matrix but explicitly
de-prioritised: "Regulator acceptance of SMT-backed arguments is harder to
establish … the pedigree preference for tabular methods means an SMT-first
stack walks in with a disadvantage."

**Implication for witness.** witness's coverage evidence has to be legible
to assessors across at least the first five domains. Mutation testing,
traceability, and sanitizer runs are all graded `strong fit` for
DO-178C+DO-333, ISO 26262 ASIL D, IEC 61508 SIL 4, and EN 50128 SIL 4 in
the blog's matrix. Witness-produced coverage needs to slot into that same
"strong fit" column.

## 3. The "overdo principle"

The blog never uses the phrase "overdo principle" as a named lemma. The
phrasing witness is aligning with is distributed across four quotable
paragraphs. The canonical forms:

> "Proofs cover all inputs. Tests cover realistic inputs. Concurrency
> checkers cover every interleaving. Mutation testing covers the test suite
> itself. Sanitizers catch what unsafe code actually does at runtime. Each
> technique answers a different question. When you do not yet know which
> question the next assessor cares about — and you will not, because you
> work across six standards written by six different committees — overdoing
> is the only honest default."

> "Regulated domains reward defense in depth, not minimum-viable
> verification. Each technique has a blind spot… Combinations shrink the
> blind spot faster than any single technique improves. Adding a second,
> independent tool at the same layer is often cheaper than tightening the
> first one beyond diminishing returns."

> "The cost of overdoing is CI budget. The cost of undercommitting is a
> certification campaign that stalls because one technique the assessor
> expected is missing. I know which I would rather pay."

**When overdo applies.** When (a) multiple independent standards could plausibly
assess the same artefact; (b) techniques at the same chain-layer have
non-overlapping blind spots; (c) the cost of the second technique is CI
cycles, not a separate qualification campaign; (d) AI-velocity code is
growing the verification surface faster than any single technique can keep
up with.

**When overdo would be overengineering.** The blog doesn't call this out
directly, but the exclusion is visible in the structure: do *not* overdo when
the second technique shares the same blind spot as the first (no new
information), when the cost is not CI budget but a parallel qualification
dossier without matching credit, or when no regulator in any of the target
domains would recognise the technique (the blog's nuclear/SMT treatment is
the worked example — an SMT-first stack "walks in with a disadvantage", so
the blog *doesn't* push harder there and instead says "Defer unless
mission-critical").

**Witness's commitment.** AGENTS.md already codifies this: when Ferrous/DLR
Rust-level MC/DC ships, witness does not become obsolete; both are adopted
because the measurement points have different blind spots. This matches
the blog's "adding a second, independent tool at the same layer is often
cheaper than tightening the first". Post-rustc Wasm coverage and pre-rustc
Rust MC/DC are that pair of tools.

## 4. Direct mentions — quotes to reuse without paraphrase

The blog does **not** mention witness by name, does **not** mention Wasm
MC/DC specifically, and does **not** mention sigil or loom as Wasm
producers. It does mention Wasm three times and rivet several times.

**On rivet:**

> "rivet — traceability across all three [layer groups]"

> "Traceability | Which requirement does each of the above satisfy? | rivet"

> "[Traceability (rivet)] ✅ shipping, living artifact"

**On Wasm (all three occurrences):**

> "Bespoke Z3 translation validation on the WASM optimizer"

> "IR-to-IR translation | Does the pipeline preserve what was proved at the
> source? | Bespoke Z3 translation validation on the WASM optimizer"

> "The WASM-IR translation validation is the under-valued asset; it is the
> kind of translation-preservation argument DO-333's source-to-object rules
> were written for. Qualification under DO-330 is the work."

**On MC/DC:** the blog never uses "MC/DC" as a phrase. The closest is the
mutation-testing row tooltip in the matrix: *"strong — test-suite adequacy
addresses the MC/DC-for-Rust gap"*. That phrase — "the MC/DC-for-Rust gap" —
is the one textual hook in the parent post that witness's own post can
cite directly when positioning itself.

**On coverage:** not used as a technique name in the chain. Coverage is
implicit inside mutation testing (cargo-mutants) and proptest rows. This
is a gap witness fills: the blog has a chain layer called "Test-suite
adequacy" (cargo-mutants) and a layer called "Property-based sampling"
(proptest), but no explicit *structural coverage* row. Witness slots in
between.

## 5. Stylistic / rhetorical patterns

**Framing complementary-not-competitive.** The blog never frames any two
techniques as rivals. The entire rhetorical move is additive: each
technique answers a "different question", each has a "blind spot",
combinations "shrink the blind spot faster than any single technique
improves". The metaphor is *layers*, and the chain diagram is stacked, not
a decision tree. Witness announcements should mirror this: do not pitch
Wasm-level MC/DC *vs.* Rust-level MC/DC; pitch them as two measurement
points on the same chain.

**How it talks about regulatory acceptance.** Three consistent moves:
1. Name the specific clause or technique class ("DO-333 permits formal
   analysis in place of testing under soundness + completeness conditions";
   "Formal verification 'highly recommended' at ASIL D"; "§FM.6.7(f)-style
   asset").
2. State the specific qualification gap as a named work item ("DO-330
   qualification dossier for Verus / Kani / Z3 TV"; "Tool Confidence Level
   qualification"; "Proven-in-use or §7.4.4 qualification argument").
3. Acknowledge paywall sourcing honestly — the standards are behind
   paywalls, claims are backed by primary open sources where available or
   named secondary vendor interpretations otherwise.

**Voice markers to match in witness copy.**
- Short sentences; no hype adverbs.
- Use of traffic-light symbols `✅ ◐ ❌ ○ ●` with explicit legend.
- "Honest assessment" as a section title convention.
- Footnotes tier their sources inline ("Primary (open access)" vs
  "Secondary").
- First-person singular ("I know which I would rather pay"), not royal we.
- Diagrams use flowchart/mermaid; no decorative assets.
- Tables used aggressively to carry the regulatory mapping.
- Acknowledge what the current stack does **not** clear
  ("honest read on what still does not clear the bar").

**The C-macro precedent move.** AGENTS.md already stakes out witness's
core rhetorical claim: "MC/DC for C is measured post-preprocessor, not on
pre-preprocessor source. Nobody argues this is illegitimate; DO-178C has
accepted it since 1992. witness measures post-rustc Wasm, which is the
structural analog." This parallels the overdo post's move of citing named
precedent (Airbus A380/A350 using Astrée for DO-178B/C DAL A) to establish
that a technique *already* has assessor acceptance. Witness's README
should lean on the post-preprocessor-C precedent the same way the parent
post leans on Airbus precedent.

## 6. Design constraints the blog imposes on witness

These are commitments witness must honour because the parent post asserts
them for the chain as a whole. If witness contradicts any of these, the
overdo citation in AGENTS.md becomes dishonest.

**C1. Evidence must be re-checkable by a small trusted checker (the
Check-It pattern).** The blog commits the chain to this in the next-steps
section:

> "untrusted prover emits a checkable proof certificate, tiny trusted
> checker validates it, only the checker is qualified under DO-330 …
> Building certificate emitters and independent checkers collapses the
> DO-330 problem from 'qualify Z3' (infeasible) to 'qualify a small
> checker' (tractable)."

witness's v1.0 milestone in AGENTS.md already aligns: "Check-It qualification
artifact — emit a checkable attestation; qualify the checker, not the
emitter." This is now a *requirement*, not an aspiration, because the
parent post has committed the whole stack to it.

**C2. Artefacts must be assessor-checkable.** From the space-domain row:
"Fits; artifacts are assessor-checkable." The implication for witness: the
branch manifest and coverage report must be human- and
tooling-readable without witness itself. JSON/YAML stable schemas, not
opaque binaries.

**C3. Semantic preservation under instrumentation is load-bearing.** The
blog elevates translation validation as the DO-333 "under-valued asset".
Witness instruments Wasm, which is a translation. The semantic-preservation
invariant already listed in AGENTS.md (§Invariants-1) is the analogue of
Z3-on-WASM-optimizer translation validation at the coverage layer. Do not
weaken it.

**C4. Traceability is rivet, not witness.** The blog explicitly locates
traceability at the rivet layer ("rivet — traceability across all three").
Witness's v0.3 plan (emit rivet evidence format) is the correct shape.
Witness must not grow its own traceability; it produces evidence that
rivet consumes.

**C5. Determinism.** The blog's chain depends on proof artefacts being
re-runnable. Witness's "reports are deterministic" invariant in AGENTS.md
is the coverage-layer equivalent — same `(module, run-data)` → same report.
Do not let HashMap iteration order leak.

**C6. Scope discipline over feature surface.** The overdo post is a
layering argument, not a feature-race argument. Witness's v0.1→v1.0
roadmap must stay disciplined per AGENTS.md; the overdo stance does not
license scope-creep within witness, it licenses parallel *tools* at the
same chain-layer.

**C7. Honest posture about what isn't cleared.** The blog ends with a
"honest assessment" table including ❌ for abstract interpretation.
Witness's README should carry a similar table showing which invariants are
shipping vs. scaffolded vs. not-yet, not an aspirational feature list.

## Adjacent drafts — status check

Both adjacent drafts referenced in AGENTS.md **do not currently exist**:

- `/Users/r/git/pulseengine/pulseengine.eu/content/blog/2026-04-24-variant-pruning-rust-mcdc.md` — **not found.**
- `/Users/r/git/pulseengine/pulseengine.eu/content/blog/2026-04-25-witness-wasm-mcdc.md` — **not found.**

The `content/blog/` directory contains posts dated through 2026-04-23
(`2026-04-23-spec-driven-development-is-half-the-loop.md`) and the overdo
post dated 2026-04-22. There is no drafts subdirectory; both files are
planned but unwritten.

**What this means for alignment work.**

1. AGENTS.md §"Blog arc" lists them as "(draft)" and "(draft, will be
   updated when v0.1 ships)" — this is now **stale** if it implies text
   exists. Either move them to "(planned)" or write the drafts. Pick one.
2. AGENTS.md references specific content from `2026-04-24-…` ("five layers
   of variant pruning: requirements → cargo features → cfg → type system →
   match arms") and from `2026-04-25-…` (witness as "the tool that turns
   the argument from prose to measurement"). Those sentences in AGENTS.md
   are currently the *canonical* statements of those claims, since no blog
   post backs them yet. Treat them as design commitments, not
   recapitulations.
3. When the drafts get written, they will need to cite the overdo post's
   "MC/DC-for-Rust gap" phrase (from the mutation-testing tooltip) as the
   textual hook, and frame witness as the post-rustc Wasm measurement
   point complementary to Ferrous/DLR Rust-level MC/DC — matching the
   overdo stance AGENTS.md already codifies.

**Claims we'll have to deliver against** (sourced from AGENTS.md because
the drafts don't exist):
- Five-layer variant pruning reduces MC/DC burden to "what one shipped
  variant actually exposes" — witness must be measuring exactly that, not
  the Cartesian product of all feature/cfg variants.
- "Once the module exists, pattern matching has already lowered to
  `br_if` / `br_table`, cfg has elided dead code, and type-state has
  resolved" — witness's v0.1 strict-per-`br_if`/`br_table` coverage is the
  direct embodiment.
- "The tool that turns the argument from prose to measurement" — witness
  announcement must report actual coverage numbers on a shipped
  pulseengine artefact, not a toy example.

**Staleness review of the planned-but-unwritten drafts:** nothing in them
can be stale since no text exists. What *is* stale is AGENTS.md's "(draft)"
labelling. Recommend updating AGENTS.md §"Blog arc" to distinguish
"(planned)" from "(draft, in progress)" once a real file exists.

---

## Alignment checklist (for witness README / announcement)

- [ ] Cite the overdo post by title and permalink.
- [ ] Use the C-macro-post-preprocessor precedent the same way the overdo
      post uses Airbus/Astrée precedent.
- [ ] Frame Ferrous/DLR Rust-level MC/DC as complementary — same chain
      layer, different blind spots. No rivalry language.
- [ ] Include an honest-assessment table (✅ ◐ ❌) of witness invariants,
      not an aspirational feature list.
- [ ] Commit v1.0 explicitly to the Check-It pattern. Qualify the checker,
      not the emitter.
- [ ] Keep traceability language pointed at rivet; witness *emits*
      evidence, does not *link* requirements.
- [ ] Match voice: short sentences, no hype adverbs, first-person singular
      where opinion is offered, footnoted sources tiered primary/secondary.
- [ ] If regulatory clauses are cited, name the clause (DO-178C §6.4.2.2,
      ISO 26262 Part 6 Table 9, EN 50128 Table A.17) and flag the gap (e.g.
      DO-330 qualification, TCL argument) in the same paragraph.
