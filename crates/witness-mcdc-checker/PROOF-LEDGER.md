# Proof ledger: MC/DC pair-search kernel

What `src/proofs.rs` proves about `find_independent_effect_pair_in`, and what it
does not. v0.47.0 FEAT-050, REQ-016. Checked by the "Kernel proof (Kani)" CI job.

## Proved (Kani 0.67.0, bounded)

For every truth table of at most **8 rows and 6 conditions** (harness
`pair_kernel_r8_c6`; `pair_kernel_r4_c5` also runs), with each condition absent
or a symbolic bool, each outcome absent or a symbolic bool, and any target
condition `t < C`:

1. **Soundness.** A returned `(a, b, label)` has `a < b < R`, and rows `a`, `b`
   satisfy the specification `is_pair`: both evaluated `t` with different
   values, both outcomes observed and different, and every other condition
   evaluated in both rows has the same value.
2. **Label.** `label` is `unique-cause` exactly when both rows evaluated every
   condition, otherwise `masking`.
3. **Preference.** If any unique-cause pair exists, a unique-cause pair is
   returned.
4. **Completeness.** `None` is returned only when no pair exists.

Four planted mutants (`ci-controls/kani-mutant-*`) each make the proof fail on
the property they break.

## Assumptions (trusted, not proved)

- **A1. The `Row` adapter.** Production calls go through `impl RowView for
  Row` (three one-line accessors over `BTreeMap`). The proof runs over an
  array-backed `RowView`; the adapter is trusted by review.
- **A2. Bounds.** Rows <= 8, conditions <= 6. Real decisions can be larger;
  the kernel has no size-dependent branch, but that is an argument, not a
  proof.
- **A3. Condition keys are below `total_conditions`.** The kernel decides
  "every condition evaluated" by `evaluated_count() == total_conditions`. A
  row holding a key `>= total_conditions` and missing a real one would count
  as full. The harness cannot produce such rows; producers must not either.
- **A4. Specification review.** `is_pair` and `full` are the specification.
  A reviewer other than the author must sign off on them; until then this
  item is open.
- **A5. Tooling.** Kani and CBMC are trusted. Harness code avoids std
  iterator adapters, which gave false counterexamples under Kani 0.67.

## Not covered

The reporter around the kernel (`witness-mcdc-core`), gap explanations
(`unverified_reason`, `recommend_gap_closure`), and everything upstream of the
truth table (instrumentation, trace parsing).
