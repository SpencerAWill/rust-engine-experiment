//! The model test: a `World` against a plain map, under random operations.

use std::collections::{BTreeMap, BTreeSet};

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

type Model = BTreeMap<u64, [Option<u32>; 4]>;

#[derive(Debug, Clone)]
enum Op {
    Spawn,
    /// A bundle shape (0 to 3, see `spawn_with`) and its values, one per element in tuple order.
    SpawnWith(usize, [u32; 3]),
    Despawn(usize),
    Insert(usize, usize, u32),
    Remove(usize, usize),
    /// Records a deferred insert in the one `Commands` buffer.
    DeferInsert(usize, usize, u32),
    /// Records a deferred remove.
    DeferRemove(usize, usize),
    /// Records a deferred despawn.
    DeferDespawn(usize),
    /// Applies the buffer.
    Apply,
}

/// A recorded command, in the model: the entity's bits, the type and the value.
#[derive(Debug, Clone, Copy)]
enum Cmd {
    Insert(u64, usize, u32),
    Remove(u64, usize),
    Despawn(u64),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        2 => Just(Op::Spawn),
        3 => (0usize..4, any::<[u32; 3]>()).prop_map(|(s, v)| Op::SpawnWith(s, v)),
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

/// Spawns the bundle of `shape` and returns the entity with the component types it has, each
/// with its value. Shapes 2 and 3 list their types out of order on purpose.
fn spawn_with(w: &mut World, shape: usize, v: [u32; 3]) -> (Entity, Vec<(usize, u32)>) {
    match shape {
        0 => (w.spawn_with((C0(v[0]),)), vec![(0, v[0])]),
        1 => (
            w.spawn_with((C0(v[0]), C1(v[1]))),
            vec![(0, v[0]), (1, v[1])],
        ),
        2 => (
            w.spawn_with((C2(v[0]), C0(v[1]))),
            vec![(2, v[0]), (0, v[1])],
        ),
        _ => (
            w.spawn_with((C3(v[0]), C1(v[1]), C0(v[2]))),
            vec![(3, v[0]), (1, v[1]), (0, v[2])],
        ),
    }
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

fn defer_insert(c: &mut Commands, e: Entity, t: usize, v: u32) {
    match t {
        0 => c.insert(e, C0(v)),
        1 => c.insert(e, C1(v)),
        2 => c.insert(e, C2(v)),
        _ => c.insert(e, C3(v)),
    }
}

fn defer_remove(c: &mut Commands, e: Entity, t: usize) {
    match t {
        0 => c.remove::<C0>(e),
        1 => c.remove::<C1>(e),
        2 => c.remove::<C2>(e),
        _ => c.remove::<C3>(e),
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

/// The sorted `to_bits` of the live entities whose model row satisfies `keep`.
fn expected(
    model: &Model,
    live: &BTreeSet<u64>,
    keep: impl Fn(&[Option<u32>; 4]) -> bool,
) -> Vec<u64> {
    live.iter()
        .copied()
        .filter(|bits| keep(&model[bits]))
        .collect()
}

fn bits(it: impl Iterator<Item = Entity>) -> Vec<u64> {
    let mut v: Vec<u64> = it.map(Entity::to_bits).collect();
    v.sort_unstable();
    v
}

/// Compares the entity sets of five queries with the model's.
fn check_queries(w: &World, model: &Model, live: &BTreeSet<u64>) {
    assert_eq!(
        bits(w.query::<(Entity, &C0, &C1)>().iter().map(|(e, _, _)| e)),
        expected(model, live, |r| r[0].is_some() && r[1].is_some()),
        "(Entity, &C0, &C1)"
    );
    assert_eq!(
        bits(
            w.query::<(Entity, &C0)>()
                .without::<C2>()
                .iter()
                .map(|(e, _)| e)
        ),
        expected(model, live, |r| r[0].is_some() && r[2].is_none()),
        "(Entity, &C0) without C2"
    );
    assert_eq!(
        bits(
            w.query::<(Entity, Option<&C3>)>()
                .with::<C1>()
                .iter()
                .map(|(e, _)| e)
        ),
        expected(model, live, |r| r[1].is_some()),
        "(Entity, Option<&C3>) with C1"
    );
    assert_eq!(
        bits(w.query::<Entity>().iter()),
        expected(model, live, |_| true),
        "Entity"
    );
    assert_eq!(
        bits(w.query::<Entity>().without::<C0>().iter()),
        expected(model, live, |r| r[0].is_none()),
        "Entity without C0"
    );
}

#[allow(clippy::many_single_char_names)] // model-test locals: w, e, t, v, k
fn run(ops: &[Op]) {
    let mut w = World::new();
    let mut spawned: Vec<Entity> = Vec::new();
    let mut model: Model = BTreeMap::new();
    let mut live: BTreeSet<u64> = BTreeSet::new();
    let mut commands = Commands::new();
    let mut recorded: Vec<Cmd> = Vec::new();
    for op in ops {
        match *op {
            Op::Spawn => {
                let e = w.spawn();
                spawned.push(e);
                model.insert(e.to_bits(), [None; 4]);
                live.insert(e.to_bits());
            }
            Op::SpawnWith(shape, v) => {
                let (e, values) = spawn_with(&mut w, shape, v);
                let mut row = [None; 4];
                for (t, value) in values {
                    row[t] = Some(value);
                }
                spawned.push(e);
                model.insert(e.to_bits(), row);
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
                // Inserting on a dead entity panics by design, so the generator skips it.
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
            Op::DeferInsert(k, t, v) if !spawned.is_empty() => {
                let e = spawned[k % spawned.len()];
                defer_insert(&mut commands, e, t, v);
                recorded.push(Cmd::Insert(e.to_bits(), t, v));
            }
            Op::DeferRemove(k, t) if !spawned.is_empty() => {
                let e = spawned[k % spawned.len()];
                defer_remove(&mut commands, e, t);
                recorded.push(Cmd::Remove(e.to_bits(), t));
            }
            Op::DeferDespawn(k) if !spawned.is_empty() => {
                let e = spawned[k % spawned.len()];
                commands.despawn(e);
                recorded.push(Cmd::Despawn(e.to_bits()));
            }
            Op::Apply => {
                assert_eq!(commands.len(), recorded.len());
                w.apply(&mut commands);
                // In order; a command whose entity is dead when it runs is skipped.
                for cmd in recorded.drain(..) {
                    match cmd {
                        Cmd::Insert(bits, t, v) if live.contains(&bits) => {
                            model.get_mut(&bits).unwrap()[t] = Some(v);
                        }
                        Cmd::Remove(bits, t) if live.contains(&bits) => {
                            model.get_mut(&bits).unwrap()[t] = None;
                        }
                        Cmd::Despawn(bits) if live.remove(&bits) => {
                            model.insert(bits, [None; 4]);
                        }
                        _ => {}
                    }
                }
                assert!(commands.is_empty());
            }
            _ => {}
        }
        assert_eq!(w.check_invariants(), Ok(()));
        assert_eq!(w.len(), live.len());
        check_queries(&w, &model, &live);
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
        run(&ops);
    }
}

/// A model over both kinds of components: typed `C0` to `C3` and raw components of three
/// layouts, in one world.
mod raw_model {
    use std::collections::BTreeMap;

    use proptest::prelude::*;

    use super::{get, insert, remove, spawn_with};
    use crate::{ComponentId, Entity, RawError, RawLayout, World};

    /// The size and alignment of raw component `r`.
    const LAYOUTS: [(usize, usize); 3] = [(4, 4), (12, 4), (16, 16)];

    /// The bytes of raw component `r` for value `v`.
    fn bytes(r: usize, v: u32) -> Vec<u8> {
        (0..LAYOUTS[r].0)
            .map(|i| v.to_le_bytes()[i % 4] ^ u8::try_from(i).unwrap())
            .collect()
    }

    #[derive(Debug, Clone, Default)]
    struct Row {
        typed: [Option<u32>; 4],
        raw: [Option<Vec<u8>>; 3],
    }

    #[derive(Debug, Clone)]
    enum Op {
        Spawn,
        SpawnWith(usize, [u32; 3]),
        /// The raw components in the bit mask, listed in reverse order if the flag is set.
        SpawnRaw(u8, bool, [u32; 3]),
        Despawn(usize),
        Insert(usize, usize, u32),
        Remove(usize, usize),
        InsertRaw(usize, usize, u32),
        RemoveRaw(usize, usize),
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            2 => Just(Op::Spawn),
            2 => (0usize..4, any::<[u32; 3]>()).prop_map(|(s, v)| Op::SpawnWith(s, v)),
            3 => (0u8..8, any::<bool>(), any::<[u32; 3]>())
                .prop_map(|(m, r, v)| Op::SpawnRaw(m, r, v)),
            1 => any::<usize>().prop_map(Op::Despawn),
            3 => (any::<usize>(), 0usize..4, any::<u32>()).prop_map(|(k, t, v)| Op::Insert(k, t, v)),
            2 => (any::<usize>(), 0usize..4).prop_map(|(k, t)| Op::Remove(k, t)),
            5 => (any::<usize>(), 0usize..3, any::<u32>())
                .prop_map(|(k, r, v)| Op::InsertRaw(k, r, v)),
            3 => (any::<usize>(), 0usize..3).prop_map(|(k, r)| Op::RemoveRaw(k, r)),
        ]
    }

    /// The id of raw component `r`, registered on first use so raw and typed ids interleave.
    fn raw_id(w: &mut World, ids: &mut [Option<ComponentId>; 3], r: usize) -> ComponentId {
        *ids[r].get_or_insert_with(|| {
            let (size, align) = LAYOUTS[r];
            w.register_raw(format!("raw{r}"), RawLayout::new(size, align).unwrap())
        })
    }

    /// Spawns the raw components in `mask` (listed in reverse order if `reverse`) and returns the
    /// entity with the row the model expects.
    fn spawn_raw(
        world: &mut World,
        ids: &mut [Option<ComponentId>; 3],
        mask: u8,
        reverse: bool,
        values: [u32; 3],
    ) -> (Entity, Row) {
        let mut chosen: Vec<usize> = (0..3).filter(|r| mask & (1 << r) != 0).collect();
        if reverse {
            chosen.reverse();
        }
        let entries: Vec<(ComponentId, Vec<u8>)> = chosen
            .iter()
            .map(|&r| (raw_id(world, ids, r), bytes(r, values[r])))
            .collect();
        let slices: Vec<(ComponentId, &[u8])> =
            entries.iter().map(|(id, b)| (*id, b.as_slice())).collect();
        let entity = world.spawn_raw(&slices).unwrap();
        let mut row = Row::default();
        for &r in &chosen {
            row.raw[r] = Some(bytes(r, values[r]));
        }
        (entity, row)
    }

    fn run(ops: &[Op]) {
        let mut world = World::new();
        let mut ids: [Option<ComponentId>; 3] = [None; 3];
        let mut spawned: Vec<Entity> = Vec::new();
        let mut model: BTreeMap<u64, Row> = BTreeMap::new();
        let mut live: Vec<u64> = Vec::new();
        for op in ops {
            match *op {
                Op::Spawn => {
                    let e = world.spawn();
                    spawned.push(e);
                    model.insert(e.to_bits(), Row::default());
                    live.push(e.to_bits());
                }
                Op::SpawnWith(shape, v) => {
                    let (e, values) = spawn_with(&mut world, shape, v);
                    let mut row = Row::default();
                    for (t, value) in values {
                        row.typed[t] = Some(value);
                    }
                    spawned.push(e);
                    model.insert(e.to_bits(), row);
                    live.push(e.to_bits());
                }
                Op::SpawnRaw(mask, reverse, v) => {
                    let (e, row) = spawn_raw(&mut world, &mut ids, mask, reverse, v);
                    spawned.push(e);
                    model.insert(e.to_bits(), row);
                    live.push(e.to_bits());
                }
                Op::Despawn(k) if !spawned.is_empty() => {
                    let e = spawned[k % spawned.len()];
                    let was_live = live.contains(&e.to_bits());
                    assert_eq!(world.despawn(e), was_live);
                    if was_live {
                        live.retain(|b| *b != e.to_bits());
                        model.insert(e.to_bits(), Row::default());
                    }
                }
                Op::Insert(k, t, v) if !spawned.is_empty() => {
                    // Inserting on a dead entity panics by design, so the generator skips it.
                    let e = spawned[k % spawned.len()];
                    if live.contains(&e.to_bits()) {
                        let old = model.get_mut(&e.to_bits()).unwrap().typed[t].replace(v);
                        assert_eq!(insert(&mut world, e, t, v), old);
                    }
                }
                Op::Remove(k, t) if !spawned.is_empty() => {
                    let e = spawned[k % spawned.len()];
                    let old = model.get_mut(&e.to_bits()).unwrap().typed[t].take();
                    assert_eq!(remove(&mut world, e, t), old);
                }
                Op::InsertRaw(k, r, v) if !spawned.is_empty() => {
                    let e = spawned[k % spawned.len()];
                    let id = raw_id(&mut world, &mut ids, r);
                    let result = world.insert_raw(e, id, &bytes(r, v));
                    if live.contains(&e.to_bits()) {
                        let old = model.get_mut(&e.to_bits()).unwrap().raw[r].replace(bytes(r, v));
                        assert_eq!(result, Ok(old.is_some()));
                    } else {
                        assert_eq!(result, Err(RawError::DeadEntity(e)));
                    }
                }
                Op::RemoveRaw(k, r) if !spawned.is_empty() => {
                    let e = spawned[k % spawned.len()];
                    let id = raw_id(&mut world, &mut ids, r);
                    let result = world.remove_raw(e, id);
                    if live.contains(&e.to_bits()) {
                        let old = model.get_mut(&e.to_bits()).unwrap().raw[r].take();
                        assert_eq!(result, Ok(old.is_some()));
                    } else {
                        assert_eq!(result, Err(RawError::DeadEntity(e)));
                    }
                }
                _ => {}
            }
            assert_eq!(world.check_invariants(), Ok(()));
            assert_eq!(world.len(), live.len());
            for e in &spawned {
                let row = &model[&e.to_bits()];
                for (t, expected) in row.typed.iter().enumerate() {
                    assert_eq!(get(&world, *e, t), *expected, "{e:?} type {t}");
                }
                for (r, expected) in row.raw.iter().enumerate() {
                    let got = ids[r].and_then(|id| world.get_raw(*e, id));
                    assert_eq!(got, expected.as_deref(), "{e:?} raw {r}");
                }
            }
        }
    }

    proptest! {
        #[test]
        fn raw_and_typed_operations_match_the_model(ops in prop::collection::vec(op(), 0..64)) {
            run(&ops);
        }
    }
}

mod workloads {
    use spike_ecs_harness::{Params, Workload, check_conformance};
    use spike_ecs_vec::add_remove::VecAddRemove;
    use spike_ecs_vec::agents::VecAgents;
    use spike_ecs_vec::entity_churn::VecEntityChurn;
    use spike_ecs_vec::frag_iter::VecFragIter;
    use spike_ecs_vec::heavy_compute::VecHeavyCompute;
    use spike_ecs_vec::simple_iter::VecSimpleIter;
    use spike_ecs_vec::sprites::VecSprites;

    use crate::add_remove::ArchetypeAddRemove;
    use crate::agents::ArchetypeAgents;
    use crate::archetype::ArchetypeId;
    use crate::entity_churn::ArchetypeEntityChurn;
    use crate::frag_iter::ArchetypeFragIter;
    use crate::heavy_compute::ArchetypeHeavyCompute;
    use crate::simple_iter::ArchetypeSimpleIter;
    use crate::sprites::ArchetypeSprites;

    fn ids(v: &[usize]) -> Vec<ArchetypeId> {
        v.iter().map(|&i| ArchetypeId::new(i)).collect()
    }

    const PARAMS: Params = Params {
        count: 1_000,
        seed: 42,
    };
    const TICKS: u64 = 120;
    const AGENT_TICKS: u64 = 1_300;

    #[test]
    fn simple_iter_archetype_matches_the_reference() {
        check_conformance::<VecSimpleIter, ArchetypeSimpleIter>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn frag_iter_archetype_matches_the_reference() {
        check_conformance::<VecFragIter, ArchetypeFragIter>(
            Params {
                count: 20,
                seed: 42,
            },
            TICKS,
        )
        .unwrap();
    }

    #[test]
    fn heavy_compute_archetype_matches_the_reference() {
        check_conformance::<VecHeavyCompute, ArchetypeHeavyCompute>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn simple_iter_archetype_layout() {
        let mut w = ArchetypeSimpleIter::setup(PARAMS);
        assert_eq!(w.archetype_count(), 2);
        assert_eq!(w.step_archetypes(), ids(&[1]));
    }

    #[test]
    fn frag_iter_archetype_layout() {
        let mut w = ArchetypeFragIter::setup(Params {
            count: 20,
            seed: 42,
        });
        assert_eq!(w.archetype_count(), 27);
        assert_eq!(w.step_archetypes(), ids(&(1..=26).collect::<Vec<_>>()));
    }

    #[test]
    fn heavy_compute_archetype_layout() {
        let mut w = ArchetypeHeavyCompute::setup(PARAMS);
        assert_eq!(w.archetype_count(), 2);
        assert_eq!(w.step_archetypes(), ids(&[1]));
    }

    #[test]
    fn add_remove_archetype_matches_the_reference() {
        check_conformance::<VecAddRemove, ArchetypeAddRemove>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn entity_churn_archetype_matches_the_reference() {
        check_conformance::<VecEntityChurn, ArchetypeEntityChurn>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn add_remove_archetype_layout() {
        let mut w = ArchetypeAddRemove::setup(PARAMS);
        assert_eq!(w.archetype_count(), 2);
        w.step();
        assert_eq!(w.archetype_count(), 3);
        // Archetypes: 0 empty, 1 (A), 2 (A, B).
        assert_eq!(w.read_archetypes(), ids(&[1, 2]));
        assert_eq!(w.remove_archetypes(), ids(&[2]));
    }

    #[test]
    fn entity_churn_archetype_layout() {
        let mut w = ArchetypeEntityChurn::setup(PARAMS);
        assert_eq!(w.archetype_count(), 2);
        assert_eq!(w.step_archetypes(), ids(&[1]));
        for _ in 0..TICKS {
            w.step();
        }
        assert_eq!(w.archetype_count(), 2);
        assert_eq!(w.step_archetypes(), ids(&[1]));
    }

    #[test]
    fn add_remove_keeps_invariants_every_tick() {
        let mut w = ArchetypeAddRemove::setup(PARAMS);
        w.world().check_invariants().unwrap();
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
        }
    }

    #[test]
    fn entity_churn_keeps_invariants_every_tick() {
        let mut w = ArchetypeEntityChurn::setup(PARAMS);
        w.world().check_invariants().unwrap();
        for _ in 0..TICKS {
            w.step();
            w.world().check_invariants().unwrap();
            assert_eq!(w.world().len(), PARAMS.count);
        }
    }

    #[test]
    fn sprites_archetype_matches_the_reference() {
        check_conformance::<VecSprites, ArchetypeSprites>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn agents_archetype_matches_the_reference() {
        check_conformance::<VecAgents, ArchetypeAgents>(PARAMS, AGENT_TICKS).unwrap();
    }

    #[test]
    fn sprites_archetype_layout() {
        let mut w = ArchetypeSprites::setup(PARAMS);
        assert_eq!(w.archetype_count(), 3);
        assert_eq!(w.move_archetypes(), ids(&[1, 2]));
        // The draw query matches the archetype without `Hidden`, the one with fewer components.
        let visible = w
            .world()
            .archetypes()
            .iter()
            .position(|a| a.components.len() == 3)
            .unwrap();
        assert_eq!(w.draw_archetypes(), ids(&[visible]));
    }

    #[test]
    fn agents_archetype_layout() {
        let mut w = ArchetypeAgents::setup(PARAMS);
        assert_eq!(w.archetype_count(), 3);
        assert_eq!(w.step_archetypes(), ids(&[1, 2]));
    }

    #[test]
    fn agents_keeps_invariants_every_tick() {
        let mut w = ArchetypeAgents::setup(PARAMS);
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
        zero_count::<ArchetypeSimpleIter>();
        zero_count::<ArchetypeFragIter>();
        zero_count::<ArchetypeHeavyCompute>();
        zero_count::<ArchetypeAddRemove>();
        zero_count::<ArchetypeEntityChurn>();
        zero_count::<ArchetypeSprites>();
        zero_count::<ArchetypeAgents>();
    }
}
