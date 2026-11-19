//! Random operation sequences on `BitArray` against a `BTreeSet<usize>` (spec §4.8).

use std::collections::BTreeSet;
use std::vec::Vec;

use proptest::prelude::*;

use crate::collections::bit_set::BitArray;
use crate::collections::tests::bounded;

#[derive(Clone, Debug)]
enum Op {
    Insert(usize),
    Remove(usize),
    TryInsert(usize),
    TryRemove(usize),
    Clear,
    RoundTripWords,
    UnionWith(Vec<usize>),
    IntersectWith(Vec<usize>),
    DifferenceWith(Vec<usize>),
    SymmetricDifferenceWith(Vec<usize>),
}

/// What an operation returned, in a form both sides can produce.
#[derive(Debug, PartialEq, Eq)]
enum Ret {
    Flag(bool),
    Try(Result<bool, (usize, usize)>),
    Unit,
}

fn index() -> impl Strategy<Value = usize> {
    prop_oneof![0..8_usize, 56..72_usize, 120..136_usize, 184..200_usize]
}

fn members() -> impl Strategy<Value = Vec<usize>> {
    prop::collection::vec(index(), 0..6)
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        index().prop_map(Op::Insert),
        index().prop_map(Op::Remove),
        index().prop_map(Op::TryInsert),
        index().prop_map(Op::TryRemove),
        Just(Op::Clear),
        Just(Op::RoundTripWords),
        members().prop_map(Op::UnionWith),
        members().prop_map(Op::IntersectWith),
        members().prop_map(Op::DifferenceWith),
        members().prop_map(Op::SymmetricDifferenceWith),
    ]
}

/// The members of `items` that fit in a `BitArray` of `capacity` bits.
fn model_of(items: &[usize], capacity: usize) -> BTreeSet<usize> {
    items.iter().copied().filter(|&i| i < capacity).collect()
}

fn from_members<const WORDS: usize>(items: &[usize]) -> BitArray<WORDS> {
    let mut array = BitArray::new();
    for &i in items {
        let _ = array.try_insert(i);
    }
    array
}

fn run<const WORDS: usize>(ops: &[Op], probe: &[usize]) {
    let capacity = BitArray::<WORDS>::CAPACITY;
    let mut array = BitArray::<WORDS>::new();
    let mut model = BTreeSet::<usize>::new();
    for op in ops {
        let (got, want) = match *op {
            // Out-of-range `insert` and `remove` panic; the panic tests cover them.
            Op::Insert(i) | Op::Remove(i) if i >= capacity => continue,
            Op::Insert(i) => (Ret::Flag(array.insert(i)), Ret::Flag(model.insert(i))),
            Op::Remove(i) => (Ret::Flag(array.remove(i)), Ret::Flag(model.remove(&i))),
            Op::TryInsert(i) => (
                Ret::Try(array.try_insert(i).map_err(|e| (e.index(), e.capacity()))),
                Ret::Try(if i < capacity {
                    Ok(model.insert(i))
                } else {
                    Err((i, capacity))
                }),
            ),
            Op::TryRemove(i) => (
                Ret::Try(array.try_remove(i).map_err(|e| (e.index(), e.capacity()))),
                Ret::Try(if i < capacity {
                    Ok(model.remove(&i))
                } else {
                    Err((i, capacity))
                }),
            ),
            Op::Clear => {
                array.clear();
                model.clear();
                (Ret::Unit, Ret::Unit)
            }
            Op::RoundTripWords => {
                array = BitArray::from_words(*array.as_words());
                (Ret::Unit, Ret::Unit)
            }
            Op::UnionWith(ref items) => {
                array.union_with(&from_members(items));
                model.extend(items.iter().copied().filter(|&i| i < capacity));
                (Ret::Unit, Ret::Unit)
            }
            Op::IntersectWith(ref items) => {
                array.intersect_with(&from_members(items));
                let other = model_of(items, capacity);
                model = model.intersection(&other).copied().collect();
                (Ret::Unit, Ret::Unit)
            }
            Op::DifferenceWith(ref items) => {
                array.difference_with(&from_members(items));
                let other = model_of(items, capacity);
                model = model.difference(&other).copied().collect();
                (Ret::Unit, Ret::Unit)
            }
            Op::SymmetricDifferenceWith(ref items) => {
                array.symmetric_difference_with(&from_members(items));
                let other = model_of(items, capacity);
                model = model.symmetric_difference(&other).copied().collect();
                (Ret::Unit, Ret::Unit)
            }
        };
        assert_eq!(got, want, "after {op:?}");
        check(&array, &model);
        check_relations(&array, &model, probe);
    }
}

/// Subset, superset and disjointness against a second set, both ways round.
fn check_relations<const WORDS: usize>(
    array: &BitArray<WORDS>,
    model: &BTreeSet<usize>,
    probe: &[usize],
) {
    let other = from_members::<WORDS>(probe);
    let other_model = model_of(probe, BitArray::<WORDS>::CAPACITY);
    assert_eq!(array.is_subset(&other), model.is_subset(&other_model));
    assert_eq!(array.is_superset(&other), model.is_superset(&other_model));
    assert_eq!(array.is_disjoint(&other), model.is_disjoint(&other_model));
    assert_eq!(other.is_subset(array), other_model.is_subset(model));
    assert_eq!(other.is_disjoint(array), other_model.is_disjoint(model));
}

fn check<const WORDS: usize>(array: &BitArray<WORDS>, model: &BTreeSet<usize>) {
    let ascending: Vec<usize> = model.iter().copied().collect();
    let descending: Vec<usize> = model.iter().rev().copied().collect();
    let limit = model.len() + 1;
    assert_eq!(bounded(array.iter(), limit), ascending);
    assert_eq!(bounded(array.iter().rev(), limit), descending);
    assert_eq!(bounded(array.into_iter(), limit), ascending);
    assert_eq!(array.len(), model.len());
    assert_eq!(array.is_empty(), model.is_empty());
    assert_eq!(array.first(), model.first().copied());
    assert_eq!(array.last(), model.last().copied());
    for j in 0..BitArray::<WORDS>::CAPACITY + 64 {
        assert_eq!(array.contains(j), model.contains(&j), "contains({j})");
    }
}

proptest! {
    #[test]
    fn bit_array_0_matches_model(
        ops in prop::collection::vec(op(), 0..200),
        probe in members(),
    ) {
        run::<0>(&ops, &probe);
    }

    #[test]
    fn bit_array_1_matches_model(
        ops in prop::collection::vec(op(), 0..200),
        probe in members(),
    ) {
        run::<1>(&ops, &probe);
    }

    #[test]
    fn bit_array_3_matches_model(
        ops in prop::collection::vec(op(), 0..200),
        probe in members(),
    ) {
        run::<3>(&ops, &probe);
    }
}
