# Security model — what witness signatures prove and what they do not

This document covers the threat model for two parallel signing chains
that ship with witness today:

1. **The predicate-envelope chain** (introduced in v0.6.4, extended in
   v0.10.0): DSSE-signed in-toto Statements carrying coverage / MC/DC
   evidence, signed with an ephemeral Ed25519 key minted per release.
2. **The release-tarball chain** (introduced in v0.10.0): cosign +
   Fulcio + Rekor keyless signatures over each binary tarball,
   compliance-evidence archive, and the WASI reporter component.

The two chains are complementary, not redundant. They prove different
things; a strict consumer verifies both. A casual consumer can verify
either and still get a useful integrity claim.

This file should be read alongside `docs/concepts.md` §6 (the signed
evidence chain at the conceptual level) and the v0.10.0 proposal at
`docs/proposals/v0.10.0.md` for the threat-model thinking that
motivated the v0.10 cosign work.

## What gets signed

### v0.9.x and v0.10.x: Predicate envelopes (DSSE + ephemeral Ed25519)

`witness predicate` builds an in-toto Statement; `witness attest`
wraps it in a DSSE envelope and signs with an Ed25519 secret key;
`witness verify` checks the envelope against the matching public key.

Two predicate types ship:

- `https://pulseengine.eu/witness-coverage/v1` — branch-coverage
  summary (total, covered, per-function tallies, uncovered list).
  This is the v0.6.4+ default; suitable for codecov-style consumers.
- `https://pulseengine.eu/witness-mcdc/v1` — full MC/DC truth tables,
  per-decision verdicts, condition pairs with interpretation, plus a
  `report_sha256` binding the canonical-JSON form of the report to
  the envelope payload. Added in v0.10.0; the artefact a regulator
  actually wants signed.

Statement subjects are populated as a tuple. The first subject is the
**instrumented** Wasm module's SHA-256 — what was measured. From
v0.10.0 onward a second subject is added: the **original**
(pre-instrumentation) module's SHA-256, recorded by `witness
instrument` into the manifest as `original_module_sha256` and read
back by `witness predicate`. Pre-v0.10.0 envelopes had this field
present-but-null; v0.10.0 fixes that (closes E1 BUG-3).

The signing primitive itself is unchanged from v0.6.x. Ed25519
(RFC 8032), 64-byte secret (32-byte seed + 32-byte public per the
`ed25519-compact` convention), 32-byte public key, 64-byte signature
base64-encoded inside the DSSE envelope's `signatures[].sig` field.
DSSE PAE wrapping per the in-toto spec.

The release pipeline mints a fresh keypair every time. See
`.github/actions/compliance/action.yml` for the keygen step that
seeds `verifying-key.pub` next to the envelope inside
`compliance-evidence.tar.gz`. There is no long-term "witness signing
key" to rotate, revoke, or worry about leaking.

### v0.10.x: Release tarballs (cosign + Fulcio + Rekor)

From v0.10.0 the release workflow at `.github/workflows/release.yml`
adds keyless cosign signing for every release asset. The job at
`create-github-release` requests an OIDC `id-token` (line 210),
installs `sigstore/cosign-installer@v3` (line 232), and runs
`cosign sign-blob --yes` against each artefact in `release-assets/`
(lines 236-249).

For each asset the workflow attaches two extra files to the GitHub
Release:

- `<asset>.sig` — the cosign signature.
- `<asset>.cert` — the Fulcio code-signing certificate that binds the
  signature to this workflow's OIDC identity. The certificate's
  Subject Alternative Name is
  `https://github.com/pulseengine/witness/.github/workflows/release.yml@refs/tags/<TAG>`
  and its issuer is `https://token.actions.githubusercontent.com`.

Cosign also writes a transparency-log entry to the public Rekor
instance at `https://rekor.sigstore.dev`. Verification re-checks the
log entry, so a downstream consumer learns whether the certificate
was logged at signing time.

There is no long-term key custody on this chain either — the Fulcio
certificate is short-lived (10 minutes) and signs a one-shot blob.
The trust root is "GitHub's OIDC issuer plus the public Rekor log,"
not a private key file.

