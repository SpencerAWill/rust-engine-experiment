//! Random operation sequences on `BitSet` against a `BTreeSet<usize>` (spec §4.8).

use std::collections::BTreeSet;
use std::vec::Vec;

use proptest::prelude::*;

use crate::collections::BitSet;
use crate::collections::tests::bounded;

#[derive(Clone, Debug)]
enum Op {
    Insert(usize),
    TryInsert(usize),
    Remove(usize),
    Clear,
    EnsureCapacity(usize),
    ShrinkToFit,
    TryExtend(Vec<usize>),
    RoundTripWords,
    CloneSwap,
    UnionWith(Vec<usize>),
    IntersectWith(Vec<usize>),
    DifferenceWith(Vec<usize>),
    SymmetricDifferenceWith(Vec<usize>),
}

#[derive(Debug, PartialEq, Eq)]
enum Ret {
    Flag(bool),
    Ok(bool),
    Unit,
}

fn index() -> impl Strategy<Value = usize> {
    prop_oneof![0..8_usize, 56..72_usize, 120..136_usize, 4_000..4_200_usize]
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
        (0..4_300_usize).prop_map(Op::EnsureCapacity),
        Just(Op::ShrinkToFit),
        members().prop_map(Op::TryExtend),
        Just(Op::RoundTripWords),
        Just(Op::CloneSwap),
        members().prop_map(Op::UnionWith),
        members().prop_map(Op::IntersectWith),
        members().prop_map(Op::DifferenceWith),
        members().prop_map(Op::SymmetricDifferenceWith),
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

fn from_members(items: &[usize]) -> BitSet {
    items.iter().copied().collect()
}

fn run(ops: &[Op], probe: &[usize]) {
    let mut set = BitSet::new();
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
            Op::RoundTripWords => {
                set = BitSet::from_words(set.as_words());
                (Ret::Unit, Ret::Unit)
            }
            Op::CloneSwap => {
                set = set.clone();
                (Ret::Unit, Ret::Unit)
            }
            Op::UnionWith(items) => {
                set.union_with(&from_members(items));
                model.extend(items.iter().copied());
                (Ret::Unit, Ret::Unit)
            }
            Op::IntersectWith(items) => {
                set.intersect_with(&from_members(items));
                let other: BTreeSet<usize> = items.iter().copied().collect();
                model = model.intersection(&other).copied().collect();
                (Ret::Unit, Ret::Unit)
            }
            Op::DifferenceWith(items) => {
                set.difference_with(&from_members(items));
                let other: BTreeSet<usize> = items.iter().copied().collect();
                model = model.difference(&other).copied().collect();
                (Ret::Unit, Ret::Unit)
            }
            Op::SymmetricDifferenceWith(items) => {
                set.symmetric_difference_with(&from_members(items));
                let other: BTreeSet<usize> = items.iter().copied().collect();
                model = model.symmetric_difference(&other).copied().collect();
                (Ret::Unit, Ret::Unit)
            }
        };
        assert_eq!(got, want, "after {op:?}");
        check(&set, &model);
        check_relations(&set, &model, probe);
    }
}

/// Subset, superset and disjointness against a second set, both ways round.
fn check_relations(set: &BitSet, model: &BTreeSet<usize>, probe: &[usize]) {
    let other = from_members(probe);
    let other_model: BTreeSet<usize> = probe.iter().copied().collect();
    assert_eq!(set.is_subset(&other), model.is_subset(&other_model));
    assert_eq!(set.is_superset(&other), model.is_superset(&other_model));
    assert_eq!(set.is_disjoint(&other), model.is_disjoint(&other_model));
    assert_eq!(other.is_subset(set), other_model.is_subset(model));
    assert_eq!(other.is_disjoint(set), other_model.is_disjoint(model));
}

fn check(set: &BitSet, model: &BTreeSet<usize>) {
    let ascending: Vec<usize> = model.iter().copied().collect();
    let descending: Vec<usize> = model.iter().rev().copied().collect();
    let limit = model.len() + 1;
    assert_eq!(bounded(set.iter(), limit), ascending);
    assert_eq!(bounded(set.iter().rev(), limit), descending);
    assert_eq!(bounded(set.into_iter(), limit), ascending);
    assert_eq!(bounded(set.clone().into_iter(), limit), ascending);
    assert_eq!(bounded(set.clone().into_iter().rev(), limit), descending);
    assert_eq!(set.len(), model.len());
    assert_eq!(set.is_empty(), model.is_empty());
    assert_eq!(set.first(), model.first().copied());
    assert_eq!(set.last(), model.last().copied());
    for j in 0..4_300 {
        assert_eq!(set.contains(j), model.contains(&j), "contains({j})");
    }
    assert!(set.capacity() >= 64 * set.as_words().len());
    assert_eq!(set.as_trimmed_words(), model_words(model));
}

proptest! {
    #[test]
    fn bit_set_matches_model(
        ops in prop::collection::vec(op(), 0..200),
        probe in members(),
    ) {
        run(&ops, &probe);
    }
}
