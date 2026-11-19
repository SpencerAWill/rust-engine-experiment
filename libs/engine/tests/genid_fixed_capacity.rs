//! A fixed-capacity map or allocator never touches the heap after construction.
//!
//! In its own binary because it installs a global allocator; `dhat` runs in testing mode, so no
//! `unsafe` is needed. WASI is excluded because `dhat` needs OS features it does not have.
#![forbid(unsafe_code)]
#![cfg(not(target_os = "wasi"))]

use engine::genid::{GenId, GenIdAllocator, GenIdMap};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

/// The fewest heap blocks `work` allocated over up to three runs, each on a fresh `setup()`.
///
/// `dhat` counts the blocks of every thread, and on a loaded machine the test harness's main
/// thread can allocate while the test runs. Code that allocates does so on every run, so one
/// clean run is enough to show that it does not. The setup is not measured, and every run starts
/// from a new one so that growth during the first run cannot hide growth in the next.
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

const N: usize = 1_000;

/// The containers of the fixed-capacity run, with room for their IDs built in advance.
struct Fixed {
    map: GenIdMap<u64>,
    lean: GenIdAllocator<u8>,
    unit: GenIdMap<()>,
    map_ids: Vec<GenId<u64>>,
    lean_ids: Vec<GenId<u8>>,
    unit_ids: Vec<GenId<()>>,
}

fn work_on_fixed(c: &mut Fixed) {
    for round in 0..20_u64 {
        c.map_ids.clear();
        c.lean_ids.clear();
        c.unit_ids.clear();
        for i in 0..N as u64 {
            c.map_ids.push(c.map.insert(i + round));
            c.lean_ids.push(c.lean.allocate());
            c.unit_ids.push(c.unit.insert(()));
        }
        assert!(c.map.try_insert(0).is_err());
        assert!(c.lean.try_allocate().is_none());
        assert!(c.unit.try_insert(()).is_err());
        for h in c.map_ids.iter().step_by(3) {
            c.map.remove(*h);
        }
        c.map.retain(|_, v| *v % 2 == 0);
        let _ = c.map.iter().count() + c.unit.iter().count();
        if round % 2 == 0 {
            c.map.clear();
            c.lean.clear();
            c.unit.clear();
        } else {
            for h in &c.lean_ids {
                c.lean.free(*h);
            }
            for h in &c.unit_ids {
                c.unit.remove(*h);
            }
            for (_, _) in c.map.drain() {}
        }
    }
}

#[test]
fn fixed_capacity_containers_never_allocate_after_construction() {
    // `dhat` allows one profiler at a time, so everything runs in this single test.
    let _profiler = dhat::Profiler::builder().testing().build();

    let blocks = fewest_blocks(
        || Fixed {
            map: GenIdMap::with_fixed_capacity(N),
            lean: GenIdAllocator::with_fixed_capacity(N),
            unit: GenIdMap::with_fixed_capacity(N),
            // The ID lists are test scaffolding, built before measuring starts.
            map_ids: Vec::with_capacity(N),
            lean_ids: Vec::with_capacity(N),
            unit_ids: Vec::with_capacity(N),
        },
        work_on_fixed,
    );
    // `dhat::assert_eq!` suspends tracking itself, so it must not be given a call that reads
    // the stats: compute the value first.
    dhat::assert_eq!(blocks, 0);

    // `with_capacity` promises the same up to its capacity: no allocation while filling it.
    let blocks = fewest_blocks(
        || {
            (
                GenIdAllocator::<u8>::with_capacity(N),
                GenIdMap::<()>::with_capacity(N),
                Vec::with_capacity(N),
                Vec::with_capacity(N),
            )
        },
        |(lean, unit, lean_ids, unit_ids)| {
            for _ in 0..N {
                lean_ids.push(lean.allocate());
                unit_ids.push(unit.insert(()));
            }
        },
    );
    dhat::assert_eq!(blocks, 0);
}
