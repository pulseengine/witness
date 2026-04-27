---
name: Harness-mode question
about: Questions about `witness run --harness <cmd>`, the harness-v1/v2 wire format, or driving witness from a non-wasmtime runtime.
title: "harness: <one-line description>"
labels: ["question", "harness"]
assignees: []
---

## What you're trying to do

<!-- e.g. "drive witness from a Node WASI harness for an embedded
     board that can't run wasmtime" or "extend our existing kiln
     harness to produce v2 snapshots for full MC/DC" -->

## Schema version you're targeting

- [ ] `witness-harness-v1` (counters only — branch coverage)
- [ ] `witness-harness-v2` (counters + brvals + brcnts + trace — MC/DC)

## What blocks you

<!-- e.g. "the v2 trace_b64 field — how do I read __witness_trace
     memory in <my-runtime>?" or "the schema dispatch fails with
     <error message>" -->

## What you've tried

```bash
# Commands or snapshot snippets.
```

## Snapshot you produced

```json
{
  "schema": "witness-harness-v2",
  "counters": { ... },
  "rows": [ ... ]
}
```

## Reference

The full schema is documented in the README's
"Harness-mode protocol" section. v1 was introduced in v0.6.0;
v2 in v0.9.5.
