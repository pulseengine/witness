# v0.9 — state of the art and the agent UX argument

> Research conducted 2026-04-25. Draft. Decisions taken in this brief
> become rivet artefacts (REQ / FEAT / DEC) when v0.9 is unsealed; until
> then they are positioning input.

This brief answers a single question: when v0.9 ships, what does witness
do that nothing else on the market does, and why does that combination
matter to both AI agents and human reviewers? It surveys the
state-of-the-art MC/DC tooling, sketches the agent UX (MCP server, tool
contract, autonomous gap-closing loop), sketches the human UX
(PR-time truth-table review, V-model click-through, tutorial-style gap
explanations), and lists ten differentiating features. The pricing
section is strategic input only; the user makes the call.

The roadmap that this brief assumes:

| Version | What ships |
|---|---|
| v0.5 (shipped) | branch coverage with workspace split, LCOV, attest |
| v0.6 (in flight) | DWARF-grounded MC/DC truth tables; fallback to strict per-`br_if` |
| v0.7 (planned) | Scale to a real Rust application; performance and ergonomics |
| v0.8 (planned) | Visualisation: HTML reports, dashboards, truth-table widget |
| **v0.9 (destination)** | **Complete MC/DC for a high-complex Rust app; agent UX; human UX; superior to LDRA / VectorCAST / Cantata / Bullseye / Squore / gcov+gcovr** |
| v1.0 | Check-It qualification artefact |

## 1. The five-sentence v0.9 positioning

1. **Witness v0.9 is the first MC/DC coverage tool whose evidence chain
   is cryptographically signed end-to-end and consumable both by humans
   and by AI agents through a Model Context Protocol server**, with every
   gap explanation traced back to a requirement in the V-model and every
   gap-closing test attributed to its author (human or agent).
2. It measures MC/DC at the WebAssembly bytecode level with **no
   condition-count cap** — covering decisions LLVM-based tools (Clang,
   rustc, cargo-llvm-cov) cannot instrument because of the 6-condition
   bitmap-encoder limit — and projects the result soundly back to source
   under a stated DWARF-correctness assumption (the coverage-lifting
   argument).
3. Where LDRA, VectorCAST, Cantata, RapiCover and Squore charge
   five-figure-per-seat licences and ship qualification kits as separate
   line items, witness ships **dual-licensed Apache-OR-MIT, free for the
   CLI and the agent integration**, with the qualification artefact
   (Check-It pattern, v1.0) designed so the small trusted checker can
   itself be qualified under DO-330.
