//! Property tests for the MC/DC pair finder.
//!
//! These properties are the auditable contract for the kernel:

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::as_conversions
)]
//!
//! 1. **Outcome-differs**: any returned pair has rows whose outcomes
//!    are different and both Some.
//! 2. **Target-differs**: any returned pair has rows whose values for
//!    `target_idx` are different.
//! 3. **Non-target compatibility**: any returned pair has rows whose
//!    values agree on every condition `i ≠ target_idx` that is present
//!    in both rows' evaluated maps.
//! 4. **Order independence**: shuffling the input row list produces a
//!    pair (possibly different ids) iff the original input does. The
//!    *existence* of a pair is invariant under row reordering.
//! 5. **Masking detection**: when the returned pair contains a row
//!    whose `evaluated` map is shorter than `total_conditions`, the
//!    interpretation is `"masking"`.
//! 6. **Unique-cause detection**: when both rows of the returned pair
//!    have `evaluated` maps of length `total_conditions`, the
//!    interpretation is `"unique-cause"`.
//! 7. **Idempotence**: calling the finder twice on the same input
//!    yields the same result.

use proptest::prelude::*;
use std::collections::BTreeMap;
use witness_mcdc_checker::{Interpretation, Row, find_independent_effect_pair};

/// Generate a Row with `n_conditions` conditions, randomly short-
/// circuiting some and randomly assigning bools to those evaluated.
fn row_strategy(row_id: u32, n_conditions: u32) -> impl Strategy<Value = Row> {
    let masks: Vec<BoxedStrategy<Option<bool>>> = (0..n_conditions)
        .map(|_| {
            prop_oneof![
                Just(None),
                any::<bool>().prop_map(Some),
                any::<bool>().prop_map(Some),
            ]
            .boxed()
        })
        .collect();
    (masks, prop_oneof![Just(None), any::<bool>().prop_map(Some)]).prop_map(
        move |(values, outcome)| {
            let mut evaluated = BTreeMap::new();
            for (i, v) in values.into_iter().enumerate() {
                if let Some(b) = v {
                    evaluated.insert(u32::try_from(i).unwrap_or(u32::MAX), b);
                }
            }
            Row {
                row_id,
                evaluated,
                outcome,
            }
        },
    )
}

fn rows_strategy(max_rows: usize, n_conditions: u32) -> impl Strategy<Value = Vec<Row>> {
    (1usize..=max_rows)
        .prop_flat_map(move |n| {
            let strategies: Vec<_> = (0..n)
                .map(|i| row_strategy(u32::try_from(i).unwrap_or(u32::MAX), n_conditions))
                .collect();
            strategies
        })
        .prop_map(|v| v)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Property 1+2+3+5+6: returned pair satisfies all four pair
    /// criteria simultaneously.
    #[test]
    fn returned_pair_satisfies_criteria(
        rows in rows_strategy(8, 4),
        target_idx in 0u32..4,
    ) {
        let n = 4usize;
        if let Some((a, b, interp)) = find_independent_effect_pair(&rows, target_idx, n) {
            let r1 = rows.iter().find(|r| r.row_id == a).expect("a in rows");
            let r2 = rows.iter().find(|r| r.row_id == b).expect("b in rows");

            // Property 2: target-differs.
            let v1 = *r1.evaluated.get(&target_idx).expect("target in r1");
            let v2 = *r2.evaluated.get(&target_idx).expect("target in r2");
            prop_assert_ne!(v1, v2);

            // Property 1: outcome-differs and both Some.
            let o1 = r1.outcome.expect("outcome in r1");
            let o2 = r2.outcome.expect("outcome in r2");
            prop_assert_ne!(o1, o2);

            // Property 3: non-target compatibility.
            for idx in 0..u32::try_from(n).unwrap() {
                if idx == target_idx { continue; }
                if let (Some(x), Some(y)) = (r1.evaluated.get(&idx), r2.evaluated.get(&idx)) {
                    prop_assert_eq!(x, y);
                }
            }

            // Property 5+6: interpretation matches actual fullness.
            let r1_full = r1.evaluated.len() == n;
            let r2_full = r2.evaluated.len() == n;
            if r1_full && r2_full {
                prop_assert_eq!(interp, Interpretation::UNIQUE_CAUSE);
            } else {
                prop_assert_eq!(interp, Interpretation::MASKING);
            }
        }
    }

    /// Property 4: existence of a pair is invariant under input
    /// reordering. (The exact pair ids may differ because the search
    /// returns the lowest-index unique-cause it sees, but the
    /// some/none distinction must agree.)
    #[test]
    fn order_independence_of_pair_existence(
        rows in rows_strategy(6, 3),
        target_idx in 0u32..3,
        seed in any::<u64>(),
    ) {
        let n = 3usize;
        let original = find_independent_effect_pair(&rows, target_idx, n);

        // Deterministic shuffle from the seed.
        let mut shuffled = rows.clone();
        let mut s = seed;
        for i in (1..shuffled.len()).rev() {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let j = (s as usize) % (i + 1);
            shuffled.swap(i, j);
        }
        let after = find_independent_effect_pair(&shuffled, target_idx, n);

        prop_assert_eq!(original.is_some(), after.is_some());

        // When both return pairs, both should agree on interpretation:
        // if the original found unique-cause, the shuffled must too
        // (unique-cause depends only on row contents, not order).
        if let (Some((_, _, i1)), Some((_, _, i2))) = (original, after) {
            // Either both unique-cause or interpretation can degrade
            // to masking only if the lowest-index pair under the new
            // ordering differs and isn't unique-cause. The strongest
            // safe property: if the input contains *any* unique-cause
            // pair, the algorithm finds one in either order.
            let has_uc_in_either = i1 == Interpretation::UNIQUE_CAUSE
                || i2 == Interpretation::UNIQUE_CAUSE;
            // If one found UC, the other must also find UC (since the
            // search does an exhaustive scan and prefers UC).
            if has_uc_in_either {
                prop_assert_eq!(i1, Interpretation::UNIQUE_CAUSE);
                prop_assert_eq!(i2, Interpretation::UNIQUE_CAUSE);
            }
        }
    }

    /// Property 7: idempotence — repeated calls produce the same result.
    #[test]
    fn idempotent(
        rows in rows_strategy(8, 4),
        target_idx in 0u32..4,
    ) {
        let a = find_independent_effect_pair(&rows, target_idx, 4);
        let b = find_independent_effect_pair(&rows, target_idx, 4);
        prop_assert_eq!(format!("{:?}", a), format!("{:?}", b));
    }

    /// Sanity: target_idx beyond `total_conditions` returns None for
    /// any rows whose evaluated maps don't include that index.
    #[test]
    fn out_of_range_target_returns_none(
        rows in rows_strategy(6, 3),
    ) {
        // No row generated by `rows_strategy(_, 3)` evaluates index 99.
        let result = find_independent_effect_pair(&rows, 99, 3);
        prop_assert!(result.is_none());
    }
}
