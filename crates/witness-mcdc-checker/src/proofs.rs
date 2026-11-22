//! Kani proof of the pair-search kernel (v0.47.0 FEAT-050, REQ-016).
//!
//! Checks [`find_independent_effect_pair_in`] over every truth table of up
//! to `R` rows and `C` conditions, with every condition either absent or a
//! symbolic bool and every outcome either absent or a symbolic bool, against
//! the declarative specification below. Bounds and assumptions are listed in
//! `PROOF-LEDGER.md`. Compiled only under `cargo kani`.
//!
//! Harness code uses index loops, not iterator adapters: Kani 0.67 reports
//! false counterexamples through `core::hint::assert_unchecked` inside
//! `filter().count()` / `iter().all()` (concrete playback of them passes).

#![allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]

use super::{Interpretation, RowView, find_independent_effect_pair_in};

/// One truth-table row over `C` conditions, stored as plain arrays.
#[derive(Clone, Copy)]
struct ARow<const C: usize> {
    id: u32,
    present: [bool; C],
    val: [bool; C],
    out: Option<bool>,
}

impl<const C: usize> RowView for ARow<C> {
    fn row_id(&self) -> u32 {
        self.id
    }
    fn get(&self, idx: u32) -> Option<bool> {
        let i = idx as usize;
        if i < C && self.present[i] {
            Some(self.val[i])
        } else {
            None
        }
    }
    fn evaluated_count(&self) -> usize {
        let mut n = 0;
        let mut i = 0;
        while i < C {
            if self.present[i] {
                n += 1;
            }
            i += 1;
        }
        n
    }
    fn outcome(&self) -> Option<bool> {
        self.out
    }
}

fn symbolic_rows<const R: usize, const C: usize>() -> [ARow<C>; R] {
    let mut rows = [ARow {
        id: 0,
        present: [false; C],
        val: [false; C],
        out: None,
    }; R];
    let mut r = 0;
    while r < R {
        rows[r].id = r as u32;
        let mut c = 0;
        while c < C {
            rows[r].present[c] = kani::any();
            rows[r].val[c] = kani::any();
            c += 1;
        }
        let has_out: bool = kani::any();
        let out: bool = kani::any();
        rows[r].out = if has_out { Some(out) } else { None };
        r += 1;
    }
    rows
}

// ---- Specification (declarative; independent of the kernel) ----------

/// Rows `a` and `b` show condition `t` independently affecting the outcome:
/// both evaluated `t` with different values, both outcomes observed and
/// different, and no other condition evaluated in both rows differs.
fn is_pair<const C: usize>(a: &ARow<C>, b: &ARow<C>, t: usize) -> bool {
    if !(a.present[t] && b.present[t]) || a.val[t] == b.val[t] {
        return false;
    }
    match (a.out, b.out) {
        (Some(x), Some(y)) if x != y => {}
        _ => return false,
    }
    let mut i = 0;
    while i < C {
        if i != t && a.present[i] && b.present[i] && a.val[i] != b.val[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Every condition was evaluated in the row.
fn full<const C: usize>(a: &ARow<C>) -> bool {
    let mut i = 0;
    while i < C {
        if !a.present[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// The kernel returns one of two labels of different lengths (12 vs 7);
/// comparing lengths keeps the harness free of a 12-byte memcmp.
fn is_unique_cause(s: &str) -> bool {
    s.len() == Interpretation::UNIQUE_CAUSE.len()
}

fn check<const R: usize, const C: usize>() {
    let rows = symbolic_rows::<R, C>();
    let t: usize = kani::any();
    kani::assume(t < C);
    let result = find_independent_effect_pair_in(&rows, t as u32, C);

    let mut exists = false;
    let mut unique_cause_exists = false;
    let mut i = 0;
    while i < R {
        let mut j = i + 1;
        while j < R {
            if is_pair(&rows[i], &rows[j], t) {
                exists = true;
                if full(&rows[i]) && full(&rows[j]) {
                    unique_cause_exists = true;
                }
            }
            j += 1;
        }
        i += 1;
    }

    match result {
        Some((a, b, interp)) => {
            // Soundness: the returned rows exist, are ordered, and form a pair.
            assert!(
                a < b && (b as usize) < R,
                "returned rows out of range or unordered"
            );
            assert!(
                is_pair(&rows[a as usize], &rows[b as usize], t),
                "returned rows are not a pair"
            );
            // The label is one of the two and matches full-ness of both rows.
            assert!(
                is_unique_cause(interp) || interp.len() == Interpretation::MASKING.len(),
                "unknown interpretation label"
            );
            let both_full = full(&rows[a as usize]) && full(&rows[b as usize]);
            assert!(
                both_full == is_unique_cause(interp),
                "label does not match full-ness"
            );
            // Preference: if a unique-cause pair exists, one is returned.
            if unique_cause_exists {
                assert!(
                    is_unique_cause(interp),
                    "unique-cause pair exists but masking returned"
                );
            }
        }
        // Completeness: no pair is reported only when none exists.
        None => assert!(!exists, "a pair exists but none was returned"),
    }
}

// unwind 14 covers the kernel's 12-byte label comparisons (memcmp).
#[kani::proof]
#[kani::unwind(14)]
fn pair_kernel_r4_c5() {
    check::<4, 5>();
}

#[kani::proof]
#[kani::unwind(14)]
fn pair_kernel_r8_c6() {
    check::<8, 6>();
}
