---
name: MC/DC reconstruction surprise
about: The MC/DC report shows numbers (proved/gap/dead/full) that don't match what you expect from the source code.
title: "mcdc: <one-line description of the surprise>"
labels: ["bug", "mcdc"]
assignees: []
---

## What I expected vs what I got

<!-- e.g. "expected 4 conditions for `(a && b) || c`; report shows 3"
     or "every row of the truth table has the same condition vector
     and I think they should differ" -->

## Source-level decision

<!-- The Rust expression. If you can, isolate it to a tiny no_std
     fixture you can share. -->

```rust
fn predicate(a: bool, b: bool, c: bool) -> bool {
    (a && b) || c
}
```

## How you exercised it

<!-- All `--invoke` / `--invoke-with-args` arguments, or your
     witness-harness-v1/v2 snapshot if you used --harness. -->

## Report excerpt

```
witness report --input run.json --format mcdc
# (paste the relevant decision)
```

## Manifest excerpt for the same decision

```json
{
  "id": ...,
  "conditions": [...],
  "source_file": "...",
  "source_line": ...,
  "chain_kind": "..."
}
```

## What you have already ruled out

<!-- e.g. "ran witness with -vv and confirmed the trace memory has
     the expected kind=0 records" or "tried with WITNESS_TRACE_PAGES=64
     and the numbers didn't change" -->
