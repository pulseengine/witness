# witness-viz Playwright suite

End-to-end tests that drive the `witness-viz` HTMX dashboard against a
real verdict-evidence bundle. This suite is the v0.9.0 self-coverage
backbone: every routing change, schema drift, and view-model break is
expected to surface here.

## Setup

```sh
npm install
npm run install-browsers
```

The default fixture is `/tmp/v081-suite/` (the v0.8.1 evidence bundle
shipped with witness). Override with:

```sh
export WITNESS_VIZ_FIXTURE=/path/to/verdict-evidence
```

The release binary at
`../../crates/witness-viz/target/release/witness-viz` must already be
built. Rebuild with `cargo build --release -p witness-viz` from the
workspace root if it is missing.

## Run

```sh
npm test
```

Headed, for debugging:

```sh
npm run test:headed
```

The Playwright `webServer` block boots `witness-viz` on port `3037`
against the fixture before the suite runs and tears it down on exit.
Tests run serially (`workers: 1`) against a single server instance.
