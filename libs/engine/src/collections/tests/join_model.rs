//! Random joins of `HierarchicalBitSet`s against `BTreeSet` intersection and difference (spec §4.8).

use std::collections::BTreeSet;
use std::vec::Vec;

use proptest::prelude::*;

use crate::collections::tests::bounded;
use crate::collections::{HierarchicalBitSet, Join};

/// Indices that straddle the word boundaries of every layer, below 1,048,576.
fn index() -> impl Strategy<Value = usize> {
    prop_oneof![
        0..8_usize,
        56..72_usize,
        4_090..4_100_usize,
        262_140..262_150_usize,
        1_048_500..1_048_576_usize,
    ]
}

fn members() -> impl Strategy<Value = Vec<usize>> {
    prop::collection::vec(index(), 0..64)
}

proptest! {
    #[test]
    fn join_matches_model(
        required in prop::collection::vec(members(), 1..=4),
        excluded in prop::collection::vec(members(), 0..=2),
        // Which sets are used again: indices into `required` then `excluded`.
        repeat_required in prop::option::of(0..4_usize),
        repeat_excluded in prop::option::weighted(0.2, 0..4_usize),
    ) {
        let build = |items: &Vec<usize>| items.iter().copied().collect::<HierarchicalBitSet>();
        let req_sets: Vec<HierarchicalBitSet> = required.iter().map(build).collect();
        let exc_sets: Vec<HierarchicalBitSet> = excluded.iter().map(build).collect();
        let mut req: Vec<&HierarchicalBitSet> = req_sets.iter().collect();
        let mut exc: Vec<&HierarchicalBitSet> = exc_sets.iter().collect();
        let mut req_models: Vec<BTreeSet<usize>> =
            required.iter().map(|v| v.iter().copied().collect()).collect();
        let mut exc_models: Vec<BTreeSet<usize>> =
            excluded.iter().map(|v| v.iter().copied().collect()).collect();
        // A repeat of a required set, and a required set also listed as excluded.
        if let Some(i) = repeat_required {
            let i = i % req_sets.len();
            req.push(&req_sets[i]);
            req_models.push(req_models[i].clone());
        }
        if let Some(i) = repeat_excluded {
            let i = i % req_sets.len();
            exc.push(&req_sets[i]);
            exc_models.push(req_models[i].clone());
        }

        let mut want: BTreeSet<usize> = req_models[0].clone();
        for m in &req_models[1..] {
            want = want.intersection(m).copied().collect();
        }
        for m in &exc_models {
            want = want.difference(m).copied().collect();
        }
        let want: Vec<usize> = want.into_iter().collect();
        let got = bounded(Join::new(&req, &exc), want.len() + 1);
        prop_assert_eq!(got, want);
    }
}
