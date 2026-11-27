//! The `hierarchy` workload on the dense-array `World` (M3.24 spec §4.3).

use engine::prelude::{Prng, WorldTransform2d};
use spike_ecs_harness::hierarchy::{
    self as rules, Depth1, Depth2, Depth3, Depth4, Depth5, Depth6, Depth7, Depth8, GlobalTransform,
    Hierarchy, LocalTransform, Node, Parent, SpawnId, forest,
};
use spike_ecs_harness::{Params, Workload};

use crate::{Entity, World};

/// `hierarchy` on a dense-array [`World`]: each level's queries are views that join the presence
/// sets of their components and the level marker, and each parent lookup is one indexed access
/// into the global-transform array.
///
/// The step spins every local, sets the roots' globals, then for each level from 2 to 8 runs a read
/// pass (one [`Lookup`](crate::Lookup) of the parents' globals, results pushed to a scratch buffer)
/// and a write pass over the level's globals, so a level reads only parents this step wrote.
#[derive(Debug)]
pub struct DenseHierarchy {
    world: World,
    scratch: Vec<GlobalTransform>,
}

/// Sets every root's global from its local.
fn roots(world: &mut World) {
    world
        .query_mut::<(&LocalTransform, &mut GlobalTransform)>()
        .with::<Depth1>()
        .for_each(|(local, global)| *global = rules::root(local));
}

/// Sets the globals of the entities at depth `D` from their parents' globals, which the previous
/// level's call already wrote: the read pass fills `scratch`, the write pass drains it in the same
/// order.
fn level<D: 'static>(world: &mut World, scratch: &mut Vec<GlobalTransform>) {
    scratch.clear();
    {
        let parents = world.lookup::<GlobalTransform>();
        let out = &mut *scratch;
        world
            .query::<(&LocalTransform, &Parent<Entity>)>()
            .with::<D>()
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
        .with::<D>()
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

/// Spawns an entity with a level's components, one insert each.
fn spawn_with<D: 'static>(
    world: &mut World,
    (id, local, global): (SpawnId, LocalTransform, GlobalTransform),
    parent: Option<Parent<Entity>>,
    depth: D,
) -> Entity {
    let e = world.spawn();
    world.insert(e, id);
    world.insert(e, local);
    world.insert(e, global);
    if let Some(parent) = parent {
        world.insert(e, parent);
    }
    world.insert(e, depth);
    e
}

/// Spawns one node with the components its level needs, linked to its parent's handle.
fn spawn(world: &mut World, node: &Node, handles: &[Entity]) -> Entity {
    let base = (
        SpawnId(node.id),
        node.local,
        GlobalTransform(WorldTransform2d::IDENTITY),
    );
    let parent = node
        .parent
        .map(|p| Parent(handles[usize::try_from(p).expect("a spawn id fits in usize")]));
    match node.level {
        1 => spawn_with(world, base, parent, Depth1),
        2 => spawn_with(world, base, parent, Depth2),
        3 => spawn_with(world, base, parent, Depth3),
        4 => spawn_with(world, base, parent, Depth4),
        5 => spawn_with(world, base, parent, Depth5),
        6 => spawn_with(world, base, parent, Depth6),
        7 => spawn_with(world, base, parent, Depth7),
        8 => spawn_with(world, base, parent, Depth8),
        other => unreachable!("the forest has levels 1 to 8, not {other}"),
    }
}

/// Adds the hash terms of every entity at depth `D`, parent included.
fn sum_level<D: 'static>(world: &World, level: u32, sum: &mut u64) {
    let ids = world.lookup::<SpawnId>();
    world
        .query::<(
            (&SpawnId, &LocalTransform),
            (&GlobalTransform, &Parent<Entity>),
        )>()
        .with::<D>()
        .for_each(|((id, local), (global, parent))| {
            let parent_id = ids.get(parent.0).expect("a parent is alive").0;
            *sum = sum.wrapping_add(rules::hash(id, level, Some(parent_id), local, global));
        });
}

impl Workload for DenseHierarchy {
    type Spec = Hierarchy;
    const BACKEND: &'static str = "dense";

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
            .for_each(rules::spin);
        roots(&mut self.world);
        levels(&mut self.world, &mut self.scratch);
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        self.world
            .query::<(&SpawnId, &LocalTransform, &GlobalTransform)>()
            .with::<Depth1>()
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
