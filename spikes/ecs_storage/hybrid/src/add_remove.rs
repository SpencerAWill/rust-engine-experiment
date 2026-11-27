//! The `add_remove` workload on the hybrid `World`.
//!
//! The passes run outside any query, so they use `World::insert` and `World::remove` directly
//! rather than a command buffer, for M3.3's reason (its spec section 4.7, which this spec's
//! section 4.9 cites).

use engine::prelude::Prng;
use spike_ecs_harness::add_remove::{self as rules, A, AddRemove, B};
use spike_ecs_harness::{Params, Workload};

use crate::{Entity, Query, With, World};

/// `add_remove` on the hybrid [`World`]: every tick inserts `B` into its sparse set for each
/// entity and removes it again; no entity changes archetype.
#[derive(Debug)]
pub struct HybridAddRemove {
    world: World,
    with_a: Vec<(Entity, f32)>,
    with_b: Vec<Entity>,
}

/// The read pass's query, shared with the test hook so the two cannot drift.
fn read_query(world: &World) -> Query<'_, (Entity, &A)> {
    world.query()
}

/// The remove pass's query, shared with the test hook so the two cannot drift.
fn remove_query(world: &World) -> Query<'_, Entity, ((), With<B>)> {
    world.query().with::<B>()
}

#[cfg(test)]
impl HybridAddRemove {
    /// The archetypes the read pass's query matches, for tests.
    pub(crate) fn read_archetypes(&self) -> Vec<crate::archetype::ArchetypeId> {
        read_query(&self.world).matched_archetypes()
    }

    /// The sparse types the read pass's query probes per row, for tests.
    pub(crate) fn read_probed(
        &self,
    ) -> Vec<(crate::archetype::ComponentId, crate::query::seal::Role)> {
        read_query(&self.world).probed()
    }

    /// Whether the read query probes every row, for tests.
    pub(crate) fn read_row_probes(&self) -> bool {
        read_query(&self.world).row_probes()
    }

    /// The sparse types the removal pass's query probes per row, for tests.
    pub(crate) fn remove_probed(
        &self,
    ) -> Vec<(crate::archetype::ComponentId, crate::query::seal::Role)> {
        remove_query(&self.world).probed()
    }

    /// Whether the remove query probes every row, for tests.
    pub(crate) fn remove_row_probes(&self) -> bool {
        remove_query(&self.world).row_probes()
    }

    /// The archetypes the removal pass's query matches, for tests.
    pub(crate) fn remove_archetypes(&self) -> Vec<crate::archetype::ArchetypeId> {
        remove_query(&self.world).matched_archetypes()
    }

    /// The number of archetypes in the world, for tests.
    pub(crate) fn archetype_count(&self) -> usize {
        self.world.archetype_count()
    }

    /// The world, for invariant checks in tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }

    /// The entities the last step removed `B` from, for tests.
    pub(crate) fn removed_last_step(&self) -> &[Entity] {
        &self.with_b
    }
}

impl Workload for HybridAddRemove {
    type Spec = AddRemove;
    const BACKEND: &'static str = "hybrid";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            world.spawn_with((rules::spawn(&prng, i),));
        }
        Self {
            world,
            with_a: Vec::with_capacity(params.count),
            with_b: Vec::with_capacity(params.count),
        }
    }

    fn step(&mut self) {
        // Two separate passes: every insert happens before the first removal (section 4.4.4
        // of the M3.2 spec).
        self.with_a.clear();
        let with_a = &mut self.with_a;
        read_query(&self.world).for_each(|(e, a)| with_a.push((e, a.0)));
        for &(e, a) in &self.with_a {
            self.world.insert::<B>(e, rules::make_b(&A(a)));
        }

        self.with_b.clear();
        let with_b = &mut self.with_b;
        remove_query(&self.world).for_each(|e| with_b.push(e));
        for &e in &self.with_b {
            self.world.remove::<B>(e);
        }
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        self.world
            .query::<(&A, Option<&B>)>()
            .for_each(|(a, b)| sum = sum.wrapping_add(rules::hash(a, b)));
        sum
    }
}
