//! The model test: a `World` against a plain map, under random operations.

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;

use crate::component::components;
use crate::{Entity, World};

#[derive(Debug, Clone, PartialEq)]
struct T0(u32);
#[derive(Debug, Clone, PartialEq)]
struct T1(u32);
#[derive(Debug, Clone, PartialEq)]
struct S0(u32);
#[derive(Debug, Clone, PartialEq)]
struct S1(u32);
components!(TableStorage: T0, T1);
components!(SparseStorage: S0, S1);

/// Per entity index: the entity and its value of each of the four types.
type Model = BTreeMap<u32, (Entity, [Option<u32>; 4])>;

#[derive(Debug, Clone)]
enum Op {
    Spawn,
    Despawn(usize),
    Insert(usize, usize, u32),
    Remove(usize, usize),
    /// One to three distinct types, in the order given, with their values.
    SpawnWith(Vec<(usize, u32)>),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        2 => Just(Op::Spawn),
        1 => any::<usize>().prop_map(Op::Despawn),
        6 => (any::<usize>(), 0usize..4, any::<u32>()).prop_map(|(k, t, v)| Op::Insert(k, t, v)),
        4 => (any::<usize>(), 0usize..4).prop_map(|(k, t)| Op::Remove(k, t)),
        3 => prop::collection::vec((0usize..4, any::<u32>()), 1..=3).prop_map(Op::SpawnWith),
    ]
}

/// Binds the component of type index `$t` with value `$v` to `$x` and evaluates `$body`.
macro_rules! with_value {
    ($t:expr, $v:expr, |$x:ident| $body:expr) => {
        match $t {
            0 => {
                let $x = T0($v);
                $body
            }
            1 => {
                let $x = T1($v);
                $body
            }
            2 => {
                let $x = S0($v);
                $body
            }
            _ => {
                let $x = S1($v);
                $body
            }
        }
    };
}

/// `spawn_with` of the given distinct types in the given order (one to three of them).
fn spawn_with(w: &mut World, items: &[(usize, u32)]) -> Entity {
    match *items {
        [(ta, va)] => with_value!(ta, va, |a| w.spawn_with((a,))),
        [(ta, va), (tb, vb)] => {
            with_value!(ta, va, |a| with_value!(tb, vb, |b| w.spawn_with((a, b))))
        }
        [(ta, va), (tb, vb), (tc, vc)] => with_value!(ta, va, |a| with_value!(
            tb,
            vb,
            |b| with_value!(tc, vc, |c| w.spawn_with((a, b, c)))
        )),
        _ => unreachable!("the generator makes one to three items"),
    }
}

fn insert(w: &mut World, e: Entity, t: usize, v: u32) -> Option<u32> {
    match t {
        0 => w.insert(e, T0(v)).map(|c| c.0),
        1 => w.insert(e, T1(v)).map(|c| c.0),
        2 => w.insert(e, S0(v)).map(|c| c.0),
        _ => w.insert(e, S1(v)).map(|c| c.0),
    }
}

fn remove(w: &mut World, e: Entity, t: usize) -> Option<u32> {
    match t {
        0 => w.remove::<T0>(e).map(|c| c.0),
        1 => w.remove::<T1>(e).map(|c| c.0),
        2 => w.remove::<S0>(e).map(|c| c.0),
        _ => w.remove::<S1>(e).map(|c| c.0),
    }
}

fn get(w: &World, e: Entity, t: usize) -> Option<u32> {
    match t {
        0 => w.get::<T0>(e).map(|c| c.0),
        1 => w.get::<T1>(e).map(|c| c.0),
        2 => w.get::<S0>(e).map(|c| c.0),
        _ => w.get::<S1>(e).map(|c| c.0),
    }
}

#[allow(clippy::many_single_char_names)] // model-test locals: w, e, t, v, k
fn run(ops: &[Op]) {
    let mut w = World::new();
    let mut spawned: Vec<Entity> = Vec::new();
    let mut model: Model = BTreeMap::new();
    let mut live: BTreeSet<u32> = BTreeSet::new();
    for op in ops {
        match *op {
            Op::Spawn => {
                let e = w.spawn();
                spawned.push(e);
                model.insert(e.index(), (e, [None; 4]));
                live.insert(e.index());
            }
            Op::SpawnWith(ref items) => {
                // A repeated type panics by design, so keep the first of each.
                let mut distinct: Vec<(usize, u32)> = Vec::new();
                for &(t, v) in items {
                    if distinct.iter().all(|&(u, _)| u != t) {
                        distinct.push((t, v));
                    }
                }
                let e = spawn_with(&mut w, &distinct);
                let mut row = [None; 4];
                for &(t, v) in &distinct {
                    row[t] = Some(v);
                }
                spawned.push(e);
                model.insert(e.index(), (e, row));
                live.insert(e.index());
            }
            Op::Despawn(k) if !spawned.is_empty() => {
                let e = spawned[k % spawned.len()];
                let was_live = model.get(&e.index()).is_some_and(|(m, _)| *m == e)
                    && live.contains(&e.index());
                assert_eq!(w.despawn(e), was_live);
                if was_live {
                    live.remove(&e.index());
                    model.remove(&e.index());
                }
            }
            Op::Insert(k, t, v) if !spawned.is_empty() => {
                // Inserting on a dead entity panics by design, so the generator skips it.
                let e = spawned[k % spawned.len()];
                if let Some((m, row)) = model.get_mut(&e.index())
                    && *m == e
                {
                    let old = row[t].replace(v);
                    assert_eq!(insert(&mut w, e, t, v), old);
                }
            }
            Op::Remove(k, t) if !spawned.is_empty() => {
                let e = spawned[k % spawned.len()];
                let old = match model.get_mut(&e.index()) {
                    Some((m, row)) if *m == e => row[t].take(),
                    _ => None,
                };
                assert_eq!(remove(&mut w, e, t), old);
            }
            _ => {}
        }
        // One mutable query: add 1 to every `T0` and every `S0` of an entity that has a `T0`.
        w.query_mut::<(&mut T0, Option<&mut S0>)>()
            .for_each(|(t, s)| {
                t.0 = t.0.wrapping_add(1);
                if let Some(s) = s {
                    s.0 = s.0.wrapping_add(1);
                }
            });
        for (_, row) in model.values_mut() {
            if let Some(t) = row[0].as_mut() {
                *t = t.wrapping_add(1);
                if let Some(s) = row[2].as_mut() {
                    *s = s.wrapping_add(1);
                }
            }
        }
        assert_eq!(w.check_invariants(), Ok(()));
        assert_eq!(w.len(), model.len());
        for e in &spawned {
            let row = match model.get(&e.index()) {
                Some((m, row)) if m == e => *row,
                _ => [None; 4],
            };
            for (t, expected) in row.iter().enumerate() {
                assert_eq!(get(&w, *e, t), *expected, "{e:?} type {t}");
            }
        }
        check_queries(&w, &model);
    }
}

