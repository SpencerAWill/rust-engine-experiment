//! The `add_remove` workload on a `bevy_ecs` `World`.
//!
//! The passes run outside any query iteration, so they use `World::entity_mut` directly.

use bevy_ecs::entity::Entity;
use bevy_ecs::query::{QueryState, With};
use bevy_ecs::world::World;
use engine::prelude::Prng;
use spike_ecs_harness::add_remove::{self as rules, A, AddRemove, B};
use spike_ecs_harness::{Params, Workload};

use crate::{Sparse, Table};

/// The read pass's query.
type ReadQuery = QueryState<(Entity, &'static Table<A>)>;

/// The remove pass's query.
type RemoveQuery = QueryState<Entity, With<Table<B>>>;

/// `add_remove` on a `bevy_ecs` [`World`]: every tick moves each entity from `(A)` to `(A, B)`
/// and back, a table row move each way.
#[derive(Debug)]
pub struct BevyAddRemove {
    world: World,
    read: ReadQuery,
    with_b: RemoveQuery,
    with_a_list: Vec<(Entity, A)>,
    with_b_list: Vec<Entity>,
}

#[cfg(test)]
impl BevyAddRemove {
    /// The world, for tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }

    /// The entities the last step removed `B` from, for tests.
    pub(crate) fn removed_last_step(&self) -> &[Entity] {
        &self.with_b_list
    }
}

impl Workload for BevyAddRemove {
    type Spec = AddRemove;
    const BACKEND: &'static str = "bevy_ecs";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            world.spawn(Table(rules::spawn(&prng, i)));
        }
        let read = world.query();
        let with_b = world.query_filtered();
        Self {
            world,
            read,
            with_b,
            with_a_list: Vec::with_capacity(params.count),
            with_b_list: Vec::with_capacity(params.count),
        }
    }

    fn step(&mut self) {
        // Two separate passes: every insert happens before the first removal (spec 4.4.4).
        self.with_a_list.clear();
        let pairs = &mut self.with_a_list;
        self.read
            .iter(&self.world)
            .for_each(|(e, a)| pairs.push((e, a.0)));
        for &(e, a) in &self.with_a_list {
            self.world.entity_mut(e).insert(Table(rules::make_b(&a)));
        }

        self.with_b_list.clear();
        let removals = &mut self.with_b_list;
        self.with_b.iter(&self.world).for_each(|e| removals.push(e));
        for &e in &self.with_b_list {
            self.world.entity_mut(e).remove::<Table<B>>();
        }
        self.world.clear_trackers();
    }

    fn checksum(&self) -> u64 {
        let Some(query) = self.world.try_query::<(&Table<A>, Option<&Table<B>>)>() else {
            return 0;
        };
        query.iter_manual(&self.world).fold(0_u64, |sum, (a, b)| {
            sum.wrapping_add(rules::hash(&a.0, b.map(|b| &b.0)))
        })
    }
}

/// The remove pass's query of the sparse variant. Its read pass reuses [`ReadQuery`], because `A`
/// stays in table storage.
type SparseRemoveQuery = QueryState<Entity, With<Sparse<B>>>;

/// `add_remove` on a `bevy_ecs` [`World`] with `B` in `SparseSet` storage: every tick adds and
/// removes `B` on each entity, which moves the entity between archetypes but no table row.
#[derive(Debug)]
pub struct BevySparseAddRemove {
    world: World,
    read: ReadQuery,
    with_b: SparseRemoveQuery,
    with_a_list: Vec<(Entity, A)>,
    with_b_list: Vec<Entity>,
}

#[cfg(test)]
impl BevySparseAddRemove {
    /// The world, for tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }

    /// The entities the last step removed `B` from, for tests.
    pub(crate) fn removed_last_step(&self) -> &[Entity] {
        &self.with_b_list
    }

    /// The world, mutably, for tests.
    pub(crate) fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }
}

impl Workload for BevySparseAddRemove {
    type Spec = AddRemove;
    const BACKEND: &'static str = "bevy_ecs_sparse";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            world.spawn(Table(rules::spawn(&prng, i)));
        }
        let read = world.query();
        let with_b = world.query_filtered();
        Self {
            world,
            read,
            with_b,
            with_a_list: Vec::with_capacity(params.count),
            with_b_list: Vec::with_capacity(params.count),
        }
    }

    fn step(&mut self) {
        // Two separate passes: every insert happens before the first removal (spec 4.4.4).
        self.with_a_list.clear();
        let pairs = &mut self.with_a_list;
        self.read
            .iter(&self.world)
            .for_each(|(e, a)| pairs.push((e, a.0)));
        for &(e, a) in &self.with_a_list {
            self.world.entity_mut(e).insert(Sparse(rules::make_b(&a)));
        }

        self.with_b_list.clear();
        let removals = &mut self.with_b_list;
        self.with_b.iter(&self.world).for_each(|e| removals.push(e));
        for &e in &self.with_b_list {
            self.world.entity_mut(e).remove::<Sparse<B>>();
        }
        self.world.clear_trackers();
    }

    fn checksum(&self) -> u64 {
        let Some(query) = self.world.try_query::<(&Table<A>, Option<&Sparse<B>>)>() else {
            return 0;
        };
        query.iter_manual(&self.world).fold(0_u64, |sum, (a, b)| {
            sum.wrapping_add(rules::hash(&a.0, b.map(|b| &b.0)))
        })
    }
}
