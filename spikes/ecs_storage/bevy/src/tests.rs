//! Conformance of the `bevy_ecs` backends against the reference, and the removal-buffer checks
//! (M3.10 R3, R4).

use spike_ecs_harness::{Params, Workload, check_conformance};

use crate::Table;
use crate::add_remove::BevyAddRemove;
use crate::agents::BevyAgents;
use crate::entity_churn::BevyEntityChurn;
use crate::frag_iter::BevyFragIter;
use crate::heavy_compute::BevyHeavyCompute;
use crate::simple_iter::BevySimpleIter;
use crate::sprites::BevySprites;

const PARAMS: Params = Params {
    count: 1_000,
    seed: 42,
};

#[test]
fn simple_iter_bevy_matches_the_reference() {
    check_conformance::<spike_ecs_vec::simple_iter::VecSimpleIter, BevySimpleIter>(PARAMS, 120)
        .unwrap();
}

#[test]
fn frag_iter_bevy_matches_the_reference() {
    let params = Params {
        count: 20,
        seed: 42,
    };
    check_conformance::<spike_ecs_vec::frag_iter::VecFragIter, BevyFragIter>(params, 120).unwrap();
}

#[test]
fn heavy_compute_bevy_matches_the_reference() {
    check_conformance::<spike_ecs_vec::heavy_compute::VecHeavyCompute, BevyHeavyCompute>(
        PARAMS, 120,
    )
    .unwrap();
}

#[test]
fn add_remove_bevy_matches_the_reference() {
    check_conformance::<spike_ecs_vec::add_remove::VecAddRemove, BevyAddRemove>(PARAMS, 120)
        .unwrap();
}

#[test]
fn entity_churn_bevy_matches_the_reference() {
    check_conformance::<spike_ecs_vec::entity_churn::VecEntityChurn, BevyEntityChurn>(PARAMS, 120)
        .unwrap();
}

#[test]
fn sprites_bevy_matches_the_reference() {
    check_conformance::<spike_ecs_vec::sprites::VecSprites, BevySprites>(PARAMS, 120).unwrap();
}

#[test]
fn agents_bevy_matches_the_reference() {
    check_conformance::<spike_ecs_vec::agents::VecAgents, BevyAgents>(PARAMS, 1_300).unwrap();
}

#[test]
fn add_remove_bevy_clears_removals_each_step() {
    let mut w = BevyAddRemove::setup(PARAMS);
    for _ in 0..3 {
        w.step();
    }
    let removed = w
        .world()
        .removed::<Table<spike_ecs_harness::add_remove::B>>()
        .count();
    assert_eq!(removed, 0, "step must end with World::clear_trackers");
}

#[test]
fn entity_churn_bevy_clears_removals_each_step() {
    let mut w = BevyEntityChurn::setup(PARAMS);
    for _ in 0..3 {
        w.step();
    }
    let removed = w
        .world()
        .removed::<Table<spike_ecs_harness::entity_churn::Lifetime>>()
        .count();
    assert_eq!(removed, 0, "step must end with World::clear_trackers");
}

#[test]
fn agents_bevy_clears_removals_each_step() {
    use crate::agents::Ttl;
    use spike_ecs_harness::agents::Seeker;

    let mut w = BevyAgents::setup(PARAMS);
    for tick in 1..=1_300 {
        w.step();
        assert_eq!(
            w.world().removed::<Table<Ttl>>().count(),
            0,
            "tick {tick}: step must end with World::clear_trackers"
        );
        assert_eq!(
            w.world().removed::<Table<Seeker>>().count(),
            0,
            "tick {tick}: step must end with World::clear_trackers"
        );
    }
    // The run must have exercised both removal kinds, or the checks above prove nothing.
    assert!(w.spawned() > PARAMS.count as u64, "no agent despawned");
    assert!(w.seeker_removals() > 0, "no seeker removed");
}
