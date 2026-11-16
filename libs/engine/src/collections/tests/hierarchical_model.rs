//! Random operation sequences on `HierarchicalBitSet` against a `BTreeSet<usize>` (spec §4.8).

use std::collections::BTreeSet;
use std::vec::Vec;

use proptest::prelude::*;

use crate::collections::tests::bounded;
use crate::collections::{BitSet, HierarchicalBitSet};

#[derive(Clone, Debug)]
enum Op {
    Insert(usize),
    TryInsert(usize),
    Remove(usize),
    Clear,
    EnsureCapacity(usize),
    ShrinkToFit,
    TryExtend(Vec<usize>),
    RoundTripBitSet,
    CloneSwap,
    UnionWith(Vec<usize>),
    IntersectWith(Vec<usize>),
    DifferenceWith(Vec<usize>),
}

#[derive(Debug, PartialEq, Eq)]
enum Ret {
    Flag(bool),
    Ok(bool),
    Unit,
}

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
    prop::collection::vec(index(), 0..6)
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        index().prop_map(Op::Insert),
        index().prop_map(Op::TryInsert),
        index().prop_map(Op::Remove),
        Just(Op::Clear),
        (0..1_048_576_usize).prop_map(Op::EnsureCapacity),
        Just(Op::ShrinkToFit),
        members().prop_map(Op::TryExtend),
        Just(Op::RoundTripBitSet),
        Just(Op::CloneSwap),
        members().prop_map(Op::UnionWith),
        members().prop_map(Op::IntersectWith),
        members().prop_map(Op::DifferenceWith),
    ]
}

/// The words a canonical (trimmed) set with these members has.
fn model_words(model: &BTreeSet<usize>) -> Vec<u64> {
    let mut words = Vec::new();
    for &i in model {
        words.resize(words.len().max(i / 64 + 1), 0_u64);
        words[i / 64] |= 1 << (i % 64);
    }
    words
}

fn run(ops: &[Op], probe_members: &[usize]) {
    let probe: HierarchicalBitSet = probe_members.iter().copied().collect();
    let probe_model: BTreeSet<usize> = probe_members.iter().copied().collect();
    let mut set = HierarchicalBitSet::new();
    let mut model = BTreeSet::<usize>::new();
    for op in ops {
        let (got, want) = match op {
            Op::Insert(i) => (Ret::Flag(set.insert(*i)), Ret::Flag(model.insert(*i))),
            Op::TryInsert(i) => (
                Ret::Ok(set.try_insert(*i).unwrap()),
                Ret::Ok(model.insert(*i)),
            ),
            Op::Remove(i) => (Ret::Flag(set.remove(*i)), Ret::Flag(model.remove(i))),
            Op::Clear => {
                set.clear();
                model.clear();
                (Ret::Unit, Ret::Unit)
            }
            Op::EnsureCapacity(n) => {
                set.ensure_capacity(*n);
                assert!(set.capacity() >= *n);
                (Ret::Unit, Ret::Unit)
            }
            Op::ShrinkToFit => {
                set.shrink_to_fit();
                (Ret::Unit, Ret::Unit)
            }
            Op::TryExtend(items) => {
                set.try_extend(items.iter().copied()).unwrap();
                model.extend(items.iter().copied());
                (Ret::Unit, Ret::Unit)
            }
            Op::RoundTripBitSet => {
                let flat = BitSet::from(&set);
                set = HierarchicalBitSet::from(&flat);
                (Ret::Unit, Ret::Unit)
            }
            Op::CloneSwap => {
                set = set.clone();
                (Ret::Unit, Ret::Unit)
            }
            Op::UnionWith(items) => {
                let other: HierarchicalBitSet = items.iter().copied().collect();
                set.union_with(&other);
                model.extend(items.iter().copied());
                (Ret::Unit, Ret::Unit)
            }
            Op::IntersectWith(items) => {
                let other: HierarchicalBitSet = items.iter().copied().collect();
                set.intersect_with(&other);
                model.retain(|m| items.contains(m));
                (Ret::Unit, Ret::Unit)
            }
            Op::DifferenceWith(items) => {
                let other: HierarchicalBitSet = items.iter().copied().collect();
                set.difference_with(&other);
                model.retain(|m| !items.contains(m));
                (Ret::Unit, Ret::Unit)
            }
        };
        assert_eq!(got, want, "after {op:?}");
        check(&set, &model);
        check_tests(&set, &model, &probe, &probe_model);
        check_tests(&probe, &probe_model, &set, &model);
    }
}

fn check_tests(
    a: &HierarchicalBitSet,
    model_a: &BTreeSet<usize>,
    b: &HierarchicalBitSet,
    model_b: &BTreeSet<usize>,
) {
    assert_eq!(a.is_subset(b), model_a.is_subset(model_b));
    assert_eq!(a.is_superset(b), model_a.is_superset(model_b));
    assert_eq!(a.is_disjoint(b), model_a.is_disjoint(model_b));
}

fn check(set: &HierarchicalBitSet, model: &BTreeSet<usize>) {
    let ascending: Vec<usize> = model.iter().copied().collect();
    let limit = model.len() + 1;
    assert_eq!(bounded(set.iter(), limit), ascending);
    assert_eq!(bounded(set.into_iter(), limit), ascending);
    assert_eq!(set.len(), model.len());
    assert_eq!(set.is_empty(), model.is_empty());
    assert_eq!(set.first(), model.first().copied());
    assert_eq!(set.last(), model.last().copied());
    for &m in model {
        assert!(set.contains(m), "contains({m})");
        assert_eq!(set.contains(m + 1), model.contains(&(m + 1)));
        assert_eq!(
            set.contains(m.wrapping_sub(1)),
            model.contains(&m.wrapping_sub(1))
        );
    }
    assert_eq!(set.as_trimmed_words(), model_words(model));
    assert_eq!(set.check_invariants(), Ok(()));
}

proptest! {
    #[test]
    fn hierarchical_matches_model(
        ops in prop::collection::vec(op(), 0..200),
        probe in members(),
    ) {
        run(&ops, &probe);
    }
}
