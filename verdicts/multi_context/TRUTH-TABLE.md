# multi_context — v0.14 inline-chain depth demo

## What this fixture demonstrates

This is the suite's first fixture written *for* the v0.13/v0.14
inline-context tracking — earlier fixtures (leap_year, triangle,
state_guard, etc.) are too small for rustc/LLVM to emit
`DW_TAG_inlined_subroutine` DIEs, so v0.13's `inline_context`
and v0.14's `inline_chain` stay empty on them.

`multi_context` uses stdlib slice helpers (`is_empty`,
`contains`, `first`) inside the predicate `is_valid`, which
triggers the inliner to produce real inlined-subroutine entries
across stdlib + the user's wrappers. The result:

- Manifest carries **16 entries** in `branch_inline_contexts` /
  `branch_inline_chains` (one per stdlib-inlined br_if).
- The decision at `memchr.rs:40` (from stdlib `slice::contains`)
  carries an inline chain **4 frames deep**: `[run-call-to-wrapper,
  wrapper-call-to-is_valid, is_valid-call-to-contains,
  contains-call-to-memchr]`.
- v3 mcdc envelope's `RowView.inline_chain` populates for every
  row that fired conditions in this decision.

## What this fixture does NOT yet demonstrate

`per_context.len() == 2` (drill-down with two buckets) is the
v0.13 design's headline use case but does not populate here.
The reason is structural:

- The two wrappers `check_first` / `check_second` get inlined
  into `run` as separate inlined-subroutine entries (good).
- The runner's trace parser emits one `DecisionRow` per
  function-return boundary (kind=2 record). Within a single
  `run` invocation, no return boundary separates the wrapper
  call sites — the inliner has folded them flat.
- The row's modal inline-context tag therefore aggregates
  across the whole invocation, picking the modal call site
  across all evaluated conditions. With the two wrappers
  contributing equal weight (or with the dispatcher's `if`
  selecting only one wrapper path per invocation), the tag is
  unambiguous but identical across all invocations' rows.
- Net: rows split between invocations have one inline_context
  each, but all rows in the same `per_context` bucket share
  that one context → only one bucket emitted.

Producing two buckets would require either (a) row-per-iteration
trace records (the v0.7.3 mechanism) firing inside a loop that
alternates between the wrappers, or (b) the runner stamping
multiple `inline_context` values per row (Vec rather than
Option). Both are v0.14.x or v0.15 work.

## Rows

| Row | invoke | which | kind | expected `is_valid` |
|---|---|---|---|---|
| 0 | `run:0,0` | first | "" | false |
| 1 | `run:0,1` | first | "x y" | false |
| 2 | `run:0,2` | first | "/abs" | true |
| 3 | `run:0,3` | first | "abs:80" | false |
| 4 | `run:1,0` | second | "" | false |
| 5 | `run:1,1` | second | "x y" | false |
| 6 | `run:1,2` | second | "/abs" | true |
| 7 | `run:1,3` | second | "abs:80" | false |

Even invocations exercise `check_first`; odd invocations exercise
`check_second`. Both call the same source predicate; chain extraction
on each row reveals which wrapper provided the path.
