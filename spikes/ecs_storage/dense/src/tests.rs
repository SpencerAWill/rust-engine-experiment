//! The model test: a `World` against a plain map, under random operations.

use std::collections::BTreeMap;

use proptest::prelude::*;

use crate::{Commands, Entity, World};

#[derive(Debug, Clone, PartialEq)]
struct C0(u32);
#[derive(Debug, Clone, PartialEq)]
struct C1(u32);
#[derive(Debug, Clone, PartialEq)]
struct C2(u32);
#[derive(Debug, Clone, PartialEq)]
struct C3(u32);

#[derive(Debug, Clone)]
enum Op {
    Spawn,
    Despawn(usize),
    Insert(usize, usize, u32),
    Remove(usize, usize),
    DeferInsert(usize, usize, u32),
    DeferRemove(usize, usize),
    DeferDespawn(usize),
    Apply,
}

/// One command recorded in the buffer, as the model keeps it.
#[derive(Debug, Clone, Copy)]
enum Recorded {
    Insert(Entity, usize, u32),
    Remove(Entity, usize),
    Despawn(Entity),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        2 => Just(Op::Spawn),
        1 => any::<usize>().prop_map(Op::Despawn),
        5 => (any::<usize>(), 0usize..4, any::<u32>()).prop_map(|(k, t, v)| Op::Insert(k, t, v)),
        3 => (any::<usize>(), 0usize..4).prop_map(|(k, t)| Op::Remove(k, t)),
        3 => (any::<usize>(), 0usize..4, any::<u32>())
            .prop_map(|(k, t, v)| Op::DeferInsert(k, t, v)),
        2 => (any::<usize>(), 0usize..4).prop_map(|(k, t)| Op::DeferRemove(k, t)),
        1 => any::<usize>().prop_map(Op::DeferDespawn),
        2 => Just(Op::Apply),
    ]
}

fn insert(world: &mut World, entity: Entity, ty: usize, val: u32) -> Option<u32> {
    match ty {
        0 => world.insert(entity, C0(val)).map(|c| c.0),
        1 => world.insert(entity, C1(val)).map(|c| c.0),
        2 => world.insert(entity, C2(val)).map(|c| c.0),
        _ => world.insert(entity, C3(val)).map(|c| c.0),
    }
}

fn remove(world: &mut World, entity: Entity, ty: usize) -> Option<u32> {
    match ty {
        0 => world.remove::<C0>(entity).map(|c| c.0),
        1 => world.remove::<C1>(entity).map(|c| c.0),
        2 => world.remove::<C2>(entity).map(|c| c.0),
        _ => world.remove::<C3>(entity).map(|c| c.0),
    }
}

fn defer_insert(commands: &mut Commands, entity: Entity, ty: usize, val: u32) {
    match ty {
        0 => commands.insert(entity, C0(val)),
        1 => commands.insert(entity, C1(val)),
        2 => commands.insert(entity, C2(val)),
        _ => commands.insert(entity, C3(val)),
    }
}

fn defer_remove(commands: &mut Commands, entity: Entity, ty: usize) {
    match ty {
        0 => commands.remove::<C0>(entity),
        1 => commands.remove::<C1>(entity),
        2 => commands.remove::<C2>(entity),
        _ => commands.remove::<C3>(entity),
    }
}

/// Applies the recorded commands to the model in order, skipping those on dead entities.
fn apply_to_model(model: &mut Model, recorded: &mut Vec<Recorded>) {
    for command in recorded.drain(..) {
        let entity = match command {
            Recorded::Insert(e, ..) | Recorded::Remove(e, _) | Recorded::Despawn(e) => e,
        };
        let live = model
            .get(&entity.index())
            .is_some_and(|(e, _)| *e == entity);
        if !live {
            continue;
        }
        match command {
            Recorded::Insert(_, ty, val) => {
                model.get_mut(&entity.index()).unwrap().1[ty] = Some(val);
            }
            Recorded::Remove(_, ty) => {
                model.get_mut(&entity.index()).unwrap().1[ty] = None;
            }
            Recorded::Despawn(_) => {
                model.remove(&entity.index());
            }
        }
    }
}

/// The model's entity sequences for the queries the test compares, in index order.
fn model_matches(
    model: &Model,
    f: impl Fn(&[Option<u32>; 4]) -> bool,
) -> Vec<(Entity, [Option<u32>; 4])> {
    model
        .values()
        .filter(|(_, v)| f(v))
        .map(|(e, v)| (*e, *v))
        .collect()
}

