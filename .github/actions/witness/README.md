# `pulseengine/witness/.github/actions/witness`

Composite GitHub Action: instrument a Wasm module, run it, emit a
signed in-toto coverage predicate, and (optionally) attach the
evidence to a GitHub release.

## Versioning

Pin to a release tag. The composite action ships in lockstep with
the witness binary it downloads; both bump together.

```yaml
# Recommended for production — exact version pin, reproducible.
- uses: pulseengine/witness/.github/actions/witness@v0.10.4

# Convenience form — track the latest v0.10.x patch (mutable, but
# scoped to one major). Update the path when v0.11 ships.
- uses: pulseengine/witness/.github/actions/witness@v0.10
```

## 8-line adoption

```yaml
- uses: pulseengine/witness/.github/actions/witness@v0.10.4
  with:
    module: build/app.wasm
    invoke: |
      run_row_0
      run_row_1
      run_row_2
    upload-to-release: ${{ startsWith(github.ref, 'refs/tags/') }}
```

That's it. The action downloads the latest `witness` + `witness-viz`
release tarball for the runner's platform, runs the full pipeline,
and (on tag-push events) uploads the unsigned predicate, signed DSSE
envelope, and verifying public key to the matching GitHub release.

## Inputs

| Name | Default | Description |
|---|---|---|
| `witness-version` | `latest` | Pin to e.g. `v0.9.9` for hermetic runs. |
| `module` | _required_ | Path to the Wasm core module. |
| `invoke` | `""` | Newline-separated list of zero-arg exports. |
| `invoke-with-args` | `""` | Newline-separated list of `name:val,...` typed-arg specs (v0.9.6+). |
| `output-dir` | `witness-output` | Where artefacts land. |
| `predicate-name` | basename | Filename of the in-toto Statement. |
| `sign` | `true` | Generate ephemeral Ed25519 keypair + sign the predicate as a DSSE envelope. |
| `upload-to-release` | `false` | When tag-pushed, attach outputs to the matching GH release. |

## Outputs

| Name | Description |
|---|---|
| `output-dir` | Path to all outputs. |
| `predicate-path` | Unsigned in-toto Statement JSON. |
| `envelope-path` | Signed DSSE envelope (when `sign=true`). |
| `verifying-key-path` | Ed25519 public key (when `sign=true`). |

## Verification

A consumer of the resulting release can verify any envelope with the
shipped public key:

```bash
witness verify \
  --envelope app-signed.dsse.json \
  --public-key app-verifying-key.pub
```

Or with cosign (DSSE-standards-compliant):

```bash
cosign verify-blob --key app-verifying-key.pub app-signed.dsse.json
```

## Platforms

Linux x86_64 + aarch64, macOS x86_64 + aarch64. Windows runners need
a different release-asset URL pattern; PR welcome.
