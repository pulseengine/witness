# Security model — what witness signatures prove and what they do not

This document covers the threat model for witness's signed coverage
evidence (DSSE-signed in-toto Statements, introduced in v0.6.4) and
the ephemeral-key approach the v0.6.x release pipeline uses.

## What a signature proves

A `signed.dsse.json` envelope in a release's `compliance-evidence/`
bundle is an in-toto Statement (predicate type
`https://pulseengine.eu/witness-coverage/v1`) wrapped in a DSSE
envelope and signed with an Ed25519 key. The `verifying-key.pub`
file in the same bundle is the corresponding public key.

If `witness verify --envelope X --public-key Y` returns `OK`, the
verifier knows:

1. **The predicate body is bit-identical to what was signed.** Any
   modification to the embedded coverage report, run record, or
   measurement metadata would invalidate the signature.
2. **The signer held the secret key matching `verifying-key.pub`.**
   For witness's release-time keys, that secret was generated and
   discarded inside the GitHub Actions runner that built the
   release.
3. **The Statement's `subject` is the SHA-256 of the instrumented
   Wasm module that produced the coverage report.** Any later
   re-instrumentation of the same source would produce a different
   subject digest.

A signed predicate verifies as a tuple — `(report, instrumented
module digest, key fingerprint)`. The release-time bundle ships the
`verifying-key.pub` so the tuple is closed: anyone with the bundle
can run the verify command without reaching for external state.

## What a signature does NOT prove

Equally important:

1. **The signature does not bind to a long-term identity.** The
   ephemeral key is fresh per release. Two different releases of
   the same module produce two different `verifying-key.pub`
   files. A consumer who wants long-term key continuity must layer
   sigstore Fulcio (or an equivalent) on top — that's v0.7+ work,
   not v0.6.x.
2. **The signature does not prove the source is the source you
   think.** It only binds to the *instrumented Wasm module's
   digest*. Reproducing the source-to-Wasm chain requires the
   surrounding build provenance (a SLSA L3 attestation, sigil's
   Wasm-build predicate, etc.) — composition with witness, not
   replacement of the surrounding chain.
3. **The signature does not prove the test inputs are
   representative.** It signs *the coverage that the supplied test
   rows produced*. A test suite that hits 100% of branches but
   doesn't represent the operational profile is still valid
   coverage evidence; the gap is in the test design, not in the
   signature.
4. **The signature does not prove the rustc / LLVM lowering
   preserved condition independence.** That's the v0.2 paper's
   coverage-lifting argument, addressed via the "witness-and-
   checker" stance (DEC-010): rustc emits the DWARF metadata that
   witness's reconstruction trusts; v1.0's Check-It-pattern
   qualification will close that loop with a small qualified
   checker.

## The ephemeral-key approach

For each release, the GitHub Actions runner:

1. Calls `witness keygen --secret /tmp/ek.sk --public verifying-key.pub`.
2. Signs every verdict's predicate with `/tmp/ek.sk`.
3. Bundles `verifying-key.pub` next to the signed envelopes.
4. Discards `/tmp/ek.sk` when the runner terminates.

The trade-off:

- **Pro:** no long-term key custody. There's no "the witness
  signing key" to rotate, revoke, or worry about getting leaked.
  Every release is self-contained.
- **Pro:** the verifying key in the bundle proves the signature
  came from the release pipeline that wrote that key — not from
  some external compromised actor with a stolen long-term key.
- **Con:** no cross-release continuity. If a downstream consumer
  wants "this is the same authority that signed v0.6.4 and
  v0.6.5", they need a higher-level binding (the GitHub release's
  signed metadata, or sigstore Fulcio's certificate trail).
- **Con:** if the GitHub Actions runner itself is compromised
  during the release, the signing key was compromised. Witness's
  signature only proves "this artefact was produced by this
  release's runner"; it doesn't transcend that trust boundary.

For v0.6.x, the ephemeral-key approach is the right scope:
- Witness is at the post-rustc Wasm measurement point. Long-term
  signing identity is the surrounding build pipeline's concern.
- Adopters who want sigstore Fulcio integration can compose
  witness's predicate with their own signing chain.
- Safety-critical adopters who require qualified key custody
  should generate their own per-release keys via their existing
  HSM / KMS infrastructure rather than relying on the GitHub
  ephemeral key.

## Key sizes and algorithms

- **Algorithm:** Ed25519 (RFC 8032). Witness uses the
  `ed25519-compact` Rust crate; the DSSE envelope follows the
  in-toto spec (`PAE` + `Sig.sigBase64`).
- **Secret key:** 64 bytes (32-byte seed + 32-byte public key per
  the ed25519-compact convention; raw, no PEM).
- **Public key:** 32 bytes raw.
- **Signature:** 64 bytes raw, base64-encoded inside the DSSE
  envelope.

PEM/DER input for keys is a v0.7+ extension. Adopters who need it
can convert their PEM-encoded Ed25519 keys to raw 64-byte format
externally (e.g. via `openssl pkey -in key.pem -outform DER` and
extracting the raw seed).

## Reporting security issues

If you find a vulnerability in witness's instrumentation,
verification, or signing path, please open an issue or contact
the maintainer (`security@pulseengine.eu`). Do not file a public
exploit before a fix is available.

The most security-relevant code paths are:

- `crates/witness-core/src/attest.rs` — DSSE signing + verification.
- `crates/witness-core/src/predicate.rs` — Statement construction
  and SHA-256 subject digest.
- `crates/witness-core/src/instrument.rs` — Wasm rewrite. A bug here
  could change observable program behaviour (REQ-004 invariant);
  fuzz-style adversarial inputs welcome.
