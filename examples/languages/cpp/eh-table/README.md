# C++ EH / switch — br_table audit probe

## What this demonstrates

Two layered findings:

1. **wasi-sdk libcxx is built without C++ exception support**
   — `__cxa_throw` is undefined at link time. The build.sh
   first tries `-fwasm-exceptions` (native wasm EH) and falls
   back to `-fno-exceptions` with integer error codes when
   wasi-sdk can't resolve the runtime.

2. **Witness's v0.9.7 br_table audit pass clusters switch
   arms correctly** — both in our hand-written 4-arm switch
   AND in vfprintf's 57-arm format-specifier dispatch.

## How to run

```sh
WASI_SDK_PATH=~/.local/opt/wasi-sdk-33.0-arm64-macos ./build.sh
witness instrument parser.wasm -o inst.wasm
```

## v0.21 results (verified 2026-05-15, fallback path)

| Metric | Value |
|---|---|
| Branches | 491 |
| `br_table_target` branches | 85 |
| `br_table_default` branches | 4 |
| `br_if` branches | 402 |
| Decisions | 80 |
| **br_table-only Decisions** | **4** ✅ |

The 4 br_table-only decisions:
- `parse_token` switch: 4 conditions (3 targets + 1 default) ✅
- `vfprintf.c:608` switch: 57 conditions (format specifier
  dispatch)
- `vfprintf.c:616` switch: 9 conditions
- `vfprintf.c:137` switch: 19 conditions

The first cluster maps exactly to our 3-case + default switch
in `parse_token`. The other three are libc internals.

## Why the `-fno-exceptions` fallback still tests the audit

The two source variants (with-EH and no-EH) produce the same
wasm `br_table` over `kind` — the EH version's landing pad
becomes a switch over thrown exception type IDs, but only at
the landing pad. Either way, the `parse_token` body's `switch
(kind)` is a br_table that v0.9.7's audit pass clusters.

## What we'd need for the EH-specific decisions

A wasi-sdk build with `__cxa_*` shims provided (or a build of
libcxxabi linking the dynamic unwinder). Until then, native
wasm-EH via `-fwasm-exceptions` is the path — but wasi-sdk
33's libcxx doesn't ship those shims either. Documented gap;
the switch-table audit isn't blocked.

## Cross-language placement

Sub-fixture under `examples/languages/cpp/` — C++ Tier A
status unchanged.
