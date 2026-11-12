//! Snapshots: the whole simulation state is plain data, so cloning it mid-run and continuing
//! both copies gives identical results (the basis for replays, save/load, and rollback).
#![forbid(unsafe_code)]

use engine::prelude::*;
use fixed_step_sim::{Sim, SimConfig};

#[test]
fn clone_mid_run_continues_identically() {
    let mut sim = Sim::new(&SimConfig::default());
    sim.run_ticks(300);
    let mut copy = sim.clone();
    sim.run_ticks(300);
    copy.run_ticks(300);
    assert_eq!(sim.checksum(), copy.checksum());
    assert_eq!(sim.summary(), copy.summary());
}

#[test]
fn split_runs_equal_single_run() {
    let mut split = Sim::new(&SimConfig::default());
    split.run_ticks(300);
    split.run_ticks(300);
    let mut single = Sim::new(&SimConfig::default());
    single.run_ticks(600);
    assert_eq!(split.checksum(), single.checksum());
    assert_eq!(split.summary(), single.summary());
}

/// Restoring raw state must give a component that *continues* identically, not merely one that
/// reports the same state back: advance each component, snapshot, restore, and keep going.
#[test]
fn restored_components_continue_identically() {
    use core::time::Duration;

    // Prng: draw some values first so the state is not the freshly seeded one.
    let mut prng = Prng::from_seed(7);
    for _ in 0..50 {
        prng.next_u64();
    }
    let mut restored = Prng::from_state(prng.to_state()).unwrap();
    for _ in 0..100 {
        assert_eq!(prng.next_u64(), restored.next_u64());
    }

    // FixedTimestep: accumulate leftover time, restore, then feed both the same frames.
    let mut timestep = FixedTimestep::from_hz(core::num::NonZeroU32::new(60).unwrap());
    let _ = timestep.accumulate(Duration::from_millis(25)).count();
    let mut twin = FixedTimestep::from_state(timestep.to_state()).unwrap();
    for frame in [7_000_000_u64, 40_000_000, 3_000_000, 16_666_667] {
        let a = timestep.accumulate(Duration::from_nanos(frame)).count();
        let b = twin.accumulate(Duration::from_nanos(frame)).count();
        assert_eq!(a, b);
        assert_eq!(timestep.alpha().to_bits(), twin.alpha().to_bits());
    }

    // ManualClock: a clock restored at `now()` advances in lockstep with the original.
    let mut clock = ManualClock::new();
    clock.advance(Duration::from_millis(123));
    let mut clone = ManualClock::starting_at(clock.now());
    clock.advance(Duration::from_millis(5));
    clone.advance(Duration::from_millis(5));
    assert_eq!(clock.now(), clone.now());
}