/// Runs the five read queries by `iter` and by `for_each` and compares them with the model.
fn check_queries(world: &World, model: &Model) {
    // (Entity, &C0, &C1)
    let want: Vec<(Entity, u32, u32)> = model_matches(model, |v| v[0].is_some() && v[1].is_some())
        .into_iter()
        .map(|(e, v)| (e, v[0].unwrap(), v[1].unwrap()))
        .collect();
    let got: Vec<_> = world
        .query::<(Entity, &C0, &C1)>()
        .iter()
        .map(|(e, a, b)| (e, a.0, b.0))
        .collect();
    assert_eq!(got, want, "(Entity, &C0, &C1) by iter");
    let mut got = Vec::new();
    world
        .query::<(Entity, &C0, &C1)>()
        .for_each(|(e, a, b)| got.push((e, a.0, b.0)));
    assert_eq!(got, want, "(Entity, &C0, &C1) by for_each");

    // (Entity, &C0) without C2
    let want: Vec<(Entity, u32)> = model_matches(model, |v| v[0].is_some() && v[2].is_none())
        .into_iter()
        .map(|(e, v)| (e, v[0].unwrap()))
        .collect();
    let got: Vec<_> = world
        .query::<(Entity, &C0)>()
        .without::<C2>()
        .iter()
        .map(|(e, a)| (e, a.0))
        .collect();
    assert_eq!(got, want, "(Entity, &C0) without C2 by iter");
    let mut got = Vec::new();
    world
        .query::<(Entity, &C0)>()
        .without::<C2>()
        .for_each(|(e, a)| got.push((e, a.0)));
    assert_eq!(got, want, "(Entity, &C0) without C2 by for_each");

    // (Entity, Option<&C3>) with C1
    let want: Vec<(Entity, Option<u32>)> = model_matches(model, |v| v[1].is_some())
        .into_iter()
        .map(|(e, v)| (e, v[3]))
        .collect();
    let got: Vec<_> = world
        .query::<(Entity, Option<&C3>)>()
        .with::<C1>()
        .iter()
        .map(|(e, c)| (e, c.map(|c| c.0)))
        .collect();
    assert_eq!(got, want, "(Entity, Option<&C3>) with C1 by iter");
    let mut got = Vec::new();
    world
        .query::<(Entity, Option<&C3>)>()
        .with::<C1>()
        .for_each(|(e, c)| got.push((e, c.map(|c| c.0))));
    assert_eq!(got, want, "(Entity, Option<&C3>) with C1 by for_each");

    // Entity, and Entity without C0
    let want: Vec<Entity> = model.values().map(|(e, _)| *e).collect();
    let got: Vec<Entity> = world.query::<Entity>().iter().collect();
    assert_eq!(got, want, "Entity by iter");
    let mut got = Vec::new();
    world.query::<Entity>().for_each(|e| got.push(e));
    assert_eq!(got, want, "Entity by for_each");

    let want: Vec<Entity> = model_matches(model, |v| v[0].is_none())
        .into_iter()
        .map(|(e, _)| e)
        .collect();
    let got: Vec<Entity> = world.query::<Entity>().without::<C0>().iter().collect();
    assert_eq!(got, want, "Entity without C0 by iter");
    let mut got = Vec::new();
    world
        .query::<Entity>()
        .without::<C0>()
        .for_each(|e| got.push(e));
    assert_eq!(got, want, "Entity without C0 by for_each");
}

/// Adds 1 to every `C0`, and to every `C1` that goes with one, in the world and the model.
fn bump(world: &mut World, model: &mut Model) {
    world
        .query_mut::<(&mut C0, Option<&mut C1>)>()
        .for_each(|(a, b)| {
            a.0 = a.0.wrapping_add(1);
            if let Some(b) = b {
                b.0 = b.0.wrapping_add(1);
            }
        });
    for (_, values) in model.values_mut() {
        if let Some(a) = values[0].as_mut() {
            *a = a.wrapping_add(1);
            if let Some(b) = values[1].as_mut() {
                *b = b.wrapping_add(1);
            }
        }
    }
}

