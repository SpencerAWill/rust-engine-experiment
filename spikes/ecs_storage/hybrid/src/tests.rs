//! The model test: a `World` against a plain map, under random operations.

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;

use crate::component::components;
use crate::{Commands, Entity, World};

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
    /// Record an insert, a remove or a despawn in the one buffer.
    DeferInsert(usize, usize, u32),
    DeferRemove(usize, usize),
    DeferDespawn(usize),
    /// Apply the buffer.
    Apply,
}

/// A recorded command in the model: the entity, and the type and value for an insert.
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
        6 => (any::<usize>(), 0usize..4, any::<u32>()).prop_map(|(k, t, v)| Op::Insert(k, t, v)),
        4 => (any::<usize>(), 0usize..4).prop_map(|(k, t)| Op::Remove(k, t)),
        3 => prop::collection::vec((0usize..4, any::<u32>()), 1..=3).prop_map(Op::SpawnWith),
        3 => (any::<usize>(), 0usize..4, any::<u32>()).prop_map(|(k, t, v)| Op::DeferInsert(k, t, v)),
        2 => (any::<usize>(), 0usize..4).prop_map(|(k, t)| Op::DeferRemove(k, t)),
        1 => any::<usize>().prop_map(Op::DeferDespawn),
        2 => Just(Op::Apply),
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

fn defer_insert(c: &mut Commands, e: Entity, t: usize, v: u32) {
    match t {
        0 => c.insert(e, T0(v)),
        1 => c.insert(e, T1(v)),
        2 => c.insert(e, S0(v)),
        _ => c.insert(e, S1(v)),
    }
}

