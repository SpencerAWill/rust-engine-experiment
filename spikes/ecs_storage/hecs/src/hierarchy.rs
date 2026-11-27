//! The `hierarchy` workload on a `hecs` `World` (M3.24 spec §4.3).

use engine::prelude::{Prng, WorldTransform2d};
use hecs::{Component, Entity, World};
use spike_ecs_harness::hierarchy::{
    self as rules, Depth1, Depth2, Depth3, Depth4, Depth5, Depth6, Depth7, Depth8, GlobalTransform,
    Hierarchy, LocalTransform, Node, Parent, SpawnId, forest,
};
use spike_ecs_harness::{Params, Workload};

/// `hierarchy` on a `hecs` [`World`]: each level's entities share an archetype of their own,
/// told apart by a depth marker.
///
/// The step spins every local, sets the roots' globals, then for each level from 2 to 8 runs a read
/// pass (a query beside one `world.view::<&GlobalTransform>()` of the parents' globals, results
/// pushed to a scratch buffer) and a write pass over the level's globals with `query_mut`, so a
/// level reads only parents this step wrote.
pub struct HecsHierarchy {
    world: World,
    scratch: Vec<GlobalTransform>,
}

impl core::fmt::Debug for HecsHierarchy {
    /// `hecs::World` has no `Debug`, so this shows the entity count only.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HecsHierarchy")
            .field("entities", &self.world.len())
            .finish_non_exhaustive()
    }
}

/// Sets every root's global from its local.
fn roots(world: &mut World) {
    world
        .query_mut::<(&LocalTransform, &mut GlobalTransform)>()
        .with::<&Depth1>()
        .into_iter()
        .for_each(|(local, global)| *global = rules::root(local));
}

/// Sets the globals of the entities at depth `D` from their parents' globals, which the previous
/// level's call already wrote: the read pass fills `scratch`, the write pass drains it in the same
/// order.
fn level<D: Component>(world: &mut World, scratch: &mut Vec<GlobalTransform>) {
    scratch.clear();
    {
        let parents = world.view::<&GlobalTransform>();
        let out = &mut *scratch;
        world
            .query::<(&LocalTransform, &Parent<Entity>)>()
            .with::<&D>()
            .iter()
            .for_each(|(local, parent)| {
                let parent_global = parents
                    .get(parent.0)
                    .expect("a parent is spawned before its children");
                out.push(rules::place(parent_global, local));
            });
    }
    let mut next = scratch.iter();
    world
        .query_mut::<&mut GlobalTransform>()
        .with::<&D>()
        .into_iter()
        .for_each(|global| *global = *next.next().expect("one scratch entry per entity"));
}

/// Every level from 2 to 8, parents first.
fn levels(world: &mut World, scratch: &mut Vec<GlobalTransform>) {
    level::<Depth2>(world, scratch);
    level::<Depth3>(world, scratch);
    level::<Depth4>(world, scratch);
    level::<Depth5>(world, scratch);
    level::<Depth6>(world, scratch);
    level::<Depth7>(world, scratch);
    level::<Depth8>(world, scratch);
}

/// Spawns one node as the bundle its level needs, linked to its parent's handle.
fn spawn(world: &mut World, node: &Node, handles: &[Entity]) -> Entity {
    let id = SpawnId(node.id);
    let local = node.local;
    let global = GlobalTransform(WorldTransform2d::IDENTITY);
    let parent = node
        .parent
        .map(|p| Parent(handles[usize::try_from(p).expect("a spawn id fits in usize")]));
    let parent = || parent.expect("a node below level 1 has a parent");
    match node.level {
        1 => world.spawn((id, local, global, Depth1)),
        2 => world.spawn((id, local, global, parent(), Depth2)),
        3 => world.spawn((id, local, global, parent(), Depth3)),
        4 => world.spawn((id, local, global, parent(), Depth4)),
        5 => world.spawn((id, local, global, parent(), Depth5)),
        6 => world.spawn((id, local, global, parent(), Depth6)),
        7 => world.spawn((id, local, global, parent(), Depth7)),
        8 => world.spawn((id, local, global, parent(), Depth8)),
        other => unreachable!("the forest has levels 1 to 8, not {other}"),
    }
}

/// Adds the hash terms of every entity at depth `D`, parent included.
fn sum_level<D: Component>(world: &World, level: u32, sum: &mut u64) {
    let ids = world.view::<&SpawnId>();
    world
        .query::<(
            (&SpawnId, &LocalTransform),
            (&GlobalTransform, &Parent<Entity>),
        )>()
        .with::<&D>()
        .iter()
        .for_each(|((id, local), (global, parent))| {
            let parent_id = ids.get(parent.0).expect("a parent is alive").0;
            *sum = sum.wrapping_add(rules::hash(id, level, Some(parent_id), local, global));
        });
}

impl Workload for HecsHierarchy {
    type Spec = Hierarchy;
    const BACKEND: &'static str = "hecs";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        let mut handles = Vec::with_capacity(params.count);
        for node in forest(&prng, params.count) {
            let entity = spawn(&mut world, &node, &handles);
            handles.push(entity);
        }
        drop(handles);
        let mut scratch = Vec::with_capacity(params.count);
        roots(&mut world);
        levels(&mut world, &mut scratch);
        Self { world, scratch }
    }

    fn step(&mut self) {
        self.world
            .query_mut::<&mut LocalTransform>()
            .into_iter()
            .for_each(rules::spin);
        roots(&mut self.world);
        levels(&mut self.world, &mut self.scratch);
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        self.world
            .query::<(&SpawnId, &LocalTransform, &GlobalTransform)>()
            .with::<&Depth1>()
            .iter()
            .for_each(|(id, local, global)| {
                sum = sum.wrapping_add(rules::hash(id, 1, None, local, global));
            });
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
