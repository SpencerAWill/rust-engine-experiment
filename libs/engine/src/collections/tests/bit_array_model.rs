//! Random operation sequences on `BitArray` against a `BTreeSet<usize>` (spec §4.8).

use std::collections::BTreeSet;
use std::vec::Vec;

use proptest::prelude::*;

use crate::collections::bit_set::BitArray;

#[derive(Clone, Debug)]
enum Op {
    Insert(usize),
    Remove(usize),
    TryInsert(usize),
    TryRemove(usize),
    Clear,
    RoundTripWords,
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

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        index().prop_map(Op::Insert),
        index().prop_map(Op::Remove),
        index().prop_map(Op::TryInsert),
        index().prop_map(Op::TryRemove),
        Just(Op::Clear),
        Just(Op::RoundTripWords),
    ]
}

fn run<const WORDS: usize>(ops: &[Op]) {
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
        };
        assert_eq!(got, want, "after {op:?}");
        check(&array, &model);
    }
}

fn check<const WORDS: usize>(array: &BitArray<WORDS>, model: &BTreeSet<usize>) {
    let ascending: Vec<usize> = model.iter().copied().collect();
    let descending: Vec<usize> = model.iter().rev().copied().collect();
    assert_eq!(array.iter().collect::<Vec<_>>(), ascending);
    assert_eq!(array.iter().rev().collect::<Vec<_>>(), descending);
    assert_eq!(array.into_iter().collect::<Vec<_>>(), ascending);
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
    fn bit_array_0_matches_model(ops in prop::collection::vec(op(), 0..200)) {
        run::<0>(&ops);
    }

    #[test]
    fn bit_array_1_matches_model(ops in prop::collection::vec(op(), 0..200)) {
        run::<1>(&ops);
    }

    #[test]
    fn bit_array_3_matches_model(ops in prop::collection::vec(op(), 0..200)) {
        run::<3>(&ops);
    }
}
