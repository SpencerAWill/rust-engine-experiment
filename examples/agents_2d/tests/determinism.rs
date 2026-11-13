//! Determinism of the simulation: same inputs, same checksum; a clone continues identically; and
//! a golden value pins the rules (it must match on every architecture).
#![forbid(unsafe_code)]

use agents_2d::{Input, Sim, SimConfig, run};

/// Checksum of `run(&SimConfig::default(), 600)`. It must be identical on every architecture CI
/// tests; a difference is a finding about same-build determinism, not a value to update.
const GOLDEN: u64 = 0xa352_0200_2876_ad91;

#[test]
fn same_config_same_summary() {
    let config = SimConfig {
        agents: 1_000,
        ..SimConfig::default()
    };
    assert_eq!(run(&config, 300), run(&config, 300));
}

#[test]
fn different_seed_different_checksum() {
    let a = SimConfig {
        agents: 1_000,
        ..SimConfig::default()
    };
    let b = SimConfig {
        seed: 43,
        ..a.clone()
    };
    assert_ne!(run(&a, 100).checksum, run(&b, 100).checksum);
}

#[test]
fn clone_mid_run_continues_identically() {
    let mut sim = Sim::new(&SimConfig {
        agents: 1_000,
        ..SimConfig::default()
    });
    for _ in 0..300 {
        sim.step(&Input::scripted(sim.tick()));
    }
    let mut copy = sim.clone();
    for _ in 300..600 {
        sim.step(&Input::scripted(sim.tick()));
        copy.step(&Input::scripted(copy.tick()));
    }
    assert_eq!(sim.summary(), copy.summary());
}

#[test]
fn golden_checksum() {
    let summary = run(&SimConfig::default(), 600);
    assert_eq!(summary.checksum, GOLDEN, "{summary:?}");
}
