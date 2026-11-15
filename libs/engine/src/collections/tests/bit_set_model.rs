//! Random operation sequences on `BitSet` against a `BTreeSet<usize>` (spec §4.8).

use std::collections::BTreeSet;
use std::vec::Vec;

use proptest::prelude::*;

use crate::collections::BitSet;

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

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        index().prop_map(Op::Insert),
        index().prop_map(Op::TryInsert),
        index().prop_map(Op::Remove),
        Just(Op::Clear),
        (0..4_300_usize).prop_map(Op::EnsureCapacity),
        Just(Op::ShrinkToFit),
        prop::collection::vec(index(), 0..6).prop_map(Op::TryExtend),
        Just(Op::RoundTripWords),
        Just(Op::CloneSwap),
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

fn run(ops: &[Op]) {
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
        };
        assert_eq!(got, want, "after {op:?}");
        check(&set, &model);
    }
}

fn check(set: &BitSet, model: &BTreeSet<usize>) {
    let ascending: Vec<usize> = model.iter().copied().collect();
    let descending: Vec<usize> = model.iter().rev().copied().collect();
    assert_eq!(set.iter().collect::<Vec<_>>(), ascending);
    assert_eq!(set.iter().rev().collect::<Vec<_>>(), descending);
    assert_eq!(set.into_iter().collect::<Vec<_>>(), ascending);
    assert_eq!(set.clone().into_iter().collect::<Vec<_>>(), ascending);
    assert_eq!(
        set.clone().into_iter().rev().collect::<Vec<_>>(),
        descending
    );
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
    fn bit_set_matches_model(ops in prop::collection::vec(op(), 0..200)) {
        run(&ops);
    }
}
