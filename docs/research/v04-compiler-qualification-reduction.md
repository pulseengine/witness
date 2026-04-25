# Compiler-qualification reduction via the witness/loom/rivet/sigil chain

The converse study to *Overdoing the verification chain*. Research conducted
2026-04-24 for v0.4 design framing.

---

## 1. Executive summary

**Question.** If the pulseengine chain (witness, loom, rivet, sigil) is built
correctly, can it *substitute* for compiler qualification when shipping a
MoonBit program to Wasm?

**Answer for ASIL B.** Mostly yes, conditional on three properties holding —
witness instrumented-coverage is qualified-checker-validated (Check-It,
v1.0), loom's translation-validation output is consumed and joined, and the
test suite exercises every branch witness reports. ISO 26262 Part 8 §11.4.5
permits TCL 1 — no qualification — when downstream activities reach a *high
degree of confidence* (TD1) in detecting tool malfunction. The chain is the
TD1 evidence.

**Answer for DAL B (DO-178C avionics).** Substitution is *weaker*. DO-178C
does not have the TCL-1 escape hatch; instead it relies on per-tool
qualification under DO-330. DAL B requires decision coverage (Annex A
Table A-7 obj. 3) and source-to-object traceability. The chain's evidence
is *credit-bearing* but does not by itself remove the need to qualify the
MoonBit→Wasm compiler under DO-330 TQL-4. For DAL A the gap widens further
— §6.4.4.2.b requires explicit object-code verification of any
compiler-introduced code that is not directly traceable to source.

**Residual qualification surface (lower bound).** Even with full chain
substitution, the small qualified checker (witness v1.0 Check-It artifact),
the loom translation-validation oracle, and witness's own determinism +
reproducible-build properties must be qualified. The MoonBit compiler's
language-frontend correctness (parsing, type-checking, lowering to its
internal IR) sits outside what witness/loom verify and remains a residual.

**Witness-side action items.** Six concrete properties, listed in §7. The
load-bearing ones: instrumented-module sentinel detection, deterministic
output, reproducible builds, and the v1.0 checker.

---

## 2. The TCL framework (ISO 26262 Part 8 §11.4)

The argument's regulatory anchor is ISO 26262 Part 8 §11. The standard is
paywalled; quotes below are taken from credited secondary sources, all
flagged. Sourcing the clauses verbatim from the standard text remains a
TODO before any external publication.

### 2.1 Tool Impact (§11.4.5.2)

ISO 26262-8 §11.4.5.2 defines Tool Impact as

