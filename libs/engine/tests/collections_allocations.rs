//! After `with_capacity(n)` or `reserve(n)`, nothing the collections do allocates (spec §4.7).
//!
//! In its own binary because it installs a global allocator; `dhat` runs in testing mode, so no
//! `unsafe` is needed. WASI is excluded because `dhat` needs OS features it does not have.
#![forbid(unsafe_code)]
#![cfg(not(target_os = "wasi"))]

use std::hint::black_box;

use engine::collections::{BitArray, BitSet, HierarchicalBitSet, IndexMap, IndexSet, Join};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const CAPACITY: usize = 1_024;
const N: u64 = CAPACITY as u64;

/// The fewest heap blocks `work` allocated over up to three runs, each on a fresh `setup()`.
///
/// `dhat` counts the blocks of every thread, and on a loaded machine the test harness's main
/// thread can allocate while the test runs. Code that allocates does so on every run, so one
/// clean run is enough to show that it does not. The setup is not measured.
fn fewest_blocks<S>(mut setup: impl FnMut() -> S, mut work: impl FnMut(&mut S)) -> u64 {
    let mut fewest = u64::MAX;
    for _ in 0..3 {
        let mut state = setup();
        let before = dhat::HeapStats::get().total_blocks;
        work(&mut state);
        let after = dhat::HeapStats::get().total_blocks;
        fewest = fewest.min(after - before);
        if fewest == 0 {
            break;
        }
    }
    fewest
}

/// Fills, queries, removes from, reorders and empties the map, then fills it again.
fn work_on_map(map: &mut IndexMap<u64, u64>) {
    for k in 0..N {
        map.insert(k, k);
    }
    let mut sum = 0;
    for k in 0..N {
        sum += map.get(&k).copied().unwrap_or(0);
    }
    sum += map.iter().map(|(_, v)| *v).sum::<u64>();
    for k in 0..N {
        if let Some(v) = map.get_mut(&k) {
            *v += 1;
        }
    }
    for k in 0..100 {
        map.swap_remove(&k);
    }
    for k in 100..200 {
        map.shift_remove(&k);
    }
    map.retain(|_, v| *v % 2 == 0);
    map.sort_keys();
    let _ = map.pop();
    map.clear();
    for k in 0..N {
        map.insert(k, k);
    }
    assert!(sum > 0);
}

fn work_on_set(set: &mut IndexSet<u64>) {
    for k in 0..N {
        set.insert(k);
    }
    let mut hits = 0;
    for k in 0..N {
        hits += usize::from(set.contains(&k));
    }
    hits += set.iter().count();
    for k in 0..100 {
        set.swap_remove(&k);
    }
    for k in 100..200 {
        set.shift_remove(&k);
    }
    set.retain(|k| k % 2 == 0);
    set.sort();
    let _ = set.pop();
    set.clear();
    for k in 0..N {
        set.insert(k);
    }
    assert!(hits > 0);
}

const BITS: usize = 4_096;

/// A set and the argument of the set-algebra calls, both made in setup.
type BitState = (BitSet, BitSet);

fn bit_state(mut set: BitSet) -> BitState {
    let mut argument = BitSet::new();
    argument.insert(0);
    argument.insert(BITS - 1);
    set.clear();
    (set, argument)
}

/// Every `BitSet` operation that spec §4.7 promises not to allocate, within `BITS` bits.
fn work_on_bit_set((set, argument): &mut BitState) {
    for i in (0..BITS).step_by(2) {
        set.insert(i);
    }
    let mut hits = 0;
    for i in 0..BITS {
        hits += usize::from(set.contains(i));
    }
    hits += set.iter().sum::<usize>() + set.iter().rev().sum::<usize>();
    hits += set.first().unwrap_or(0) + set.last().unwrap_or(0) + set.len();
    for i in (0..BITS).step_by(4) {
        set.remove(i);
    }
    set.union_with(argument);
    set.symmetric_difference_with(argument);
    set.intersect_with(argument);
    set.difference_with(argument);
    hits += usize::from(set.is_subset(argument))
        + usize::from(set.is_superset(argument))
        + usize::from(set.is_disjoint(argument));
    set.clear();
    for i in (0..BITS).step_by(2) {
        set.insert(i);
    }
    black_box(hits);
}

