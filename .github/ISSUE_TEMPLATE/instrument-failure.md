---
name: Instrument failure
about: `witness instrument` rejects, errors, or produces a manifest that doesn't match expectations.
title: "instrument: <one-line description>"
labels: ["bug", "instrument"]
assignees: []
---

## What happened

<!-- e.g. "instrument exited 1 with `not supported yet`" or
     "manifest has 0 decisions but the source has obvious if/else chains" -->

## What I expected

<!-- e.g. "manifest with branches and DWARF-grounded decisions" -->

## Reproduction

```bash
# Commands you ran. Include the witness version (`witness --version`) and
# the rustc target (`wasm32-unknown-unknown` vs `wasm32-wasip2` matters!).
```

## Module attributes

- witness version:
- rustc version:
- wasm target:
- module size (bytes):
- has DWARF debug info (compiled with `[profile.release] debug = true`)?
- is this a core module (`\0asm\01\00\00\00`) or a Component (`\0asm\0d\00\01\00`)?

## Manifest excerpt

<!-- Paste the first 30 lines of <module>.witness.json, especially the
     "branches" array head and "decisions" array head. -->

```json
```

## Logs

<!-- Run with `-v` for info-level logs and paste here. -->

```
```
