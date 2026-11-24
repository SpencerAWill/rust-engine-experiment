//! Conformance of the `bevy_ecs` backends against the reference, and the removal-buffer checks
//! (M3.10 R3, R4).

use spike_ecs_harness::{Params, Workload, check_conformance};

use crate::add_remove::{BevyAddRemove, BevySparseAddRemove};
use crate::agents::{BevyAgents, BevySparseAgents};
use crate::entity_churn::BevyEntityChurn;
use crate::frag_iter::BevyFragIter;
use crate::heavy_compute::BevyHeavyCompute;
use crate::random_access::BevyRandomAccess;
use crate::simple_iter::BevySimpleIter;
use crate::sprites::BevySprites;
use crate::{Sparse, Table};

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
fn random_access_bevy_matches_the_reference() {
    check_conformance::<spike_ecs_vec::random_access::VecRandomAccess, BevyRandomAccess>(
        PARAMS, 700,
    )
    .unwrap();
}

#[test]
fn random_access_bevy_clears_removals_each_step() {
    let mut w = BevyRandomAccess::setup(PARAMS);
    for tick in 1..=700 {
        w.step();
        let removed = w
            .world()
            .removed::<Table<spike_ecs_harness::random_access::Lifetime>>()
            .count();
        assert_eq!(
            removed, 0,
            "tick {tick}: step must end with World::clear_trackers"
        );
    }
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

#[test]
fn add_remove_bevy_sparse_matches_the_reference() {
    check_conformance::<spike_ecs_vec::add_remove::VecAddRemove, BevySparseAddRemove>(PARAMS, 120)
        .unwrap();
}

#[test]
fn agents_bevy_sparse_matches_the_reference() {
    check_conformance::<spike_ecs_vec::agents::VecAgents, BevySparseAgents>(PARAMS, 1_300).unwrap();
}

#[test]
fn add_remove_bevy_sparse_clears_removals_each_step() {
    let mut w = BevySparseAddRemove::setup(PARAMS);
    for _ in 0..3 {
        w.step();
    }
    let removed = w
        .world()
        .removed::<Sparse<spike_ecs_harness::add_remove::B>>()
        .count();
    assert_eq!(removed, 0, "step must end with World::clear_trackers");
}

#[test]
fn agents_bevy_sparse_clears_removals_each_step() {
    use crate::agents::Ttl;
    use spike_ecs_harness::agents::Seeker;

    let mut w = BevySparseAgents::setup(PARAMS);
    for tick in 1..=1_300 {
        w.step();
        assert_eq!(
            w.world().removed::<Table<Ttl>>().count(),
            0,
            "tick {tick}: step must end with World::clear_trackers"
        );
        assert_eq!(
            w.world().removed::<Sparse<Seeker>>().count(),
            0,
            "tick {tick}: step must end with World::clear_trackers"
        );
    }
    assert!(w.spawned() > PARAMS.count as u64, "no agent despawned");
    assert!(w.seeker_removals() > 0, "no seeker removed");
}

/// The storage type of component `C` in `world`.
fn storage<C: bevy_ecs::component::Component>(
    world: &bevy_ecs::world::World,
) -> bevy_ecs::component::StorageType {
    let id = world
        .components()
        .component_id::<C>()
        .expect("the component is registered");
    world
        .components()
        .get_info(id)
        .expect("the id has info")
        .storage_type()
}

#[test]
fn bevy_storage_is_table_for_every_type() {
    use crate::agents::{AgentId, Ttl, Velocity};
    use bevy_ecs::component::StorageType::Table as T;
    use engine::prelude::Transform2d;
    use spike_ecs_harness::add_remove::{A, B};
    use spike_ecs_harness::agents::Seeker;

    let mut ar = BevyAddRemove::setup(PARAMS);
    ar.step();
    assert_eq!(storage::<Table<A>>(ar.world()), T);
    assert_eq!(storage::<Table<B>>(ar.world()), T);

    let mut ag = BevyAgents::setup(PARAMS);
    ag.step();
    let w = ag.world();
    assert_eq!(storage::<Table<AgentId>>(w), T);
    assert_eq!(storage::<Table<Transform2d>>(w), T);
    assert_eq!(storage::<Table<Velocity>>(w), T);
    assert_eq!(storage::<Table<Ttl>>(w), T);
    assert_eq!(storage::<Table<Seeker>>(w), T);
}

#[test]
fn bevy_sparse_storage_is_sparse_for_b_and_seeker_only() {
    use crate::agents::{AgentId, Ttl, Velocity};
    use bevy_ecs::component::StorageType::{SparseSet, Table as T};
    use engine::prelude::Transform2d;
    use spike_ecs_harness::add_remove::{A, B};
    use spike_ecs_harness::agents::Seeker;

    let mut ar = BevySparseAddRemove::setup(PARAMS);
    ar.step();
    assert_eq!(storage::<Table<A>>(ar.world()), T);
    assert_eq!(storage::<Sparse<B>>(ar.world()), SparseSet);

    let mut ag = BevySparseAgents::setup(PARAMS);
    ag.step();
    let w = ag.world();
    assert_eq!(storage::<Table<AgentId>>(w), T);
    assert_eq!(storage::<Table<Transform2d>>(w), T);
    assert_eq!(storage::<Table<Velocity>>(w), T);
    assert_eq!(storage::<Table<Ttl>>(w), T);
    assert_eq!(storage::<Sparse<Seeker>>(w), SparseSet);
}

#[test]
fn bevy_sparse_insert_moves_archetype_not_table() {
    use bevy_ecs::entity::Entity;
    use bevy_ecs::query::With;
    use spike_ecs_harness::add_remove::{A, B};

    let mut w = BevySparseAddRemove::setup(PARAMS);
    let world = w.world_mut();
    let e = world
        .query_filtered::<Entity, With<Table<A>>>()
        .iter(world)
        .next()
        .expect("setup spawned entities");
    let tables = world.storages().tables.len();
    let archetype = world.entity(e).archetype().id();
    world.entity_mut(e).insert(Sparse(B(1.0)));
    assert_eq!(world.storages().tables.len(), tables, "a table was added");
    assert_ne!(
        world.entity(e).archetype().id(),
        archetype,
        "same archetype"
    );
}

#[test]
fn add_remove_bevy_never_registers_sparse_b() {
    use spike_ecs_harness::add_remove::B;

    let mut w = BevyAddRemove::setup(PARAMS);
    for _ in 0..3 {
        w.step();
    }
    assert!(w.world().components().component_id::<Sparse<B>>().is_none());
}

#[test]
fn add_remove_bevy_sparse_never_registers_table_b() {
    use spike_ecs_harness::add_remove::B;

    let mut w = BevySparseAddRemove::setup(PARAMS);
    for _ in 0..3 {
        w.step();
    }
    assert!(w.world().components().component_id::<Table<B>>().is_none());
}
