//! Steady-state allocation test, in its own binary because it installs a global allocator.
//! `dhat` runs in testing mode, so no `unsafe` is needed in our code.
#![forbid(unsafe_code)]

use agents_2d::{Frame, Input, Sim, SimConfig, View, render};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[test]
fn zero_allocations_in_steady_state() {
    let _profiler = dhat::Profiler::builder().testing().build();
    let mut sim = Sim::new(&SimConfig::default());
    let mut pixels = vec![0_u32; 640 * 360];
    let view = View::fit(640, 360);
    let mut frame = Frame::new(&mut pixels, 640, 360).expect("buffer matches the size");
    let mut tick = |sim: &mut Sim| {
        sim.step(&Input::scripted(sim.tick()));
        render(sim, &view, &mut frame);
    };
    for _ in 0..120 {
        tick(&mut sim); // warm up
    }
    let before = dhat::HeapStats::get().total_blocks;
    for _ in 0..600 {
        tick(&mut sim);
    }
    // Read the stat first: `dhat::assert_eq!` suspends tracking itself, and `HeapStats::get`
    // does too, so calling it inside the macro nests the suspension and dhat panics.
    let after = dhat::HeapStats::get().total_blocks;
    dhat::assert_eq!(after, before);
}