fn get(world: &World, entity: Entity, ty: usize) -> Option<u32> {
    match ty {
        0 => world.get::<C0>(entity).map(|c| c.0),
        1 => world.get::<C1>(entity).map(|c| c.0),
        2 => world.get::<C2>(entity).map(|c| c.0),
        _ => world.get::<C3>(entity).map(|c| c.0),
    }
}

/// The live entities, by index: each one's ID and its four optional values.
type Model = BTreeMap<u32, (Entity, [Option<u32>; 4])>;

fn run(ops: &[Op]) {
    let mut world = World::new();
    let mut spawned: Vec<Entity> = Vec::new();
    let mut model = Model::new();
    let mut commands = Commands::new();
    let mut recorded: Vec<Recorded> = Vec::new();
    for op in ops {
        match *op {
            Op::Spawn => {
                let entity = world.spawn();
                spawned.push(entity);
                model.insert(entity.index(), (entity, [None; 4]));
            }
            Op::Despawn(k) if !spawned.is_empty() => {
                let entity = spawned[k % spawned.len()];
                let was_live = model
                    .get(&entity.index())
                    .is_some_and(|(e, _)| *e == entity);
                assert_eq!(world.despawn(entity), was_live);
                if was_live {
                    model.remove(&entity.index());
                }
            }
            Op::Insert(k, ty, val) if !spawned.is_empty() => {
                let entity = spawned[k % spawned.len()];
                if let Some((live, values)) = model.get_mut(&entity.index())
                    && *live == entity
                {
                    let old = values[ty].replace(val);
                    assert_eq!(insert(&mut world, entity, ty, val), old);
                }
            }
            Op::Remove(k, ty) if !spawned.is_empty() => {
                let entity = spawned[k % spawned.len()];
                let old = match model.get_mut(&entity.index()) {
                    Some((live, values)) if *live == entity => values[ty].take(),
                    _ => None,
                };
                assert_eq!(remove(&mut world, entity, ty), old);
            }
            Op::DeferInsert(k, ty, val) if !spawned.is_empty() => {
                // Unlike `World::insert`, a recorded insert on a dead entity is allowed.
                let entity = spawned[k % spawned.len()];
                defer_insert(&mut commands, entity, ty, val);
                recorded.push(Recorded::Insert(entity, ty, val));
            }
            Op::DeferRemove(k, ty) if !spawned.is_empty() => {
                let entity = spawned[k % spawned.len()];
                defer_remove(&mut commands, entity, ty);
                recorded.push(Recorded::Remove(entity, ty));
            }
            Op::DeferDespawn(k) if !spawned.is_empty() => {
                let entity = spawned[k % spawned.len()];
                commands.despawn(entity);
                recorded.push(Recorded::Despawn(entity));
            }
            Op::Apply => {
                assert_eq!(commands.len(), recorded.len());
                world.apply(&mut commands);
                apply_to_model(&mut model, &mut recorded);
                assert!(commands.is_empty());
            }
            _ => {}
        }
        assert_eq!(world.check_invariants(), Ok(()));
        assert_eq!(world.len(), model.len());
        check_queries(&world, &model);
        bump(&mut world, &mut model);
        assert_eq!(world.check_invariants(), Ok(()));
        for entity in &spawned {
            let expected = match model.get(&entity.index()) {
                Some((live, values)) if live == entity => *values,
                _ => [None; 4],
            };
            for (ty, want) in expected.iter().enumerate() {
                assert_eq!(get(&world, *entity, ty), *want, "{entity:?} type {ty}");
            }
        }
    }
}

proptest! {
    #[test]
    fn world_matches_the_model(ops in prop::collection::vec(op(), 0..64)) {
        run(&ops);
    }
}

mod workloads {
    use core::any::TypeId;

    use spike_ecs_harness::{Params, Workload, check_conformance};
    use spike_ecs_vec::add_remove::VecAddRemove;
    use spike_ecs_vec::agents::VecAgents;
    use spike_ecs_vec::entity_churn::VecEntityChurn;
    use spike_ecs_vec::frag_iter::VecFragIter;
    use spike_ecs_vec::heavy_compute::VecHeavyCompute;
    use spike_ecs_vec::many_queries::VecManyQueries;
    use spike_ecs_vec::random_access::VecRandomAccess;
    use spike_ecs_vec::simple_iter::VecSimpleIter;
    use spike_ecs_vec::sprites::VecSprites;
    use spike_ecs_vec::wide_entities::VecWideEntities;

