//! Determinism: the same configuration gives the same result, and only the seeds that matter
//! change it.
#![forbid(unsafe_code)]

use fixed_step_sim::{SimConfig, run};

/// The default configuration's checksum, frozen. It must match on every architecture: the sim
/// uses only basic IEEE `Vec3`/`f32` operations (spec §5.6 item 8).
const GOLDEN: u64 = 0x1ac4_a73e_2c89_41d1;

#[test]
fn same_seed_same_checksum() {
    assert_eq!(run(&SimConfig::default()), run(&SimConfig::default()));
}

#[test]
fn different_seed_different_checksum() {
    let other = SimConfig {
        seed: 43,
        ..SimConfig::default()
    };
    assert_ne!(run(&other).checksum, run(&SimConfig::default()).checksum);
}

#[test]
fn jitter_split_does_not_matter() {
    let expected = run(&SimConfig::default()).checksum;
    for jitter_seed in [8, 9999] {
        let config = SimConfig {
            jitter_seed,
            ..SimConfig::default()
        };
        assert_eq!(run(&config).checksum, expected, "jitter seed {jitter_seed}");
    }
}

#[test]
fn summary_reports_the_requested_ticks_and_a_full_pool() {
    let config = SimConfig::default();
    let summary = run(&config);
    assert_eq!(summary.tick, config.ticks);
    assert_eq!(summary.live, config.particles);
    assert!(summary.capacity >= config.particles);
}

#[test]
fn golden_checksum() {
    assert_eq!(run(&SimConfig::default()).checksum, GOLDEN);
}
