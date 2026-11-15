//! Fuzzes `BitSet` and `BitArray<4>` against `BTreeSet` reference models.
//!
//! Every operation is applied to a `BitSet`, to a `BitArray<4>` (with indices taken `% 256`) and
//! to a model of each, and after each one the members, `len`, `first` and `last` are compared.

#![no_main]
#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use arbitrary::Arbitrary;
use engine::collections::{BitArray, BitSet};
use libfuzzer_sys::fuzz_target;

const WORDS: usize = 4;
/// Bits in the `BitArray`.
const ARRAY_BITS: usize = WORDS * 64;

#[derive(Arbitrary, Debug)]
enum Op {
    Insert(u16),
    Remove(u16),
    Contains(u16),
    Clear,
    EnsureCapacity(u16),
    ShrinkToFit,
    UnionWith(Vec<u16>),
    IntersectWith(Vec<u16>),
    DifferenceWith(Vec<u16>),
    SymmetricDifferenceWith(Vec<u16>),
    IsSubset(Vec<u16>),
    IsDisjoint(Vec<u16>),
    RoundTripWords,
    /// Takes `n` items from the back of `iter()` and checks them against the model.
    Reverse(u8),
}

fn model_of(items: &[u16], modulus: usize) -> BTreeSet<usize> {
    items.iter().map(|&i| usize::from(i) % modulus).collect()
}

fn set_of(items: &[u16]) -> BitSet {
    items.iter().map(|&i| usize::from(i)).collect()
}

fn array_of(items: &[u16]) -> BitArray<WORDS> {
    let mut array = BitArray::new();
    for &i in items {
        array.insert(usize::from(i) % ARRAY_BITS);
    }
    array
}

fn check_set(set: &BitSet, model: &BTreeSet<usize>) {
    assert!(set.iter().eq(model.iter().copied()), "BitSet members");
    assert_eq!(set.len(), model.len());
    assert_eq!(set.first(), model.first().copied());
    assert_eq!(set.last(), model.last().copied());
}

fn check_array(array: &BitArray<WORDS>, model: &BTreeSet<usize>) {
    assert!(array.iter().eq(model.iter().copied()), "BitArray members");
    assert_eq!(array.len(), model.len());
    assert_eq!(array.first(), model.first().copied());
    assert_eq!(array.last(), model.last().copied());
}

fuzz_target!(|ops: Vec<Op>| {
    let mut set = BitSet::new();
    let mut set_model = BTreeSet::<usize>::new();
    let mut array = BitArray::<WORDS>::new();
    let mut array_model = BTreeSet::<usize>::new();

    for op in ops {
        match op {
            Op::Insert(i) => {
                let i = usize::from(i);
                assert_eq!(set.insert(i), set_model.insert(i));
                let j = i % ARRAY_BITS;
                assert_eq!(array.insert(j), array_model.insert(j));
            }
            Op::Remove(i) => {
                let i = usize::from(i);
                assert_eq!(set.remove(i), set_model.remove(&i));
                let j = i % ARRAY_BITS;
                assert_eq!(array.remove(j), array_model.remove(&j));
            }
            Op::Contains(i) => {
                let i = usize::from(i);
                assert_eq!(set.contains(i), set_model.contains(&i));
                assert_eq!(array.contains(i), array_model.contains(&i));
            }
            Op::Clear => {
                set.clear();
                set_model.clear();
                array.clear();
                array_model.clear();
            }
            Op::EnsureCapacity(n) => {
                set.ensure_capacity(usize::from(n));
                assert!(set.capacity() >= usize::from(n));
            }
            Op::ShrinkToFit => set.shrink_to_fit(),
            Op::UnionWith(items) => {
                set.union_with(&set_of(&items));
                set_model.extend(items.iter().map(|&i| usize::from(i)));
                array.union_with(&array_of(&items));
                array_model.extend(model_of(&items, ARRAY_BITS));
            }
            Op::IntersectWith(items) => {
                set.intersect_with(&set_of(&items));
                set_model = set_model
                    .intersection(&model_of(&items, usize::MAX))
                    .copied()
                    .collect();
                array.intersect_with(&array_of(&items));
                array_model = array_model
                    .intersection(&model_of(&items, ARRAY_BITS))
                    .copied()
                    .collect();
            }
            Op::DifferenceWith(items) => {
                set.difference_with(&set_of(&items));
                set_model = set_model
                    .difference(&model_of(&items, usize::MAX))
                    .copied()
                    .collect();
                array.difference_with(&array_of(&items));
                array_model = array_model
                    .difference(&model_of(&items, ARRAY_BITS))
                    .copied()
                    .collect();
            }
            Op::SymmetricDifferenceWith(items) => {
                set.symmetric_difference_with(&set_of(&items));
                set_model = set_model
                    .symmetric_difference(&model_of(&items, usize::MAX))
                    .copied()
                    .collect();
                array.symmetric_difference_with(&array_of(&items));
                array_model = array_model
                    .symmetric_difference(&model_of(&items, ARRAY_BITS))
                    .copied()
                    .collect();
            }
            Op::IsSubset(items) => {
                assert_eq!(
                    set.is_subset(&set_of(&items)),
                    set_model.is_subset(&model_of(&items, usize::MAX))
                );
                assert_eq!(
                    array.is_subset(&array_of(&items)),
                    array_model.is_subset(&model_of(&items, ARRAY_BITS))
                );
            }
            Op::IsDisjoint(items) => {
                assert_eq!(
                    set.is_disjoint(&set_of(&items)),
                    set_model.is_disjoint(&model_of(&items, usize::MAX))
                );
                assert_eq!(
                    array.is_disjoint(&array_of(&items)),
                    array_model.is_disjoint(&model_of(&items, ARRAY_BITS))
                );
            }
            Op::RoundTripWords => {
                set = BitSet::from_words(set.as_words());
                array = BitArray::from_words(*array.as_words());
            }
            Op::Reverse(n) => {
                let n = usize::from(n);
                assert!(set.iter().rev().take(n).eq(set_model.iter().rev().take(n).copied()));
                assert!(
                    array
                        .iter()
                        .rev()
                        .take(n)
                        .eq(array_model.iter().rev().take(n).copied())
                );
            }
        }
        check_set(&set, &set_model);
        check_array(&array, &array_model);
    }
});
