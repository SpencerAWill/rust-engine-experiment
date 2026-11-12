//! Steady-state allocation test, in its own binary because it installs a global allocator.
//! `dhat` runs in testing mode, so no `unsafe` is needed in our code.
#![forbid(unsafe_code)]

use fixed_step_sim::{Sim, SimConfig};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[test]
fn zero_allocations_in_steady_state() {
    let _profiler = dhat::Profiler::builder().testing().build();
    let mut sim = Sim::new(&SimConfig::default());
    sim.run_ticks(120); // warm up
    let before = dhat::HeapStats::get().total_blocks;
    sim.run_ticks(600);
    // Read the stat first: `dhat::assert_eq!` suspends tracking itself, and `HeapStats::get`
    // does too, so calling it inside the macro nests the suspension and dhat panics.
    let after = dhat::HeapStats::get().total_blocks;
    dhat::assert_eq!(after, before);
}
