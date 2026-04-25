# Sigil predicate-format brief (for witness v0.3)

Investigation of how `sigil` at `/Users/r/git/pulseengine/sigil` handles
in-toto attestation predicates, conducted 2026-04-25, to inform witness
v0.3's coverage-predicate emission.

## Executive summary

**Sigil accepts any predicate-type URI and treats predicates as opaque
JSON.** There is no built-in registry, no schema validation, no dispatch
table per type. Witness v0.3 can emit a coverage predicate today with
no sigil-side change. Recommended type URL:
`https://pulseengine.eu/witness-coverage/v1` (or
`https://wsc.dev/witness-coverage/v1` to align with the wsc.dev
namespace already used by sibling tools).

## Bundle / envelope structure

Sigil wraps in-toto Statements in DSSE (Dead Simple Signing Envelope)
form. On disk: JSON, typically `*.json`.

**Outer wrapper** — `src/lib/src/dsse.rs:32`:

```rust
pub struct DsseEnvelope {
    pub payload: String,        // base64-encoded JSON bytes
    pub payload_type: String,   // "application/vnd.in-toto+json"
    pub signatures: Vec<DsseSignature>,
}
```

**Inner payload** — in-toto Statement v1.0 per `src/lib/src/intoto.rs:29`:

```rust
pub struct Statement<P> {
    #[serde(rename = "_type")]
    pub statement_type: String,    // "https://in-toto.io/Statement/v1"
    pub subject: Vec<Subject>,
    pub predicate_type: String,    // arbitrary URI; not validated
    pub predicate: P,              // generic over payload type
}
```

`P` is parameterised — the deserialiser picks the type the caller asks
for; sigil itself uses `Statement<serde_json::Value>` for the generic
read path and lets callers re-deserialise into a typed body when they
recognise the `predicate_type`.

Signing uses ECDSA or Ed25519 over the PAE (Pre-Authentication
Encoding) of `payload_type` + `payload` bytes.

## Predicate-type discovery

There is no dispatcher. Predicates are read as opaque JSON; the caller
pattern-matches on `predicate_type` if it wants typed access.

Known type-URL constants in `src/lib/src/intoto.rs:287`:

```rust
pub mod predicate_types {
    pub const SLSA_PROVENANCE_V1: &str = "https://slsa.dev/provenance/v1";
    pub const SLSA_VSA_V1: &str = "https://slsa.dev/verification_summary/v1";
    pub const WSC_TRANSFORMATION_V1: &str = "https://wsc.dev/transformation/v1";
    pub const WSC_COMPOSITION_V1: &str = "https://wsc.dev/composition/v1";
    pub const WSC_TRANSCODING_V1: &str = "https://wsc.dev/transcoding/v1";
    pub const SPDX_DOCUMENT: &str = "https://spdx.dev/Document";
    pub const CYCLONEDX_BOM: &str = "https://cyclonedx.org/bom";
}
```

Unknown types deserialise and pass through opaquely. This is intentional
for forward compatibility.

## Existing pulseengine predicate types

| Type URL | Body shape (key fields) | Source |
|---|---|---|
| `https://wsc.dev/transformation/v1` | `source: {digest, signatureStatus}, compiler: {name, version}, target: {architecture, outputFormat}` | emitted by transformation tools (loom, kiln) |
| `https://wsc.dev/composition/v1` | `version, tool, tool_version, components: [{id, hash, source?}], integrator?` | `src/lib/src/composition/mod.rs:116` |
| `https://wsc.dev/transcoding/v1` | `source: {digest, signatureStatus, slsaLevel?}, compiler, target, compilationParameters?` | `src/lib/src/transcoding.rs:30` |

Convention: subject is the **output** artifact (e.g. compiled binary,
fused module). The source artifact appears in the predicate body's
nested `source` field.

## Subject convention for coverage attestations

Witness should put the **instrumented Wasm module** in the subject
field, with the original (pre-instrumentation) module digest in the
predicate body's `original_module` field — mirroring the
transcoding/transformation pattern.

```json
"subject": [
  {
    "name": "app.instrumented.wasm",
    "digest": {
      "sha256": "<sha256 of instrumented bytes>"
    }
  }
]
```

## Recommendation for witness v0.3

### Predicate type URL

`https://pulseengine.eu/witness-coverage/v1` — or, if pulseengine is
moving toward the wsc.dev namespace for tool ecosystems,
`https://wsc.dev/witness-coverage/v1`. Pick one; document it; do not
change post-v0.3 without bumping the type version.

