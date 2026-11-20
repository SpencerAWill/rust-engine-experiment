//! The model test: a `World` against a plain map, under random operations.

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;

use crate::{Entity, World, WorldBuilder};

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

fn insert(w: &mut World, e: Entity, t: usize, v: u32) -> Option<u32> {
    match t {
        0 => w.insert(e, C0(v)).map(|c| c.0),
        1 => w.insert(e, C1(v)).map(|c| c.0),
        2 => w.insert(e, C2(v)).map(|c| c.0),
        _ => w.insert(e, C3(v)).map(|c| c.0),
    }
}

fn remove(w: &mut World, e: Entity, t: usize) -> Option<u32> {
    match t {
        0 => w.remove::<C0>(e).map(|c| c.0),
        1 => w.remove::<C1>(e).map(|c| c.0),
        2 => w.remove::<C2>(e).map(|c| c.0),
        _ => w.remove::<C3>(e).map(|c| c.0),
    }
}

fn get(w: &World, e: Entity, t: usize) -> Option<u32> {
    match t {
        0 => w.get::<C0>(e).map(|c| c.0),
        1 => w.get::<C1>(e).map(|c| c.0),
        2 => w.get::<C2>(e).map(|c| c.0),
        _ => w.get::<C3>(e).map(|c| c.0),
    }
}

fn build(setup: usize) -> World {
    let b = WorldBuilder::new();
    match setup {
        0 => b.build(),
        1 => b.group::<(C0, C1)>().build(),
        _ => b.group::<(C0, C1, C2)>().build(),
    }
}

#[allow(clippy::many_single_char_names)] // model-test locals: w, e, t, v, k
fn run(setup: usize, ops: &[Op]) {
    let mut w = build(setup);
    let mut spawned: Vec<Entity> = Vec::new();
    let mut model: BTreeMap<u64, [Option<u32>; 4]> = BTreeMap::new();
    let mut live: BTreeSet<u64> = BTreeSet::new();
    for op in ops {
        match *op {
            Op::Spawn => {
                let e = w.spawn();
                spawned.push(e);
                model.insert(e.to_bits(), [None; 4]);
                live.insert(e.to_bits());
            }
            Op::Despawn(k) if !spawned.is_empty() => {
                let e = spawned[k % spawned.len()];
                let was_live = live.remove(&e.to_bits());
                assert_eq!(w.despawn(e), was_live);
                if was_live {
                    model.insert(e.to_bits(), [None; 4]);
                }
            }
            Op::Insert(k, t, v) if !spawned.is_empty() => {
                let e = spawned[k % spawned.len()];
                if live.contains(&e.to_bits()) {
                    let old = model.get_mut(&e.to_bits()).unwrap()[t].replace(v);
                    assert_eq!(insert(&mut w, e, t, v), old);
                }
            }
            Op::Remove(k, t) if !spawned.is_empty() => {
                let e = spawned[k % spawned.len()];
                let old = if live.contains(&e.to_bits()) {
                    model.get_mut(&e.to_bits()).unwrap()[t].take()
                } else {
                    None
                };
                assert_eq!(remove(&mut w, e, t), old);
            }
            _ => {}
        }
        assert_eq!(w.check_invariants(), Ok(()));
        assert_eq!(w.len(), live.len());
        for e in &spawned {
            for (t, expected) in model[&e.to_bits()].iter().enumerate() {
                assert_eq!(get(&w, *e, t), *expected, "{e:?} type {t}");
            }
        }
    }
}

proptest! {
    #[test]
    fn world_matches_the_model(ops in prop::collection::vec(op(), 0..64)) {
        for setup in 0..3 {
            run(setup, &ops);
        }
    }
}