### Both chains ship; they prove different things

| Chain | Binds | Trust root | What a forgery requires |
|-------|-------|------------|-------------------------|
| Predicate DSSE | `(report, instrumented module sha256, original module sha256, ephemeral pubkey)` | The bundle ships its own `verifying-key.pub`; closed under self-verification | Compromise the GitHub Actions runner *during* the release, OR substitute the public key in the bundle (defeats the closed-loop self-verification but produces a bundle whose contents disagree with the upstream release page) |
| Cosign / Fulcio / Rekor | `(tarball sha256, workflow identity, tag)` | GitHub OIDC issuer + Sigstore Fulcio CA + Rekor transparency log | Compromise GitHub OIDC, Fulcio, or Rekor (collectively); OR push a malicious commit to a tag-pointed ref before the workflow runs |

A consumer who only verifies the tarball signature gets a strong
"this came from the witness release pipeline" claim but no insight
into the predicate body. A consumer who only verifies the predicate
envelope gets a strong "the truth table matches the signed evidence"
claim but no anchor for the public key. Verifying both ties the two
ends together: the cosign signature attests the tarball, and the
predicate envelope inside that tarball attests the evidence body.

## Threat model

### Threats addressed

- **Compromised maintainer machine.** The release workflow runs on
  GitHub-hosted runners; neither chain involves a key file under the
  maintainer's control. A maintainer with leaked SSH keys cannot
  forge a release without also pushing a malicious tag commit, and
  the Fulcio certificate's SAN binds to the workflow path so the
  forgery would be visible in the certificate.
- **Predicate substitution.** The DSSE signature covers the full
  Statement payload. Swapping the embedded coverage report, run
  record, MC/DC truth table, or measurement metadata invalidates the
  signature. For the MC/DC predicate, `report_sha256` is itself a
  field inside the signed body — tampering with `report.json` next to
  the envelope is detected by `witness verify --check-content` (added
  in v0.10.0).
- **Tarball tampering after release.** A `cosign verify-blob` failure
  is the canonical detection. The Fulcio certificate's SAN must
  match the verifier's expected workflow identity exactly; a
  re-signed tarball under a different identity fails the certificate
  check.
- **Replay across modules.** The Statement's first subject is the
  SHA-256 of the instrumented Wasm module that produced the report.
  An attacker cannot detach the signed envelope and re-attach it to
  a different module — the subject digest would not match. From
  v0.10.0 onward the second subject (the original module SHA-256)
  closes the same check on the pre-instrumentation side, so a
  re-instrumentation of *different source* under the same instrument
  command is detected.
- **Rekor log tampering.** `cosign verify-blob` re-checks the
  transparency-log inclusion proof against the public Rekor instance
  by default. A signature without a Rekor entry, or with a forged
  entry, fails verification.
- **Rebuild-time clock divergence.** `predicate.rs` honours
  `SOURCE_DATE_EPOCH` for the `measured_at` field (v0.10.0). The
  release workflow exports `SOURCE_DATE_EPOCH` from the tag commit's
  Unix timestamp (`.github/workflows/release.yml` lines 142-151,
  added in v0.10.4), so byte-identical predicate envelopes are
  reproducible from a clean rebuild of the same tag.
- **Key rotation against the predicate chain.** Each release mints a
  fresh ephemeral keypair, so "rotation" is automatic; there is no
  durable signing identity to revoke. The cosign chain inherits
  this property from Fulcio's short-lived certificates.

### Threats NOT addressed (and why)

- **The signature does not prove the source is the source you
  think.** It binds to the *Wasm module's digest*. Reproducing the
  source-to-Wasm chain requires the surrounding build provenance —
  a SLSA L3 provenance attestation, sigil's Wasm-build predicate,
  reproducible cargo-build pinning. Witness composes with that
  chain; it does not replace it.
- **The signature does not prove the test inputs are
  representative.** It signs the coverage that the supplied test
  rows produced. A test suite that hits 100% of branches but doesn't
  represent the operational profile is still valid coverage
  evidence; the gap is in the test design.