/// The set under test, two more required sets, an excluded one, and the argument of the
/// set-algebra calls (built with capacity 65,536 but holding only indices below `BITS`).
struct HierarchicalState {
    set: HierarchicalBitSet,
    other_required: [HierarchicalBitSet; 2],
    excluded: HierarchicalBitSet,
    argument: HierarchicalBitSet,
}

fn hierarchical_state(set: HierarchicalBitSet) -> HierarchicalState {
    let make = |step: usize| {
        let mut other = HierarchicalBitSet::with_capacity(BITS);
        for i in (0..BITS).step_by(step) {
            other.insert(i);
        }
        other
    };
    let mut argument = HierarchicalBitSet::with_capacity(65_536);
    argument.insert(0);
    argument.insert(100);
    argument.insert(BITS - 1);
    HierarchicalState {
        set,
        other_required: [make(2), make(3)],
        excluded: make(7),
        argument,
    }
}

/// Every `HierarchicalBitSet` and `Join` operation that spec §4.6 promises not to allocate,
/// within `BITS` bits.
fn work_on_hierarchical_bit_set(state: &mut HierarchicalState) {
    let HierarchicalState {
        set,
        other_required,
        excluded,
        argument,
    } = state;
    for i in (0..BITS).step_by(2) {
        set.insert(i);
    }
    let mut hits = 0;
    for i in 0..BITS {
        hits += usize::from(set.contains(i));
    }
    hits += set.first().unwrap_or(0) + set.last().unwrap_or(0) + set.len();
    hits += set.iter().sum::<usize>();
    let [second, third] = &*other_required;
    let required = [&*set, second, third];
    let excluded_sets = [&*excluded];
    hits += Join::new(&required, &excluded_sets).sum::<usize>();
    hits += Join::try_new(&required, &excluded_sets).map_or(0, Iterator::count);
    set.intersect_with(argument);
    set.difference_with(argument);
    for i in (0..BITS).step_by(2) {
        set.insert(i);
    }
    set.union_with(argument);
    hits += usize::from(set.is_subset(argument))
        + usize::from(set.is_superset(argument))
        + usize::from(set.is_disjoint(argument));
    for i in (0..BITS).step_by(4) {
        set.remove(i);
    }
    set.clear();
    for i in (0..BITS).step_by(2) {
        set.insert(i);
    }
    black_box(hits);
}

/// The hierarchical bitset checks of spec §4.6.
fn check_hierarchical_bitsets_never_allocate() {
    let with_capacity = fewest_blocks(
        || hierarchical_state(HierarchicalBitSet::with_capacity(BITS)),
        work_on_hierarchical_bit_set,
    );
    let ensure_capacity = fewest_blocks(
        || {
            let mut set = HierarchicalBitSet::new();
            set.ensure_capacity(BITS);
            hierarchical_state(set)
        },
        work_on_hierarchical_bit_set,
    );
    let limit = fewest_blocks(HierarchicalBitSet::new, |set| {
        black_box(set.try_insert(HierarchicalBitSet::MAX_BITS).is_err());
        black_box(set.try_insert(usize::MAX).is_err());
    });
    let join = fewest_blocks(
        || hierarchical_state(HierarchicalBitSet::new()),
        |state| {
            let required = [&state.set, &state.excluded];
            let excluded = [&state.argument];
            black_box(Join::new(&required, &excluded).next());
            black_box(Join::try_new(&required, &excluded).is_ok());
            black_box(Join::try_new(&[], &[]).is_err());
        },
    );

    dhat::assert_eq!(with_capacity, 0, "HierarchicalBitSet::with_capacity");
    dhat::assert_eq!(ensure_capacity, 0, "HierarchicalBitSet::ensure_capacity");
    dhat::assert_eq!(limit, 0, "HierarchicalBitSet try_insert at the limit");
    dhat::assert_eq!(join, 0, "Join::new and try_new");
}

