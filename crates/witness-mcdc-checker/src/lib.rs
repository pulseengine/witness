//! Pure-stdlib MC/DC independent-effect pair finder.
//!
//! This crate is the **qualifiable kernel** of witness: a self-contained
//! ~70 LoC implementation of the masking MC/DC pair search (DO-178C
//! accepted variant) that safety-critical adopters can audit, formally
//! reason about, or re-implement, *without* trusting the rest of the
//! witness toolchain (wasmtime, walrus, gimli, serde).
//!
//! # What it does
//!
//! Given a list of decision rows (each row is a sparse map of
//! `condition_index -> evaluated_bool` plus an optional outcome),
//! [`find_independent_effect_pair`] returns two row ids whose pair
//! demonstrates that condition `target_idx` independently affects the
//! decision outcome under masking MC/DC.
//!
//! # Pair criterion
//!
//! Returned pair `(row_a, row_b, interpretation)` satisfies all of:
//!
//! 1. Both rows have `target_idx` evaluated.
//! 2. The two rows' values for `target_idx` differ.
//! 3. For every other condition index `i ≠ target_idx`: if `i` is
//!    present in BOTH rows' evaluated maps, the values must agree.
//!    (Indices present in only one row are compatible by definition —
//!    that's the masking allowance.)
//! 4. Both rows have `outcome = Some(_)` and outcomes differ.
//!
//! # Interpretation
//!
//! - `"unique-cause"` — both rows fully evaluate every condition (no
//!   short-circuiting on either side). Strongest MC/DC guarantee.
//! - `"masking"` — at least one condition was short-circuited in at
//!   least one row of the pair. Acceptable under DO-178C masking MC/DC.
//!
//! Unique-cause pairs are preferred when both kinds are available.
//!
//! # Why a separate crate
//!
//! Witness's qualification dossier targets DO-178C. A reviewer's first
//! question is "what code do I have to trust?" Extracting this crate
//! shrinks that surface to a few hundred lines of pure stdlib code with
//! no I/O, no parsing, no FFI, no allocator-heavy data structures
//! beyond `Vec<Row>`. The rest of witness (instrumentation, wasm
//! execution, DWARF reconstruction, signing) feeds this kernel; the
//! kernel itself can be re-implemented from scratch in C or SPARK if
//! needed and cross-checked against witness output.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

use alloc::collections::BTreeMap;

/// One decision row: sparse condition vector + optional outcome.
///
/// `evaluated` is sparse: condition indices that were short-circuited
/// (never evaluated in this row) are absent. The pair-finder uses
/// absent-vs-present to detect masking.
///
/// `outcome = None` means "decision was reached but the boolean result
/// was not observed" (e.g. function panicked or returned mid-chain).
/// Such rows can never be part of a proving pair under criterion 4.
#[derive(Debug, Clone)]
pub struct Row {
    /// Stable row identifier carried back in the returned pair so
    /// callers can correlate against their own row table.
    pub row_id: u32,
    /// `condition_index -> evaluated_bool`. Sparse map: missing key
    /// means the condition was short-circuited and not evaluated.
    pub evaluated: BTreeMap<u32, bool>,
    /// Decision outcome, if observed.
    pub outcome: Option<bool>,
}

/// Interpretation tag returned alongside a proving pair.
///
/// - `Self::UNIQUE_CAUSE` (`"unique-cause"`) — both rows of the pair
///   fully evaluate every condition. No short-circuiting in either
///   row. Strictest MC/DC guarantee.
/// - `Self::MASKING` (`"masking"`) — at least one condition was
///   short-circuited in at least one row of the pair. Acceptable under
///   DO-178C masking MC/DC.
pub struct Interpretation;

impl Interpretation {
    pub const UNIQUE_CAUSE: &'static str = "unique-cause";
    pub const MASKING: &'static str = "masking";
}

