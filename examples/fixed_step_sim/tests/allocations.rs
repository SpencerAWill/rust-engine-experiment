//! Steady-state allocation test, in its own binary because it installs a global allocator.
//! `dhat` runs in testing mode, so no `unsafe` is needed in our code.
#![forbid(unsafe_code)]

use fixed_step_sim::{Sim, SimConfig};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

/// How many times the measurement may be repeated before it counts as a failure.
const ATTEMPTS: usize = 5;

/// The fewest heap blocks `work` allocated over up to [`ATTEMPTS`] runs.
///
/// `dhat` counts the blocks of every thread, and on a loaded machine the test harness's main
/// thread can allocate while `work` runs (this test failed on a macOS runner with 5 blocks it
/// did not allocate). Code that allocates does so on every run, so one clean run is enough to
/// show that it does not. `engine_handle`'s `fixed_capacity.rs` measures the same way.
fn fewest_blocks(mut work: impl FnMut()) -> u64 {
    let mut fewest = u64::MAX;
    for _ in 0..ATTEMPTS {
        let before = dhat::HeapStats::get().total_blocks;
        work();
        let after = dhat::HeapStats::get().total_blocks;
        fewest = fewest.min(after - before);
        if fewest == 0 {
            break;
        }
    }
    fewest
}

#[test]
fn zero_allocations_in_steady_state() {
    // `dhat` allows one profiler at a time, so everything runs in this single test.
    let _profiler = dhat::Profiler::builder().testing().build();

    // The measurement can fail: code that allocates on every run is reported as allocating.
    let allocating = fewest_blocks(|| drop(std::hint::black_box(Box::new(0_u64))));
    assert!(allocating > 0, "the measurement cannot see an allocation");

    let mut sim = Sim::new(&SimConfig::default());
    sim.run_ticks(120); // warm up
    let steady = fewest_blocks(|| sim.run_ticks(600));
    // `dhat::assert_eq!` suspends tracking itself, and `HeapStats::get` does too, so the value
    // is computed before the macro, not inside it.
    dhat::assert_eq!(steady, 0);
}