    use crate::World;
    use crate::add_remove::DenseAddRemove;
    use crate::agents::DenseAgents;
    use crate::entity_churn::DenseEntityChurn;
    use crate::frag_iter::DenseFragIter;
    use crate::heavy_compute::DenseHeavyCompute;
    use crate::many_queries::DenseManyQueries;
    use crate::query::Joined;
    use crate::random_access::DenseRandomAccess;
    use crate::simple_iter::DenseSimpleIter;
    use crate::sprites::DenseSprites;
    use crate::wide_entities::DenseWideEntities;

    const PARAMS: Params = Params {
        count: 1_000,
        seed: 42,
    };
    const TICKS: u64 = 120;
    const AGENT_TICKS: u64 = 1_300;
    const RANDOM_ACCESS_TICKS: u64 = 700;

    #[test]
    fn random_access_dense_matches_the_reference() {
        check_conformance::<VecRandomAccess, DenseRandomAccess>(PARAMS, RANDOM_ACCESS_TICKS)
            .unwrap();
    }

    /// The joined list for the component types `T`, named by the world's own ids.
    fn ids(world: &World, types: &[TypeId]) -> Vec<Joined> {
        types
            .iter()
            .map(|ty| Joined::Component(world.component_id(*ty).expect("type has a column")))
            .collect()
    }

