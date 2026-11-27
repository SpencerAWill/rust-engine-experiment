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

/// `value` encoded into a buffer pre-filled with junk, then decoded.
fn round_trip<T: crate::Codec>(value: &T) -> T {
    let mut bytes = vec![0xAA_u8; T::LAYOUT.size()];
    value.encode(&mut bytes);
    T::decode(&bytes)
}

/// A workload with no entities checksums to 0 and survives a step.
fn zero_count<W: spike_ecs_harness::Workload>() {
    let mut w = W::setup(spike_ecs_harness::Params { count: 0, seed: 7 });
    assert_eq!(w.checksum(), 0);
    w.step();
    assert_eq!(w.checksum(), 0);
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

/// The raw query: validation before any table, matching and order, the table view's rules.
mod raw_query {
    use crate::archetype::ArchetypeId;
    use crate::{
        ComponentId, Entity, RawLayout, RawQuery, RawQueryError, RawRun, RawRunMut, World,
    };

    fn lay(size: usize, align: usize) -> RawLayout {
        RawLayout::new(size, align).unwrap()
    }

    fn message(payload: &(dyn core::any::Any + Send)) -> &str {
        payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or_default()
    }

    /// A world with raw components `a` (4 bytes), `b` (8), `c` (1) and `d` (4), and no entity.
    fn world4() -> (World, [ComponentId; 4]) {
        let mut w = World::new();
        let ida = w.register_raw("a", lay(4, 4));
        let idb = w.register_raw("b", lay(8, 4));
        let idc = w.register_raw("c", lay(1, 1));
        let idd = w.register_raw("d", lay(4, 4));
        (w, [ida, idb, idc, idd])
    }

    fn word(n: u32) -> [u8; 4] {
        n.to_le_bytes()
    }

    fn spawn(w: &mut World, values: &[(ComponentId, &[u8])]) -> Entity {
        w.spawn_raw(values).unwrap()
    }

    /// The entity list of every table a read run visits.
    fn visited(run: RawRun<'_, '_>) -> Vec<Vec<Entity>> {
        let mut tables = Vec::new();
        run.for_each_table(|t| tables.push(t.entities().to_vec()));
        tables
    }

    fn visited_mut(run: RawRunMut<'_, '_>) -> Vec<Vec<Entity>> {
        let mut tables = Vec::new();
        run.for_each_table(|t| tables.push(t.entities().to_vec()));
        tables
    }

    fn rejected(w: &mut World, q: &RawQuery) -> (RawQueryError, usize) {
        // Both entry points; a rejected query visits nothing.
        let mut count = 0;
        let read = match w.query_raw(q) {
            Ok(run) => {
                run.for_each_table(|_| count += 1);
                None
            }
            Err(e) => Some(e),
        };
        let write = match w.query_raw_mut(q) {
            Ok(run) => {
                run.for_each_table(|_| count += 1);
                None
            }
            Err(e) => Some(e),
        };
        let err = write.or(read).expect("the query was rejected");
        (err, count)
    }

    #[test]
    fn unknown_id_is_rejected_before_any_table() {
        let (mut w, [ida, ..]) = world4();
        spawn(&mut w, &[(ida, &word(1))]);
        let ghost = ComponentId::new(99);
        for q in [
            RawQuery::new().read(ghost),
            RawQuery::new().read(ida).write(ghost),
            RawQuery::new().read(ida).with(ghost),
            RawQuery::new().read(ida).without(ghost),
        ] {
            assert_eq!(rejected(&mut w, &q), (RawQueryError::UnknownId(ghost), 0));
        }
    }

    #[test]
    fn not_raw_is_rejected_before_any_table() {
        let (mut w, [ida, ..]) = world4();
        spawn(&mut w, &[(ida, &word(1))]);
        let e = w.spawn();
        w.insert(e, 5_u32);
        let typed = w.component_id::<u32>().unwrap();
        for q in [
            RawQuery::new().read(typed),
            RawQuery::new().read(ida).write(typed),
            RawQuery::new().read(ida).with(typed),
            RawQuery::new().read(ida).without(typed),
        ] {
            assert_eq!(rejected(&mut w, &q), (RawQueryError::NotRaw(typed), 0));
        }
        assert!(w.check_invariants().is_ok());
    }

    #[test]
    fn repeated_id_is_rejected_before_any_table() {
        let (mut w, [ida, idb, ..]) = world4();
        spawn(&mut w, &[(ida, &word(1)), (idb, &[0; 8])]);
        for q in [
            RawQuery::new().read(ida).read(ida),
            RawQuery::new().read(ida).write(ida),
            RawQuery::new().read(ida).with(ida),
            RawQuery::new().read(ida).without(ida),
            RawQuery::new().with(idb).without(idb),
            RawQuery::new().with(idb).with(idb),
            RawQuery::new().without(idb).without(idb),
        ] {
            let (err, count) = rejected(&mut w, &q);
            assert!(matches!(err, RawQueryError::Repeated(_)), "{q:?}");
            assert_eq!(count, 0);
        }
        assert_eq!(
            rejected(&mut w, &RawQuery::new().read(ida).with(ida)).0,
            RawQueryError::Repeated(ida)
        );
    }

    #[test]
    fn write_in_read_query_is_rejected() {
        let (mut w, [ida, idb, ..]) = world4();
        spawn(&mut w, &[(ida, &word(1)), (idb, &[0; 8])]);
        let q = RawQuery::new().read(ida).write(idb);
        let mut count = 0;
        match w.query_raw(&q) {
            Err(e) => assert_eq!(e, RawQueryError::WriteInReadQuery(idb)),
            Ok(run) => {
                run.for_each_table(|_| count += 1);
                panic!("a write fetch was accepted by query_raw");
            }
        }
        assert_eq!(count, 0);
        // The same query is fine in the exclusive entry point.
        assert_eq!(visited_mut(w.query_raw_mut(&q).unwrap()).len(), 1);
    }

    #[test]
    fn raw_query_errors_say_what_to_do() {
        let id = ComponentId::new(3);
        assert_eq!(
            RawQueryError::NotRaw(id).to_string(),
            "component id 3 is a Rust type's; use the typed API"
        );
        for e in [
            RawQueryError::UnknownId(id),
            RawQueryError::Repeated(id),
            RawQueryError::WriteInReadQuery(id),
        ] {
            let text = e.to_string();
            assert!(text.contains("component id 3"), "{text}");
            let _: &dyn core::error::Error = &e;
        }
        assert!(
            RawQueryError::WriteInReadQuery(id)
                .to_string()
                .contains("query_raw_mut")
        );
    }

    #[test]
    fn raw_query_visits_matching_non_empty_archetypes_in_creation_order() {
        let (mut w, [ida, idb, idc, idd]) = world4();
        // Archetype 1 is [a, b], 2 is [a], 3 is [a, c] (excluded), 4 is [a, d] (emptied), 5 is
        // [b] (does not match), 6 is [a, b] again for later entities (same archetype as 1).
        let e1 = spawn(&mut w, &[(ida, &word(1)), (idb, &[1; 8])]);
        let e2 = spawn(&mut w, &[(ida, &word(2))]);
        spawn(&mut w, &[(ida, &word(3)), (idc, &[3])]);
        let gone = spawn(&mut w, &[(ida, &word(4)), (idd, &word(4))]);
        spawn(&mut w, &[(idb, &[5; 8])]);
        let e6 = spawn(&mut w, &[(idb, &[6; 8]), (ida, &word(6))]);
        assert!(w.despawn(gone));
        assert_eq!(w.archetype_count(), 6);

        let q = RawQuery::new().read(ida).without(idc);
        let run = w.query_raw(&q).unwrap();
        let ids: Vec<usize> = run.matched_archetypes().iter().map(|i| i.index()).collect();
        assert_eq!(ids, vec![1, 2, 4], "the emptied archetype matches too");
        assert_eq!(visited(run), vec![vec![e1, e6], vec![e2]]);
        assert_eq!(
            visited_mut(w.query_raw_mut(&q).unwrap()),
            vec![vec![e1, e6], vec![e2]]
        );
        assert_eq!(
            w.query_raw(&q).unwrap().matched_archetypes(),
            vec![
                ArchetypeId::new(1),
                ArchetypeId::new(2),
                ArchetypeId::new(4)
            ]
        );
    }

    #[test]
    fn raw_query_with_filters_only() {
        let (mut w, [ida, idb, idc, _]) = world4();
        let e1 = spawn(&mut w, &[(ida, &word(1)), (idb, &[1; 8])]);
        let e2 = spawn(&mut w, &[(ida, &word(2))]);
        let e3 = spawn(&mut w, &[(idc, &[3])]);
        let q = RawQuery::new().with(ida);
        assert_eq!(visited(w.query_raw(&q).unwrap()), vec![vec![e1], vec![e2]]);
        let q = RawQuery::new().without(ida);
        // The empty archetype has no rows, so it is skipped; `c`'s archetype is visited.
        assert_eq!(visited(w.query_raw(&q).unwrap()), vec![vec![e3]]);
        // No fetch and no filter: every non-empty archetype.
        assert_eq!(
            visited(w.query_raw(&RawQuery::new()).unwrap()),
            vec![vec![e1], vec![e2], vec![e3]]
        );
        let mut seen = 0;
        w.query_raw(&RawQuery::new().with(ida))
            .unwrap()
            .for_each_table(|t| {
                assert_eq!(t.len(), t.entities().len());
                seen += t.len();
            });
        assert_eq!(seen, 2);
    }

    #[test]
    fn column_on_a_write_fetch_panics() {
        let (mut w, [ida, ..]) = world4();
        spawn(&mut w, &[(ida, &word(1))]);
        let q = RawQuery::new().write(ida);
        let err = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
            w.query_raw_mut(&q).unwrap().for_each_table(|t| {
                let _ = t.column(0);
            });
        }))
        .unwrap_err();
        assert_eq!(
            message(&*err),
            "RawTable::column: fetch 0 is a write fetch; take it with column_mut"
        );
    }

    #[test]
    fn column_mut_on_a_read_fetch_panics() {
        let (mut w, [ida, idb, ..]) = world4();
        spawn(&mut w, &[(ida, &word(1)), (idb, &[0; 8])]);
        let q = RawQuery::new().write(ida).read(idb);
        let err = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
            w.query_raw_mut(&q).unwrap().for_each_table(|mut t| {
                let _ = t.column_mut(1);
            });
        }))
        .unwrap_err();
        assert_eq!(
            message(&*err),
            "RawTable::column_mut: fetch 1 is a read fetch; take it with column"
        );
    }

    #[test]
    fn column_mut_twice_panics() {
        let (mut w, [ida, ..]) = world4();
        spawn(&mut w, &[(ida, &word(1))]);
        let q = RawQuery::new().write(ida);
        let err = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
            w.query_raw_mut(&q).unwrap().for_each_table(|mut t| {
                let _first = t.column_mut(0);
                let _second = t.column_mut(0);
            });
        }))
        .unwrap_err();
        assert_eq!(
            message(&*err),
            "RawTable::column_mut: fetch 0 was already taken for this table"
        );
    }

    #[test]
    fn a_fetch_that_does_not_exist_panics() {
        let (mut w, [ida, ..]) = world4();
        spawn(&mut w, &[(ida, &word(1))]);
        let q = RawQuery::new().read(ida);
        let err = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
            w.query_raw(&q).unwrap().for_each_table(|t| {
                let _ = t.column(1);
            });
        }))
        .unwrap_err();
        assert_eq!(
            message(&*err),
            "RawTable::column: fetch 1 does not exist; the query has 1 fetches"
        );
    }

    #[test]
    fn a_second_table_can_take_the_same_fetch_again() {
        let (mut w, [ida, idb, ..]) = world4();
        spawn(&mut w, &[(ida, &word(1))]);
        spawn(&mut w, &[(ida, &word(2)), (idb, &[0; 8])]);
        let mut taken = 0;
        w.query_raw_mut(&RawQuery::new().write(ida))
            .unwrap()
            .for_each_table(|mut t| {
                let col = t.column_mut(0);
                assert_eq!(col.len(), 1);
                taken += 1;
            });
        assert_eq!(taken, 2);
    }

    #[test]
    fn two_write_fetches_are_usable_at_once() {
        let (mut w, [ida, idb, ..]) = world4();
        let e = spawn(&mut w, &[(ida, &word(1)), (idb, &word(2).repeat(2))]);
        let q = RawQuery::new().write(ida).write(idb);
        w.query_raw_mut(&q).unwrap().for_each_table(|mut t| {
            let (ca, cb) = (t.column_mut(0), t.column_mut(1));
            ca.rows_mut().next().unwrap().copy_from_slice(&word(7));
            cb.rows_mut().next().unwrap().fill(9);
        });
        assert_eq!(w.get_raw(e, ida), Some(&word(7)[..]));
        assert_eq!(w.get_raw(e, idb), Some(&[9; 8][..]));
        w.check_invariants().unwrap();
    }

    #[test]
    fn writes_through_column_mut_are_seen_by_get_raw() {
        let (mut w, [ida, idb, ..]) = world4();
        let es: Vec<Entity> = (0..5)
            .map(|n| {
                spawn(
                    &mut w,
                    &[(ida, &word(n)), (idb, &[u8::try_from(n).unwrap(); 8])],
                )
            })
            .collect();
        let q = RawQuery::new().write(ida).read(idb);
        w.query_raw_mut(&q).unwrap().for_each_table(|mut t| {
            let (ca, cb) = (t.column_mut(0), t.column(1));
            let words = ca.cast_mut::<u32>().unwrap();
            for (x, bytes) in words.iter_mut().zip(cb.rows()) {
                *x += u32::from(bytes[0]) * 100 + 1;
            }
        });
        for (n, e) in es.iter().enumerate() {
            let want = u32::try_from(n).unwrap() * 101 + 1;
            assert_eq!(w.get_raw(*e, ida), Some(&word(want)[..]));
        }
        w.check_invariants().unwrap();
    }

    #[test]
    fn a_read_fetch_in_a_mut_run_gives_a_shared_view() {
        let (mut w, [ida, idb, ..]) = world4();
        let e = spawn(&mut w, &[(ida, &word(3)), (idb, &[1; 8])]);
        let q = RawQuery::new().read(idb).write(ida);
        let mut seen = Vec::new();
        w.query_raw_mut(&q).unwrap().for_each_table(|mut t| {
            let (rb, wa) = (t.column(0), t.column_mut(1));
            // The shared view is `Copy` and outlives the call.
            let again = rb;
            seen.push((rb.get(0).unwrap().to_vec(), again.len(), wa.len()));
        });
        assert_eq!(seen, vec![(vec![1; 8], 1, 1)]);
        assert_eq!(w.get_raw(e, idb), Some(&[1; 8][..]));
    }

    #[test]
    fn a_mixed_table_hands_out_only_the_fetched_columns() {
        let mut w = World::new();
        let ida = w.register_raw("a", lay(4, 4));
        // Registering u32 here, between the raw ids, puts its column between theirs.
        let probe = w.spawn();
        w.insert(probe, 0_u32);
        w.despawn(probe);
        let idb = w.register_raw("b", lay(8, 4));
        let idd = w.register_raw("d", lay(4, 4));
        // Columns in id order: raw a (unfetched), typed u32 (unfetched), raw b, raw d.
        let e = w.spawn();
        w.insert_raw(e, ida, &word(1)).unwrap();
        w.insert(e, 5_u32);
        w.insert_raw(e, idb, &[2; 8]).unwrap();
        w.insert_raw(e, idd, &word(4)).unwrap();
        // A fetch order that differs from the column order.
        let q = RawQuery::new().read(idd).write(idb);
        let mut seen = Vec::new();
        w.query_raw_mut(&q).unwrap().for_each_table(|mut t| {
            let (rd, mut wb) = (t.column(0), t.column_mut(1));
            seen.push((rd.get(0).unwrap().to_vec(), wb.get(0).unwrap().to_vec()));
            wb.get_mut(0).unwrap().fill(9);
        });
        assert_eq!(seen, vec![(word(4).to_vec(), vec![2; 8])]);
        assert_eq!(w.get_raw(e, idb), Some(&[9; 8][..]));
        let mut read = Vec::new();
        w.query_raw(&RawQuery::new().read(ida).read(idb))
            .unwrap()
            .for_each_table(|t| {
                read.push((t.column(0).get(0).unwrap().to_vec(), t.column(1).len()));
            });
        assert_eq!(read, vec![(word(1).to_vec(), 1)]);
        assert_eq!(w.get::<u32>(e), Some(&5));
        w.check_invariants().unwrap();
    }

    #[test]
    fn typed_query_ignores_raw_columns() {
        let (mut w, [ida, ..]) = world4();
        let e = w.spawn();
        w.insert(e, 5_u32);
        w.insert_raw(e, ida, &word(9)).unwrap();
        let other = w.spawn();
        w.insert(other, 6_u32);
        // The typed query matches on its own ids and sees both entities.
        let mut sum = 0;
        w.query::<&u32>().for_each(|v| sum += *v);
        assert_eq!(sum, 11);
        // The raw query matches the mixed archetype and never touches its typed column.
        let q = RawQuery::new().read(ida);
        assert_eq!(visited(w.query_raw(&q).unwrap()), vec![vec![e]]);
        let mut got = Vec::new();
        w.query_raw_mut(&RawQuery::new().write(ida))
            .unwrap()
            .for_each_table(|mut t| got.push(t.column_mut(0).get(0).unwrap().to_vec()));
        assert_eq!(got, vec![word(9).to_vec()]);
        w.check_invariants().unwrap();
    }
}