> "the possibility that a malfunction of a particular software tool can
> introduce or fail to detect errors in a safety-related item or element
> being developed."
> (paraphrased via Embitel's vendor explainer; primary clause is paywalled.)

Two values:

- **TI 1** — tool cannot introduce or fail-to-detect errors.
- **TI 2** — tool can introduce or fail-to-detect errors.

A compiler is unambiguously TI 2: the executable artefact is its output,
and a faulty compiler can introduce arbitrary divergence from the source.

### 2.2 Tool Error Detection (§11.4.5.3 / §11.4.5.4)

Three values:

- **TD 1** — *high* degree of confidence that a tool malfunction (and its
  erroneous output) will be prevented or detected by downstream
  verification activities.
- **TD 2** — *medium* degree.
- **TD 3** — selected in all other cases.

The standard explicitly permits "tool-external measures implemented as
part of the software lifecycle (e.g., guidelines, tests, and reviews)" to
count toward TD. (ISO 26262-8 §11.4.5.3, secondary source: Embitel,
heicon-ulm.)

### 2.3 TCL determination (§11.4.5.5)

The matrix:

| TI \ TD | TD 1 | TD 2 | TD 3 |
|---|---|---|---|
| TI 1 | TCL 1 | TCL 1 | TCL 1 |
| TI 2 | TCL 1 | TCL 2 | TCL 3 |

**TCL 1 — no qualification activities required.** The full qualification
weight (§11.4.6 — §11.4.9) only applies for TCL 2/3.

**Implication for the substitution argument.** A TI 2 compiler combined
with TD 1 downstream verification yields TCL 1. The argument under
investigation is: *can the witness/loom/rivet/sigil chain provide TD 1
evidence for the MoonBit compiler*? The standard permits this in
principle. The remaining question is whether the chain actually delivers
high confidence in detection, not whether the framework allows it.

---

## 3. The substitution argument in three load-bearing steps

### Step A — witness measures branch coverage at the artefact

**Claim.** Branch coverage measured on the produced Wasm satisfies ISO
26262-6 Table 12's structural-coverage requirement at ASIL B.

**Evidence supporting.**
- ISO 26262-6 Table 12 lists branch coverage as "highly recommended" (++)
  at ASIL B, C, D (Parasoft secondary; primary paywalled).
- The C-macro precedent: post-preprocessor C MC/DC has been DO-178B/C
  acceptable since 1992. Witness measures post-MoonBit-compiler Wasm; the
  structural analogue holds. (`docs/research/mcdc-bytecode-research.md`
  §2.)
- JaCoCo on JVM bytecode is a shipped, accepted precedent for
  bytecode-level branch coverage. (mcdc-bytecode-research.md §1.1.)

**Evidence against.**
- ISO 26262 expects coverage to be measured *of the source code*. Wasm is
  not the source. The argument relies on the post-preprocessor-C analogy
  holding for an entirely different language frontend.
- TODO: source the DO-178C Annex A clause that explicitly accepts
  post-preprocessor C MC/DC. AGENTS.md asserts the precedent; the
  authoritative clause text in an open document is still missing.

### Step B — loom verifies the optimisation pipeline per-input

**Claim.** loom's Z3 translation validation, when it succeeds for an
input, proves that the optimised Wasm preserves the semantics of the
unoptimised Wasm for that input. Iterating over all test-suite inputs
gives a proof for the test-covered subset.

**Evidence supporting.**
- Translation validation (Pnueli, Siegel, Singerman 1998) is the standard
  formal apparatus and is cited approvingly in DO-333 as a formal-method
  technique that can substitute for testing under soundness +
  completeness conditions.
- The overdo blog explicitly calls this out: "The WASM-IR translation
  validation is the under-valued asset; it is the kind of
  translation-preservation argument DO-333's source-to-object rules were
  written for. Qualification under DO-330 is the work."
  (`docs/research/overdo-alignment.md` §4.)

**Evidence against.**
- Per-input translation validation only covers inputs the test suite
  actually runs. It is a *coverage-conditioned* argument: the
  optimisation is verified for *I ∈ TestSuite*, not ∀I. This is exactly
  why witness's role is load-bearing — coverage decides which inputs got
  the proof.
- loom verifies the *optimiser*, not the *MoonBit→Wasm compiler*. The
  step from MoonBit source to the unoptimised Wasm is *not* covered by
  loom. This is a critical gap — see §6.

### Step C — Check-It validates the chain for the assessor

**Claim.** A small qualified checker (witness v1.0) consumes the
witness coverage attestation, the loom proof certificates, and the
sigil-signed bundle, and produces a single yes/no checkable artefact for
the assessor. The unqualified emitters (witness, loom) emit certificates;
the qualified checker validates them.

**Evidence supporting.**
- The Check-It pattern is a recognised formal-methods qualification
  strategy. Per the overdo post (quoted in `overdo-alignment.md` §6, C1):
  "untrusted prover emits a checkable proof certificate, tiny trusted
  checker validates it, only the checker is qualified under DO-330 …
  Building certificate emitters and independent checkers collapses the
  DO-330 problem from 'qualify Z3' (infeasible) to 'qualify a small
  checker' (tractable)."
- witness's v1.0 milestone in DESIGN.md and AGENTS.md already commits to
  this shape.

**Evidence against.**
- A qualified checker is itself a qualification campaign. The argument
  reduces compiler qualification to checker qualification, but the
  checker is not free — it must itself meet DO-330 / ISO 26262 Part 8
  TCL 3 standards for use at ASIL D. At ASIL B the checker can be TCL 1
  if its own errors are detectable downstream, which is the
  same-shaped argument applied recursively.
- The Check-It pattern works best when the certificate is small and the
  validation is decidable. Coverage attestations are well-suited;
  translation-validation certificates from Z3 are larger and require
  more checker logic.

---

## 4. MoonBit-specific factors

The substitution argument's strength varies with the source language. C
puts a vast undefined-behaviour surface between the source and the
translated artefact; tools like Astrée exist precisely because that
surface is too wide to clear by translation validation alone. MoonBit's
calculation is different.

**Factors that *help* the substitution argument:**

- **Statically typed, no implicit numeric conversions.** Recent MoonBit
  releases have moved toward stricter explicit conversions (string→view
  is one of the few permitted implicits). Compare with C, where
  implicit-conversion bugs are a primary undefined-behaviour source.
  (Source: MoonBit weekly updates, MoonBit `docs.moonbitlang.com`.)
- **Option types and exhaustive pattern matching.** No null pointers; the
  type system forces the absence-case to be handled. Adding an enum
  variant produces a compile error at every match site that doesn't
  cover it. (Source: MoonBit language tour.)
- **Wasm-GC backend.** When MoonBit targets the wasm-gc backend, runtime
  values use Wasm's native GC reference types; the compiler does not
  need to lower into a hand-written runtime layer. The transformation
  surface that loom would have to translation-validate is narrower than
  C/Rust→Wasm via LLVM. (Source: MoonBit FFI docs; WebAssembly GC
  proposal.)
- **No bounds-check elision (in safe MoonBit).** Like Rust, but without
  Rust's `unsafe` escape hatch as a default tool. TODO: source
  MoonBit's `unsafe`/FFI surface size precisely; current evidence says
  it is "experimental" and "should only be used for experimentation."

**Factors that *hurt* the substitution argument:**

- **MoonBit is a new language with no qualification precedent.** Unlike
  Rust (Ferrocene), C (multiple qualified compilers), or Ada
  (SPARK/CompCert-style), MoonBit has no industrial-grade certification
  history. Assessors will be the first to see it for any safety domain.
- **The MoonBit compiler is unqualified and changing rapidly.** Weekly
  releases (`moonbitlang.com/weekly-updates/`) increase the version
  surface witness has to track.
- **MoonBit's IR/lowering is not formally specified.** Unlike Wasm,
  which has a machine-readable spec and a reference test suite, the
  MoonBit→Wasm compiler's intermediate representation is internal and
  may change without language-level versioning. Translation validation
  by loom on the optimiser does not cover the language-frontend
  lowering, and there is no spec to validate against. **This is the
  largest residual.**
- **Multiple backends.** MoonBit targets wasm-gc, JS, and native. The
  substitution argument only holds for the wasm path; ASIL evidence
  collected on Wasm does not transfer to the native backend.

---

## 5. The minimum residual qualification surface

Even if every other step composes correctly, four items must still be
qualified for ASIL B. They are the lower bound.

1. **The witness Check-It checker (v1.0).** The whole argument reduces
   to "qualify the checker, not the emitter." The checker itself is a
   small piece of code (target: a few thousand lines of Rust per
   AGENTS.md / DESIGN.md), and qualifying it under DO-330 / ISO 26262
   Part 8 TCL 3 is the explicit v1.0 deliverable. Estimated effort: a
   single qualification campaign per major version of the checker.
2. **The loom translation-validation oracle.** loom emits proof
   certificates; the certificates themselves do not need a qualified
   prover, but the *certificate format* and *checker* do. This is the
   same argument as item 1 applied to loom's output. (See the overdo
   blog quote in `overdo-alignment.md` §4: "Qualification under DO-330
   is the work.")
3. **Witness's own determinism + reproducibility properties.** If
   witness is non-deterministic, the same `(module, run-data)` produces
   different reports, the chain breaks. If witness's own build is
   non-reproducible, an assessor cannot replay the evidence.
   AGENTS.md §Invariants 3 already locks this; the implication for v0.4
   is that witness must include its own build hash in its emitted
   evidence and refuse to run if the binary disagrees.
4. **The Wasm reference interpreter (or a qualified equivalent).** The
   semantic-preservation invariant (DESIGN.md §Invariants 1) is verified
   "by round-trip testing against the wasm-tools reference interpreter."
   That interpreter is currently unqualified. Either: (a) qualify it,
   (b) replace it with a qualified Wasm runtime (kiln's qualification
   path is the natural candidate), or (c) ship the round-trip evidence
   with the explicit caveat and let the assessor accept it as
   experience-from-use.

**Items not on the residual list (because the chain covers them):**

- Compiler optimisation correctness — covered by loom translation
  validation (per-input, conditional on test coverage).
- Branch-coverage measurement — covered by witness instrumentation
  (verified by the Check-It checker).
- Requirement-to-test traceability — covered by rivet
  (`docs/research/rivet-evidence-consumer.md`).
- Evidence integrity — covered by sigil DSSE bundles
  (`docs/research/sigil-predicate-format.md`).

---

## 6. Honest counter-argument — when substitution breaks

The substitution argument has three concrete failure modes. None of these
are theoretical; they are the load-bearing reasons the overdo stance
exists in the first place.

### 6.1 Bugs that affect the test harness identically

If the MoonBit compiler miscompiles in a way that changes the
test-harness's behaviour the same way it changes the system-under-test's
behaviour, the test passes, witness reports the branch as covered, loom's
translation validation succeeds (the optimiser is correct on the
unoptimised-but-already-wrong Wasm), and the chain emits a green
attestation for incorrect software.

Example: a miscompiled comparison operator. Both `==` in the test and
`==` in the SUT use the same compiler intrinsic; if the intrinsic is
broken, both broken in the same way.

**Defence.** Independent test oracles. Where possible, the test-harness
arithmetic and comparisons should be done in the host runtime (Rust on
the wasmtime side), not inside the Wasm module. This pushes some test
logic outside the compromised compiler. Documented as v0.4 action item.

### 6.2 Bugs in instructions witness does not instrument

Witness instruments `br_if`, `if-else`, `br_table`. It does not
instrument arithmetic, memory access, or floating-point operations. A
miscompiled `f64.div` produces no branch-coverage evidence either way —
the bug is invisible to witness.

**Defence.** Out of scope for witness alone. The overdo stance addresses
this by *adding* sanitisers and proptest at adjacent layers; the
substitution argument cannot resolve it. Document the limit explicitly:
*witness's TD evidence covers control flow, not data flow*.

### 6.3 Bugs in the MoonBit→Wasm lowering the loom optimiser does not see

loom validates Wasm-to-Wasm transformations. It does not validate
MoonBit-to-Wasm lowering. A miscompilation in the MoonBit compiler's
frontend (typing → IR → unoptimised Wasm) is not reachable by loom's Z3
translation validation. If witness and the test suite happen to cover
the affected branches with inputs that don't trigger the miscompilation,
the bug ships.

This is the most serious gap. The substitution argument *requires*
something to verify the MoonBit→Wasm step. Candidates:

- A qualified MoonBit compiler (defeats the whole purpose; this is
  Ferrocene's path).
- Per-input MoonBit-to-Wasm translation validation. Practically
  difficult — MoonBit has no formal semantics yet. Open research
  question.
- Differential testing across MoonBit backends (wasm-gc vs JS vs
  native). Catches divergence but does not catch consistent
  miscompilations.
- Property-based testing on MoonBit programs with cross-checked
  reference implementations. Best near-term option; folds into the
  overdo stance's proptest layer rather than the substitution argument.

**Honest verdict.** For ASIL B the substitution argument requires
explicit acknowledgement of this gap as a residual TI 2 / TD 2 (medium
detection) item. It does not vanish; it just becomes manageable when the
test suite + MoonBit's small surface make a TD 1 → TD 2 step tolerable
under §11.4.5.5's "TI 2 + TD 2 = TCL 2" — which then *does* require
qualification activities.

---

## 7. Witness-side action items for v0.4

The substitution argument imposes specific obligations on witness that
are not v0.1—v0.3 deliverables. Each of these is necessary; together
they may be sufficient (with the §5 residuals).

1. **Instrumented-module sentinel.** Witness must emit a tamper-evident
   sentinel (e.g. a known-counter+known-runs invariant) that the
   Check-It checker validates. Without it, the chain cannot detect a
   downstream agent that quietly modified the instrumented module
   between instrumentation and execution.
2. **Deterministic, reproducible witness binary.** AGENTS.md
   §Invariants 3 covers report determinism. v0.4 extends this to the
   witness binary itself: same source → same binary, byte-for-byte. The
   evidence must include witness's build hash.
3. **loom evidence consumer.** v0.4 of witness consumes loom's
   translation-validation output and joins it with coverage evidence.
   The schema is part of v0.4 (DESIGN.md roadmap).
4. **Coverage-lifting soundness theorem in machine-checkable form.**
   v0.2 ships the prose; v0.4 should ship the proof in a form (Verus,
   Coq, or similar) that the Check-It checker can validate. Without
   this, the lifting is an unproven assumption inside the chain.
5. **Per-target `br_table` counting and DWARF reconstruction (already
   v0.2, must not regress).** The substitution argument needs accurate
   pattern-match coverage; if `br_table` defaults to "executed once" on
   one of N arms, MoonBit's heavy use of pattern matching defeats the
   coverage claim.
6. **Honest-assessment table in every emitted attestation.** Per
   `docs/research/overdo-alignment.md` C7. The attestation must list
   what the chain does and does not cover (control flow yes, data flow
   no, frontend lowering no). Assessors must see the residuals
   explicitly.

---

## 8. References

- ISO 26262-8:2018, Part 8 §11.4.5 — Tool Confidence Level determination.
  Standard text paywalled. Secondary sources used:
  - [Embitel: Software Tool Qualification in ISO 26262 Development](https://www.embitel.com/blog/embedded-blog/why-is-software-tool-qualification-indispensable-in-iso-26262-based-software-development)
  - [BTC Embedded: ISO 26262 tool qualification — When and how to perform it](https://www.btc-embedded.com/when-and-how-to-qualify-tools-according-to-iso-26262/)
  - [HEICON Ulm: ISO 26262 confidence in the use of software tools](https://heicon-ulm.de/en/iso-26262-confidence-in-the-use-of-softwar-tools-a-feasible-strategy/)
  - [Reactive Systems Reactis Safety Manual — Tool Classification](https://reactive-systems.com/reactis-safety-manual/tool-classification.html)
- ISO 26262-6:2018 Table 12 — structural coverage at the unit level.
  Secondary: [Parasoft: Code Coverage — ISO 26262 Software Compliance](https://www.parasoft.com/learning-center/iso-26262/code-coverage/).
- DO-178C §6.4.4.2.b — object code verification at DAL A. Secondary:
  [Rapita Systems: Does DO-178C require object code structural coverage?](https://www.rapitasystems.com/blog/does-do-178c-require-object-code-structural-coverage)
  / [LDRA: Source-to-object code traceability](https://ldra.com/capabilities/object-code-verification/).
- DO-178C §12.2 / DO-330 — tool qualification framework. Secondary:
  [AFuzion: DO-330 Introduction — Tool Qualification](https://afuzion.com/do-330-introduction-tool-qualification/).
- DO-178C Annex A Tables A-7 / A-2 — structural coverage objectives by
  DAL. Secondary: [TheCloudStrap: DO-178C Objectives List](https://thecloudstrap.com/do-178c-objectives-list/).
- DO-333 — Formal Methods Supplement. Secondary references via the
  overdo blog (see `docs/research/overdo-alignment.md` §4).
- Pnueli, Siegel, Singerman (1998) — *Translation Validation*. Standard
  reference for the technique. Cited in
  `docs/research/mcdc-bytecode-research.md` §3.
- CompCert — directly qualified C compiler with Coq-proven correctness.
  TODO: source the specific DO-178C qualification credit CompCert
  carries; the search returned vendor and conference references but no
  primary qualification dossier.
- Ferrocene — qualified Rust toolchain (ISO 26262 ASIL D, IEC 61508
  SIL 4, IEC 62304 Class C). Path is *direct qualification*, not
  substitution.
  - [Ferrous Systems: Officially Qualified — Ferrocene](https://ferrous-systems.com/blog/officially-qualified-ferrocene/)
  - [Ferrocene public docs](https://public-docs.ferrocene.dev/main/index.html)
- "Sealed Rust" / Ferrous-Systems — pre-Ferrocene plan for Rust in
  safety-critical. [`ferrous-systems/sealed-rust`](https://github.com/ferrous-systems/sealed-rust)
  / [Sealed Rust the Pitch](https://ferrous-systems.com/blog/sealed-rust-the-pitch/).
- MoonBit language and toolchain.
  - [MoonBit Language Tour](https://tour.moonbitlang.com/)
  - [MoonBit FFI docs](https://docs.moonbitlang.com/en/latest/language/ffi.html)
  - [Introduction to MoonBit — The New Stack](https://thenewstack.io/introduction-to-moonbit-a-new-language-toolchain-for-wasm/)
  - [Dancing with LLVM: A MoonBit Chronicle](https://www.moonbitlang.com/pearls/moonbit-and-llvm-1)
- Astrée — Airbus DO-178B/C abstract-interpretation precedent. Cited as
  the "named precedent" template in the overdo blog.
  - [Astrée — Wikipedia](https://en.wikipedia.org/wiki/Astr%C3%A9e_(static_analysis))
  - [Experimental Assessment of Astrée on Safety-Critical Avionics Software](https://www.astree.ens.fr/papers/astree_airbus_safecomp2007.pdf)
- Local repo cross-references:
  - `/Users/r/git/pulseengine/witness/AGENTS.md` — project framing,
    invariants.
  - `/Users/r/git/pulseengine/witness/DESIGN.md` — roadmap, v0.4 / v1.0
    goals, the Check-It commitment.
  - `/Users/r/git/pulseengine/witness/docs/research/overdo-alignment.md`
    — the parent argument the substitution stance is bracketing.
  - `/Users/r/git/pulseengine/witness/docs/research/mcdc-bytecode-research.md`
    — JaCoCo precedent, post-preprocessor C precedent, rustc-mcdc
    6-condition cap.
- TODO sources to chase before external publication:
  - DO-178C Annex A clause text accepting post-preprocessor C MC/DC.
  - CompCert's specific DO-178C qualification credit (primary).
  - ISO 26262-8 §11.4.5 clause text verbatim (primary; currently only
    secondary).
  - MoonBit `unsafe` / FFI surface — formal scope and frequency-of-use
    in shipped MoonBit code.
