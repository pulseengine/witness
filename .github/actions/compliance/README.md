# Witness Compliance Evidence

Composite action invoked from release.yml to generate the release-time
evidence bundle. Mirrors the rivet `.github/actions/compliance` shape
adapted to witness's domain (coverage reports, in-toto coverage
predicates, branch manifests).

## Inputs / outputs

See `action.yml`. The defaults work for a no-coverage release (just
build artifacts); set `run-json` and `modules` for full evidence.

## Wiring into release.yml

```yaml
- uses: ./.github/actions/compliance
  id: evidence
  with:
    report-label: ${{ github.ref_name }}
    run-json: ""
    modules: "[]"
```

## Schema URLs

- `https://pulseengine.eu/witness-coverage/v1` - predicate type
- `https://pulseengine.eu/witness-rivet-evidence/v1` - rivet evidence schema
