# C++ — leap-year fixture (cross-language probe)

## What this demonstrates

First C++ probe for witness. Builds the canonical leap-year
predicate via wasi-sdk's `clang++ --target=wasm32-wasip1 -O0
-std=c++20`, exercising:

1. **Template monomorphisation** — `leap_year<std::uint32_t>`
   is the function instantiation we expect to see in the
   manifest's function-name column.
2. **wasi-sdk linking** — same toolchain as the C wasi-sdk
   fixture; tests whether C++ adds anything beyond C.
3. **v0.19 IfThen clustering** — clang++ lowers `&&`/`||` to
   `if/else` + 1 `br_if` (same as clang C). Required for the
   predicate's 2 br_ifs to cluster.

## How to run

```sh
# Defaults to -O0 (the 79-decision case + visible template name).
WASI_SDK_PATH=~/.local/opt/wasi-sdk-33.0-arm64-macos ./build.sh
witness instrument leap.wasm -o inst.wasm

# To reproduce the -O1 upstream-blocked case:
OPT=-O1 WASI_SDK_PATH=~/.local/opt/wasi-sdk-33.0-arm64-macos ./build.sh
```

## v0.20 results (verified 2026-05-14)

| Metric | Value |
|---|---|
| Branches | 489 |
| Decisions | 79 |
| Inline contexts populated | 92 |
| Inline chains populated | 92 |
| Template instantiation visible | ✅ `bool leap_year<unsigned int>(unsigned int)` |
| leap_year cluster | 1 Decision (`chain_kind = or` ✅) |

Structural numbers match the C wasi-sdk fixture exactly because
both link the same libc and most decisions live there. The C++-
specific signal is the **`function_name` column**: the manifest
correctly preserves the demangled template instantiation
`bool leap_year<unsigned int>(unsigned int)`. That signal is
how C++ teams with deep template hierarchies will identify
which monomorphised copy a branch came from.

## The mangled-name signal

Each row in `manifest.branches[]` carries a `function_name`
field that gimli + walrus extract from the wasm name section
+ DWARF. For C++ this is the mangled-then-demangled signature.
Example from this fixture:

```
id=2 fn='bool leap_year<unsigned int>(unsigned int)' kind=br_if off=598
id=3 fn='bool leap_year<unsigned int>(unsigned int)' kind=br_if off=617
```

The DECISION groups these two br_ifs (`chain_kind = or`), but
the source-file label is unreliable here for the same reason
as the C wasi-sdk fixture: wasm-ld doesn't relocate DWARF
addresses per-CU, so `lookup_line(byte_offset)` returns
`__init_tls.c:47` rather than `leap.cpp:25`. **The function
name is the authoritative signal** when CU-line attribution
is fuzzy.

## What worked, what's gappy

✅ Template monomorphisation captured in function names — C++
   teams can audit per-template-instantiation
✅ `chain_kind = or` detection on the leap_year cluster
✅ Inline chains populate (92 entries — across libc + libc++)
✅ 79 Decisions reconstructed at `-O0` (full libc coverage)
⚠️ Source-file attribution unreliable (wasm-ld DWARF gap)
❌ `-O1+` still hits the same wasm-ld gap — 0 decisions

## What a deeper C++ probe should add

- **Virtual dispatch** — `call_indirect` shows up in branch
  counts but isn't a "decision" in MC/DC sense. A fixture
  with virtual functions would demonstrate the no-op for the
  decision-clustering view.
- **Exception handling** — wasm EH lowers to br_table-style
  arms; an explicit exception fixture would exercise the
  br_table decision pass.
- **Standard library short-circuits** — a fixture using
  `std::any_of` / `std::all_of` would exercise STL-inlined
  predicates; the inline chain tracker should make those
  visible.

## Cross-language placement

C++ promoted to Tier A: end-to-end works on this fixture.
Same upstream wasm-ld gap as C wasi-sdk caveats apply.
