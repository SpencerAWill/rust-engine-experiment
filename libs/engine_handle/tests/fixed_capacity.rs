//! A fixed-capacity pool or allocator never touches the heap after construction.
//!
//! In its own binary because it installs a global allocator; `dhat` runs in testing mode, so no
//! `unsafe` is needed. WASI is excluded because `dhat` needs OS features it does not have.
#![forbid(unsafe_code)]
#![cfg(not(target_os = "wasi"))]

use engine_handle::{DenseHandleAllocator, Handle, HandleAllocator, HandlePool};

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

/// The containers of the fixed-capacity run, with room for their handles built in advance.
struct Fixed {
    pool: HandlePool<u64>,
    lean: HandleAllocator<u8>,
    dense: DenseHandleAllocator<u8>,
    pool_handles: Vec<Handle<u64>>,
    lean_handles: Vec<Handle<u8>>,
    dense_handles: Vec<Handle<u8>>,
}

fn work_on_fixed(c: &mut Fixed) {
    for round in 0..20_u64 {
        c.pool_handles.clear();
        c.lean_handles.clear();
        c.dense_handles.clear();
        for i in 0..N as u64 {
            c.pool_handles.push(c.pool.insert(i + round));
            c.lean_handles.push(c.lean.allocate());
            c.dense_handles.push(c.dense.allocate());
        }
        assert!(c.pool.try_insert(0).is_err());
        assert!(c.lean.try_allocate().is_none());
        assert!(c.dense.try_allocate().is_none());
        for h in c.pool_handles.iter().step_by(3) {
            c.pool.remove(*h);
        }
        c.pool.retain(|_, v| *v % 2 == 0);
        let _ = c.pool.iter().count() + c.dense.iter().count();
        if round % 2 == 0 {
            c.pool.clear();
            c.lean.clear();
            c.dense.clear();
        } else {
            for h in &c.lean_handles {
                c.lean.free(*h);
            }
            for h in &c.dense_handles {
                c.dense.free(*h);
            }
            for (_, _) in c.pool.drain() {}
        }
    }
}

#[test]
fn fixed_capacity_containers_never_allocate_after_construction() {
    // `dhat` allows one profiler at a time, so everything runs in this single test.
    let _profiler = dhat::Profiler::builder().testing().build();

    let blocks = fewest_blocks(
        || Fixed {
            pool: HandlePool::with_fixed_capacity(N),
            lean: HandleAllocator::with_fixed_capacity(N),
            dense: DenseHandleAllocator::with_fixed_capacity(N),
            // The handle lists are test scaffolding, built before measuring starts.
            pool_handles: Vec::with_capacity(N),
            lean_handles: Vec::with_capacity(N),
            dense_handles: Vec::with_capacity(N),
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
                HandleAllocator::<u8>::with_capacity(N),
                DenseHandleAllocator::<u8>::with_capacity(N),
                Vec::with_capacity(2 * N),
            )
        },
        |(lean, dense, handles)| {
            for _ in 0..N {
                handles.push(lean.allocate());
                handles.push(dense.allocate());
            }
        },
    );
    dhat::assert_eq!(blocks, 0);
}
