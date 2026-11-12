//! A fixed-capacity pool or allocator never touches the heap after construction.
//!
//! In its own binary because it installs a global allocator; `dhat` runs in testing mode, so no
//! `unsafe` is needed. WASI is excluded because `dhat` needs OS features it does not have.
#![forbid(unsafe_code)]
#![cfg(not(target_os = "wasi"))]

use engine_handle::{DenseHandleAllocator, HandleAllocator, HandlePool};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[test]
fn fixed_capacity_containers_never_allocate_after_construction() {
    const N: usize = 1_000;
    // `dhat` allows one profiler at a time, so everything runs in this single test.
    let _profiler = dhat::Profiler::builder().testing().build();

    let mut pool = HandlePool::<u64>::with_fixed_capacity(N);
    let mut lean = HandleAllocator::<u8>::with_fixed_capacity(N);
    let mut dense = DenseHandleAllocator::<u8>::with_fixed_capacity(N);
    // The handle lists below are test scaffolding, built before measuring starts.
    let mut pool_handles = Vec::with_capacity(N);
    let mut lean_handles = Vec::with_capacity(N);
    let mut dense_handles = Vec::with_capacity(N);

    let before = dhat::HeapStats::get().total_blocks;
    for round in 0..20_u64 {
        pool_handles.clear();
        lean_handles.clear();
        dense_handles.clear();
        for i in 0..N as u64 {
            pool_handles.push(pool.insert(i + round));
            lean_handles.push(lean.allocate());
            dense_handles.push(dense.allocate());
        }
        assert!(pool.try_insert(0).is_err());
        assert!(lean.try_allocate().is_none());
        assert!(dense.try_allocate().is_none());
        for h in pool_handles.iter().step_by(3) {
            pool.remove(*h);
        }
        pool.retain(|_, v| *v % 2 == 0);
        let _ = pool.iter().count() + dense.iter().count();
        if round % 2 == 0 {
            pool.clear();
            lean.clear();
            dense.clear();
        } else {
            for h in &lean_handles {
                lean.free(*h);
            }
            for h in &dense_handles {
                dense.free(*h);
            }
            for (_, _) in pool.drain() {}
        }
    }
    // Read the stat first: `dhat::assert_eq!` suspends tracking itself, so calling
    // `HeapStats::get` inside it nests the suspension and dhat panics.
    let after = dhat::HeapStats::get().total_blocks;
    dhat::assert_eq!(after, before);
}