fn defer_remove(c: &mut Commands, e: Entity, t: usize) {
    match t {
        0 => c.remove::<T0>(e),
        1 => c.remove::<T1>(e),
        2 => c.remove::<S0>(e),
        _ => c.remove::<S1>(e),
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

/// Applies recorded commands to the model in order; one on a dead entity is skipped (specs-3).
fn apply_to_model(
    model: &mut Model,
    live: &mut BTreeSet<u32>,
    recorded: impl Iterator<Item = Recorded>,
) {
    for command in recorded {
        match command {
            Recorded::Insert(e, t, v) => {
                if let Some((m, row)) = model.get_mut(&e.index())
                    && *m == e
                {
                    row[t] = Some(v);
                }
            }
            Recorded::Remove(e, t) => {
                if let Some((m, row)) = model.get_mut(&e.index())
                    && *m == e
                {
                    row[t] = None;
                }
            }
            Recorded::Despawn(e) => {
                if model.get(&e.index()).is_some_and(|(m, _)| *m == e) {
                    model.remove(&e.index());
                    live.remove(&e.index());
                }
            }
        }
    }
}

/// Records a deferred operation in the buffer and in the model's list; none if nothing spawned.
fn record(op: &Op, spawned: &[Entity], buffer: &mut Commands, recorded: &mut Vec<Recorded>) {
    if spawned.is_empty() {
        return;
    }
    let pick = |k: usize| spawned[k % spawned.len()];
    match *op {
        Op::DeferInsert(k, t, v) => {
            defer_insert(buffer, pick(k), t, v);
            recorded.push(Recorded::Insert(pick(k), t, v));
        }
        Op::DeferRemove(k, t) => {
            defer_remove(buffer, pick(k), t);
            recorded.push(Recorded::Remove(pick(k), t));
        }
        Op::DeferDespawn(k) => {
            buffer.despawn(pick(k));
            recorded.push(Recorded::Despawn(pick(k)));
        }
        _ => unreachable!("only deferred operations are recorded"),
    }
}

#[allow(clippy::many_single_char_names)] // model-test locals: w, e, t, v, k
fn run(ops: &[Op]) {
    let mut w = World::new();
    let mut spawned: Vec<Entity> = Vec::new();
    let mut model: Model = BTreeMap::new();
    let mut live: BTreeSet<u32> = BTreeSet::new();
    let mut buffer = Commands::new();
    let mut recorded: Vec<Recorded> = Vec::new();
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
            Op::DeferInsert(..) | Op::DeferRemove(..) | Op::DeferDespawn(_) => {
                record(op, &spawned, &mut buffer, &mut recorded);
            }
            Op::Apply => {
                assert_eq!(buffer.len(), recorded.len());
                w.apply(&mut buffer);
                assert!(buffer.is_empty());
                apply_to_model(&mut model, &mut live, recorded.drain(..));
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

mod workloads {
    use spike_ecs_harness::{Params, Workload, check_conformance};
    use spike_ecs_vec::add_remove::VecAddRemove;
    use spike_ecs_vec::agents::VecAgents;
    use spike_ecs_vec::entity_churn::VecEntityChurn;
    use spike_ecs_vec::frag_iter::VecFragIter;
    use spike_ecs_vec::heavy_compute::VecHeavyCompute;
    use spike_ecs_vec::random_access::VecRandomAccess;
    use spike_ecs_vec::simple_iter::VecSimpleIter;
    use spike_ecs_vec::sprites::VecSprites;

    use crate::add_remove::HybridAddRemove;
    use crate::agents::HybridAgents;
    use crate::archetype::ArchetypeId;
    use crate::entity_churn::HybridEntityChurn;
    use crate::frag_iter::HybridFragIter;
    use crate::heavy_compute::HybridHeavyCompute;
    use crate::query::seal::Role;
    use crate::random_access::HybridRandomAccess;
    use crate::simple_iter::HybridSimpleIter;
    use crate::sprites::HybridSprites;

    fn ids(v: &[usize]) -> Vec<ArchetypeId> {
        v.iter().map(|&i| ArchetypeId::new(i)).collect()
    }

    const PARAMS: Params = Params {
        count: 1_000,
        seed: 42,
    };
    const FRAG_PARAMS: Params = Params {
        count: 20,
        seed: 42,
    };
    const TICKS: u64 = 120;
    const AGENT_TICKS: u64 = 1_300;
    const RANDOM_ACCESS_TICKS: u64 = 700;

    #[test]
    fn simple_iter_hybrid_matches_the_reference() {
        check_conformance::<VecSimpleIter, HybridSimpleIter>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn frag_iter_hybrid_matches_the_reference() {
        check_conformance::<VecFragIter, HybridFragIter>(FRAG_PARAMS, TICKS).unwrap();
    }

    #[test]
    fn heavy_compute_hybrid_matches_the_reference() {
        check_conformance::<VecHeavyCompute, HybridHeavyCompute>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn simple_iter_hybrid_matches_its_archetypes() {
        let mut w = HybridSimpleIter::setup(PARAMS);
        assert_eq!(w.archetype_count(), 2);
        w.step();
        assert_eq!(w.step_archetypes(), ids(&[1]));
        assert_eq!(w.step_probed(), []);
        assert!(!w.step_row_probes());
    }

    #[test]
    fn frag_iter_hybrid_matches_its_archetypes() {
        let mut w = HybridFragIter::setup(FRAG_PARAMS);
        assert_eq!(w.archetype_count(), 27);
        w.step();
        assert_eq!(w.step_archetypes(), ids(&(1..=26).collect::<Vec<_>>()));
        assert_eq!(w.step_probed(), []);
        assert!(!w.step_row_probes());
    }

    #[test]
    fn heavy_compute_hybrid_matches_its_archetypes() {
        let mut w = HybridHeavyCompute::setup(PARAMS);
        assert_eq!(w.archetype_count(), 2);
        w.step();
        assert_eq!(w.step_archetypes(), ids(&[1]));
        assert_eq!(w.step_probed(), []);
        assert!(!w.step_row_probes());
    }

    #[test]
    fn add_remove_hybrid_matches_the_reference() {
        check_conformance::<VecAddRemove, HybridAddRemove>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn entity_churn_hybrid_matches_the_reference() {
        check_conformance::<VecEntityChurn, HybridEntityChurn>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn add_remove_hybrid_matches_its_archetypes() {
        let mut w = HybridAddRemove::setup(PARAMS);
        assert_eq!(w.archetype_count(), 2);
        w.step();
        // Archetypes: 0 empty, 1 (A). `B` is sparse, so no archetype names it.
        assert_eq!(w.archetype_count(), 2);
        assert_eq!(w.read_archetypes(), ids(&[1]));
        assert_eq!(w.read_probed(), []);
        assert!(!w.read_row_probes());
        // The removal query requires only a sparse type, so it walks every archetype.
        assert_eq!(w.remove_archetypes(), ids(&[0, 1]));
        let b = w
            .world()
            .component_id_of(std::any::TypeId::of::<spike_ecs_harness::add_remove::B>())
            .expect("B is registered after the first insert");
        assert_eq!(w.remove_probed(), [(b, Role::Required)]);
        assert!(w.remove_row_probes());
    }

    #[test]
    fn entity_churn_hybrid_matches_its_archetypes() {
        let mut w = HybridEntityChurn::setup(PARAMS);
        assert_eq!(w.archetype_count(), 2);
        w.step();
        assert_eq!(w.step_archetypes(), ids(&[1]));
        assert_eq!(w.step_probed(), []);
        assert!(!w.step_row_probes());
        for _ in 0..TICKS {
            w.step();
        }
        assert_eq!(w.archetype_count(), 2);
        assert_eq!(w.step_archetypes(), ids(&[1]));
    }

    #[test]
    fn add_remove_keeps_invariants_every_tick() {
        let mut w = HybridAddRemove::setup(PARAMS);
        w.world().check_invariants().unwrap();
        let after_setup = w.archetype_count();
        for _ in 0..TICKS {
            w.step();
            w.world().check_invariants().unwrap();
            let mut with_b = 0;
            w.world()
                .query::<crate::Entity>()
                .with::<spike_ecs_harness::add_remove::B>()
                .for_each(|_| with_b += 1);
            assert_eq!(with_b, 0, "every B is removed by the end of a tick");
            assert_eq!(w.world().len(), PARAMS.count);
            assert_eq!(w.archetype_count(), after_setup, "no archetype after setup");
        }
    }

    #[test]
    fn entity_churn_keeps_invariants_every_tick() {
        let mut w = HybridEntityChurn::setup(PARAMS);
        w.world().check_invariants().unwrap();
        for _ in 0..TICKS {
            w.step();
            w.world().check_invariants().unwrap();
            assert_eq!(w.world().len(), PARAMS.count);
        }
    }

    #[test]
    fn sprites_hybrid_matches_the_reference() {
        check_conformance::<VecSprites, HybridSprites>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn agents_hybrid_matches_the_reference() {
        check_conformance::<VecAgents, HybridAgents>(PARAMS, AGENT_TICKS).unwrap();
    }

    #[test]
    fn sprites_hybrid_matches_its_archetypes() {
        let mut w = HybridSprites::setup(PARAMS);
        assert_eq!(w.archetype_count(), 3);
        w.step();
        assert_eq!(w.move_archetypes(), ids(&[1, 2]));
        assert_eq!(w.move_probed(), []);
        assert!(!w.move_row_probes());
        // The draw query matches the archetype without `Hidden`, the one with fewer components.
        let visible = w
            .world()
            .parts()
            .0
            .iter()
            .position(|a| a.components.len() == 3)
            .unwrap();
        assert_eq!(w.draw_archetypes(), ids(&[visible]));
        assert_eq!(w.draw_probed(), []);
        assert!(!w.draw_row_probes());
    }

    #[test]
    fn agents_hybrid_matches_its_archetypes() {
        let mut w = HybridAgents::setup(PARAMS);
        assert_eq!(w.archetype_count(), 2);
        w.step();
        assert_eq!(w.step_archetypes(), ids(&[1]));
        let seeker = w
            .world()
            .component_id_of(std::any::TypeId::of::<spike_ecs_harness::agents::Seeker>())
            .expect("Seeker is registered by the first respawn with one");
        assert_eq!(w.step_probed(), [(seeker, Role::Optional)]);
        // An optional sparse type is looked up in `get`, not gated per row: no row probe.
        assert!(!w.step_row_probes());
    }

    #[test]
    fn agents_keeps_invariants_every_tick() {
        let mut w = HybridAgents::setup(PARAMS);
        w.world().check_invariants().unwrap();
        let after_setup = w.archetype_count();
        for _ in 0..AGENT_TICKS {
            w.step();
            w.world().check_invariants().unwrap();
            assert_eq!(w.world().len(), PARAMS.count);
            assert_eq!(
                w.archetype_count(),
                after_setup,
                "a Seeker toggle moves no row"
            );
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
        zero_count::<HybridSimpleIter>();
        zero_count::<HybridFragIter>();
        zero_count::<HybridHeavyCompute>();
        zero_count::<HybridAddRemove>();
        zero_count::<HybridEntityChurn>();
        zero_count::<HybridSprites>();
        zero_count::<HybridAgents>();
    }

    #[test]
    fn random_access_hybrid_matches_the_reference() {
        check_conformance::<VecRandomAccess, HybridRandomAccess>(PARAMS, RANDOM_ACCESS_TICKS)
            .unwrap();
    }

    /// The retargeted handle is not hashed, so the reference comparison cannot see pass B write it
    /// back. A target is stale after a step only if its slot was refilled in that step: an older
    /// stale handle means the write-back is missing.
    #[test]
    fn random_access_hybrid_writes_back_retargets() {
        let mut w = HybridRandomAccess::setup(PARAMS);
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
    fn random_access_hybrid_zero_count_is_valid() {
        zero_count::<HybridRandomAccess>();
    }

    /// ecs-11: the workload's six types are all table types, so the world built by setup and
    /// steps holds no sparse set at all, and none is registered for any of them.
    #[test]
    fn random_access_hybrid_registers_no_sparse_set() {
        use crate::component::StorageKind;
        use spike_ecs_harness::random_access::{
            Lifetime, Position, Slot, SpawnId, Target, Velocity,
        };

        let mut w = HybridRandomAccess::setup(PARAMS);
        for _ in 0..5 {
            w.step();
        }
        let world = w.world();
        assert_eq!(world.parts().1.len(), 0, "no sparse set exists");
        let types = [
            core::any::TypeId::of::<SpawnId>(),
            core::any::TypeId::of::<Slot>(),
            core::any::TypeId::of::<Position>(),
            core::any::TypeId::of::<Velocity>(),
            core::any::TypeId::of::<Lifetime>(),
            core::any::TypeId::of::<Target<crate::Entity>>(),
        ];
        for ty in types {
            let id = world.component_id_of(ty).expect("registered by setup");
            assert_eq!(world.component_info(id).kind, StorageKind::Table);
        }
        world.check_invariants().unwrap();
    }
}

mod lookup {
    use crate::component::components;
    use crate::{Entity, World};

    #[derive(Debug, PartialEq)]
    struct A(u32);
    #[derive(Debug, PartialEq)]
    struct B(u32);
    #[derive(Debug, PartialEq)]
    struct SA(u32);
    #[derive(Debug, PartialEq)]
    struct SB(u32);
    components!(TableStorage: A, B);
    components!(SparseStorage: SA, SB);

    /// Spawns until a handle with `like`'s index and a higher generation comes back.
    fn reuse_index(w: &mut World, like: Entity) -> Entity {
        loop {
            let e = w.spawn_with((A(9),));
            if e.index() == like.index() {
                return e;
            }
        }
    }

    #[test]
    fn lookup_agrees_with_get_for_a_table_type() {
        let mut w = World::new();
        let with = w.spawn_with((A(1), B(2)));
        let without = w.spawn_with((B(3),));
        let old = w.spawn_with((A(5),));
        w.despawn(old);
        let new = reuse_index(&mut w, old);
        assert!(new.generation() > old.generation());
        // The free list is last in, first out and is empty here, so `dead` takes a new index
        // and, freed, leaves it on the list: nothing spawns before it is asserted on below.
        let dead = w.spawn_with((A(4),));
        w.despawn(dead);
        assert_ne!(dead.index(), old.index());

        let lookup = w.lookup::<A>();
        let both = |e| (lookup.get(e), w.get::<A>(e));
        assert_eq!(both(with), (Some(&A(1)), Some(&A(1))));
        assert_eq!(both(without), (None, None));
        assert_eq!(both(dead), (None, None));
        assert_eq!(both(old), (None, None));
    }

    #[test]
    fn lookup_agrees_with_get_for_a_sparse_type() {
        let mut w = World::new();
        let with = w.spawn_with((A(1),));
        w.insert(with, SA(1));
        let without = w.spawn_with((A(3),));
        let old = w.spawn_with((A(5),));
        w.insert(old, SA(5));
        w.despawn(old);
        let new = reuse_index(&mut w, old);
        w.insert(new, SA(6));
        assert!(new.generation() > old.generation());
        // The free list is last in, first out and is empty here, so `dead` takes a new index
        // and, freed, leaves it on the list: nothing spawns before it is asserted on below.
        let dead = w.spawn_with((A(4),));
        w.insert(dead, SA(4));
        w.despawn(dead);
        assert_ne!(dead.index(), old.index());

        let lookup = w.lookup::<SA>();
        let both = |e| (lookup.get(e), w.get::<SA>(e));
        assert_eq!(both(with), (Some(&SA(1)), Some(&SA(1))));
        assert_eq!(both(without), (None, None));
        assert_eq!(both(dead), (None, None));
        assert_eq!(both(old), (None, None));
        assert_eq!(lookup.get(new), Some(&SA(6)));
    }

    /// §4.6: a `Lookup` is `Clone` whatever its `T`, for a table type and a sparse type.
    #[test]
    fn lookup_clones_and_reads_through_the_clone() {
        // Neither type is `Clone` or `Debug`.
        struct Plain(u32);
        struct Thin(u32);
        components!(TableStorage: Plain);
        components!(SparseStorage: Thin);

        let mut w = World::new();
        let e = w.spawn_with((Plain(1),));
        w.insert(e, Thin(2));

        let table = w.lookup::<Plain>();
        let table_clone = table.clone();
        assert_eq!(table_clone.get(e).map(|p| p.0), Some(1));
        assert_eq!(table.get(e).map(|p| p.0), Some(1));
        assert_eq!(format!("{table_clone:?}"), "Lookup { .. }");

        let sparse = w.lookup::<Thin>();
        let sparse_clone = sparse.clone();
        assert_eq!(sparse_clone.get(e).map(|t| t.0), Some(2));
        assert_eq!(sparse.get(e).map(|t| t.0), Some(2));
    }

    #[test]
    fn lookup_of_an_unregistered_type_is_none() {
        let mut w = World::new();
        let e = w.spawn_with((A(1),));
        let table = w.lookup::<B>();
        assert_eq!(table.get(e), None);
        assert_eq!(table.get(e), w.get::<B>(e));
        let sparse = w.lookup::<SB>();
        assert_eq!(sparse.get(e), None);
        assert_eq!(sparse.get(e), w.get::<SB>(e));
    }
}
