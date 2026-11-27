//! The `hierarchy` workload on a `bevy_ecs` `World` (M3.24 spec §4.3).

use bevy_ecs::entity::Entity;
use bevy_ecs::query::{QueryState, With};
use bevy_ecs::world::World;
use engine::prelude::Prng;
use spike_ecs_harness::hierarchy::{
    self as rules, Depth1, Depth2, Depth3, Depth4, Depth5, Depth6, Depth7, Depth8, GlobalTransform,
    Hierarchy, LocalTransform, Node, Parent, SpawnId, forest,
};
use spike_ecs_harness::{Params, Workload};

use crate::Table;

/// Every local, for the spin.
type Update = QueryState<&'static mut Table<LocalTransform>>;

/// The roots: a local and the global it sets.
type Roots = QueryState<
    (
        &'static Table<LocalTransform>,
        &'static mut Table<GlobalTransform>,
    ),
    With<Table<Depth1>>,
>;

/// The read pass of the level at depth `D`: each entity's local and its parent's handle.
type Read<D> = QueryState<
    (
        &'static Table<LocalTransform>,
        &'static Table<Parent<Entity>>,
    ),
    With<Table<D>>,
>;

/// The write pass of the level at depth `D`: the globals.
type Write<D> = QueryState<&'static mut Table<GlobalTransform>, With<Table<D>>>;

/// Any entity's global, read by handle.
type GlobalLookup = QueryState<&'static Table<GlobalTransform>>;

/// One read and one write query per depth from 2 to 8: eight distinct types, so no array.
#[derive(Debug)]
struct Levels {
    read2: Read<Depth2>,
    write2: Write<Depth2>,
    read3: Read<Depth3>,
    write3: Write<Depth3>,
    read4: Read<Depth4>,
    write4: Write<Depth4>,
    read5: Read<Depth5>,
    write5: Write<Depth5>,
    read6: Read<Depth6>,
    write6: Write<Depth6>,
    read7: Read<Depth7>,
    write7: Write<Depth7>,
    read8: Read<Depth8>,
    write8: Write<Depth8>,
}

/// `hierarchy` on a `bevy_ecs` [`World`]: every type is stored in a table, every query is a
/// `QueryState` built in `setup` after the spawns, and a parent's global is read through a
/// `QueryState<&Table<GlobalTransform>>` with `get_manual`.
///
/// The step spins every local, sets the roots' globals, then for each level from 2 to 8 runs a read
/// pass (results pushed to a scratch buffer) and a write pass over the level's globals, so a level
/// reads only parents this step wrote.
#[derive(Debug)]
pub struct BevyHierarchy {
    world: World,
    update: Update,
    roots: Roots,
    lookup: GlobalLookup,
    levels: Levels,
    scratch: Vec<GlobalTransform>,
}

/// Sets the globals of the entities at depth `D` from their parents' globals, which the previous
/// level's call already wrote: the read pass fills `scratch`, the write pass drains it in the same
/// order.
fn level<D: Send + Sync + 'static>(
    world: &mut World,
    lookup: &GlobalLookup,
    read: &Read<D>,
    write: &mut Write<D>,
    scratch: &mut Vec<GlobalTransform>,
) {
    scratch.clear();
    {
        let world = &*world;
        let out = &mut *scratch;
        read.iter_manual(world).for_each(|(local, parent)| {
            let parent_global = lookup
                .get_manual(world, parent.0.0)
                .expect("a parent is spawned before its children");
            out.push(rules::place(&parent_global.0, &local.0));
        });
    }
    let mut next = scratch.iter();
    write.iter_mut(world).for_each(|mut global| {
        global.0 = *next.next().expect("one scratch entry per entity");
    });
}

impl BevyHierarchy {
    /// Sets the roots' globals, then every level from 2 to 8, parents first.
    fn propagate(&mut self) {
        let world = &mut self.world;
        let lookup = &self.lookup;
        let scratch = &mut self.scratch;
        self.roots
            .iter_mut(world)
            .for_each(|(local, mut global)| global.0 = rules::root(&local.0));
        let l = &mut self.levels;
        level(world, lookup, &l.read2, &mut l.write2, scratch);
        level(world, lookup, &l.read3, &mut l.write3, scratch);
        level(world, lookup, &l.read4, &mut l.write4, scratch);
        level(world, lookup, &l.read5, &mut l.write5, scratch);
        level(world, lookup, &l.read6, &mut l.write6, scratch);
        level(world, lookup, &l.read7, &mut l.write7, scratch);
        level(world, lookup, &l.read8, &mut l.write8, scratch);
    }
}