    #[test]
    fn simple_iter_dense_matches_the_reference() {
        check_conformance::<VecSimpleIter, DenseSimpleIter>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn frag_iter_dense_matches_the_reference() {
        check_conformance::<VecFragIter, DenseFragIter>(
            Params {
                count: 20,
                seed: 42,
            },
            TICKS,
        )
        .unwrap();
    }

    #[test]
    fn heavy_compute_dense_matches_the_reference() {
        check_conformance::<VecHeavyCompute, DenseHeavyCompute>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn simple_iter_dense_joins() {
        use spike_ecs_harness::simple_iter::{Position, Velocity};
        let mut w = DenseSimpleIter::setup(Params {
            count: 10,
            seed: 42,
        });
        let want = ids(
            w.world(),
            &[TypeId::of::<Position>(), TypeId::of::<Velocity>()],
        );
        assert_eq!(w.step_joined(), (want, vec![]));
    }

    #[test]
    fn frag_iter_dense_joins() {
        use spike_ecs_harness::frag_iter::Data;
        let mut w = DenseFragIter::setup(Params { count: 2, seed: 42 });
        let want = ids(w.world(), &[TypeId::of::<Data>()]);
        assert_eq!(w.step_joined(), (want, vec![]));
    }

    #[test]
    fn heavy_compute_dense_joins() {
        use spike_ecs_harness::heavy_compute::{Matrix, Position, Rotation, Velocity};
        let mut w = DenseHeavyCompute::setup(Params {
            count: 10,
            seed: 42,
        });
        let want = ids(
            w.world(),
            &[
                TypeId::of::<Matrix>(),
                TypeId::of::<Position>(),
                TypeId::of::<Rotation>(),
                TypeId::of::<Velocity>(),
            ],
        );
        assert_eq!(w.step_joined(), (want, vec![]));
    }

    #[test]
    fn add_remove_dense_matches_the_reference() {
        check_conformance::<VecAddRemove, DenseAddRemove>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn add_remove_dense_inserts_on_every_entity_each_step() {
        let mut w = DenseAddRemove::setup(PARAMS);
        for tick in 1..=TICKS {
            w.step();
            let removed = w.removed_last_step();
            assert_eq!(
                removed.len(),
                PARAMS.count,
                "the step removed B from the wrong number of entities at tick {tick}"
            );
            let distinct: std::collections::BTreeSet<u64> =
                removed.iter().map(|e| e.to_bits()).collect();
            assert_eq!(
                distinct.len(),
                PARAMS.count,
                "an entity was removed twice in one step at tick {tick}"
            );
        }
    }

    #[test]
    fn wide_entities_dense_matches_the_reference() {
        check_conformance::<VecWideEntities, DenseWideEntities>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn wide_entities_dense_zero_count_is_valid() {
        zero_count::<DenseWideEntities>();
    }

    /// Steps 120 times; after each step every entity must have been collected for removal, once,
    /// which only happens if the insert pass gave each one an `Extra` (ecs-11: the checksum cannot
    /// see the removal list).
    #[test]
    fn wide_entities_dense_inserts_on_every_entity_each_step() {
        let mut w = DenseWideEntities::setup(PARAMS);
        for tick in 1..=TICKS {
            w.step();
            let removed = w.removed_last_step();
            assert_eq!(
                removed.len(),
                PARAMS.count,
                "the step removed Extra from the wrong number of entities at tick {tick}"
            );
            let distinct: std::collections::BTreeSet<u64> =
                removed.iter().map(|e| e.to_bits()).collect();
            assert_eq!(
                distinct.len(),
                PARAMS.count,
                "an entity was removed twice in one step at tick {tick}"
            );
        }
    }

    #[test]
    fn entity_churn_dense_matches_the_reference() {
        check_conformance::<VecEntityChurn, DenseEntityChurn>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn add_remove_dense_joins() {
        use spike_ecs_harness::add_remove::{A, B};
        // One step first: `B` has no column until the first insert.
        let mut w = DenseAddRemove::setup(Params { count: 10, seed: 1 });
        w.step();
        let read = ids(w.world(), &[TypeId::of::<A>()]);
        assert_eq!(w.read_joined(), (read, vec![]));
        let remove = ids(w.world(), &[TypeId::of::<B>()]);
        assert_eq!(w.remove_joined(), (remove, vec![]));
    }

    #[test]
    fn entity_churn_dense_joins() {
        use spike_ecs_harness::entity_churn::{Lifetime, Position, Velocity};
        let mut w = DenseEntityChurn::setup(Params { count: 10, seed: 1 });
        let want = ids(
            w.world(),
            &[
                TypeId::of::<Position>(),
                TypeId::of::<Velocity>(),
                TypeId::of::<Lifetime>(),
            ],
        );
        assert_eq!(w.step_joined(), (want, vec![]));
    }

    #[test]
    fn add_remove_keeps_invariants_every_tick() {
        use spike_ecs_harness::add_remove::{A, B};
        let mut w = DenseAddRemove::setup(PARAMS);
        w.world().check_invariants().unwrap();
        for _ in 0..TICKS {
            w.step();
            w.world().check_invariants().unwrap();
            assert_eq!(w.world().len(), PARAMS.count);
            assert_eq!(w.world().query::<&B>().into_iter().count(), 0);
            assert_eq!(w.world().query::<&A>().into_iter().count(), PARAMS.count);
        }
    }

    #[test]
    fn entity_churn_keeps_invariants_every_tick() {
        let mut w = DenseEntityChurn::setup(PARAMS);
        w.world().check_invariants().unwrap();
        for _ in 0..TICKS {
            w.step();
            w.world().check_invariants().unwrap();
            assert_eq!(w.world().len(), PARAMS.count);
        }
    }

    #[test]
    fn sprites_dense_matches_the_reference() {
        check_conformance::<VecSprites, DenseSprites>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn agents_dense_matches_the_reference() {
        check_conformance::<VecAgents, DenseAgents>(PARAMS, AGENT_TICKS).unwrap();
    }

    #[test]
    fn sprites_dense_joins() {
        use engine::prelude::Transform2d;
        use spike_ecs_harness::sprites::{Hidden, Sprite, Velocity2d};
        let mut w = DenseSprites::setup(Params { count: 10, seed: 1 });
        let want = ids(
            w.world(),
            &[TypeId::of::<Transform2d>(), TypeId::of::<Velocity2d>()],
        );
        assert_eq!(w.move_joined(), (want, vec![]));
        let want = ids(
            w.world(),
            &[TypeId::of::<Transform2d>(), TypeId::of::<Sprite>()],
        );
        let hidden = ids(w.world(), &[TypeId::of::<Hidden>()]);
        assert_eq!(w.draw_joined(), (want, hidden));
    }

    #[test]
    fn agents_dense_joins() {
        use crate::agents::{Ttl, Velocity};
        use engine::prelude::Transform2d;
        let mut w = DenseAgents::setup(Params { count: 10, seed: 1 });
        let want = ids(
            w.world(),
            &[
                TypeId::of::<Transform2d>(),
                TypeId::of::<Velocity>(),
                TypeId::of::<Ttl>(),
            ],
        );
        assert_eq!(w.step_joined(), (want, vec![]));
    }

    #[test]
    fn agents_keeps_invariants_every_tick() {
        let mut w = DenseAgents::setup(PARAMS);
        w.world().check_invariants().unwrap();
        for _ in 0..AGENT_TICKS {
            w.step();
            w.world().check_invariants().unwrap();
            assert_eq!(w.world().len(), PARAMS.count);
        }
    }

    fn zero_count<W: Workload>() {
        let mut w = W::setup(Params { count: 0, seed: 7 });
        assert_eq!(w.checksum(), 0);
        w.step();
        assert_eq!(w.checksum(), 0);
    }

    #[test]
    fn zero_count_is_valid() {
        zero_count::<DenseSimpleIter>();
        zero_count::<DenseFragIter>();
        zero_count::<DenseHeavyCompute>();
        zero_count::<DenseAddRemove>();
        zero_count::<DenseEntityChurn>();
        zero_count::<DenseSprites>();
        zero_count::<DenseAgents>();
    }

    /// The retargeted handle is not hashed, so the reference comparison cannot see pass B write it
    /// back. A target is stale after a step only if its slot was refilled in that step: an older
    /// stale handle means the write-back is missing.
    #[test]
    fn random_access_dense_writes_back_retargets() {
        let mut w = DenseRandomAccess::setup(PARAMS);
        let mut stale = 0_usize;
        for tick in 1..=RANDOM_ACCESS_TICKS {
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
    fn random_access_dense_zero_count_is_valid() {
        zero_count::<DenseRandomAccess>();
    }

    #[test]
    fn many_queries_dense_matches_the_reference() {
        check_conformance::<VecManyQueries, DenseManyQueries>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn many_queries_dense_zero_count_is_valid() {
        zero_count::<DenseManyQueries>();
    }
}

mod lookup {
    use crate::{Entity, World};

    #[derive(Debug, PartialEq)]
    struct A(u32);
    #[derive(Debug, PartialEq)]
    struct B(u32);

    fn spawn(w: &mut World, a: Option<u32>, b: Option<u32>) -> Entity {
        let e = w.spawn();
        if let Some(a) = a {
            w.insert(e, A(a));
        }
        if let Some(b) = b {
            w.insert(e, B(b));
        }
        e
    }

    /// Spawns until a handle with `like`'s index and a higher generation comes back.
    fn reuse_index(w: &mut World, like: Entity) -> Entity {
        loop {
            let e = spawn(w, Some(9), None);
            if e.index() == like.index() {
                return e;
            }
        }
    }

    #[test]
    fn lookup_agrees_with_get() {
        let mut w = World::new();
        let with = spawn(&mut w, Some(1), Some(2));
        let without = spawn(&mut w, None, Some(3));
        let old = spawn(&mut w, Some(5), None);
        w.despawn(old);
        let new = reuse_index(&mut w, old);
        assert!(new.generation() > old.generation());
        // The free list is last in, first out and is empty here, so `dead` takes a new index
        // and, freed, leaves it on the list: nothing spawns before it is asserted on below.
        let dead = spawn(&mut w, Some(4), None);
        w.despawn(dead);
        assert_ne!(dead.index(), old.index());

        let lookup = w.lookup::<A>();
        let both = |e| (lookup.get(e), w.get::<A>(e));
        // Live with `A`.
        assert_eq!(both(with), (Some(&A(1)), Some(&A(1))));
        // Live without `A`.
        assert_eq!(both(without), (None, None));
        // Despawned.
        assert_eq!(both(dead), (None, None));
        // Stale, with its index reused by a live entity.
        assert_eq!(both(old), (None, None));
    }

    /// §4.6: a `Lookup` is `Debug` and `Clone` whatever its `T`, and its `Debug` is short.
    #[test]
    fn lookup_is_debug_and_clone_without_bounds_on_t() {
        struct Plain(u32);

        let mut w = World::new();
        let e = w.spawn();
        w.insert(e, Plain(1));
        let lookup = w.lookup::<Plain>();
        assert_eq!(format!("{lookup:?}"), "Lookup { .. }");
        assert_eq!(lookup.clone().get(e).map(|p| p.0), Some(1));
    }

    #[test]
    fn lookup_of_an_unregistered_type_is_none() {
        let mut w = World::new();
        let e = spawn(&mut w, Some(1), None);
        let lookup = w.lookup::<B>();
        assert_eq!(lookup.get(e), None);
        assert_eq!(lookup.get(e), w.get::<B>(e));
    }
}
