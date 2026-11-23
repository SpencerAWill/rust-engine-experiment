//! Conformance of the `hecs` backend against the reference (M3.10 R3).

use spike_ecs_harness::{Params, check_conformance};

use crate::add_remove::HecsAddRemove;
use crate::agents::HecsAgents;
use crate::entity_churn::HecsEntityChurn;
use crate::frag_iter::HecsFragIter;
use crate::heavy_compute::HecsHeavyCompute;
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