- **The signature does not prove rustc / LLVM lowering preserved
  condition independence.** That is the v0.2 paper's coverage-
  lifting argument, addressed via the "witness-and-checker" stance
  (DEC-010): rustc emits the DWARF metadata that witness's
  reconstruction trusts; v1.0's Check-It-pattern qualification will
  close that loop with a small qualified checker. v0.9.x's
  `witness-mcdc-checker` extracted crate (~70 LoC, no runtime deps)
  is the audit-in-isolation surface for the independent-effect-pair
  logic.
- **The cosign chain does not protect against compromise of the
  GitHub Actions runner *during* the release.** If the runner is
  compromised mid-job, both the predicate signing and the cosign
  signing happen under attacker control. This is structural for
  every CI-based signing scheme; the mitigation is GitHub-side
  hardening (branch protection, required reviews, runner image
  attestations) and is out of scope for witness itself.
- **`sigstore/cosign-installer@v3` is a tag-major reference,
  not a pinned SHA.** A malicious force-push to the `v3` tag (which
  the cosign team controls, not us) could swap the cosign binary at
  install time. We accept this trade-off because pinning to a
  digest creates a rotation burden the project does not have
  capacity to carry. Consumers who need stricter pinning should
  vendor a fork of `release.yml` with the cosign-installer SHA
  pinned.

### Threats deferred to v0.11+

- **macOS Developer ID signing + Apple notarisation.** Today
  v0.10.x macOS tarballs ship unsigned at the Apple-platform level;
  users have to `xattr -d com.apple.quarantine` after download.
  Planned for v0.11; requires the maintainer's Developer ID
  certificate plumbing and an Apple notary submission step. Tracked
  in the v0.10.0 proposal as item 20.
- **Reproducible cargo dependency graph.** The release workflow
  pins direct workspace dependencies via `Cargo.toml`'s
  `[workspace.dependencies]` block but does not pin transitive deps
  beyond what `Cargo.lock` records. Reproducing a v0.10.x release
  requires the committed `Cargo.lock` plus the same crates.io index
  state at build time. crates.io is append-only in practice;
  yanked-version recovery is a v0.11+ concern.
- **Predicate envelope Rekor binding.** The current `witness attest`
  signs locally with the ephemeral Ed25519 key and writes the
  envelope to disk. It does not push anything to Rekor. v0.12+ will
  integrate `cosign attest` (or equivalent) so the predicate
  envelope itself gets a Rekor entry, closing the "no public log
  for the envelope" gap. Until then, the public log surface is the
  cosign signature over the *tarball* (which contains the envelope)
  rather than the envelope directly.
- **WIT-bindgen FFI surface fuzzing.** `crates/witness-component`
  is the WASI 0.2.9 reporter component (renamed in v0.9.9 from
  `witness-component-...wasm` to `witness-reporter-component-...wasm`
  to clarify it is not an instrumentable target). Its FFI surface
  has unit-test coverage but no fuzzing harness. Tracked for v0.11.

## Verifying a witness release

### Step-by-step: cosign verify-blob

For every asset on a v0.10.x GitHub Release:

```sh
cosign verify-blob \
  --certificate-identity 'https://github.com/pulseengine/witness/.github/workflows/release.yml@refs/tags/v0.10.0' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  --signature witness-v0.10.0-x86_64-apple-darwin.tar.gz.sig \
  --certificate witness-v0.10.0-x86_64-apple-darwin.tar.gz.cert \
  witness-v0.10.0-x86_64-apple-darwin.tar.gz
```

Replace `v0.10.0` with the tag you downloaded; replace the
asset name with the asset you want to verify. The same command
shape applies to:

- Each platform tarball (`witness-<TAG>-<TARGET>.tar.gz` or `.zip`).
- The compliance-evidence archive
  (`witness-<TAG>-compliance-evidence.tar.gz`).
- The WASI reporter component
  (`witness-reporter-component-<TAG>-wasm32-wasip2.wasm`).

