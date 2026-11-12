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

#[test]
fn raw_state_roundtrips_mid_run() {
    let mut sim = Sim::new(&SimConfig::default());
    sim.run_ticks(300);

    assert_eq!(
        Prng::from_state(sim.prng().to_state()).as_ref(),
        Some(sim.prng())
    );

    let restored = FixedTimestep::from_state(sim.timestep().to_state()).unwrap();
    assert_eq!(restored.to_state(), sim.timestep().to_state());

    assert_eq!(
        ManualClock::starting_at(sim.clock().now()).now(),
        sim.clock().now()
    );
}