/// Per-index rows of the live entities, for the query checks: `(entity index, values)`.
fn rows(model: &Model) -> Vec<(u32, [Option<u32>; 4])> {
    model.iter().map(|(i, (_, row))| (*i, *row)).collect()
}

/// The entity sets and values of six queries, through `iter` and `for_each`, against the model.
fn check_queries(w: &World, model: &Model) {
    let rows = rows(model);
    let pick = |f: &dyn Fn(&[Option<u32>; 4]) -> bool| -> Vec<u32> {
        rows.iter().filter(|(_, r)| f(r)).map(|(i, _)| *i).collect()
    };
    let sorted = |mut v: Vec<u32>| {
        v.sort_unstable();
        v
    };

    // (Entity, &T0, &S0)
    let want: Vec<(u32, u32, u32)> = rows
        .iter()
        .filter_map(|(i, r)| Some((*i, r[0]?, r[2]?)))
        .collect();
    let q = || w.query::<(Entity, &T0, &S0)>();
    let mut got: Vec<_> = q().iter().map(|(e, a, b)| (e.index(), a.0, b.0)).collect();
    got.sort_unstable();
    assert_eq!(got, want);
    let mut got = Vec::new();
    q().for_each(|(e, a, b)| got.push((e.index(), a.0, b.0)));
    got.sort_unstable();
    assert_eq!(got, want);

    // (Entity, &T0) without S1
    let want = pick(&|r| r[0].is_some() && r[3].is_none());
    let q = || w.query::<(Entity, &T0)>().without::<S1>();
    let got = sorted(q().iter().map(|(e, _)| e.index()).collect());
    assert_eq!(got, want);
    let mut got = Vec::new();
    q().for_each(|(e, _)| got.push(e.index()));
    assert_eq!(sorted(got), want);

    // (Entity, Option<&S1>) with T1
    let want: Vec<(u32, Option<u32>)> = rows
        .iter()
        .filter(|(_, r)| r[1].is_some())
        .map(|(i, r)| (*i, r[3]))
        .collect();
    let q = || w.query::<(Entity, Option<&S1>)>().with::<T1>();
    let mut got: Vec<_> = q()
        .iter()
        .map(|(e, s)| (e.index(), s.map(|s| s.0)))
        .collect();
    got.sort_unstable();
    assert_eq!(got, want);
    let mut got = Vec::new();
    q().for_each(|(e, s)| got.push((e.index(), s.map(|s| s.0))));
    got.sort_unstable();
    assert_eq!(got, want);

    // Entity with S0, Entity, Entity without T0
    let cases: [(Vec<u32>, Vec<u32>, Vec<u32>); 3] = [
        (
            sorted(
                w.query::<Entity>()
                    .with::<S0>()
                    .iter()
                    .map(Entity::index)
                    .collect(),
            ),
            {
                let mut v = Vec::new();
                w.query::<Entity>()
                    .with::<S0>()
                    .for_each(|e| v.push(e.index()));
                sorted(v)
            },
            pick(&|r| r[2].is_some()),
        ),
        (
            sorted(w.query::<Entity>().iter().map(Entity::index).collect()),
            {
                let mut v = Vec::new();
                w.query::<Entity>().for_each(|e| v.push(e.index()));
                sorted(v)
            },
            pick(&|_| true),
        ),
        (
            sorted(
                w.query::<Entity>()
                    .without::<T0>()
                    .iter()
                    .map(Entity::index)
                    .collect(),
            ),
            {
                let mut v = Vec::new();
                w.query::<Entity>()
                    .without::<T0>()
                    .for_each(|e| v.push(e.index()));
                sorted(v)
            },
            pick(&|r| r[0].is_none()),
        ),
    ];
    for (by_iter, by_each, want) in cases {
        assert_eq!(by_iter, want);
        assert_eq!(by_each, want);
    }
}

proptest! {
    #[test]
    fn world_matches_the_model(ops in prop::collection::vec(op(), 0..64)) {
        run(&ops);
    }
}
