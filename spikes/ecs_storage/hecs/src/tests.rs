//! Conformance of the `hecs` backend against the reference (M3.10 R3).

use spike_ecs_harness::{Params, Workload, check_conformance};

use crate::add_remove::HecsAddRemove;
use crate::agents::HecsAgents;
use crate::entity_churn::HecsEntityChurn;
use crate::frag_iter::HecsFragIter;
use crate::heavy_compute::HecsHeavyCompute;
use crate::many_queries::HecsManyQueries;
use crate::random_access::HecsRandomAccess;
use crate::simple_iter::HecsSimpleIter;
use crate::sprites::HecsSprites;
use crate::wide_entities::HecsWideEntities;

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
fn add_remove_hecs_inserts_on_every_entity_each_step() {
    let mut w = HecsAddRemove::setup(PARAMS);
    for tick in 1..=120 {
        w.step();
        let removed = w.removed_last_step();
        assert_eq!(
            removed.len(),
            PARAMS.count,
            "the step removed B from the wrong number of entities at tick {tick}"
        );
        let distinct: std::collections::BTreeSet<_> = removed.iter().collect();
        assert_eq!(
            distinct.len(),
            PARAMS.count,
            "an entity was removed twice in one step at tick {tick}"
        );
    }
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

#[test]
fn many_queries_hecs_matches_the_reference() {
    check_conformance::<spike_ecs_vec::many_queries::VecManyQueries, HecsManyQueries>(PARAMS, 120)
        .unwrap();
}

#[test]
fn many_queries_hecs_zero_count_is_valid() {
    let mut w = HecsManyQueries::setup(Params { count: 0, seed: 42 });
    w.step();
    assert_eq!(w.checksum(), 0);
}

#[test]
fn wide_entities_hecs_matches_the_reference() {
    check_conformance::<spike_ecs_vec::wide_entities::VecWideEntities, HecsWideEntities>(
        PARAMS, 120,
    )
    .unwrap();
}

#[test]
fn wide_entities_hecs_zero_count_is_valid() {
    let mut w = HecsWideEntities::setup(Params { count: 0, seed: 7 });
    assert_eq!(w.checksum(), 0);
    w.step();
    assert_eq!(w.checksum(), 0);
}

#[test]
fn wide_entities_hecs_inserts_on_every_entity_each_step() {
    let mut w = HecsWideEntities::setup(PARAMS);
    for tick in 1..=120 {
        w.step();
        let removed = w.removed_last_step();
        assert_eq!(
            removed.len(),
            PARAMS.count,
            "the step removed Extra from the wrong number of entities at tick {tick}"
        );
        let distinct: std::collections::BTreeSet<_> = removed.iter().collect();
        assert_eq!(
            distinct.len(),
            PARAMS.count,
            "an entity was removed twice in one step at tick {tick}"
        );
    }
}

#[test]
fn wide_entities_hecs_places_entities_in_one_archetype() {
    let w = HecsWideEntities::setup(PARAMS);
    // The root archetype and the 24-type one.
    assert_eq!(w.world().archetypes().len(), 2);
}
