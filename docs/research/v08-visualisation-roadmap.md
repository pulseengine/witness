# v0.8 — visualisation roadmap

How witness ships a UI that humans and AI agents can both query, why
that UI is itself an Axum-on-`wasm32-wasip2` Component, and what we
borrow from rivet's HTMX + Playwright pattern. Survey-driven brief
written 2026-04-25 against witness `main` and rivet `main`.

---

## 1. Recommended architecture (the call)

Ship `witness-viz` as an Axum Router compiled to `wasm32-wasip2` via
[`wstd-axum`](https://docs.rs/wstd-axum/latest/wstd_axum/), runnable two
ways: `wasmtime serve witness-viz.component.wasm --dir=./reports` for
the standalone case, or composed against `witness-core` via `wac` for
the in-process case. The HTML is rendered server-side with **`maud`**
(macro-typed templates that return `axum::response::Html` directly,
work in the wasi-http proxy world, and avoid the dual-edit pain rivet
already wears with raw `format!`). HTMX 2.x is bundled inline as a byte
array in the binary — same pattern rivet uses (`include_str!` of
`htmx.min.js`) — so the visualiser has zero CDN dependency and runs
fully offline. Data input is the existing v0.5 `run-record.json`
plus `*.witness.json` manifest as the **only** stable contract; the
Component-Model interface is a v0.8.x stretch and not the load-bearing
path. Verification is Playwright copying rivet's `webServer` pattern
verbatim, with the visualiser's own MC/DC numbers added to the witness
self-coverage suite (witness measuring itself measuring itself).

This gets us: zero-coupling data flow, runtime-agnostic distribution,
the same dependency surface rivet has already de-risked, and a UI that
falls out of the existing report JSON without changing any v0.5 file
formats.

---

## 2. Axum-on-wasi feasibility report

### 2.1 The path that works: `wstd-axum` on wasi-http

`wstd-axum` is the bridge crate that lets an `axum::Router` run on
`wasm32-wasip2` by mapping it onto the `wasi:http/proxy` world that
`wasmtime serve` implements. The pattern is one proc-macro
(`#[wstd_axum::http_server]`) on `fn main() -> Router`, then build with
`cargo build --target wasm32-wasip2` and run with
`wasmtime serve -Scli target/wasm32-wasip2/release/witness-viz.wasm`.
The crate explicitly notes: axum features that depend on hyper or
tokio (`http1`, `http2`, `ws`) are unsupported because `wasi-http`
provides HTTP at the import-interface level, not over raw sockets
(see [wstd-axum docs](https://docs.rs/wstd-axum/latest/wstd_axum/)
and [lib.rs/wstd-axum](https://lib.rs/crates/wstd-axum)).

For witness-viz that is fine: we serve HTML and JSON, we do not own a
WebSocket, we do not need HTTP/2 server push. Long-poll is replaceable
with HTMX's `hx-trigger="every 1s"` if we ever want live coverage
updates during a run.

### 2.2 What rivet has already de-risked

`/Users/r/git/pulseengine/rivet/rivet-cli/src/serve/` is a 5,908-line
production Axum + HTMX server. It uses:

- `axum 0.8`, `tower-http 0.6` (CORS + fs).
- Inline asset bundling: `const HTMX_JS: &str = include_str!("../../assets/htmx.min.js");`
- Plain `format!`-based HTML strings (no templating engine), which
  has cost them maintainability — we do not copy that part.
- An `embedded_wasm` module that ships the spar WASM core inline.
- Routes split into `mod.rs`, `views.rs`, `components.rs`, `layout.rs`,
  `api.rs`.
- An MCP transport co-mounted on the same Axum router via
  `rmcp::transport::streamable_http_server::StreamableHttpService` —
  this is the precedent for the "AI agents query the same surface as
  humans" claim in §3.5.

Rivet does **not** today target `wasm32-wasip2`. They run native. So
witness-viz takes their structural pattern (inline assets, HTMX swap
targets, `webServer` Playwright config) and pushes it one step further
onto the wasi-http proxy world. The diff from rivet's setup is small:
swap `tokio` runtime for `wstd`, swap `format!` for `maud`, drop the
`hyper` features.

### 2.3 `wasmtime serve` mode vs embedded-Component mode

| Mode | When | Pros | Cons |
|---|---|---|---|
| `wasmtime serve witness-viz.component.wasm --dir=./reports:/reports` | User wants to view a finished run | Single binary, no host coupling, works with any Wasm runtime that implements `wasi:http/proxy` | Process boundary; mounting reports dir requires explicit `--dir` |
| Composed via `wac plug` into `witness serve` (host CLI embeds wasmtime + witness-core + witness-viz) | User wants `witness serve report.json` in one command | One UX entry point; in-process so report data is passed by reference, not file mount | Host has to embed two components; binary size grows |
| Native build (no wasm) | CI / dev on machines without wasmtime | Fast iteration, normal `cargo run` | Defeats the runtime-agnostic story; only useful as a dev convenience |

We ship **all three**, gated by Cargo features: `default = ["native"]`
for dev, `wasm` for the publishable Component, `embed` for the host
composition. Rivet's `serve_variant.spec.ts` already proves the
"binary serves itself" pattern at 1377 lines of integration code.

### 2.4 WIT interface (deferred, but sketched)

The Component-Model contract for v0.8.1+ is:

```wit
package pulseengine:witness-viz@0.8.0;

interface report-source {
    record run-summary {
        module-id: string,
        total-decisions: u32,
        covered-decisions: u32,
        mcdc-percent: f32,
    }
    get-summary: func() -> result<run-summary, string>;
    get-decision: func(decision-id: string) -> result<decision-detail, string>;
    list-uncovered: func() -> list<gap-entry>;
}

world witness-viz {
    import report-source;
    export wasi:http/incoming-handler;
}
```

In v0.8.0 the visualiser does not import this; it reads
`run-record.json` from a mounted directory. In v0.8.1 we add the WIT
import and let `wac plug witness-viz.wasm --plug witness-core.wasm`
produce a single composed component for in-process embedding.

### 2.5 Existing precedent in the wild (and the gap)

- [`bytecodealliance/sample-wasi-http-rust`](https://github.com/bytecodealliance/sample-wasi-http-rust) —
  a `wasi:http` server component in Rust. Not Axum, but shows the
  proxy world end-to-end.
- [`SaritNike/wasm-component-model-runner`](https://github.com/SaritNike/wasm-component-model-runner) —
  Axum *host* orchestrating Wasm guests. Inverted from what we want
  (host is native; guests are wasm) but the harness is an existence
  proof that Axum + wasmtime + the Component Model compose cleanly.
- `wstd-axum` itself ships a `hello-world` example that runs end-to-end
  under `wasmtime serve`.
- **Gap:** there is no public reference combining Axum-on-wasip2 +
  HTMX + a non-trivial domain UI. Witness-viz is plausibly the first
  public exemplar. That is a positioning win (blog post arc material)
  but also means we eat the integration cost ourselves.

Sources:
[WASIp2 in Wasmtime](https://docs.wasmtime.dev/examples-wasip2.html),
[wasm32-wasip2 Rust target](https://doc.rust-lang.org/nightly/rustc/platform-support/wasm32-wasip2.html),
[wasmtime_wasi_http API](https://docs.wasmtime.dev/api/wasmtime_wasi_http/index.html),
[Bytecode Alliance: invoking component functions](https://bytecodealliance.org/articles/invoking-component-functions-in-wasmtime-cli),
[Wasmtime Component-Model docs](https://component-model.bytecodealliance.org/running-components/wasmtime.html).

---

## 3. UI design proposal

The two audiences (human reviewers and AI agents) get the same data
from the same routes. Humans get HTML; agents get JSON. Content
negotiation lives on the `Accept` header — an HTMX request wins the
HTML fragment, a `curl -H "Accept: application/json"` or an MCP tool
call wins the JSON payload. One pipeline, two skins.

### 3.1 Truth-table view

Permalink: `/decision/<module>/<function>/<offset>`

```
 Decision DEC-rt-187: `is_eligible`  (src/auth.rs:42, conditions a,b,c)
 ─────────────────────────────────────────────────────────────────────
   row | a | b | c | outcome | witnessed | test-row link
   ──────────────────────────────────────────────────────────────────
    1  | T | T | T |   T     |    YES    | tests::auth::happy_path
    2  | T | T | F |   F     |    YES    | tests::auth::missing_role
    3  | T | F | * |   F     |    -      |    *** GAP ***
    4  | F | * | * |   F     |    YES    | tests::auth::not_logged_in
 ─────────────────────────────────────────────────────────────────────
   independent-effect pairs:
     a:  rows {1, 4}   PROVEN
     b:  rows {1, 3}   GAP (row 3 missing)
     c:  rows {1, 2}   PROVEN
   MC/DC: 2 of 3 conditions independently demonstrated  (66.7%)
   ─────────────────────────────────────────────────────────────────
   [view raw counters]   [permalink]   [emit-as-rivet-evidence]
```

Cell encoding: `T`/`F` for booleans; `*` for short-circuited
(don't-care); `?` for not-yet-executed (dynamic gap); `×` for
structurally-impossible (DWARF says the row cannot be generated). The
gap row is rendered with a red left-border and the test name slot
shows `*** GAP ***`. The independent-effect-pair block underneath the
table is the *evidence* — that is the thing a reviewer cites in a PR.

### 3.2 Gap drill-down

Permalink: `/gap/<module>/<function>/<offset>?condition=<idx>`

```
 GAP — Decision DEC-rt-187, condition `b`
 ─────────────────────────────────────────────────────────────────────
   To prove condition `b` independently affects outcome:
     - Need a test row where (a, b, c) = (T, F, *)
     - Currently no test in the suite reaches this row.

   Source-level shape:
     if a && b && c { eligible() } else { reject() }    src/auth.rs:42

   Wasm-level shape:
       func[17]  br_if @0x12  (a)
                 br_if @0x18  (b)   <-- this br_if is unwitnessed
                 br_if @0x1e  (c)

   Suggested test:
     #[test]
     fn b_alone_blocks_eligibility() {
         let user = User { has_account: true, has_role: false, .. };
         //                              ^^^^^ b = false; a, c true
         assert!(!is_eligible(&user));
     }

   How witness will know:
     The new test execution will increment counter
     __witness_counter_42 to >= 1, marking row 3 witnessed.
 ─────────────────────────────────────────────────────────────────────
   [copy-test-stub]   [show-cfg]   [open-in-editor]
```

The `[copy-test-stub]` button is HTMX `hx-get="/gap/.../stub" hx-target="#clipboard-staging"` — server returns plain text, HTMX swaps it into a hidden `<pre>` and a tiny vanilla-JS snippet copies to clipboard. No SPA framework needed.

### 3.3 V-model navigation

```
 ┌── REQ ──────────┐    ┌── FEAT ─────────┐    ┌── DEC ──────────┐
 │ REQ-007         │    │ FEAT-002 v0.2   │    │ DEC-006         │
 │ MC/DC report    │◀──▶│ Condition       │◀──▶│ DWARF           │
 │                 │    │ decomposition   │    │ reconstruction  │
 └────────┬────────┘    └────────┬────────┘    └────────┬────────┘
          │                      │                      │
          ▼                      ▼                      ▼
 ┌─ Decision ──────┐    ┌─ Conditions ────┐    ┌─ Wasm br_if ────┐
 │ is_eligible     │───▶│ a, b, c         │───▶│ @0x12,18,1e     │
 │ (3 conds, 66%)  │    │ (b uncovered)   │    │ (br_if 0x18)    │
 └────────┬────────┘    └────────┬────────┘    └────────┬────────┘
          │                      │                      │
          ▼                      ▼                      ▼
 ┌─ Test rows ─────┐    ┌─ Indep pairs ───┐    ┌─ Witness ───────┐
 │ rows 1,2,3,4    │───▶│ a:{1,4} b:{1,3} │───▶│ counter_42=0    │
 │ row 3 = GAP     │    │ c:{1,2}         │    │ row 3 unproven  │
 └─────────────────┘    └─────────────────┘    └─────────────────┘
```

Each box is an HTMX swap target. Clicking a `REQ-007` pill navigates
to `/req/REQ-007` and the right-side panels reload showing every
decision allocated to that requirement. Clicking a Wasm offset opens
`/wasm/<offset>` showing the disassembled function with a highlight on
the unwitnessed `br_if`. The links between rivet artefacts (REQ ↔
FEAT ↔ DEC) come from the v0.3 `witness rivet-evidence` data, which
v0.6 already produces.

### 3.4 Per-decision permalinks

Every interesting view URL is **stable across runs and content-
addressed by decision id**. A reviewer can paste
`https://reports.example.com/decision/auth.wasm/is_eligible/0x12`
into a PR comment and it renders correctly against any run-record
that contains that decision. Decision ids derive from
`(module-hash, function-id, byte-offset)` — already stable in v0.5.

### 3.5 AI-agent query API

Same routes, served as JSON when `Accept: application/json` (or as
an explicit `?format=json` for shells that fight content negotiation).
Plus an MCP server mounted on the same Axum router under `/mcp` —
copying rivet's pattern (rivet mounts `StreamableHttpService` from
`rmcp::transport::streamable_http_server`). MCP tools we expose:

- `witness.list_decisions(module)`
- `witness.get_truth_table(decision_id)`
- `witness.list_gaps(?min_severity)`
- `witness.suggest_test(decision_id, condition_index)` — returns the
  test stub that fills the gap, same content as the
  `[copy-test-stub]` button.
- `witness.coverage_summary()` — overall MC/DC numbers.

The MCP surface is **strictly a subset of the HTTP surface** — no
private MCP-only routes. This keeps the contract auditable: anything
an agent can ask, a human can ask too, and the test suite covers both
through the same Playwright spec.

REST option vs GraphQL option: REST. The data shape is fixed (truth
tables are tables, decisions are decisions); GraphQL's flexibility
buys nothing here and the wasi-http proxy world makes a GraphQL server
a heavier lift than a JSON-returning Axum handler.

---

## 4. Data flow — choice and justification

### 4.1 The four candidates

| Path | Coupling to witness internals | Failure mode | When it shines |
|---|---|---|---|
| Read `run-record.json` from mounted dir | Zero (file format only) | Unparseable JSON | The default; works against any version of witness whose schema we still understand |
| Component-Model interface (witness-viz imports witness-core) | Tight (WIT must match) | Component composition fails | In-process queries, large run-records that don't fit through serde round-trips well |
| HTTP API exposed by witness CLI (`witness serve --report run.json`) | Medium (HTTP shape) | Network / port collisions | When witness is already running as a daemon |
| Bundled `.wasm` shipped per release, run via `wasmtime serve` against a local file | Zero (file format) | Same as path 1 | The "give a customer the artefact and an LCOV file" use case |

### 4.2 Pick and rationale

**Primary path: read `run-record.json` (path 1) plus the manifest from a mounted directory.** This is the only path that survives every refactor we have queued for v0.6 (loom DWARF preservation), v0.7 (real-app scaling), and the still-unsettled component-model coverage shape. The visualiser depends only on the v0.5 schema; if witness-core changes its internals, the visualiser does not have to ship a new version. Path 4 is the *distribution* of path 1 — same code, packaged for offline use.

**Secondary, gated behind a feature flag:** path 2, the WIT-imported `witness-core`. We sketch the WIT in §2.4 but defer the binding to v0.8.1. It buys us nothing the file path does not, until run-records grow large enough that JSON serialisation hurts.

**Explicitly declined: path 3.** A long-running `witness serve` daemon adds operational surface (port management, auth, lifetime) without giving us any capability that "open the report directory in the visualiser" does not already give us. CI pipelines that want a one-shot view can run `wasmtime serve` for the duration of the PR check and tear it down.

### 4.3 What the file path looks like in practice

```
$ witness instrument app.wasm -o app.inst.wasm
$ witness run app.inst.wasm --invoke main -o run.json
$ witness report run.json --format json -o coverage.json
$ wasmtime serve witness-viz.component.wasm \
      --dir=$PWD:/reports \
      --addr=127.0.0.1:8080
$ open http://127.0.0.1:8080/?report=/reports/coverage.json
```

The visualiser reads `coverage.json` plus the sibling
`app.inst.witness.json` manifest, which already contains everything it
needs: branch ids, source mappings (when DWARF was present), decision
groupings (v0.6+), and per-counter values.

---

## 5. Playwright test plan

### 5.1 Adopting rivet's pattern verbatim

The rivet config we copy:

```ts
// /Users/r/git/pulseengine/rivet/tests/playwright/playwright.config.ts
export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  timeout: 30_000,
  retries: process.env.CI ? 1 : 0,
  workers: 1,                       // serial — single server instance
  use: {
    baseURL: "http://localhost:3003",
    trace: "on-first-retry",
    screenshot: "only-on-failure",
  },
  webServer: {
    command: "cargo run --release -- serve --port 3003",
    port: 3003,
    timeout: 120_000,
    reuseExistingServer: !process.env.CI,
    cwd: "../..",
  },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});
```

For witness-viz, the only change is `webServer.command`:

```ts
webServer: {
  command: "wasmtime serve --addr=127.0.0.1:3037 \
            ../../target/wasm32-wasip2/release/witness-viz.component.wasm \
            --dir=../fixtures:/reports",
  port: 3037,
  timeout: 120_000,
  reuseExistingServer: !process.env.CI,
}
```

Rivet's `helpers.ts` (the `waitForHtmx`, `htmxNavigate`, `assertUrl`,
`countTableRows` quartet) is copied verbatim — those four helpers are
the smallest viable surface and rivet's 23 spec files all use them.

### 5.2 Spec files we ship in v0.8.0

Mirroring rivet's naming:

| Spec | What it asserts |
|---|---|
| `truth-table.spec.ts` | A decision page renders with the expected row count, the right cells show T/F/*, gap rows have the `gap` class, the independent-effect-pair block names the right rows. |
| `gap-drilldown.spec.ts` | `/gap/...` returns the test stub, the copy-stub button puts text into a known DOM target, the stub compiles when piped to `cargo check` (subprocess-asserted). |
| `vmodel-nav.spec.ts` | Click chain REQ → FEAT → DEC → decision → condition → row keeps URL state, back-button restores prior view. |
| `permalink.spec.ts` | `GET /decision/<m>/<f>/<o>` returns the same DOM regardless of which page you arrived from; URL is in `<link rel=canonical>`. |
| `agent-api.spec.ts` | `Accept: application/json` returns parseable JSON whose schema matches `witness-viz-api/v1`. |
| `mcp.spec.ts` | The `/mcp` MCP transport answers `tools/list` and the listed tool names match the documented surface. |
| `coverage-summary.spec.ts` | The dashboard `/` shows overall MC/DC %, decision counts, gap counts; numbers match `coverage.json`. |
| `offline.spec.ts` | With network requests blocked except to `localhost`, every page still renders (assets are inline). |
| `accessibility.spec.ts` | `axe-playwright` finds no critical violations on the truth-table view. |
| `helpers.ts` | Copied from rivet; extended with `waitForCounters`, `loadFixtureReport`. |

### 5.3 Coverage of the visualiser by witness itself

`witness-viz` is a `wasm32-wasip2` Component. Witness measures
`wasm32-wasip2` Components. Therefore:

```
$ cargo build -p witness-viz --target wasm32-wasip2 --release
$ witness instrument target/wasm32-wasip2/release/witness-viz.wasm \
      -o target/wasm32-wasip2/release/witness-viz.inst.wasm
$ wasmtime serve target/wasm32-wasip2/release/witness-viz.inst.wasm \
      --dir=tests/fixtures:/reports &
$ npm --prefix tests/playwright test
$ # Playwright run produced HTTP traffic; counters in the running
$ # component captured branch/decision execution.
$ witness report-globals --addr=127.0.0.1:3037 \
      --manifest target/.../witness-viz.witness.json \
      -o viz-self-coverage.json
```

That `report-globals` subcommand (new in v0.8) reads exported
`__witness_counter_*` globals out of the live wasmtime instance. The
result, `viz-self-coverage.json`, is fed back into a *second*
witness-viz instance and rendered. **The visualiser visualises its own
coverage.** That is the v0.8 demo screenshot.

The Playwright suite becomes the witness-viz "test harness" in the
v0.1 sense: its `npm test` invocation is the input that drives the
component, the counters are the output, the resulting MC/DC numbers
are evidence we ship in the release. We add a CI gate: visualiser
MC/DC ≥ 70% (v0.8) → ≥ 80% (v0.9) → ≥ 90% (v1.0), the same ratchet
witness applies to itself.

### 5.4 V-model linkage for the visualiser

Each `*.spec.ts` is annotated with a Playwright tag `@VERIFIES:REQ-NNN`
(parsed by the rivet evidence emitter). On a successful run:

```
REQ-040 (truth-table view rendering)
    ↓ verified-by
truth-table.spec.ts (8 tests, all green)
    ↓ executes
witness-viz.component.wasm (instrumented)
    ↓ produces
viz-self-coverage.json
    ↓ aggregated as
rivet evidence (witness-rivet-evidence/v1)
    ↓ links back to
REQ-040 (closes the loop)
```

This is the spec-driven-development closure: visualiser requirements
trace to Playwright tests trace to the visualiser's own MC/DC table.
Any uncovered visualiser branch becomes a *visible* gap in the
visualiser of the visualiser.

---

## 6. Competitive UI scan

| Feature | LDRA TBmanager | VectorCAST | Codecov / Coveralls | SonarQube | LCOV `genhtml` | **witness-viz (v0.8)** |
|---|---|---|---|---|---|---|
| Decision-level view | yes (DO-178C MC/DC view) | yes (test-case matrix) | no — branch-only via LCOV | branch only | branch only | **yes — first-class** |
| Truth-table render | yes | yes (limited to 6 cond) | no | no | no | **yes — uncapped** |
| Independent-effect pair callout | implicit (must read MC/DC table) | yes | no | no | no | **yes — explicit, named** |
| Gap → test-stub suggestion | no | partial (Test Insight) | no | no | no | **yes — copy-paste stub** |
| AI-agent / programmatic API | proprietary | proprietary | REST | REST | none | **REST + MCP, schema'd** |
| Permalinks per decision | no | no | per-file | per-file | per-file | **per-decision** |
| Source ↔ bytecode dual view | no (source only) | no | no (source only) | no | no | **yes (Wasm + DWARF)** |
| Offline / single-binary | no (server install) | no (client install) | no (cloud) | no (server) | yes (static HTML) | **yes (`wasmtime serve` + 1 file)** |
| V-model traceability surfaced in UI | yes (TBreq integration) | yes (System Test) | no | no | no | **yes (rivet artefact links)** |
| Cost | five figures/year | five figures/year | per-seat | per-seat / OSS | $0 | $0 (Apache-2.0/MIT) |
| Qualifies under DO-178C / ISO 26262 | yes (via tool qual kit) | yes | no | no | no | **target v1.0 (Check-It pattern)** |

What we steal:

- **From LDRA / VectorCAST:** the discipline of showing the
  independent-effect pair *as data*, not buried in a colour. Their UIs
  are ugly but their information density is correct. We keep their
  density and add HTMX interactivity.
- **From genhtml:** the static-HTML offline use case. Our `wasmtime
  serve` mode is genhtml-with-interactivity. genhtml's market-share
  proves people *do* want to dump a coverage report into a tarball
  and email it; we honour that.
- **From Codecov:** PR-comment integration. v0.8 ships a
  `witness-comment` GitHub Action that posts the gap drill-down as a
  comment, linking permalinks back to the deployed visualiser.

Where we are unambiguously superior because of decision-level structure:

1. **Uncapped condition count.** Clang and rustc cap MC/DC at 6
   conditions. LDRA/VectorCAST inherit Clang's cap when targeting
   LLVM-emitted coverage. Witness is uncapped (DEC-008-equivalent).
   The truth-table view renders 8-, 12-, 16-condition decisions
   without paginating to a degraded shape.
2. **Bytecode + source dual-view.** Nobody else can show the user the
   `br_if` and the `if` in the same pane because nobody else measures
   at both levels. v0.8 makes this a left/right split-pane on the
   decision page.
3. **AI-first audit surface.** The MCP server-mounted-on-Axum pattern
   lets an agent reviewer do a self-driven gap analysis. None of the
   incumbents have an MCP surface (their APIs predate MCP and their
   business models discourage opening up).
4. **Variant / cargo-feature aware (v0.9 stretch).** When witness
   measures multiple variants of the same module (rivet's
   `feature-model.yaml` supplies the variant names), the visualiser
   shows the variant-pruning argument in action: "this decision exists
   only in variant `tls=ring`; under variant `tls=rustls` it is
   structurally absent." That is the variant-pruning blog post made
   interactive — incumbents have nothing comparable.

---

## 7. Provisional rivet artefacts (drafts for v0.8)

IDs continue from current highs (REQ-022, FEAT-011, DEC-012). Numbering
leaves a gap for v0.6 and v0.7 (REQ-023..REQ-039 reserved). All entries
land in `artifacts/v08-*.yaml` files; all start `status: draft`.

### 7.1 Requirements

```yaml
- id: REQ-040
  type: requirement
  title: MC/DC truth-table visualiser as a Wasm Component
  status: draft
  description: >
    The system shall provide a graphical visualisation of MC/DC
    coverage data, packaged as a wasm32-wasip2 Component runnable
    under `wasmtime serve` or composed in-process with witness-core.
    The visualiser shall consume run-record.json and the witness
    manifest as its sole stable input, with no required network
    dependencies at render time.
  tags: [v0.8, viz, component]
  fields: { priority: must, category: functional }

- id: REQ-041
  type: requirement
  title: Permalinks per decision and per gap
  status: draft
  description: >
    Each decision identified in the run-record shall be addressable
    by a stable URL of the shape /decision/<module>/<function>/<offset>.
    Each unproven independent-effect pair shall be addressable as
    /gap/<module>/<function>/<offset>?condition=<idx>. Permalinks
    shall remain stable across re-runs that do not change the
    underlying CFG.
  tags: [v0.8, viz, traceability]
  fields: { priority: must, category: functional }

- id: REQ-042
  type: requirement
  title: AI-agent query surface (REST + MCP)
  status: draft
  description: >
    The visualiser shall expose every decision, gap, summary, and
    test-stub suggestion to programmatic agents under two surfaces:
    (1) HTTP routes returning JSON when Accept: application/json,
    (2) an MCP transport mounted at /mcp listing the tools defined
    in REQ-042-tools. The MCP surface shall be a strict subset of
    the HTTP surface; no MCP-only capability shall exist.
  tags: [v0.8, viz, agents, mcp]
  fields: { priority: must, category: functional }

- id: REQ-043
  type: requirement
  title: Visualiser self-coverage reported in releases
  status: draft
  description: >
    Each release of witness-viz shall ship its own MC/DC coverage
    report, produced by instrumenting the visualiser Component and
    measuring it against the Playwright suite. The release artefact
    shall include both the .wasm Component and the
    viz-self-coverage.json file.
  tags: [v0.8, viz, dogfood]
  fields: { priority: should, category: process }

- id: REQ-044
  type: requirement
  title: Offline operation (no CDN, no external assets)
  status: draft
  description: >
    The visualiser shall render every documented view with no
    outbound network requests beyond the host running it. HTMX,
    fonts, and any required JS shall be bundled inline as binary
    assets in the Component.
  tags: [v0.8, viz, offline]
  fields: { priority: must, category: non-functional }

- id: REQ-045
  type: requirement
  title: Gap drill-down emits a copyable test stub
  status: draft
  description: >
    For each unproven independent-effect pair, the visualiser shall
    offer a generated Rust test-stub that, if implemented to make
    the assertions hold, would witness the missing row of the truth
    table. The stub shall reference the source-level decision
    location when DWARF data is available; otherwise a Wasm-level
    stub citing the relevant exported counter.
  tags: [v0.8, viz, gap-analysis]
  fields: { priority: should, category: functional }
```

### 7.2 Features

```yaml
- id: FEAT-040
  type: feature
  title: v0.8 — Axum + HTMX visualiser as wasm32-wasip2 Component
  status: draft
  description: >
    `witness-viz` crate, building to a wasm32-wasip2 Component via
    wstd-axum. Maud templating; HTMX inline; routes for /, /decision,
    /gap, /req, /mcp; fixture reports under tests/fixtures.
    Distributed both as a standalone .component.wasm and composed
    via wac plug into the witness CLI.
  tags: [v0.8, viz]
  fields: { phase: phase-8 }
  links:
    - { type: satisfies, target: REQ-040 }
    - { type: satisfies, target: REQ-044 }

- id: FEAT-041
  type: feature
  title: v0.8 — Truth-table + gap-drilldown views
  status: draft
  description: >
    Server-rendered HTML pages for decision truth tables (with
    independent-effect pair highlighting) and per-gap drill-down
    (with copy-paste test-stub generation). All pages reachable
    via stable permalinks.
  tags: [v0.8, viz, mcdc]
  fields: { phase: phase-8 }
  links:
    - { type: satisfies, target: REQ-041 }
    - { type: satisfies, target: REQ-045 }

- id: FEAT-042
  type: feature
  title: v0.8 — REST + MCP agent surface
  status: draft
  description: >
    JSON content-negotiated routes plus an MCP transport mounted
    on the same Axum router (rmcp::transport::streamable_http_server),
    copying rivet's pattern. Tools: list_decisions, get_truth_table,
    list_gaps, suggest_test, coverage_summary.
  tags: [v0.8, viz, mcp]
  fields: { phase: phase-8 }
  links:
    - { type: satisfies, target: REQ-042 }

- id: FEAT-043
  type: feature
  title: v0.8 — Playwright suite + visualiser self-coverage CI
  status: draft
  description: >
    Playwright tests under tests/playwright/ adopting rivet's
    webServer + helpers.ts pattern. CI gate: visualiser-measured-by-
    itself MC/DC >= 70% in v0.8, ratcheting to >= 80% in v0.9 and
    >= 90% in v1.0. Each release ships viz-self-coverage.json.
  tags: [v0.8, viz, ci, testing]
  fields: { phase: phase-8 }
  links:
    - { type: verifies, target: REQ-040 }
    - { type: verifies, target: REQ-041 }
    - { type: satisfies, target: REQ-043 }
```

### 7.3 Design decisions

```yaml
- id: DEC-040
  type: design-decision
  title: Axum + wstd-axum on wasm32-wasip2 for the visualiser
  status: draft
  description: >
    Adopt wstd-axum for the visualiser's HTTP server so the entire
    component is a single .wasm runnable under `wasmtime serve`.
    Rejected alternatives: native-only Axum (defeats the runtime-
    agnostic story); Yew/Leptos SPA (huge JS surface, no offline
    story without a separate static-asset server); plain
    wasmtime_wasi_http handler without Axum (loses the routing /
    middleware ergonomics rivet has already de-risked).
  tags: [v0.8, viz, architecture]

- id: DEC-041
  type: design-decision
  title: Maud over Askama for HTML templating
  status: draft
  description: >
    Maud's macro form returns Markup that implements
    axum::response::IntoResponse, integrates cleanly with HTMX, and
    avoids the dual-edit (template-file + struct) maintenance pain
    rivet's format!-based serve module exhibits. Askama's Jinja
    flavour is fine but adds a build-time dependency and a second
    file per view.
  tags: [v0.8, viz, templating]

- id: DEC-042
  type: design-decision
  title: run-record.json file mount is the primary data path
  status: draft
  description: >
    The visualiser reads the existing v0.5 run-record.json plus
    manifest from a directory mounted by `wasmtime serve --dir=...`.
    A WIT-imported witness-core (Component-Model composition via wac
    plug) is sketched but explicitly deferred to v0.8.1 — the file-
    based path is zero-coupling and survives every refactor on the
    v0.6/v0.7 horizon.
  tags: [v0.8, viz, data-flow]

- id: DEC-043
  type: design-decision
  title: MCP surface is a strict subset of the HTTP surface
  status: draft
  description: >
    Every MCP tool shall correspond to one HTTP route returning
    JSON. There shall be no MCP-private capability. This keeps the
    audit surface single, makes the Playwright suite cover the agent
    contract by transitivity, and prevents drift between human-
    facing and agent-facing data models.
  tags: [v0.8, viz, agents]

- id: DEC-044
  type: design-decision
  title: HTMX 2.x bundled inline; no CDN
  status: draft
  description: >
    `include_bytes!` HTMX into the Component binary, following
    rivet's inline-asset pattern. This satisfies REQ-044 (offline)
    and removes a release-time supply-chain concern. The cost is
    ~50KB in the .wasm; acceptable.
  tags: [v0.8, viz, offline]

- id: DEC-045
  type: design-decision
  title: Visualiser is itself instrumented by witness in CI
  status: draft
  description: >
    The visualiser is a wasm32-wasip2 Component; witness measures
    Components; therefore witness measures the visualiser. The CI
    pipeline runs Playwright against an instrumented witness-viz
    and ratchets the resulting MC/DC % as a release gate. Closes
    the spec-driven loop: the visualiser's requirements verify
    themselves, in production, through their own tooling.
  tags: [v0.8, viz, dogfood, mcdc]
```

---

## 8. Risk register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `wstd-axum` does not yet support `axum 0.8` features we need (e.g., `Query` extractor, `IntoResponse` for Maud) | Medium | High — blocks the wasi build | Pin axum + wstd-axum versions known to compose; ship a fallback `feature = "native"` build that uses tokio + hyper directly so dev iteration never blocks on wstd-axum upstream; upstream PRs to wstd-axum where missing |
| `wasmtime serve --dir` mount semantics on Windows / non-Unix hosts | Medium | Medium | Document the Linux/macOS path; on Windows, fall back to embedded mode (`witness serve --report run.json`) which loads the file in the host CLI and pipes bytes to the component via WIT |
| HTMX 2.x bundle bloat in the .wasm | Low | Low | HTMX min is ~14KB gzipped; `include_bytes!` bloats the .wasm by its raw size (~50KB). Acceptable. Re-evaluate at v0.9 if total Component size exceeds 2MB |
| Playwright `webServer` pattern breaks because `wasmtime serve` does not accept a `Ctrl-C` cleanly when invoked by Node child_process | Medium | Medium | Wrap in a tiny `wasmtime-supervisor` script that handles SIGINT; or run wasmtime under `tini`. Rivet's experience here is `cargo run` which is well-behaved; we adopt rivet's wrapper pattern |
| MC/DC view becomes unreadable at >12 conditions | Medium | Medium | Provide a "compact mode" that collapses identical short-circuit prefixes; show only the rows that change between adjacent conditions; offer JSON export for offline analysis |
| AI-agent surface gets prompt-injected via decision titles or test names that come from arbitrary user code | Medium | Medium | Treat all decision metadata as untrusted; HTML-escape via Maud's auto-escaping (default-on); for MCP responses, return only structured fields, never reflect user-supplied free text into tool descriptions |
| The "visualiser visualises itself" demo is too clever to debug when it fails | Medium | Low | The dogfood loop runs in CI but is not the only test surface; the Playwright suite is the load-bearing verification, the self-coverage step is reporting-only |
| Component-model coverage shape (v0.6 work) changes the run-record schema, breaking the v0.8 visualiser | Low | High | The v0.5 schema is the contract until v0.8 ships; if v0.6/v0.7 require a schema bump, version the visualiser alongside. Document the schema-version field as load-bearing in DEC-042 |
| Rivet's `format!`-based templating is faster to copy than Maud's macro approach, tempting the implementer to skip Maud | Medium | Low | Decision is recorded in DEC-041 with explicit rationale; PR review enforces |
| Browsers without modern HTMX support (corporate IE-mode environments, etc.) | Low | Low | Out of scope. Document Chromium / Firefox / Safari latest as the support matrix; CI runs Chromium via Playwright |
| The "embed witness-viz inside witness host" composition breaks because two components want different `wasi:http/proxy` versions | Medium | Medium | Pin the wasi-http version in both crates' Cargo.tomls; use `wac` resolution explicitly; document in DEC-040 |

---

## Sources

- [Wasmtime: Examples for WASIp2](https://docs.wasmtime.dev/examples-wasip2.html)
- [Rust target reference: wasm32-wasip2](https://doc.rust-lang.org/nightly/rustc/platform-support/wasm32-wasip2.html)
- [`wstd-axum` on docs.rs](https://docs.rs/wstd-axum/latest/wstd_axum/)
- [`wstd-axum` on lib.rs](https://lib.rs/crates/wstd-axum)
- [`wasmtime_wasi_http` API](https://docs.wasmtime.dev/api/wasmtime_wasi_http/index.html)
- [Bytecode Alliance: invoking component functions in wasmtime CLI](https://bytecodealliance.org/articles/invoking-component-functions-in-wasmtime-cli)
- [Wasmtime docs: running components](https://component-model.bytecodealliance.org/running-components/wasmtime.html)
- [`bytecodealliance/sample-wasi-http-rust`](https://github.com/bytecodealliance/sample-wasi-http-rust)
- [`SaritNike/wasm-component-model-runner`](https://github.com/SaritNike/wasm-component-model-runner)
- [Building a fast website with the MASH stack in Rust (Schwartz)](https://emschwartz.me/building-a-fast-website-with-the-mash-stack-in-rust/)
- [Trying out HTMX with Rust (Finnie)](https://www.joshfinnie.com/blog/trying-out-htmx-with-rust/)
- Rivet repository at `/Users/r/git/pulseengine/rivet/` — `rivet-cli/src/serve/` (Axum + HTMX + inline assets pattern), `tests/playwright/playwright.config.ts` (webServer pattern), `tests/playwright/helpers.ts` (waitForHtmx), `tests/playwright/coverage-view.spec.ts` (spec template)
- Witness internals: `DESIGN.md`, `docs/research/v05-component-witness.md` (Component-Model boundary already proven), `docs/research/v05-lcov-format.md`, `docs/research/mcdc-bytecode-research.md`