/// Steps `w` `ticks` times; after each step every one of the `count` entities must have been
/// collected for removal, once, which only happens if the insert pass gave each one a
/// `component` (ecs-11: the checksum cannot see the removal list).
fn assert_inserts_on_every_entity_each_step<W: spike_ecs_harness::Workload>(
    mut w: W,
    component: &str,
    count: usize,
    ticks: u64,
    removed_last_step: fn(&W) -> &[Entity],
) {
    for tick in 1..=ticks {
        w.step();
        let removed = removed_last_step(&w);
        assert_eq!(
            removed.len(),
            count,
            "the step removed {component} from the wrong number of entities at tick {tick}"
        );
        let distinct: BTreeSet<u64> = removed.iter().map(|e| e.to_bits()).collect();
        assert_eq!(
            distinct.len(),
            count,
            "an entity was removed twice in one step at tick {tick}"
        );
    }
}

mod workloads {
    use spike_ecs_harness::{Params, Workload, check_conformance};
    use spike_ecs_vec::add_remove::VecAddRemove;
    use spike_ecs_vec::agents::VecAgents;
    use spike_ecs_vec::entity_churn::VecEntityChurn;
    use spike_ecs_vec::frag_iter::VecFragIter;
    use spike_ecs_vec::heavy_compute::VecHeavyCompute;
    use spike_ecs_vec::hierarchy::VecHierarchy;
    use spike_ecs_vec::many_queries::VecManyQueries;
    use spike_ecs_vec::random_access::VecRandomAccess;
    use spike_ecs_vec::rare_components::VecRareComponents;
    use spike_ecs_vec::simple_iter::VecSimpleIter;
    use spike_ecs_vec::sprites::VecSprites;
    use spike_ecs_vec::wide_entities::VecWideEntities;

