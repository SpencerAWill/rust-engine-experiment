//! Conformance of the `hecs` backend against the reference (M3.10 R3).

use spike_ecs_harness::{Params, Workload, check_conformance};

use crate::add_remove::HecsAddRemove;
use crate::agents::HecsAgents;
use crate::entity_churn::HecsEntityChurn;
use crate::frag_iter::HecsFragIter;
use crate::heavy_compute::HecsHeavyCompute;
use crate::random_access::HecsRandomAccess;
use crate::simple_iter::HecsSimpleIter;
use crate::sprites::HecsSprites;

const PARAMS: Params = Params {
    count: 1_000,
    seed: 42,
};

#[test]
fn simple_iter_hecs_matches_the_reference() {
    check_conformance::<spike_ecs_vec::simple_iter::VecSimpleIter, HecsSimpleIter>(PARAMS, 120)
        .unwrap();
}

#[test]
fn frag_iter_hecs_matches_the_reference() {
    let params = Params {
        count: 20,
        seed: 42,
    };
    check_conformance::<spike_ecs_vec::frag_iter::VecFragIter, HecsFragIter>(params, 120).unwrap();
}

#[test]
fn heavy_compute_hecs_matches_the_reference() {
    check_conformance::<spike_ecs_vec::heavy_compute::VecHeavyCompute, HecsHeavyCompute>(
        PARAMS, 120,
    )
    .unwrap();
}

#[test]
fn add_remove_hecs_matches_the_reference() {
    check_conformance::<spike_ecs_vec::add_remove::VecAddRemove, HecsAddRemove>(PARAMS, 120)
        .unwrap();
}

#[test]
fn entity_churn_hecs_matches_the_reference() {
    check_conformance::<spike_ecs_vec::entity_churn::VecEntityChurn, HecsEntityChurn>(PARAMS, 120)
        .unwrap();
}

#[test]
fn sprites_hecs_matches_the_reference() {
    check_conformance::<spike_ecs_vec::sprites::VecSprites, HecsSprites>(PARAMS, 120).unwrap();
}

#[test]
fn agents_hecs_matches_the_reference() {
    check_conformance::<spike_ecs_vec::agents::VecAgents, HecsAgents>(PARAMS, 1_300).unwrap();
}

#[test]
fn random_access_hecs_matches_the_reference() {
    check_conformance::<spike_ecs_vec::random_access::VecRandomAccess, HecsRandomAccess>(
        PARAMS, 700,
    )
    .unwrap();
}

/// The retargeted handle is not hashed, so the reference comparison cannot see pass B write it
/// back. A target is stale after a step only if its slot was refilled in that step: an older
/// stale handle means the write-back is missing.
#[test]
fn random_access_hecs_writes_back_retargets() {
    let mut w = HecsRandomAccess::setup(PARAMS);
    let mut stale = 0_usize;
    for tick in 1..=700 {
        let first_new = w.next_id();
        w.step();
        for occupant in w.stale_target_occupants() {
            assert!(
                occupant >= first_new,
                "tick {tick}: a stale target's slot holds spawn {occupant}, spawned before this step (first new: {first_new})"
            );
            stale += 1;
        }
    }
    assert!(stale > 0, "no stale target seen over the run");
}
