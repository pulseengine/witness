# C++ virtual dispatch — MC/DC null-result probe

## What this demonstrates

Doctrinal proof that **virtual dispatch is not a Decision** in
MC/DC sense. clang lowers `obj->virtualMethod()` to a vtable
load + `call_indirect` — witness's branch-detection pass
(`walk_collect`) explicitly does NOT count `call_indirect`,
because the receiver-type "branch" is runtime dispatch, not a
predicate the testcase can exercise via input selection.

## How to run

```sh
WASI_SDK_PATH=~/.local/opt/wasi-sdk-33.0-arm64-macos ./build.sh
witness instrument shapes.wasm -o inst.wasm
```

## v0.21 results (verified 2026-05-15)

| Metric | Value |
|---|---|
| Total branches | 627 |
| Total Decisions | 107 |
| Branches attributed to `Shape`/`Square`/`Rect`/`Triangle`/`Circle`/`compute_area`/`run_` | **0** ✅ |
| Inline contexts populated | 194 |

The 107 Decisions all come from libc + libc++ runtime (printf,
ctors, EH unwinder skeletons) — same pattern as the
`leap-year-wasi` fixture. **None** are attributed to the
virtual-dispatch site, confirming the doctrine.

## Why this matters for DO-178C reviewers

A reviewer who counts every dynamic dispatch site as a
"decision" overcounts MC/DC obligations dramatically. Witness
treats them correctly: dynamic dispatch is a runtime fact, not
a structural decision. This fixture is the negative control
that proves the rule.

## Cross-language placement

C++ stays Tier A; this is a sub-fixture under
`examples/languages/cpp/` exercising a specific property.