4. The reviewer experience is **the truth table, not the percentage**:
   the PR check renders the MC/DC truth table for every changed
   decision, highlights the missing-witness rows in tutorial style
   ("to prove `b` independently affects this decision you need a test
   where `(a=T, b=F, c=T)` produces the opposite outcome…"), and
   cross-links to the rivet REQ / FEAT / DEC chain.
5. The agent experience is **a contract, not a chatbot**: the MCP
   server exposes `get_decision_truth_table`, `find_missing_witness`,
   `propose_test_row_to_close_gap`, and the witness re-run verifies the
   agent-authored test actually closed the gap and signs the result with
   the agent's identity in the rivet evidence chain.

## 2. State-of-the-art feature matrix

Caveat: this matrix synthesises vendor-page claims and recent literature
([RapitaSystems on RapiCover MC/DC](https://www.rapitasystems.com/products/rapicover),
[LDRA MC/DC capability page](https://ldra.com/capabilities/mc-dc/),
[VectorCAST/QA factsheet](https://cdn.vector.com/cms/content/products/VectorCAST/Docs/Datasheets/English/VectorCAST_QA_Factsheet_EN.pdf),
[QA Systems Cantata blog](https://www.qa-systems.com/blog/mc-dc-coverage-a-critical-technique/),
[Bullseye features](https://www.bullseye.com/product.html),
[Squore via Vector](https://vector-softwarequality.com/static-code-analysis/squore),
[gcov MC/DC arxiv 2501.02133](https://arxiv.org/abs/2501.02133),
[rustc MC/DC tracking](https://github.com/rust-lang/rust/issues/124144),
[MoonBit coverage docs](https://docs.moonbitlang.com/en/latest/toolchain/moon/coverage.html)).
Numbers in italics are vendor claims, not independently verified.

| Capability | LDRA Testbed | VectorCAST | Cantata | RapiCover | Bullseye | Squore | gcov-14 + gcovr | cargo-llvm-cov | tarpaulin | MoonBit | **witness v0.9** |
|---|---|---|---|---|---|---|---|---|---|---|---|
| **MC/DC support** | yes | yes | yes | yes | C/D only, no MC/DC | aggregator | yes (masking BDD) | yes (unstable, nightly) | branch only | branch only | **yes (DWARF reconstruction at Wasm)** |
| **Condition-count cap** | n/a | n/a | n/a | _1000_ | n/a | n/a | n/a (BDD) | **6** | n/a | n/a | **none — no encoder constraint** |
| **Languages** | C/C++/Ada/Java/etc | C/C++/Ada | C/C++ | C/C++/Ada | C/C++/C# | aggregator | C/C++/D/Rust | Rust | Rust | MoonBit | **any source compiled to wasm32 with DWARF (Rust today; C/C++/AssemblyScript/Zig as DWARF emitters mature)** |
| **Measurement point** | source via instrumentation | source via instrumentation | source via instrumentation | source via instrumentation | source via instrumentation | aggregator | LLVM IR | LLVM IR (rustc lowers) | ptrace | source | **Wasm bytecode (post-rustc, post-LLVM, post-loom-fusion in v0.6+)** |
| **Open source** | no, commercial | no, commercial | no, commercial | no, commercial | no, commercial | no, commercial | yes (GPL) | yes (Apache+MIT) | yes (MIT) | yes (Apache 2) | **yes (Apache-OR-MIT)** |
| **Tool qualification** | DO-178C qualification kit, separate SKU | qualification kit, separate SKU | qualification kit, separate SKU | qualification kit, separate SKU | qualification kit, separate SKU | n/a | none | none | none | none | **Check-It pattern at v1.0 — qualify the small checker, not the large emitter** |
| **DSSE-signed evidence** | no | no | no | no | no | no | no | no | no | no | **yes, via sigil + in-toto coverage predicate** |
| **V-model traceability** | partial via TBmanager | partial via VectorCAST manager | external | external | none | yes (its core value) | none | none | none | none | **yes, native via rivet** |
| **MCP server / agent API** | none | none | none | none | none | none | none | none | none | none | **yes** |
| **PR-time truth-table review** | no (desktop GUI) | no (desktop GUI) | no (desktop GUI) | no (desktop GUI) | no | no | no | no | no | no | **yes (GitHub check + VS Code extension)** |
| **Tutorial-style gap explanation** | no | no | no | partial ("highlights missing vectors") | no | no | no | no | no | no | **yes — the "to prove condition X you need test row Y" pattern** |
| **Pricing** | _five figures per seat per year, plus qual kit_ | _five figures per seat per year, plus qual kit_ | _five figures per seat per year, plus qual kit_ | _five figures per seat per year, plus qual kit_ | _$1k–$3k per seat_ | _five-figure tier with Vector_ | free | free | free | free | **free (CLI + agent); paid hosted dashboard considered, see §6** |

**Where witness can be superior** — the specific feature, the specific
why-we-win:

- **No condition-count cap.** Clang and rustc cap MC/DC at 6 conditions
  per decision because LLVM encodes condition combinations as integers
  in `[0, 2^N)` against a coverage bitmap
  ([MaskRay's writeup](https://maskray.me/blog/2024-01-28-mc-dc-and-compiler-implementations)).
  Witness emits one mutable Wasm global per branch — there is no
  encoder. RapiCover claims 1000 conditions per decision but charges
  for it; gcov-14's BDD approach is unbounded but lives only in C/C++/D
  and the experimental Rust path. Witness, on Rust-via-Wasm, is
  unbounded *and* free.
- **Cryptographically signed evidence.** No commercial MC/DC tool today
  ships DSSE-signed in-toto attestations as their default report
  format. Witness already has the predicate format
  (`https://pulseengine.eu/witness-coverage/v1`), the DSSE envelope path
  via wsc-attestation, and a sigil bundle reader that consumes the
  predicate opaquely. v0.9 adds the rivet-evidence trace from REQ
  through DEC to the signed predicate and back.
- **Bytecode measurement point survives the optimiser.** Source-level
  MC/DC (every commercial tool, plus Clang/rustc) instruments before
  LLVM optimises. If `LLVM` constant-folds a branch away or the linker
  inlines a function, the source-level instrumentation either lies
  ("uncovered" branches that can't run) or is silently elided. Witness
  measures *what actually runs*. This is the same argument JaCoCo wins
  against source-level Java coverage tools, generalised to Wasm.
- **Agent-native API.** Parasoft has shipped an MCP server for C/C++test
  CT in March 2026
  ([Parasoft MCP announcement](https://www.parasoft.com/news/c-cpp-test-automation-certified-googletest-agentic-ai/)).
  No other MC/DC tool has. Witness ships an MCP server with first-class
  tool calls for *MC/DC-specific* operations (truth tables, missing
  witnesses, gap-closing test rows) — not general-purpose "give me the
  coverage report" calls.
- **PR-time human UX.** Every commercial tool (LDRA, VectorCAST,
  Cantata, RapiCover) is a desktop GUI. SonarQube and Codecov ship PR
  comments but show only line/branch percentages. Witness can be the
  first tool to render the actual MC/DC truth table inline in a GitHub
  pull request and explain the missing rows in tutorial form.

**Where we will not beat the incumbents:**

- LDRA, VectorCAST, Cantata, RapiCover all have **20+ years of certified
  audit trails** and reference customers in DAL A avionics. Witness
  must compete on capability and UX, not on ecosystem maturity.
- LDRA's **multi-language coverage** (C, C++, Ada, Java, assembly) is
  the advantage of being old. Witness covers only what compiles to
  Wasm with DWARF — Rust today, C/C++/Zig/AssemblyScript later.
- VectorCAST's **CBA (Coverage By Analysis)** lets engineers mark code
  sections "covered by review, not test." Witness has no equivalent
  today and probably should not invent one before v1.0.

## 3. AI-agent UX design

### 3.1 The contract, not the chatbot

The framing matters. Existing "AI testing" products
([Parasoft](https://www.parasoft.com/blog/ai-agents-mcp-servers-software-quality/),
[TestSprite](https://www.testsprite.com/),
[AccelQ autonomous testing](https://www.accelq.com/blog/autonomous-testing/),
[Tricentis agentic guide](https://www.tricentis.com/learn/agentic-testing))
phrase the agent as a generator: "the agent writes tests for you." That
framing leaves verification on the floor. Witness's framing inverts:
the agent is a *gap-closer under verification*.

The contract:

1. Witness reports a gap. (Decision D, condition `b`, missing witness
   row `(a=T, b=F, c=T)`.)
2. Agent proposes a test (or change to an existing test) that intends
   to produce that row.
3. Witness re-runs the harness with the proposed test included.
4. Witness verifies the row was actually produced and the gap closed.
5. The signed evidence records *the agent's identity* and the
   gap-closure proof, not just "test passed."

This is verifiable agentic coverage. It is the same shape as Parasoft's
MCP server flow but with three differences witness can claim as
superior: (a) the verification loop is mechanical, not LLM-judged; (b)
the gap target is MC/DC condition-level, not branch percentage; (c) the
attribution is signed and ends up in the rivet V-model graph.

### 3.2 The MCP server tool surface

Tool calls (proposed; will be specified in `wit/witness-mcp.wit` and
`docs/research/v09-mcp-tool-surface.md` when v0.9 unseals):

```
get_module_summary(module_path) -> {
    decisions: [DecisionSummary],
    coverage_overall: f32,
    rivet_artefacts: [{ id, type, status }],
}

get_decision_truth_table(decision_id) -> {
    conditions: [ConditionMeta],
    rows: [{ assignment, decision_outcome, hit_count, witnessed_by_test: Option<TestId> }],
    mcdc_pairs: [{ condition_id, true_row, false_row, satisfied: bool }],
    source_location: SourceLocation,
    rivet_links: [ArtefactRef],
}

find_missing_witness(decision_id, condition_id) -> {
    needed_row: Option<RowAssignment>,    // None if MC/DC already met
    paired_row_present: Option<RowAssignment>,
    rationale: String,                    // human-readable tutorial text
    suggested_test_shape: TestSkeleton,   // function under test, inputs needed
}

propose_test_row_to_close_gap(decision_id, condition_id, test_source: String) -> {
    accepted: bool,
    will_run_in: Duration,                // estimate
    queue_id: QueueId,
}

verify_gap_closed(queue_id) -> {
    decision_id, condition_id,
    closed: bool,
    new_truth_table: TruthTable,
    signed_envelope: DsseEnvelope,        // if closed
    failure_reason: Option<String>,       // if not
}

list_uncovered_conditions(filter: { module?, function?, requirement? })
    -> [{ decision_id, condition_id, priority: enum, rivet_links }]

attribute_test_to_agent(test_id, agent_identity: AgentId) -> ()
```

A typical loop, on a hypothetical decision in a Rust function `validate`:

```
agent: list_uncovered_conditions({ module: "auth.wasm" })
witness: [{ decision_id: "auth.rs::42::DEC-1", condition_id: "c2",
            rivet_links: ["REQ-014"] }]

agent: get_decision_truth_table("auth.rs::42::DEC-1")
witness: { conditions: [c0, c1, c2, c3], rows: [
            { (T,T,F,T), Pass, hits: 12, by: "tests::auth::happy_path" },
            { (T,T,T,T), Pass, hits: 7,  by: "tests::auth::admin" },
            { (F,_,_,_), Fail, hits: 3,  by: "tests::auth::no_user" },
            ... ],
          mcdc_pairs: [{ c2, true_row: needed, false_row: (T,T,F,T), satisfied: false }] }

agent: find_missing_witness("auth.rs::42::DEC-1", "c2")
witness: { needed_row: (T,T,T,T), paired_row_present: (T,T,F,T),
           rationale: "To prove condition c2 (the `is_admin` flag)
                       independently affects the validate decision, you
                       need a test where (user=valid, role=member,
                       is_admin=true, has_2fa=true) produces Pass *and*
                       a test where (user=valid, role=member,
                       is_admin=false, has_2fa=true) also produces Pass.
                       The second row exists (tests::auth::happy_path);
                       the first does not.",
           suggested_test_shape: TestSkeleton {
               fn_under_test: "validate",
               inputs_needed: { user: "valid", role: "member",
                                is_admin: true, has_2fa: true },
               expected_outcome: Pass } }

agent: propose_test_row_to_close_gap("auth.rs::42::DEC-1", "c2",
       "#[test]\nfn admin_member_with_2fa() {\n    let u = User::new(...);\n    assert!(validate(&u));\n}")
witness: { accepted: true, queue_id: q42 }

[witness re-runs the harness, re-reads counters, re-checks MC/DC]

agent: verify_gap_closed(q42)
witness: { closed: true, new_truth_table: ...,
           signed_envelope: <DSSE bytes attesting to the new coverage>,
           failure_reason: None }

agent: attribute_test_to_agent("admin_member_with_2fa", "claude-opus-4-7")
```

### 3.3 V-model traceability for agent contributions

When an agent closes a gap, the rivet graph gains:

- A new `test` artefact (`TEST-NNN`), `status: approved`, with a
  `created_by: agent` field and a signed identity.
- A `verifies` link from `TEST-NNN` to the requirement that owned the
  decision (`REQ-014` in the example).
- A `traces-to` link from `TEST-NNN` to the decision id
  (`auth.rs::42::DEC-1`) recorded in the manifest.
- A signed `witness-coverage/v1` predicate enumerating the new truth
  table and the closed MC/DC pair, included in the next sigil bundle.

This is the difference between "an agent wrote a test" (today's
agentic-AI testing pitch) and "an agent contributed verifiable evidence
that a regulated requirement is now MC/DC-covered, signed by the
agent's identity, recorded in the V-model." The latter is what witness
v0.9 ships.

### 3.4 The autonomous loop in CI

In a CI pipeline, the loop becomes fully unattended:

```
on: pull_request

steps:
  - witness instrument app.wasm
  - witness run  --harness "cargo test"
  - witness report --format json --diff-against origin/main
  - if uncovered_conditions.len() > 0:
      - agent: claude-code with witness-mcp configured
      - agent.run("close all uncovered conditions in this PR diff")
      - if all closed: agent commits the new tests, witness signs, PR
        check turns green
      - if not all closed: PR check fails with the unclosable list and
        a tutorial message for the human reviewer
  - witness attest > coverage.dsse
  - sigil bundle add coverage.dsse
```

Critical safeguard: agents propose tests, witness verifies. The agent
cannot mark a gap closed; only the re-run can. The signed envelope is
the only artefact a downstream consumer trusts.

## 4. Human UX design

### 4.1 PR-time review — the truth table, not the percentage

Today's PR comment from Codecov/SonarQube:

> Coverage decreased from 85.4% to 85.1% (-0.3%). 4 new uncovered lines.

What witness v0.9 posts on a PR:

```
witness — coverage diff for PR #142

Decisions changed: 3
  - auth.rs::42 — validate
  - auth.rs::78 — check_2fa
  - session.rs::15 — refresh

auth.rs::42 — validate (4 conditions, MC/DC required)
  truth table:
    +---+---+---+---+--------+---------------------------+
    | u | r | a |2fa| result | witnessed by              |
    +---+---+---+---+--------+---------------------------+
    | T | T | T | T | Pass   | tests::auth::admin        |
    | T | T | F | T | Pass   | tests::auth::happy_path   |
    | T | T | T | F |   ?    | (no test)                 |  <-- missing
    | F | * | * | * | Fail   | tests::auth::no_user      |
    +---+---+---+---+--------+---------------------------+

  MC/DC status: 3/4 conditions independently witnessed
    c0 (u)   ✓   pair: (T,_,_,_) vs (F,_,_,_)
    c1 (r)   ✓   pair: tests::auth::* covers role variation
    c2 (a)   ✓   pair: admin vs happy_path
    c3 (2fa) ✗   missing the (T,T,T,F) row

  To close the gap on c3:
    Add a test that exercises validate(u=T, r=T, a=T, 2fa=F) and
    asserts the outcome differs from the (T,T,T,T) row currently
    covered by tests::auth::admin. Without this row, we cannot prove
    that the 2fa flag independently affects the validate decision.

  Rivet V-model trace:
    REQ-014  Authentication requires 2FA for admin actions
    └─ FEAT-022  Admin gating in validate()
        └─ DEC-019  validate() decision logic
            └─ auth.rs::42 (this decision)

  [view in dashboard] [view in editor] [ask agent to close gap]
```

The "ask agent to close gap" link triggers the §3.2 MCP loop in a
side-channel CI run, posts the agent-authored test as a suggested
commit, and updates the PR comment when the re-run signs the new
envelope.

### 4.2 V-model click-through

Witness v0.9 ships a small dashboard (extending v0.8's visualisation)
where the navigation is:

```
[REQ-014] Authentication requires 2FA for admin actions
   ↓ refined-by
[FEAT-022] Admin gating in validate()
   ↓ implemented-by
[DEC-019] validate() decision logic
   ↓ traces-to
[auth.rs::42 — validate decision]
   ↓ has
[Truth table | Conditions | Missing rows | Tests | Signed envelopes]
   ↓
[<DSSE bytes>]
[Verify signature] [Show signer identity] [Show Rekor entry]
```

Every leaf is a hash that lives in a sigil bundle. Every internal node
is a rivet artefact that lives in `artifacts/*.yaml`. The dashboard is
a thin renderer over the two; witness produces no state of its own.

### 4.3 Editor integration

A VS Code extension (`vscode-witness`) that:

- Renders gutter highlights for covered / uncovered branches,
  distinguishing "covered" from "covered but MC/DC-unwitnessed."
- Hover on a decision shows the truth table inline.
- "Quick fix" code action: "Add test to close MC/DC gap on condition X
  of this decision." Triggers the §3.2 MCP loop, opens the agent's
  proposed test in a diff view, runs the verification, and offers a
  commit.
- Status-bar widget: "validate(): MC/DC 3/4 conditions witnessed."

The extension is a thin client over the MCP server, so it works against
any agent (Claude, Cursor, Continue, GitHub Copilot Chat) that speaks
MCP. Differentiation: this is the only MC/DC editor experience on the
market — the existing tools (LDRA, VectorCAST, Cantata, RapiCover) are
desktop GUIs that are not editor-integrated.

### 4.4 Tutorial-style gap explanations

The example in §4.1 demonstrates the pattern. The principle: every gap
report answers two questions in plain English:

1. **What test do I need to add?** (concrete row assignment, concrete
   inputs)
2. **Why this specific row?** (the MC/DC pair argument: this row vs
   that row, with this condition flipped, with everything else held)

Both questions are easy to answer mechanically given the truth table
and the manifest. Neither is answered by any commercial MC/DC tool
today; RapiCover comes closest with "highlight the missing vectors" but
does not produce the prose argument or the V-model link. This is
where v0.9 wins on UX.

## 5. The ten superiority features

The list that justifies the five-sentence positioning. Each is a
concrete v0.9 deliverable; each maps to a rivet REQ when v0.9 unseals.

1. **No condition-count cap on MC/DC decisions.** Clang and rustc cap
   at 6, RapiCover at 1000. Witness has no encoder constraint.
2. **DSSE-signed coverage evidence end-to-end** via sigil + in-toto.
   No other MC/DC tool ships signed reports as the default format.
3. **MCP server with MC/DC-native tool calls.** Not "give me the
   coverage report" but `find_missing_witness`,
   `propose_test_row_to_close_gap`, `verify_gap_closed`.
4. **Agent attribution in the V-model.** Tests authored by an agent are
   linked to the agent's signed identity in the rivet graph.
5. **Verifiable autonomous gap closure.** Witness re-runs the harness
   to verify the gap is closed; agents cannot mark themselves done.
6. **Tutorial-style gap explanations.** Every uncovered condition gets
   a paragraph answering "what test do I need" and "why this row."
7. **PR-time truth-table review.** Reviewers see the actual MC/DC
   truth table inline in the GitHub PR, not a percentage.
8. **V-model click-through dashboard.** REQ → FEAT → DEC → decision →
   conditions → rows → signed envelopes, with every node hashed.
9. **VS Code extension** with gutter highlights distinguishing "covered"
   from "covered but MC/DC-unwitnessed," and a "close gap" code action.
10. **Free under Apache-OR-MIT.** Free CLI, free agent integration, no
    qualification-kit upcharge. The qualification artefact at v1.0
    qualifies the small Check-It checker, not the emitter.

## 6. Pricing and distribution thoughts

**Strategic input only.** The user makes the call. Three plausible
models, with risk/reward:

### 6.1 Free everything (Apache-OR-MIT throughout)

What it looks like: CLI, MCP server, VS Code extension, dashboard, all
open source. No commercial tier.

- **Pro:** maximum adoption, alignment with rivet/sigil/loom/meld
  ecosystem licensing, the position witness already ships under.
- **Pro:** every blog post about "MC/DC for AI-authored Rust" lands
  with a tool the reader can install and run today.
- **Con:** no funding mechanism for the qualification artefact at
  v1.0, which is real engineering work (potentially DO-330-grade).
- **Con:** no funding for hosted dashboard ops if the dashboard goes
  beyond a static HTML report.

### 6.2 Open-core: free CLI, paid hosted dashboard / agent-as-a-service

What it looks like: CLI and MCP server stay Apache-OR-MIT and free.
The hosted dashboard (where teams view their coverage history,
multi-repo aggregates, agent-authored test review queues) is a paid
SaaS. The agent integration in CI (where witness orchestrates the
agent loop unattended) is metered: free tier for OSS projects,
per-team-per-month for commercial.

- **Pro:** funds the v1.0 qualification work and the ongoing dashboard
  ops without compromising the OSS story.
- **Pro:** matches the Sentry / Codecov / Sonar shape that the market
  already understands.
- **Con:** the line between "free CLI" and "paid hosted" needs careful
  drawing; if the agent loop is the killer feature, hiding it behind a
  paywall undermines the positioning.

### 6.3 Free for non-commercial, paid for commercial / DAL A use

What it looks like: dual-licence. Apache-OR-MIT for non-commercial
and individual use; commercial-friendly ("you are deploying to a
paying customer") triggers a reasonable per-seat or per-deployment
licence. Qualification kit included.

- **Pro:** mirrors the LDRA / VectorCAST / Cantata model the regulated
  industry already understands and budgets for. Funds qualification
  and certified support.
- **Con:** breaks the PulseEngine arc's open-source commitment. The
  blog posts pitch witness as the open alternative; a commercial
  upcharge contradicts that.
- **Con:** the dual-licence verification machinery (who is
  "commercial") is genuine ongoing legal cost.

### 6.4 Recommendation (strategic input)

**Option 6.2.** Keep the CLI, MCP server, and VS Code extension free
under Apache-OR-MIT. Run a small hosted dashboard as a paid SaaS for
teams that want the V-model click-through across repos. Offer the
qualification kit at v1.0 as a paid line item with a clear support
SLA. This funds the qualification work without breaking the open-core
story — the same shape Sentry, Codecov, and Snyk used to grow without
losing developer trust.

The user makes the call. This brief flags the decision as v0.9-blocking
because the dashboard scope (paid vs free) drives the v0.8 work.

## 7. Provisional rivet artefacts for v0.9

These are draft IDs that should land in
`artifacts/v09-requirements.yaml`,
`artifacts/v09-features.yaml`,
`artifacts/v09-decisions.yaml` when v0.9 unseals. They are *not*
canonical; they are this brief's draft proposal.

### 7.1 Draft requirements

```yaml
# artifacts/v09-requirements.yaml (draft)
artifacts:
  - id: REQ-040
    title: Complete MC/DC for a high-complex Rust application
    status: draft
    description: >
      The system shall measure and report MC/DC coverage on a real
      Rust application of at least 50 KLOC, with at least 200 distinct
      MC/DC decisions, on the wasm32-wasip2 target, with no spurious
      gaps caused by tool limitations.
    tags: [v0.9, mcdc, scale]
    fields:
      priority: must
      category: functional

  - id: REQ-041
    title: MCP server for MC/DC coverage tool calls
    status: draft
    description: >
      The system shall ship a Model Context Protocol server exposing
      get_decision_truth_table, find_missing_witness,
      propose_test_row_to_close_gap, verify_gap_closed, and
      list_uncovered_conditions tool calls. The contract is specified
      in wit/witness-mcp.wit and validated by an end-to-end test
      against a reference MCP client.
    tags: [v0.9, agent, mcp]

  - id: REQ-042
    title: Agent attribution in the V-model evidence chain
    status: draft
    description: >
      Tests authored by an AI agent shall be recorded in the rivet
      graph with a `created_by: agent` field and a signed agent
      identity, and shall be linked via `verifies` to the requirement
      that owned the decision they closed.
    tags: [v0.9, agent, rivet]

  - id: REQ-043
    title: PR-time truth-table review experience
    status: draft
    description: >
      The system shall provide a GitHub Action that posts a PR comment
      rendering the MC/DC truth table for every decision changed in
      the diff, the missing-witness rows in tutorial form, and the
      rivet REQ → FEAT → DEC → decision trace.
    tags: [v0.9, human-ux, ci]

  - id: REQ-044
    title: VS Code extension with MC/DC gutter highlights
    status: draft
    description: >
      The system shall provide a VS Code extension `vscode-witness`
      with gutter highlights distinguishing covered, uncovered, and
      MC/DC-unwitnessed branches; hover-truth-table; and a "close
      gap" code action that triggers the MCP gap-closing loop.
    tags: [v0.9, human-ux, editor]

  - id: REQ-045
    title: Tutorial-style gap explanation
    status: draft
    description: >
      For every uncovered MC/DC condition, the system shall generate a
      plain-English paragraph stating (a) the row assignment that
      would close the gap, (b) the existing paired row used in the
      MC/DC argument, and (c) the rationale (which condition needs to
      be shown to independently affect the decision).
    tags: [v0.9, human-ux, agent]
```

### 7.2 Draft features

```yaml
# artifacts/v09-features.yaml (draft)
artifacts:
  - id: FEAT-040
    title: v0.9 — high-complex Rust app dogfood
    status: draft
    links: [{ type: satisfies, target: REQ-040 }]

  - id: FEAT-041
    title: v0.9 — witness-mcp server crate
    status: draft
    links: [{ type: satisfies, target: REQ-041 }]

  - id: FEAT-042
    title: v0.9 — agent-attributed test artefact in rivet
    status: draft
    links: [{ type: satisfies, target: REQ-042 }]

  - id: FEAT-043
    title: v0.9 — actions/witness-pr-truth-table
    status: draft
    links: [{ type: satisfies, target: REQ-043 }]

  - id: FEAT-044
    title: v0.9 — vscode-witness extension
    status: draft
    links: [{ type: satisfies, target: REQ-044 }]

  - id: FEAT-045
    title: v0.9 — tutorial-style gap explanations
    status: draft
    links: [{ type: satisfies, target: REQ-045 }]
```

### 7.3 Draft design decisions

```yaml
# artifacts/v09-decisions.yaml (draft)
artifacts:
  - id: DEC-040
    title: MCP server is the agent contract surface, not a chatbot
    status: draft
    rationale: >
      Agents propose tests via tool calls; witness verifies via re-run.
      The agent never marks itself done. The signed envelope is the
      only trusted artefact. Rejected: chat-style "ask the agent to
      improve coverage" loops where the agent self-reports success.
    links: [{ type: satisfies, target: REQ-041 }]

  - id: DEC-041
    title: Agent identity is signed; attribution lives in the rivet graph
    status: draft
    rationale: >
      Sigil's existing DSSE / Ed25519 path is reused for agent
      identity. The rivet graph gains a `created_by` field on test
      artefacts. Rejected: out-of-band attribution (agent name in
      commit message only) — not auditable for regulatory use.
    links: [{ type: satisfies, target: REQ-042 }]

  - id: DEC-042
    title: PR comment renders the truth table; percentage is secondary
    status: draft
    rationale: >
      The reviewer's job is to evaluate whether the missing-witness
      rows matter. A percentage hides that. Rejected: SonarQube /
      Codecov-style percent-and-delta as the primary view.
    links: [{ type: satisfies, target: REQ-043 }]

  - id: DEC-043
    title: VS Code extension is a thin MCP client; no separate state
    status: draft
    rationale: >
      The extension reuses the MCP server. No duplication of the
      truth-table logic or the gap-closing loop. Works against any
      MCP-speaking agent. Rejected: bespoke extension RPC.
    links: [{ type: satisfies, target: REQ-044 }]
```

These artefacts are draft input, not the v0.9 plan. The v0.9 plan
unseals when v0.6 has shipped its MC/DC truth tables and v0.7 has
demonstrated the scale claim. Until then, this brief is positioning.

## 8. Risk register — where we could fail to be superior

A short, honest list. Each risk has a mitigation; some risks are real
enough to make us reconsider scope.

### 8.1 The DWARF reconstruction is fragile

If the v0.6 reconstruction algorithm has gaps — macro expansion,
inlining, optimisation-induced CFG fragmentation that breaks decision
grouping — then v0.9's "complete MC/DC on a high-complex Rust app"
claim is at risk. The C-macro precedent says we don't have to be
perfect, but we have to be honest about what we measure.

**Mitigation.** Strict per-`br_if` fallback (already designed) when
DWARF is absent or the reconstruction is ambiguous. The tutorial-style
gap explanations should distinguish "MC/DC-unwitnessed (decision-level)"
from "branch-unwitnessed (per-`br_if`)" so the reviewer knows which
regime the decision is in.

### 8.2 No real Rust app actually tests this scale at v0.9 timing

If by v0.9 we don't have a 50-KLOC Rust application willing to be the
dogfood target, the scale claim is a slide-deck assertion, not a
shipped result.

**Mitigation.** Identify candidates now (rivet itself? sigil itself?
loom? a public Rust crate of the right complexity?). Stage the dogfood
through v0.7 and v0.8 so v0.9 lands on a known-working target.

### 8.3 The agent-authored test is plausible but wrong

An agent proposes a test that produces the needed row but the assertion
is incorrect — the test passes, the row is witnessed, but the
underlying behaviour is wrong. Witness has verified the *coverage*
claim, not the *correctness* claim.

**Mitigation.** This is the standard limitation of coverage-as-evidence
and not unique to agents. Document it explicitly. The MC/DC truth table
makes the assertion the agent committed to *very visible*: a reviewer
sees "agent says (T,T,T,F) → Pass, but the spec says it should be
Fail" if they read the table.

### 8.4 Parasoft (or any incumbent) ships their MCP server first and ours looks like a knockoff

Parasoft already shipped an MCP server in March 2026 for C/C++. If LDRA
or VectorCAST follow before v0.9, the "agent-native" claim weakens.

**Mitigation.** The differentiation is *MC/DC-native* tool calls
(`find_missing_witness`, `propose_test_row_to_close_gap`) and *signed
attribution* in a V-model graph, not generic "agent gets coverage
data." Even if Parasoft ships generic agent integration, witness's
specific tool surface and the rivet/sigil chain are unique.

### 8.5 The qualification story stays vapour through v0.9

If we never ship the v1.0 Check-It pattern, the regulated-industry
claim is unfounded — the LDRA/VectorCAST/Cantata advantage on
qualification persists. v0.9's "superior" positioning would have to
exclude DAL A use.

**Mitigation.** Ship a Check-It-pattern *prototype* in v0.9 — even an
unqualified one — to prove the architecture supports it. The full
qualification at v1.0 is a separate engineering and regulatory
project.

### 8.6 The free-tier / commercial-tier line is mis-drawn

If we put the agent integration behind a paywall (option 6.2 in §6),
the "agent UX is superior" pitch loses to anyone shipping it free.
If we put nothing behind a paywall (option 6.1), there's no funding
for v1.0.

**Mitigation.** This is a strategic call for the user. The brief
flags it; the brief does not resolve it.

### 8.7 RapiCover already does most of what we claim, just on C

If a reader compares feature-for-feature, RapiCover ships:
unbounded conditions (1000), MC/DC reporting, missing-vector
highlighting, multi-target. The witness-superior claims reduce to
(a) free, (b) Wasm/Rust target, (c) signed evidence, (d) agent UX.
Three of those four are real. One — "free" — is not technical
superiority but distribution superiority.

**Mitigation.** Be precise in the positioning. Witness is superior to
RapiCover *for Rust-via-Wasm projects that want signed evidence and
agent integration*. We are not superior for Ada DAL A avionics, where
RapiCover wins on certification heritage and we cannot.

### 8.8 The "PR truth table" feature gets ignored in practice

Reviewers might prefer the percentage because the truth table is too
much information for a quick PR pass. If we ship the feature and
nobody clicks past the line "MC/DC 3/4," the human-UX claim
collapses to a vanity feature.

**Mitigation.** Make the percentage the headline, the truth table a
collapsed-by-default detail block, and the tutorial gap explanation a
"why this matters" link. Measure click-through on the dashboard.

---

## Sources and references

Market scan:

- [LDRA Testbed and TBvision](https://ldra.com/products/ldra-testbed-tbvision/)
- [LDRA MC/DC capability](https://ldra.com/capabilities/mc-dc/)
- [LDRA DO-178C technical briefing PDF](https://ldra.com/wp-content/uploads/ldra/LDRA_tool_suite_and_DO-178C_Technical_Briefing_v2.0.pdf)
- [VectorCAST product page](https://www.vector.com/us/en/products/products-a-z/software/vectorcast/)
- [VectorCAST/QA factsheet](https://cdn.vector.com/cms/content/products/VectorCAST/Docs/Datasheets/English/VectorCAST_QA_Factsheet_EN.pdf)
- [VectorCAST and writing test cases to meet MCDC requirements](https://support.vector.com/kb/?id=kb_article_view&sysparm_article=KB0012681)
- [Cantata MC/DC blog](https://www.qa-systems.com/blog/mc-dc-coverage-a-critical-technique/)
- [Cantata Hybrid datasheet](https://www.qa-systems.com/resource/cantatahybrid-datasheet/)
- [QA Systems C/C++ test automation with Claude Code](https://www.qa-systems.com/blog/c-c-test-automation-with-claude-code-and-cantata)
- [RapiCover product page](https://www.rapitasystems.com/products/rapicover)
- [Rapita first coverage tool to support all CAST-10 interpretations](https://www.rapitasystems.com/news/first-coverage-tool-support-all-interpretations-cast-10-decisions-announced)
- [Bullseye product features](https://www.bullseye.com/product.html)
- [Bullseye does not support MC/DC (MathWorks Q&A)](https://www.mathworks.com/matlabcentral/answers/326682-mc-dc-code-coverage-not-possible-integrating-bullseyecoverage-tool)
- [Squore via Vector](https://vector-softwarequality.com/static-code-analysis/squore)
- [Squore Cobertura format reference](https://doc.squore.net/18.1.4/cli_reference/sect_dp_std_Cobertura.html)
- [Coverity / MC/DC question (Synopsys community)](https://sig-synopsys--sigstage.sandbox.my.site.com/community/s/article/How-do-we-go-about-performing-MC-DC-code-coverage-on-Coverity-analysis)
- [Synopsys MC/DC blog](https://www.synopsys.com/blogs/chip-design/mc-dc-struggle-reaching-100-percent.html)
- [GCC 14 MC/DC arxiv 2501.02133](https://arxiv.org/abs/2501.02133)
- [GCC 14 invoking gcov](https://gcc.gnu.org/onlinedocs/gcc/Invoking-Gcov.html)
- [gcovr GCC 14 MC/DC tracking issue](https://github.com/gcovr/gcovr/issues/913)
- [MaskRay: MC/DC and compiler implementations](https://maskray.me/blog/2024-01-28-mc-dc-and-compiler-implementations)
- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov)
- [tarpaulin](https://github.com/xd009642/tarpaulin)
- [rustc MC/DC tracking #124144](https://github.com/rust-lang/rust/issues/124144)
- [Rust instrumentation-based code coverage](https://doc.rust-lang.org/rustc/instrument-coverage.html)
- [MoonBit coverage docs](https://docs.moonbitlang.com/en/latest/toolchain/moon/coverage.html)
- [JaCoCo counters reference](https://www.eclemma.org/jacoco/trunk/doc/counters.html)

Agent UX prior art:

- [Anthropic MCP introduction](https://www.anthropic.com/news/model-context-protocol)
- [MCP specification (2025-11-25)](https://modelcontextprotocol.io/specification/2025-11-25)
- [MCP donation to Agentic AI Foundation](https://www.anthropic.com/news/donating-the-model-context-protocol-and-establishing-of-the-agentic-ai-foundation)
- [Parasoft MCP server announcement](https://www.parasoft.com/news/c-cpp-test-automation-certified-googletest-agentic-ai/)
- [Parasoft AI agents and MCP servers blog](https://www.parasoft.com/blog/ai-agents-mcp-servers-software-quality/)
- [Agentic QE Fleet (open-source)](https://github.com/proffesor-for-testing/agentic-qe)
- [TestSprite MCP](https://www.testsprite.com/use-cases/en/software-testing-mcp)
- [Tricentis agentic testing guide](https://www.tricentis.com/learn/agentic-testing)
- [Mabl AI agent frameworks](https://www.mabl.com/blog/ai-agent-frameworks-end-to-end-test-automation)
- [BrowserStack agentic AI in testing](https://www.browserstack.com/guide/agentic-ai-in-testing)
- [The Rise of Agentic Testing arxiv 2601.02454](https://arxiv.org/abs/2601.02454)

In-toto / signed evidence:

- [in-toto attestation framework](https://github.com/in-toto/attestation)
- [in-toto v1 predicate spec](https://github.com/in-toto/attestation/blob/main/spec/v1/predicate.md)
- [in-toto attestations and SLSA](https://slsa.dev/blog/2023/05/in-toto-and-slsa)