/// Spawns one node as the bundle its level needs, linked to its parent's handle.
fn spawn(world: &mut World, node: &Node, handles: &[Entity]) -> Entity {
    let id = Table(SpawnId(node.id));
    let local = Table(node.local);
    let global = Table(GlobalTransform(engine::prelude::WorldTransform2d::IDENTITY));
    let parent = node.parent.map(|p| {
        Table(Parent(
            handles[usize::try_from(p).expect("a spawn id fits in usize")],
        ))
    });
    let parent = || parent.expect("a node below level 1 has a parent");
    match node.level {
        1 => world.spawn((id, local, global, Table(Depth1))).id(),
        2 => world
            .spawn((id, local, global, parent(), Table(Depth2)))
            .id(),
        3 => world
            .spawn((id, local, global, parent(), Table(Depth3)))
            .id(),
        4 => world
            .spawn((id, local, global, parent(), Table(Depth4)))
            .id(),
        5 => world
            .spawn((id, local, global, parent(), Table(Depth5)))
            .id(),
        6 => world
            .spawn((id, local, global, parent(), Table(Depth6)))
            .id(),
        7 => world
            .spawn((id, local, global, parent(), Table(Depth7)))
            .id(),
        8 => world
            .spawn((id, local, global, parent(), Table(Depth8)))
            .id(),
        other => unreachable!("the forest has levels 1 to 8, not {other}"),
    }
}

impl Workload for BevyHierarchy {
    type Spec = Hierarchy;
    const BACKEND: &'static str = "bevy_ecs";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        let mut handles = Vec::with_capacity(params.count);
        for node in forest(&prng, params.count) {
            let entity = spawn(&mut world, &node, &handles);
            handles.push(entity);
        }
        drop(handles);
        let update = world.query();
        let roots = world.query_filtered();
        let lookup = world.query();
        let levels = Levels {
            read2: world.query_filtered(),
            write2: world.query_filtered(),
            read3: world.query_filtered(),
            write3: world.query_filtered(),
            read4: world.query_filtered(),
            write4: world.query_filtered(),
            read5: world.query_filtered(),
            write5: world.query_filtered(),
            read6: world.query_filtered(),
            write6: world.query_filtered(),
            read7: world.query_filtered(),
            write7: world.query_filtered(),
            read8: world.query_filtered(),
            write8: world.query_filtered(),
        };
        let mut this = Self {
            world,
            update,
            roots,
            lookup,
            levels,
            scratch: Vec::with_capacity(params.count),
        };
        this.propagate();
        this
    }

    fn step(&mut self) {
        self.update
            .iter_mut(&mut self.world)
            .for_each(|mut local| rules::spin(&mut local.0));
        self.propagate();
        self.world.clear_trackers();
    }

    fn checksum(&self) -> u64 {
        /// Adds the hash terms of every entity at depth `D`, parent included.
        fn sum_level<D: Send + Sync + 'static>(world: &World, level: u32, sum: &mut u64) {
            let Some(query) = world.try_query_filtered::<(
                (&Table<SpawnId>, &Table<LocalTransform>),
                (&Table<GlobalTransform>, Option<&Table<Parent<Entity>>>),
            ), With<Table<D>>>() else {
                return;
            };
            query
                .iter_manual(world)
                .for_each(|((id, local), (global, parent))| {
                    let parent_id = parent.map(|p| {
                        world
                            .get::<Table<SpawnId>>(p.0.0)
                            .expect("a parent is alive")
                            .0
                            .0
                    });
                    *sum =
                        sum.wrapping_add(rules::hash(&id.0, level, parent_id, &local.0, &global.0));
                });
        }
        let mut sum = 0_u64;
        sum_level::<Depth1>(&self.world, 1, &mut sum);
        sum_level::<Depth2>(&self.world, 2, &mut sum);
        sum_level::<Depth3>(&self.world, 3, &mut sum);
        sum_level::<Depth4>(&self.world, 4, &mut sum);
        sum_level::<Depth5>(&self.world, 5, &mut sum);
        sum_level::<Depth6>(&self.world, 6, &mut sum);
        sum_level::<Depth7>(&self.world, 7, &mut sum);
        sum_level::<Depth8>(&self.world, 8, &mut sum);
        sum
    }
}