/// Every `BitArray<4>` operation.
fn work_on_bit_array(array: &mut BitArray<4>) {
    let other = BitArray::<4>::from_words([1, 2, 3, 4]);
    let mut hits = 0;
    for i in 0..BitArray::<4>::CAPACITY {
        array.insert(i);
        hits += usize::from(array.contains(i));
        hits += usize::from(array.remove(i));
        hits += usize::from(array.try_insert(i).expect("in range"));
        hits += usize::from(array.try_remove(i).expect("in range"));
    }
    hits += usize::from(array.try_insert(BitArray::<4>::CAPACITY).is_err());
    hits += usize::from(array.try_remove(BitArray::<4>::CAPACITY).is_err());
    array.union_with(&other);
    hits += array.len() + array.first().unwrap_or(0) + array.last().unwrap_or(0);
    hits += array.iter().sum::<usize>() + array.iter().rev().sum::<usize>();
    array.symmetric_difference_with(&other);
    array.intersect_with(&other);
    array.difference_with(&other);
    hits += usize::from(array.is_subset(&other))
        + usize::from(array.is_superset(&other))
        + usize::from(array.is_disjoint(&other))
        + usize::from(array.is_empty());
    hits += array.as_words().len();
    *array = BitArray::from_words(*array.as_words());
    array.clear();
    black_box(hits);
}

/// The bitset checks of spec §4.7. They run inside the test below, which owns the profiler.
fn check_bitsets_never_allocate() {
    let with_capacity = fewest_blocks(|| bit_state(BitSet::with_capacity(BITS)), work_on_bit_set);
    let ensure_capacity = fewest_blocks(
        || {
            let mut set = BitSet::new();
            set.ensure_capacity(BITS);
            bit_state(set)
        },
        work_on_bit_set,
    );
    // `other` has trailing zero words (4,000 was inserted, then removed), but `union_with` grows
    // `self` only to `other`'s trimmed length.
    let union_trailing_zeros = fewest_blocks(
        || {
            let mut set = BitSet::with_capacity(64);
            set.insert(1);
            let mut other = BitSet::with_capacity(BITS);
            other.insert(2);
            other.insert(4_000);
            other.remove(4_000);
            (set, other)
        },
        |(set, other)| {
            set.union_with(other);
            set.symmetric_difference_with(other);
        },
    );
    let limit = fewest_blocks(BitSet::new, |set| {
        black_box(set.try_insert(BitSet::MAX_BITS).is_err());
        black_box(set.try_insert(usize::MAX).is_err());
    });
    let array = fewest_blocks(BitArray::<4>::new, work_on_bit_array);

    dhat::assert_eq!(with_capacity, 0, "BitSet::with_capacity");
    dhat::assert_eq!(ensure_capacity, 0, "BitSet::ensure_capacity");
    dhat::assert_eq!(union_trailing_zeros, 0, "union_with, trailing zeros");
    dhat::assert_eq!(limit, 0, "try_insert at the limit");
    dhat::assert_eq!(array, 0, "BitArray");
}

#[test]
fn collections_never_allocate_within_their_reserved_capacity() {
    // `dhat` allows one profiler at a time, so everything runs in this single test.
    let _profiler = dhat::Profiler::builder().testing().build();

    // `dhat::assert_eq!` suspends tracking itself, so it must not be given a call that reads
    // the stats: compute each value first.
    let map_with_capacity = fewest_blocks(
        || IndexMap::<u64, u64>::with_capacity(CAPACITY),
        work_on_map,
    );
    let map_reserve = fewest_blocks(
        || {
            let mut map = IndexMap::<u64, u64>::new();
            map.reserve(CAPACITY);
            map
        },
        work_on_map,
    );
    let set_with_capacity = fewest_blocks(|| IndexSet::<u64>::with_capacity(CAPACITY), work_on_set);
    dhat::assert_eq!(map_with_capacity, 0);
    dhat::assert_eq!(map_reserve, 0);
    dhat::assert_eq!(set_with_capacity, 0);

    check_bitsets_never_allocate();
    check_hierarchical_bitsets_never_allocate();
}
