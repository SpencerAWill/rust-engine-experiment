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