    use crate::add_remove::ArchetypeAddRemove;
    use crate::agents::ArchetypeAgents;
    use crate::archetype::ArchetypeId;
    use crate::entity_churn::ArchetypeEntityChurn;
    use crate::frag_iter::ArchetypeFragIter;
    use crate::heavy_compute::ArchetypeHeavyCompute;
    use crate::hierarchy::ArchetypeHierarchy;
    use crate::many_queries::ArchetypeManyQueries;
    use crate::random_access::ArchetypeRandomAccess;
    use crate::rare_components::ArchetypeRareComponents;
    use crate::simple_iter::ArchetypeSimpleIter;
    use crate::sprites::ArchetypeSprites;
    use crate::wide_entities::ArchetypeWideEntities;

    fn ids(v: &[usize]) -> Vec<ArchetypeId> {
        v.iter().map(|&i| ArchetypeId::new(i)).collect()
    }

    const PARAMS: Params = Params {
        count: 1_000,
        seed: 42,
    };
    const RARE_PARAMS: Params = Params {
        count: 10_000,
        seed: 42,
    };
    const TICKS: u64 = 120;
    const AGENT_TICKS: u64 = 1_300;
    const RANDOM_ACCESS_TICKS: u64 = 700;

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
    fn many_queries_archetype_matches_the_reference() {
        check_conformance::<VecManyQueries, ArchetypeManyQueries>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn many_queries_archetype_zero_count_is_valid() {
        super::zero_count::<ArchetypeManyQueries>();
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
    fn random_access_archetype_matches_the_reference() {
        check_conformance::<VecRandomAccess, ArchetypeRandomAccess>(PARAMS, RANDOM_ACCESS_TICKS)
            .unwrap();
    }

    /// The retargeted handle is not hashed, so the reference comparison cannot see pass B write it
    /// back. A target is stale after a step only if its slot was refilled in that step: an older
    /// stale handle means the write-back is missing.
    #[test]
    fn random_access_archetype_writes_back_retargets() {
        let mut w = ArchetypeRandomAccess::setup(PARAMS);
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
    fn random_access_archetype_zero_count_is_valid() {
        zero_count::<ArchetypeRandomAccess>();
    }

    /// R4: over `RANDOM_ACCESS_TICKS` ticks at 1,000 entities, some target handle is stale while
    /// its slot's roster entry has the same index and a higher generation, so the retarget path runs.
    #[test]
    fn random_access_sees_a_reused_index() {
        use spike_ecs_harness::random_access::Target;

        use crate::Entity;

        let mut w = ArchetypeRandomAccess::setup(PARAMS);
        let mut seen = false;
        for _ in 0..RANDOM_ACCESS_TICKS {
            w.step();
            let roster = w.roster();
            w.world().query::<&Target<Entity>>().for_each(|t| {
                let r = roster[t.slot as usize];
                if !w.world().is_alive(t.entity)
                    && r.index() == t.entity.index()
                    && r.generation() > t.entity.generation()
                {
                    seen = true;
                }
            });
        }
        assert!(
            seen,
            "no stale target with a reused index in {RANDOM_ACCESS_TICKS} ticks"
        );
    }

    #[test]
    fn hierarchy_archetype_matches_the_reference() {
        check_conformance::<VecHierarchy, ArchetypeHierarchy>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn hierarchy_archetype_zero_count_is_valid() {
        super::zero_count::<ArchetypeHierarchy>();
    }

    /// Spec 4.5: the empty archetype and one table per level, before and after steps.
    #[test]
    fn hierarchy_archetype_holds_one_table_per_level() {
        let mut w = ArchetypeHierarchy::setup(PARAMS);
        assert_eq!(w.archetype_count(), 9);
        w.world().check_invariants().unwrap();
        w.step();
        assert_eq!(w.archetype_count(), 9);
        w.world().check_invariants().unwrap();
        for _ in 0..9 {
            w.step();
        }
        assert_eq!(w.archetype_count(), 9);
        w.world().check_invariants().unwrap();
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
    fn add_remove_archetype_inserts_on_every_entity_each_step() {
        super::assert_inserts_on_every_entity_each_step(
            ArchetypeAddRemove::setup(PARAMS),
            "B",
            PARAMS.count,
            TICKS,
            ArchetypeAddRemove::removed_last_step,
        );
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

    use super::zero_count;

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

    #[test]
    fn wide_entities_archetype_matches_the_reference() {
        check_conformance::<VecWideEntities, ArchetypeWideEntities>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn wide_entities_archetype_zero_count_is_valid() {
        zero_count::<ArchetypeWideEntities>();
    }

    #[test]
    fn wide_entities_archetype_inserts_on_every_entity_each_step() {
        super::assert_inserts_on_every_entity_each_step(
            ArchetypeWideEntities::setup(PARAMS),
            "Extra",
            PARAMS.count,
            TICKS,
            ArchetypeWideEntities::removed_last_step,
        );
    }

    #[test]
    fn wide_entities_archetype_places_entities_in_one_archetype() {
        let mut w = ArchetypeWideEntities::setup(PARAMS);
        w.world().check_invariants().unwrap();
        assert_eq!(w.archetype_count(), 2);
        w.step();
        assert_eq!(w.archetype_count(), 3);
        for _ in 0..9 {
            w.step();
        }
        assert_eq!(w.archetype_count(), 3);
        w.world().check_invariants().unwrap();
    }

    #[test]
    fn rare_components_archetype_matches_the_reference() {
        check_conformance::<VecRareComponents, ArchetypeRareComponents>(RARE_PARAMS, TICKS)
            .unwrap();
    }

    #[test]
    fn rare_components_archetype_zero_count_is_valid() {
        zero_count::<ArchetypeRareComponents>();
    }

    #[test]
    fn rare_components_archetype_places_rare_entities_in_their_own_table() {
        let mut w = ArchetypeRareComponents::setup(RARE_PARAMS);
        let check = |w: &mut ArchetypeRareComponents| {
            assert_eq!(w.archetype_count(), 3);
            w.world().check_invariants().unwrap();
            let matched = w.step_archetypes();
            assert_eq!(matched.len(), 1);
            assert_eq!(w.world().archetypes()[matched[0].index()].table.len(), 78);
        };
        check(&mut w);
        w.step();
        check(&mut w);
    }
}

mod lookup {
    use crate::{Entity, World};

    #[derive(Debug, PartialEq)]
    struct A(u32);
    #[derive(Debug, PartialEq)]
    struct B(u32);

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
    fn lookup_agrees_with_get() {
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
        // Live with `A`.
        assert_eq!(both(with), (Some(&A(1)), Some(&A(1))));
        // Live without `A`.
        assert_eq!(both(without), (None, None));
        // Despawned.
        assert_eq!(both(dead), (None, None));
        // Stale, with its index reused by a live entity.
        assert_eq!(both(old), (None, None));
    }

    #[test]
    fn lookup_of_an_unregistered_type_is_none() {
        let mut w = World::new();
        let e = w.spawn_with((A(1),));
        let lookup = w.lookup::<B>();
        assert_eq!(lookup.get(e), None);
        assert_eq!(lookup.get(e), w.get::<B>(e));
    }
}

mod bytes_simple_iter {
    use engine::prelude::{Mat4, Vec3};
    use proptest::prelude::*;
    use spike_ecs_harness::simple_iter::{Matrix, Position, Rotation, Velocity};
    use spike_ecs_harness::{Params, Workload, check_conformance};
    use spike_ecs_vec::simple_iter::VecSimpleIter;

    use super::{round_trip, zero_count};
    use crate::Codec;
    use crate::archetype::ArchetypeId;
    use crate::bytes_simple_iter::{BytesCastSimpleIter, BytesCopySimpleIter};
    use crate::simple_iter::ArchetypeSimpleIter;

    const PARAMS: Params = Params {
        count: 1_000,
        seed: 42,
    };
    const TICKS: u64 = 120;

    fn vec3_bits(v: Vec3) -> [u32; 3] {
        v.to_array().map(f32::to_bits)
    }

    fn vec3_from_bits(bits: [u32; 3]) -> Vec3 {
        Vec3::from_array(bits.map(f32::from_bits))
    }

    proptest! {
        #[test]
        fn codec_round_trips_position(bits in any::<[u32; 3]>()) {
            let value = Position(vec3_from_bits(bits));
            prop_assert_eq!(vec3_bits(round_trip(&value).0), bits);
        }

        #[test]
        fn codec_round_trips_rotation(bits in any::<[u32; 3]>()) {
            let value = Rotation(vec3_from_bits(bits));
            prop_assert_eq!(vec3_bits(round_trip(&value).0), bits);
        }

        #[test]
        fn codec_round_trips_velocity(bits in any::<[u32; 3]>()) {
            let value = Velocity(vec3_from_bits(bits));
            prop_assert_eq!(vec3_bits(round_trip(&value).0), bits);
        }

        #[test]
        fn codec_round_trips_matrix(bits in any::<[u32; 16]>()) {
            let value = Matrix(Mat4::from_cols_array(&bits.map(f32::from_bits)));
            let back = round_trip(&value);
            prop_assert_eq!(back.0.to_cols_array().map(f32::to_bits), bits);
        }
    }

    #[test]
    fn codec_layouts_are_the_field_sizes() {
        assert_eq!(Position::LAYOUT.size(), 12);
        assert_eq!(Position::LAYOUT.align(), 4);
        assert_eq!(Matrix::LAYOUT.size(), 64);
        assert_eq!(Matrix::LAYOUT.align(), 4);
    }

    #[test]
    fn codec_writes_fields_in_declaration_order() {
        let mut bytes = [0_u8; 12];
        Position(Vec3::new(1.0, 2.0, 3.0)).encode(&mut bytes);
        let mut expected = Vec::new();
        for v in [1.0_f32, 2.0, 3.0] {
            expected.extend_from_slice(&v.to_le_bytes());
        }
        assert_eq!(bytes.as_slice(), expected.as_slice());
    }

    #[test]
    fn simple_iter_bytes_copy_matches_the_reference() {
        check_conformance::<VecSimpleIter, BytesCopySimpleIter>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn simple_iter_bytes_cast_matches_the_reference() {
        check_conformance::<VecSimpleIter, BytesCastSimpleIter>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn simple_iter_bytes_copy_has_the_typed_archetype_count() {
        let mut typed = ArchetypeSimpleIter::setup(PARAMS);
        let mut bytes = BytesCopySimpleIter::setup(PARAMS);
        assert_eq!(bytes.archetype_count(), typed.archetype_count());
        typed.step();
        bytes.step();
        assert_eq!(bytes.archetype_count(), typed.archetype_count());
        assert_eq!(bytes.step_archetypes(), vec![ArchetypeId::new(1)]);
    }

    #[test]
    fn simple_iter_bytes_cast_has_the_typed_archetype_count() {
        let mut typed = ArchetypeSimpleIter::setup(PARAMS);
        let mut bytes = BytesCastSimpleIter::setup(PARAMS);
        assert_eq!(bytes.archetype_count(), typed.archetype_count());
        typed.step();
        bytes.step();
        assert_eq!(bytes.archetype_count(), typed.archetype_count());
        assert_eq!(bytes.step_archetypes(), vec![ArchetypeId::new(1)]);
    }

    #[test]
    fn zero_count_is_valid_bytes_simple_iter() {
        zero_count::<BytesCopySimpleIter>();
        zero_count::<BytesCastSimpleIter>();
    }
}

mod bytes_add_remove {
    use proptest::prelude::*;
    use spike_ecs_harness::add_remove::{A, B};
    use spike_ecs_harness::{Params, Workload, check_conformance};
    use spike_ecs_vec::add_remove::VecAddRemove;

    use super::{round_trip, zero_count};
    use crate::add_remove::ArchetypeAddRemove;
    use crate::archetype::ArchetypeId;
    use crate::bytes_add_remove::{BytesCastAddRemove, BytesCopyAddRemove};

    const PARAMS: Params = Params {
        count: 1_000,
        seed: 42,
    };
    const TICKS: u64 = 120;

    proptest! {
        #[test]
        fn codec_round_trips_a(bits in any::<u32>()) {
            prop_assert_eq!(round_trip(&A(f32::from_bits(bits))).0.to_bits(), bits);
        }

        #[test]
        fn codec_round_trips_b(bits in any::<u32>()) {
            prop_assert_eq!(round_trip(&B(f32::from_bits(bits))).0.to_bits(), bits);
        }
    }

    #[test]
    fn add_remove_bytes_copy_matches_the_reference() {
        check_conformance::<VecAddRemove, BytesCopyAddRemove>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn add_remove_bytes_cast_matches_the_reference() {
        check_conformance::<VecAddRemove, BytesCastAddRemove>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn add_remove_archetype_bytes_copy_inserts_on_every_entity_each_step() {
        super::assert_inserts_on_every_entity_each_step(
            BytesCopyAddRemove::setup(PARAMS),
            "B",
            PARAMS.count,
            TICKS,
            BytesCopyAddRemove::removed_last_step,
        );
    }

    #[test]
    fn add_remove_archetype_bytes_cast_inserts_on_every_entity_each_step() {
        super::assert_inserts_on_every_entity_each_step(
            BytesCastAddRemove::setup(PARAMS),
            "B",
            PARAMS.count,
            TICKS,
            BytesCastAddRemove::removed_last_step,
        );
    }

    #[test]
    fn add_remove_bytes_copy_has_the_typed_archetype_count() {
        let mut typed = ArchetypeAddRemove::setup(PARAMS);
        let mut bytes = BytesCopyAddRemove::setup(PARAMS);
        assert_eq!(bytes.archetype_count(), typed.archetype_count());
        typed.step();
        bytes.step();
        assert_eq!(bytes.archetype_count(), typed.archetype_count());
        assert_eq!(bytes.read_archetypes(), typed.read_archetypes());
        assert_eq!(bytes.remove_archetypes(), vec![ArchetypeId::new(2)]);
        bytes.world().check_invariants().unwrap();
    }

    #[test]
    fn add_remove_bytes_cast_has_the_typed_archetype_count() {
        let mut typed = ArchetypeAddRemove::setup(PARAMS);
        let mut bytes = BytesCastAddRemove::setup(PARAMS);
        assert_eq!(bytes.archetype_count(), typed.archetype_count());
        typed.step();
        bytes.step();
        assert_eq!(bytes.archetype_count(), typed.archetype_count());
        assert_eq!(bytes.read_archetypes(), typed.read_archetypes());
        assert_eq!(bytes.remove_archetypes(), vec![ArchetypeId::new(2)]);
        bytes.world().check_invariants().unwrap();
    }

    #[test]
    fn zero_count_is_valid_bytes_add_remove() {
        zero_count::<BytesCopyAddRemove>();
        zero_count::<BytesCastAddRemove>();
    }
}

mod bytes_frag_iter {
    use proptest::prelude::*;
    use spike_ecs_harness::frag_iter::{Data, Z};
    use spike_ecs_harness::{Params, Workload, check_conformance};
    use spike_ecs_vec::frag_iter::VecFragIter;

    use super::{round_trip, zero_count};
    use crate::archetype::ArchetypeId;
    use crate::bytes_frag_iter::{BytesCastFragIter, BytesCopyFragIter};
    use crate::frag_iter::ArchetypeFragIter;

    const PARAMS: Params = Params {
        count: 20,
        seed: 42,
    };
    const TICKS: u64 = 120;

    proptest! {
        #[test]
        fn codec_round_trips_every_letter(bits in any::<u32>()) {
            for index in 0..spike_ecs_harness::frag_iter::LETTERS {
                let letter = spike_ecs_harness::frag_iter::Letter::from_index(
                    index,
                    f32::from_bits(bits),
                );
                let bytes = crate::bytes_frag_iter::encode_letter(&letter);
                let back = crate::bytes_frag_iter::decode_letter(index, &bytes);
                prop_assert_eq!(back.index(), index);
                prop_assert_eq!(back.value().to_bits(), bits);
            }
        }

        #[test]
        fn codec_round_trips_data(bits in any::<u32>()) {
            prop_assert_eq!(round_trip(&Data(f32::from_bits(bits))).0.to_bits(), bits);
        }

        #[test]
        fn codec_round_trips_letter_z(bits in any::<u32>()) {
            prop_assert_eq!(round_trip(&Z(f32::from_bits(bits))).0.to_bits(), bits);
        }
    }

    #[test]
    fn frag_iter_bytes_copy_matches_the_reference() {
        check_conformance::<VecFragIter, BytesCopyFragIter>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn frag_iter_bytes_cast_matches_the_reference() {
        check_conformance::<VecFragIter, BytesCastFragIter>(PARAMS, TICKS).unwrap();
    }

    #[test]
    fn frag_iter_bytes_copy_has_the_typed_archetype_count() {
        let mut typed = ArchetypeFragIter::setup(PARAMS);
        let mut bytes = BytesCopyFragIter::setup(PARAMS);
        assert_eq!(bytes.archetype_count(), 27);
        assert_eq!(bytes.archetype_count(), typed.archetype_count());
        typed.step();
        bytes.step();
        assert_eq!(bytes.archetype_count(), 27);
        assert_eq!(bytes.archetype_count(), typed.archetype_count());
        assert_eq!(bytes.step_archetypes(), typed.step_archetypes());
        let all: Vec<_> = (1..=26).map(ArchetypeId::new).collect();
        assert_eq!(bytes.step_archetypes(), all);
        bytes.world().check_invariants().unwrap();
    }

    #[test]
    fn frag_iter_bytes_cast_has_the_typed_archetype_count() {
        let mut typed = ArchetypeFragIter::setup(PARAMS);
        let mut bytes = BytesCastFragIter::setup(PARAMS);
        assert_eq!(bytes.archetype_count(), 27);
        assert_eq!(bytes.archetype_count(), typed.archetype_count());
        typed.step();
        bytes.step();
        assert_eq!(bytes.archetype_count(), 27);
        assert_eq!(bytes.archetype_count(), typed.archetype_count());
        assert_eq!(bytes.step_archetypes(), typed.step_archetypes());
        bytes.world().check_invariants().unwrap();
    }

    #[test]
    fn zero_count_is_valid_bytes_frag_iter() {
        zero_count::<BytesCopyFragIter>();
        zero_count::<BytesCastFragIter>();
    }
}
