//! The model test: a `World` against a plain map, under random operations.

use std::collections::BTreeMap;

use proptest::prelude::*;

use crate::{Entity, World};

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
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        2 => Just(Op::Spawn),
        1 => any::<usize>().prop_map(Op::Despawn),
        5 => (any::<usize>(), 0usize..4, any::<u32>()).prop_map(|(k, t, v)| Op::Insert(k, t, v)),
        3 => (any::<usize>(), 0usize..4).prop_map(|(k, t)| Op::Remove(k, t)),
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
            _ => {}
        }
        assert_eq!(world.check_invariants(), Ok(()));
        assert_eq!(world.len(), model.len());
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
