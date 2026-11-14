//! After `with_capacity(n)` or `reserve(n)`, nothing the collections do allocates (spec §4.7).
//!
//! In its own binary because it installs a global allocator; `dhat` runs in testing mode, so no
//! `unsafe` is needed. WASI is excluded because `dhat` needs OS features it does not have.
#![forbid(unsafe_code)]
#![cfg(not(target_os = "wasi"))]

use engine_collections::{IndexMap, IndexSet};

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
}
