# C++ STL short-circuit — inline chain depth probe

## What this demonstrates

Goal: exercise `std::any_of` / `std::all_of` short-circuiting
to demonstrate v0.14's inline-chain tracker captures
multi-level STL inlines. The lambda predicate gets inlined
into the iterator loop, which is inlined into the caller —
ideally producing chains of depth 3+.

**Reality**: deep STL inline chains require `-O1`+, which
trips the wasm-ld DWARF address-relocation gap and collapses
the line program. At `-O0` STL functions aren't inlined, so
chains stay shallow. This fixture exposes the
optimisation-vs-DWARF tradeoff cleanly.

## How to run

```sh
# Default OPT=-O0 — keeps DWARF intact, no deep chains.
WASI_SDK_PATH=~/.local/opt/wasi-sdk-33.0-arm64-macos ./build.sh
witness instrument check.wasm -o inst.wasm

# Try OPT=-O1 to see the wasm-ld DWARF collapse.
OPT=-O1 WASI_SDK_PATH=~/.local/opt/wasi-sdk-33.0-arm64-macos ./build.sh
witness instrument check.wasm -o inst-o1.wasm
```

## v0.21 results (verified 2026-05-15)

| Build | Branches | Decisions | Inline contexts | Chains of depth ≥ 2 |
|---|---|---|---|---|
| `-O0` | 491 | 78 | 105 | **26** ✅ |
| `-O1` | 649 | 0 | 0 | 0 |

At `-O0`, **26 of 105 inline contexts have chains of depth 2**
— all inside `printf_core`, where vfprintf.c's macro
expansions inline into the dispatch function. Example:

```
id=249 fn='printf_core'
    chain (2): vfprintf.c:232 -> vfprintf.c:165
```

This is real cross-CU inline tracking working on a non-Rust
toolchain, which is the v0.14 chain feature's whole point.

## The STL-specific signal that didn't fire

The original goal — chains showing `lambda → std::any_of →
caller` — needed `-O1` for STL functions to inline. With `-O1`
hitting the wasm-ld gap (0 decisions, 0 chains), the chain
data isn't there.

Unblocking would need either:
- Upstream wasm-ld fix for DWARF address relocation at `-O1`+
- A custom `-O0 -finline-functions=any_of,all_of` build (not
  a standard clang flag — would require an LLVM pass plugin)

Documented as a limitation of the current toolchain stack.

## What this fixture proves anyway

- v0.14 inline chains DO populate on real C++ code (26 depth-
  2 chains in printf_core)
- The chain tracker is language-agnostic (Rust verdicts/,
  TinyGo leap-year, and now C++ all populate chains)
- The `-O1` upstream gap is reproducible on any wasi-sdk
  binary — not a property of a specific source pattern

## Cross-language placement

Sub-fixture under `examples/languages/cpp/` — C++ Tier A
status unchanged. The depth-2 chains are an additive data
point.