/// Find a pair of rows that prove condition `target_idx` independently
/// affects the decision outcome under masking MC/DC.
///
/// Returns `Some((row_id_a, row_id_b, interpretation))` if a pair
/// exists, `None` otherwise. `interpretation` is one of
/// [`Interpretation::UNIQUE_CAUSE`] or [`Interpretation::MASKING`].
///
/// `total_conditions` is the total number of conditions in the
/// decision. Used to determine whether both rows of a candidate pair
/// have fully-populated condition vectors (unique-cause vs masking).
///
/// Search is `O(n²)` in the row count and short-circuits on the first
/// unique-cause pair. Row order does not affect correctness — the
/// returned pair is the lowest-index pair under input ordering.
///
// SAFETY-REVIEW: arithmetic on `i + 1` index bumps is bounded by
// `rows.len()` (Vec length, fits in usize) and `total_conditions`
// (manifest entry count, fits in u32 by construction).
// Wraparound is impossible for any non-degenerate input.
#[allow(clippy::arithmetic_side_effects)]
pub fn find_independent_effect_pair(
    rows: &[Row],
    target_idx: u32,
    total_conditions: usize,
) -> Option<(u32, u32, &'static str)> {
    let mut best: Option<(u32, u32, &'static str)> = None;
    for i in 0..rows.len() {
        for j in (i + 1)..rows.len() {
            // SAFETY-REVIEW: `i` and `j` are bounded by `rows.len()`.
            #[allow(clippy::indexing_slicing)]
            let (r1, r2) = (&rows[i], &rows[j]);
            let v1 = match r1.evaluated.get(&target_idx) {
                Some(v) => *v,
                None => continue,
            };
            let v2 = match r2.evaluated.get(&target_idx) {
                Some(v) => *v,
                None => continue,
            };
            if v1 == v2 {
                continue;
            }
            let (o1, o2) = match (r1.outcome, r2.outcome) {
                (Some(a), Some(b)) => (a, b),
                _ => continue,
            };
            if o1 == o2 {
                continue;
            }
            // Check non-target compatibility under masking.
            let mut compatible = true;
            for idx in 0..u32::try_from(total_conditions).unwrap_or(0) {
                if idx == target_idx {
                    continue;
                }
                match (r1.evaluated.get(&idx), r2.evaluated.get(&idx)) {
                    (Some(a), Some(b)) if a != b => {
                        compatible = false;
                        break;
                    }
                    _ => {}
                }
            }
            if !compatible {
                continue;
            }
            // Determine interpretation.
            let r1_full = r1.evaluated.len() == total_conditions;
            let r2_full = r2.evaluated.len() == total_conditions;
            let interp = if r1_full && r2_full {
                Interpretation::UNIQUE_CAUSE
            } else {
                Interpretation::MASKING
            };
            // Prefer unique-cause when found; remember it and continue
            // searching only if the current best is masking.
            match (&best, interp) {
                (None, _) => {
                    best = Some((r1.row_id, r2.row_id, interp));
                }
                (Some((_, _, current)), s)
                    if s == Interpretation::UNIQUE_CAUSE
                        && *current != Interpretation::UNIQUE_CAUSE =>
                {
                    best = Some((r1.row_id, r2.row_id, interp));
                }
                _ => {}
            }
            if let Some((_, _, k)) = best
                && k == Interpretation::UNIQUE_CAUSE
            {
                return best;
            }
        }
    }
    best
}

/// Helper: build a [`Row`] from a slice of `(index, value)` pairs and
/// an optional outcome. Useful in tests and for callers that don't
/// want to construct `BTreeMap` literals by hand.
pub fn row(row_id: u32, evaluated: &[(u32, bool)], outcome: Option<bool>) -> Row {
    Row {
        row_id,
        evaluated: evaluated.iter().copied().collect(),
        outcome,
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn leap_year_three_conditions_full_pair_set() {
        // Decision: (c0 && c1) || c2  (leap_year semantics)
        // 4 rows from verdicts/leap_year/TRUTH-TABLE.md.
        let rows = vec![
            row(0, &[(0, false), (2, false)], Some(false)),
            row(1, &[(0, true), (1, true)], Some(true)),
            row(2, &[(0, true), (1, false), (2, false)], Some(false)),
            row(3, &[(0, true), (1, false), (2, true)], Some(true)),
        ];
        // c0 should pair via row 0 + row 1 (or similar).
        let p0 = find_independent_effect_pair(&rows, 0, 3).unwrap();
        assert_ne!(p0.0, p0.1);
        // c1 should pair via row 1 + row 2.
        let p1 = find_independent_effect_pair(&rows, 1, 3).unwrap();
        assert_ne!(p1.0, p1.1);
        // c2 should pair via row 2 + row 3.
        let p2 = find_independent_effect_pair(&rows, 2, 3).unwrap();
        assert_eq!(p2.2, Interpretation::UNIQUE_CAUSE);
    }

    #[test]
    fn no_pair_when_only_one_row() {
        let rows = vec![row(0, &[(0, true), (1, true)], Some(true))];
        assert!(find_independent_effect_pair(&rows, 0, 2).is_none());
    }

    #[test]
    fn no_pair_when_outcomes_match() {
        // c0 differs but outcomes are the same — not a proving pair.
        let rows = vec![
            row(0, &[(0, true), (1, true)], Some(true)),
            row(1, &[(0, false), (1, true)], Some(true)),
        ];
        assert!(find_independent_effect_pair(&rows, 0, 2).is_none());
    }

    #[test]
    fn no_pair_when_target_value_matches() {
        // c0 same value on both rows → cannot prove independent effect.
        let rows = vec![
            row(0, &[(0, true), (1, true)], Some(true)),
            row(1, &[(0, true), (1, false)], Some(false)),
        ];
        assert!(find_independent_effect_pair(&rows, 0, 2).is_none());
    }

    #[test]
    fn no_pair_when_other_condition_differs() {
        // c0 differs but c1 also differs — non-target compatibility fails.
        let rows = vec![
            row(0, &[(0, true), (1, true)], Some(true)),
            row(1, &[(0, false), (1, false)], Some(false)),
        ];
        assert!(find_independent_effect_pair(&rows, 0, 2).is_none());
    }

    #[test]
    fn masking_interpretation_when_short_circuit() {
        // Row 0 short-circuits (only c0 evaluated). Row 1 evaluates both.
        // c0 differs, outcomes differ, no other-condition conflict.
        let rows = vec![
            row(0, &[(0, false)], Some(false)),
            row(1, &[(0, true), (1, true)], Some(true)),
        ];
        let p = find_independent_effect_pair(&rows, 0, 2).unwrap();
        assert_eq!(p.2, Interpretation::MASKING);
    }

    #[test]
    fn unique_cause_preferred_over_masking() {
        // Decision: c0 || c1.
        // Row 0 short-circuits on c0=true (only c0 evaluated, outcome true).
        // Row 1 evaluates both: c0=false, c1=true, outcome true.
        // Row 2 evaluates both: c0=false, c1=false, outcome false.
        //
        // Candidate pairs for c0:
        //   - rows 0+2: c0 differs (true/false), outcomes differ (true/false),
        //               row 0 is short-circuited → MASKING.
        //   - rows 1+2: c0 same (false/false) → not a candidate.
        // For c1: rows 1+2 → c1 differs (true/false), outcomes differ
        //         (true/false), both rows fully evaluated → UNIQUE-CAUSE.
        //
        // For c0 the only proving pair is masking.
        let rows = vec![
            row(0, &[(0, true)], Some(true)),
            row(1, &[(0, false), (1, true)], Some(true)),
            row(2, &[(0, false), (1, false)], Some(false)),
        ];
        let p_c0 = find_independent_effect_pair(&rows, 0, 2).unwrap();
        assert_eq!(p_c0.2, Interpretation::MASKING);

        // For c1 there should be a unique-cause pair.
        let p_c1 = find_independent_effect_pair(&rows, 1, 2).unwrap();
        assert_eq!(p_c1.2, Interpretation::UNIQUE_CAUSE);
    }

    #[test]
    fn unique_cause_chosen_when_both_kinds_available() {
        // Decision: c0 ∧ c1 (so both must be true for outcome=true).
        // Pairs for c0:
        //   - rows 0+2: row 0 is masking (only c0 evaluated, c0=false,
        //               outcome=false); row 2: c0=true, c1=true, outcome=true.
        //               c0 differs (false/true), outcome differs → MASKING pair.
        //   - rows 1+2: c0 differs (false/true), c1=true on both, outcomes
        //               differ (false/true), both fully evaluated → UNIQUE-CAUSE.
        // Search must prefer unique-cause.
        let rows = vec![
            row(0, &[(0, false)], Some(false)),
            row(1, &[(0, false), (1, true)], Some(false)),
            row(2, &[(0, true), (1, true)], Some(true)),
        ];
        let p = find_independent_effect_pair(&rows, 0, 2).unwrap();
        assert_eq!(p.2, Interpretation::UNIQUE_CAUSE);
        // The unique-cause pair is rows 1 + 2.
        assert_eq!((p.0, p.1), (1, 2));
    }

    #[test]
    fn no_pair_when_target_not_evaluated() {
        // Both rows lack c1 in their evaluated map.
        let rows = vec![
            row(0, &[(0, true)], Some(false)),
            row(1, &[(0, false)], Some(true)),
        ];
        assert!(find_independent_effect_pair(&rows, 1, 2).is_none());
    }

    #[test]
    fn pair_with_outcome_none_rejected() {
        // Even though c0 differs, one row has no observed outcome.
        let rows = vec![
            row(0, &[(0, true), (1, true)], None),
            row(1, &[(0, false), (1, true)], Some(false)),
        ];
        assert!(find_independent_effect_pair(&rows, 0, 2).is_none());
    }
}