### Predicate body schema

Based on witness's existing `Report` plus the metadata an auditor needs
to correlate the attestation with the original module:

```json
{
  "predicateType": "https://pulseengine.eu/witness-coverage/v1",
  "predicate": {
    "coverage": {
      "schema_version": "2",
      "total_branches": 42,
      "covered_branches": 35,
      "coverage_ratio": 0.833,
      "per_function": [
        {
          "function_index": 1,
          "function_name": "process",
          "total": 10,
          "covered": 9
        }
      ],
      "uncovered": [
        {
          "branch_id": 5,
          "function_index": 1,
          "instr_index": 42,
          "kind": "br_if"
        }
      ],
      "decisions": []
    },
    "measurement": {
      "harness": "cargo test --target wasm32-wasip1",
      "measured_at": "2026-04-25T10:30:00Z",
      "witness_version": "0.3.0"
    },
    "original_module": {
      "name": "app.wasm",
      "digest": {
        "sha256": "<sha256 of pre-instrumentation bytes>"
      }
    }
  }
}
```

### Worked example

Full DSSE-wrapped Statement (signature elided for brevity):

```json
{
  "payload": "<base64(statement_json)>",
  "payloadType": "application/vnd.in-toto+json",
  "signatures": [
    {
      "sig": "<base64(ed25519-sig)>",
      "keyid": "witness-signer-2026"
    }
  ]
}
```

Where the decoded payload is:

```json
{
  "_type": "https://in-toto.io/Statement/v1",
  "subject": [
    {
      "name": "app.instrumented.wasm",
      "digest": {"sha256": "abcdef..."}
    }
  ],
  "predicateType": "https://pulseengine.eu/witness-coverage/v1",
  "predicate": {
    "coverage": { /* witness::report::Report */ },
    "measurement": { "harness": "...", "measured_at": "...", "witness_version": "0.3.0" },
    "original_module": { "name": "app.wasm", "digest": {"sha256": "..."} }
  }
}
```

## CLI surface (proposed)

```bash
witness predicate \
  --run witness-run.json \
  --module app.instrumented.wasm \
  --original app.wasm \
  --output coverage-predicate.json
```

Outputs the **unwrapped Statement** (in-toto JSON, not yet a DSSE
envelope). Signing and DSSE wrapping are sigil's job — witness produces
the predicate body, sigil bundles and signs it. This keeps witness out
of the key-management business.

For users who want a one-shot signed bundle, witness can call out to
sigil:

```bash
witness predicate ... --output coverage-predicate.json
sigil attest --predicate coverage-predicate.json --output coverage.dsse.json
```

## Predicate-type registration

**No registration is needed.** Sigil reads any `predicateType` value
without validation. Witness can ship today.

## Open questions

1. **Multiple predicates per bundle.** In-toto allows one predicate per
   Statement. Composing witness's coverage with loom's
   transformation/v1 attestation requires either (a) two separate
   bundles cross-referenced by subject digest, or (b) a future
   bundle-of-bundles convention. Defer to v0.4.
2. **Schema validation.** Sigil accepts any JSON. If witness wants
   sigil to enforce the witness-coverage/v1 schema during verification,
   sigil would need a registry of `(type_url, json_schema)` pairs. v0.4
   concern.
3. **`pulseengine.eu/...` vs `wsc.dev/...` namespace.** The existing
   sibling predicates use `wsc.dev/`. Pick one for consistency before
   v0.3 ships.

## Cited sigil source

| File | Lines | What's there |
|---|---|---|
| `src/lib/src/intoto.rs` | 29–83 | `Statement<P>` definition |
| `src/lib/src/intoto.rs` | 85–98 | `Statement::from_json` / `from_json_bytes` — generic deserialisation, no type dispatch |
| `src/lib/src/intoto.rs` | 287–311 | `predicate_types` known-URL constants |
| `src/lib/src/dsse.rs` | 32–47 | `DsseEnvelope`, `DsseSignature` |
| `src/lib/src/dsse.rs` | 145–176 | `DsseEnvelope::verify` — payload extraction |
| `src/lib/src/composition/mod.rs` | 116–139 | wsc.dev/composition/v1 body shape |
| `src/lib/src/transcoding.rs` | 30–165 | wsc.dev/transcoding/v1 body shape |
| `src/lib/src/transcoding.rs` | 200–220 | example of subject = output, source-in-predicate convention |

Sub-agent's verbatim findings; file paths and the
"opaque-by-design" claim verified by the main thread before the brief
was committed.
