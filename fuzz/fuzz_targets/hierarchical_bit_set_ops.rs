//! Fuzzes `HierarchicalBitSet` and `Join` against `BTreeSet` reference models.
//!
//! Three sets are kept side by side with a model each. After every operation the members, `len`,
//! `first`, `last` and `check_invariants` of all three are checked. Every index is taken
//! `% 1_048_576` so the sets stay small enough to fuzz quickly while still using four layers.
//! The code under test is only iterated through `take`, so a bug cannot make the target loop
//! or allocate without bound.

#![no_main]
#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use arbitrary::Arbitrary;
use engine::collections::{BitSet, HierarchicalBitSet, Join};
use libfuzzer_sys::fuzz_target;

/// Indices are taken modulo this (spec section 4.8).
const RANGE: usize = 1_048_576;
/// Sets under test.
const SETS: usize = 3;
/// Longest required or excluded list a `Join` uses.
const MAX_JOIN_SETS: usize = 8;

#[derive(Arbitrary, Debug)]
enum Op {
    Insert(u8, u32),
    Remove(u8, u32),
    Clear(u8),
    EnsureCapacity(u8, u32),
    ShrinkToFit(u8),
    UnionWith(u8, u8),
    IntersectWith(u8, u8),
    DifferenceWith(u8, u8),
    Tests(u8, u8),
    Join(Vec<u8>, Vec<u8>),
    RoundTripBitSet(u8),
}

fn pick(which: u8) -> usize {
    usize::from(which) % SETS
}

fn index(i: u32) -> usize {
    i as usize % RANGE
}

fn check(set: &HierarchicalBitSet, model: &BTreeSet<usize>) {
    assert!(
        set.iter().take(model.len() + 1).eq(model.iter().copied()),
        "members"
    );
    assert_eq!(set.len(), model.len());
    assert_eq!(set.is_empty(), model.is_empty());
    assert_eq!(set.first(), model.first().copied());
    assert_eq!(set.last(), model.last().copied());
    assert!(set.check_invariants().is_ok(), "check_invariants");
}

fuzz_target!(|ops: Vec<Op>| {
    let mut sets = [
        HierarchicalBitSet::new(),
        HierarchicalBitSet::new(),
        HierarchicalBitSet::new(),
    ];
    let mut models: [BTreeSet<usize>; SETS] = Default::default();

    for op in ops {
        match op {
            Op::Insert(w, i) => {
                let (w, i) = (pick(w), index(i));
                assert_eq!(sets[w].insert(i), models[w].insert(i));
            }
            Op::Remove(w, i) => {
                let (w, i) = (pick(w), index(i));
                assert_eq!(sets[w].remove(i), models[w].remove(&i));
                assert_eq!(sets[w].contains(i), false);
            }
            Op::Clear(w) => {
                let w = pick(w);
                sets[w].clear();
                models[w].clear();
            }
            Op::EnsureCapacity(w, n) => {
                let (w, n) = (pick(w), index(n));
                sets[w].ensure_capacity(n);
                assert!(sets[w].capacity() >= n);
            }
            Op::ShrinkToFit(w) => sets[pick(w)].shrink_to_fit(),
            Op::UnionWith(a, b) => {
                let (a, b) = (pick(a), pick(b));
                let other = sets[b].clone();
                sets[a].union_with(&other);
                models[a] = models[a].union(&models[b]).copied().collect();
            }
            Op::IntersectWith(a, b) => {
                let (a, b) = (pick(a), pick(b));
                let other = sets[b].clone();
                sets[a].intersect_with(&other);
                models[a] = models[a].intersection(&models[b]).copied().collect();
            }
            Op::DifferenceWith(a, b) => {
                let (a, b) = (pick(a), pick(b));
                let other = sets[b].clone();
                sets[a].difference_with(&other);
                models[a] = models[a].difference(&models[b]).copied().collect();
            }
            Op::Tests(a, b) => {
                let (a, b) = (pick(a), pick(b));
                assert_eq!(sets[a].is_subset(&sets[b]), models[a].is_subset(&models[b]));
                assert_eq!(
                    sets[a].is_superset(&sets[b]),
                    models[a].is_superset(&models[b])
                );
                assert_eq!(
                    sets[a].is_disjoint(&sets[b]),
                    models[a].is_disjoint(&models[b])
                );
                assert_eq!(sets[a] == sets[b], models[a] == models[b]);
            }
            Op::Join(required, excluded) => {
                let required: Vec<usize> = required.iter().map(|&w| pick(w)).collect();
                let excluded: Vec<usize> = excluded.iter().map(|&w| pick(w)).collect();
                let required = &required[..required.len().min(MAX_JOIN_SETS)];
                let excluded = &excluded[..excluded.len().min(MAX_JOIN_SETS)];
                let required_sets: Vec<&HierarchicalBitSet> =
                    required.iter().map(|&w| &sets[w]).collect();
                let excluded_sets: Vec<&HierarchicalBitSet> =
                    excluded.iter().map(|&w| &sets[w]).collect();
                match Join::try_new(&required_sets, &excluded_sets) {
                    Err(_) => assert!(required.is_empty(), "try_new failed with required sets"),
                    Ok(join) => {
                        assert!(!required.is_empty());
                        let expected: BTreeSet<usize> = models[required[0]]
                            .iter()
                            .copied()
                            .filter(|i| {
                                required.iter().all(|&w| models[w].contains(i))
                                    && !excluded.iter().any(|&w| models[w].contains(i))
                            })
                            .collect();
                        assert!(
                            join.take(expected.len() + 1).eq(expected.iter().copied()),
                            "join members"
                        );
                    }
                }
            }
            Op::RoundTripBitSet(w) => {
                let w = pick(w);
                let flat = BitSet::from(&sets[w]);
                assert!(flat.iter().take(models[w].len() + 1).eq(models[w].iter().copied()));
                sets[w] = HierarchicalBitSet::from(&flat);
            }
        }
        for (set, model) in sets.iter().zip(&models) {
            check(set, model);
        }
    }
});
