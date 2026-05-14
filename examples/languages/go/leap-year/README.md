# Go (TinyGo) — leap-year fixture (cross-language probe)

## What this demonstrates

First TinyGo / Go probe for witness. Builds the canonical
leap-year predicate via `tinygo build -target wasm-unknown
-opt 1`, then runs it through witness.

Background: the standard `go build` for wasm produces output
without DWARF debug info, so witness can't attribute branches
to source. TinyGo uses LLVM as its backend AND emits DWARF —
making it the practical path for Go MC/DC coverage on wasm.

## How to run

```sh
./build.sh                                # produces leap.wasm via tinygo
witness instrument leap.wasm -o inst.wasm
```

## v0.19 results (verified 2026-05-14, TinyGo 0.41.1)

| Metric | Value |
|---|---|
| Branches | 23 |
| Decisions | **4** ✅ |
| leap.go decisions | **2** (at line 28, two inline instances) ✅ |
| TinyGo runtime decisions | 2 (`float.go:118`, `float.go:153`) |
| `chain_kind` detection | `or` on one of the leap.go decisions ✅ |
| Inline contexts populated | 2 ✅ |
| Inline chains populated | 2 ✅ |

This is the strongest cross-language probe so far: TinyGo
gives us **(a)** correct source attribution to `leap.go:28`,
**(b)** working chain-kind heuristic, **(c)** real inline-chain
tracking exposing two distinct call sites of `leapYear`
(inlined into `main` + the exported `run` function), and
**(d)** runtime-internal branch coverage on TinyGo's `float.go`
math primitives.

## The two leap.go decisions are inline copies

`leapYear` is `//go:noinline` in our source, but TinyGo at
`-opt 1` still emitted two instances — one likely from a
direct call in `main`'s init path, one from the exported
`run` function. Each instance has its own br_if pair
clustering into a Decision at the same source line. This is
exactly the multi-context scenario that v0.13's `inline_context`
tagging + v0.14's chain tracking were designed for. The
per_context verdict view in mcdc-v3 envelopes can distinguish
these.

## What TinyGo handles well

- **DWARF survives linking** — TinyGo's LLD-based wasm linker
  preserves both the line program AND populates inlined-
  subroutine address ranges. This is the cleanest DWARF we've
  seen on a non-Rust toolchain.
- **Source attribution exact** — `leap.go:28` matches the
  predicate line in source.
- **`chain_kind = or` fires** — same heuristic that classifies
  rustc's `||` correctly recognises Go's `||` lowering.
- **Inline chains populate** — proves witness's v0.14 inline-
  context tracking is fully cross-language; not rust-specific.

## What's worth noting

- **Decisions on TinyGo runtime code** (`float.go:118` /
  `float.go:153`) — TinyGo's runtime ships with embedded
  conditional logic in its math primitives. Witness sees
  these as legitimate decisions. For DO-178C structural-
  coverage purposes, runtime-internal decisions count
  (you must show the runtime's branches were exercised by
  your test inputs).
- **Two leap.go decisions, not one** — TinyGo replicates
  `leapYear` to multiple call sites. The per-context view
  is the right way to read this.

## Cross-language placement

Promoting Go (via TinyGo) from Tier C (untested) → Tier A
(verified end-to-end). The matrix in `docs/cross-language.md`
notes TinyGo as the Go path; standard `go build` for wasm
emits no DWARF and remains in Tier D.