`cosign verify-blob` exits 0 on success and prints
`Verified OK`. On failure it prints the specific check that failed
(certificate identity mismatch, OIDC issuer mismatch, signature
mismatch, or Rekor inclusion proof failure) and exits non-zero.

### Step-by-step: witness verify

After unpacking the compliance-evidence archive, the bundle contains
one DSSE envelope per verdict plus a `verifying-key.pub`. To verify
each envelope:

```sh
witness verify \
  --envelope verdicts/leap-year/signed.dsse.json \
  --public-key verifying-key.pub
```

`witness verify` exits 0 on success and prints
`predicateType: https://pulseengine.eu/witness-coverage/v1` (or
`witness-mcdc/v1`) on success. To additionally validate that the
inline MC/DC report matches the signed `report_sha256`:

```sh
witness verify --check-content \
  --envelope verdicts/leap-year/signed.dsse.json \
  --public-key verifying-key.pub \
  --report verdicts/leap-year/report.json
```

`--check-content` is the v0.10.0 addition that closes the predicate-
substitution attack surface for the MC/DC chain.

### Combined: prove tarball provenance + predicate integrity in one CI job

A consumer who wants the strongest claim chains the two:

```sh
# 1. Verify the cosign signature on the compliance-evidence tarball.
cosign verify-blob \
  --certificate-identity 'https://github.com/pulseengine/witness/.github/workflows/release.yml@refs/tags/v0.10.0' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  --signature witness-v0.10.0-compliance-evidence.tar.gz.sig \
  --certificate witness-v0.10.0-compliance-evidence.tar.gz.cert \
  witness-v0.10.0-compliance-evidence.tar.gz

# 2. Unpack and verify each predicate envelope inside.
tar -xzf witness-v0.10.0-compliance-evidence.tar.gz
cd compliance-evidence
for env in verdicts/*/signed.dsse.json; do
  witness verify --check-content \
    --envelope "$env" \
    --public-key verifying-key.pub \
    --report "$(dirname "$env")/report.json"
done
```

If both steps return success, the consumer knows:

1. The tarball came from the witness release workflow at the named
   tag (cosign + Fulcio + Rekor).
2. The ephemeral public key inside the tarball was the one written
   by that workflow (it sits next to the signatures it signed).
3. Every signed predicate inside the bundle is byte-identical to
   what the workflow signed (DSSE).
4. Every report's MC/DC content matches the `report_sha256` field
   inside the signed predicate (`--check-content`).
5. Every Statement's instrumented and original-module subjects are
   the SHA-256 digests of the modules that actually produced the
   evidence.

Anything an attacker substitutes downstream — tarball, envelope,
public key, report, manifest, or instrumented module — fails one of
the five checks.

## Reporting vulnerabilities

If you find a vulnerability in witness's instrumentation,
verification, or signing path, please open a GitHub issue at
<https://github.com/pulseengine/witness/issues> or email the
maintainer at `security@pulseengine.eu`. Do not file a public
exploit before a fix is available; coordinate disclosure on a
reasonable timeline (90 days is the default expectation, sooner
for actively-exploited findings).

The most security-relevant code paths:

- `crates/witness-core/src/attest.rs` — DSSE signing + verification.
- `crates/witness-core/src/predicate.rs` — Statement construction,
  SHA-256 subject digests, `SOURCE_DATE_EPOCH` honouring,
  `report_sha256` binding for the MC/DC predicate.
- `crates/witness-core/src/instrument.rs` — Wasm rewrite. A bug here
  could change observable program behaviour (REQ-004 invariant);
  fuzz-style adversarial inputs are particularly welcome.
- `crates/witness-mcdc-checker/src/lib.rs` — the no-deps
  independent-effect-pair logic, audit-in-isolation surface for the
  MC/DC verdict.
- `.github/workflows/release.yml` — release pipeline, including the
  cosign signing step (lines 223-249) and the `SOURCE_DATE_EPOCH`
  derivation (lines 142-151). Findings against the workflow shape
  itself (not just the binaries it produces) are in scope.
