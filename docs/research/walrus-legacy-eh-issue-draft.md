# Panic in IR builder on legacy `try`/`catch`: synthesized catch-handler frame type not found (`local_function/mod.rs:1729`)

> **Filed 2026-05-22 as https://github.com/wasm-bindgen/walrus/issues/315**
> (the walrus repo moved `rustwasm/walrus` → `wasm-bindgen/walrus`).
> This file is the local working copy + provenance record; the filed
> version omits the "History" section and carries an AI-assistance
> footer instead.

**walrus versions affected:** 0.26.1 **and** 0.26.2 (latest) — identical panic
**Minimal repro:** 8 lines of `.wat`, no GC, below

## Summary

`Module::from_buffer` panics while building the IR for any function
containing a legacy-exceptions `try` whose `catch` / `catch_all`
handler has a non-trivial synthesized signature.

When walrus translates `Operator::Catch`, it synthesizes the handler
control-frame signature as `[tag param types] → [try block's end
types]` and resolves it via `InstrSeqType::existing` →
`ModuleTypes::find`. `find` **only matches an already-declared
`func` type in the type arena**. When the synthesized
`(params, results)` pair was never declared as a single `func`
type, `find` returns `None`, and the `.ok_or_else(...)` error is
`.unwrap()`-ed → panic.

This was introduced by **#293 ("feat: legacy exceptions")** — `git
log -S` blames the `push_control(BlockKind::Catch, …)` line to
commit `706692b`. It is **not** a GC bug (an earlier draft of this
report misattributed it to #304 — see "History" below).

## Minimal reproduction

```wat
(module
  (tag $e (param i32))
  (func $f (result i32)
    try (result i32)
      i32.const 0
    catch $e
    end))
```

```rust
let bytes = wat::parse_str(SRC)?;          // or wasm-tools parse
walrus::Module::from_buffer(&bytes)?;       // <-- panics
```

The module validates cleanly:
`wasm-tools validate --features=all` → OK.

## Expected

`Module::from_buffer` succeeds (the module is valid wasm), or at
worst returns an `Err` — not a panic.

## Actual

```
thread 'main' panicked at walrus-0.26.2/src/module/functions/local_function/mod.rs:1729:18:
called `Result::unwrap()` on an `Err` value: type: [I32] -> [I32]

Caused by:
    attempted to push a control frame for an instruction sequence with a type that does not exist
```

Note the frame type is `[I32] -> [I32]` — no GC types involved. The
tag's param type is `[i32]` and the enclosing `try (result i32)`
has end types `[i32]`, so the synthesized handler signature is
`[i32] → [i32]`. The module declares `[i32]→[]` (the tag) and
`[]→[i32]` (function `$f`) but never `[i32]→[i32]`, so
`ModuleTypes::find` returns `None`.

## Root condition

Any legacy `try` whose `catch` / `catch_all` handler frame
signature `[tag-params] → [try-end-types]` does not happen to
coincide with an already-declared `func` type. This is common — it
fires whenever the pair is non-trivial and not separately declared.

`block` / `loop` / `if` / bare `try` do **not** hit this: the binary
blocktype encoding forces a multi-value block to reference a
declared type index, so `find` always succeeds. `Catch` /
`CatchAll` are the only arms that *fabricate* a signature with no
guaranteed declared type. (`Else` reuses the parent frame's types,
so it's incidentally safe.)

## Suggested fix

The catch path should not require a pre-declared type. Synthesize
it via `InstrSeqType::new` (which calls `types.add`) instead of
`InstrSeqType::existing` / `find`, for the `Catch` and `CatchAll`
arms.

## Relevant source

- Panic site: `src/module/functions/local_function/mod.rs:1729`
  (`Operator::Catch`); same mechanism at `:1745` (`CatchAll`)
- `impl_push_control` + the unwrapped error:
  `src/module/functions/local_function/context.rs:183-189`
- `InstrSeqType::existing`: `src/ir/mod.rs:88-98`
- `ModuleTypes::find` (declared-types-only lookup):
  `src/module/types.rs:427-439`
- Origin: commit `706692b`, "feat: legacy exceptions (#293)"

## GC-flavoured variant (optional)

The bug surfaced originally on a Kotlin/Wasm module that combines
legacy `try` with GC reference types. For completeness, the same
panic with GC types in the signature:

```wat
(module
  (type $s (struct (field i32)))
  (tag $e (param (ref null $s)))
  (func $f (result (ref null $s))
    try (result (ref null $s))
      ref.null $s
    catch $e
    end))
```

The GC types are incidental — the `i32` repro above is the real
minimal case.

## Environment

- walrus 0.26.1 and 0.26.2 (crates.io) — verified identical
- host: aarch64-apple-darwin, rustc stable

## Offer

Happy to send a PR adding both `.wat` cases (the `i32` and the
`ref` variant) as regression fixtures under `tests/` if that's the
preferred form.

---

## History (not part of the filed issue)

The first draft of this report misdiagnosed the bug as a GC
type-section / block-type-resolution gap (#304). A maintainer-style
review built five hand-written `.wat` cases and found:

- A `block` with a concrete (indexed) GC heap type in both param
  and result **parses fine** — refuting the GC block-type theory.
- The `i32`-only `try`/`catch` case (no GC at all) **panics
  identically** — proving the bug is purely legacy-EH.

The draft above is the corrected version. Verified locally: the
8-line `i32` repro panics `walrus::Module::from_buffer` at
`mod.rs:1729` with `type: [I32] -> [I32]`.
