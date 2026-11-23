//! The `simple_iter` workload on the hybrid `World`.

use engine::prelude::Prng;
use spike_ecs_harness::simple_iter::{self as rules, Position, SimpleIter, Velocity};
use spike_ecs_harness::{Params, Workload};

use crate::{QueryMut, World};

/// `simple_iter` on the hybrid [`World`]: one table holding every entity.
#[derive(Debug)]
pub struct HybridSimpleIter {
    world: World,
}

/// The step's query, shared with the test hook so the two cannot drift.
fn step_query(world: &mut World) -> QueryMut<'_, (&mut Position, &Velocity)> {
    world.query_mut()
}

#[cfg(test)]
impl HybridSimpleIter {
    /// The archetypes the step's query matches, for tests.
    pub(crate) fn step_archetypes(&mut self) -> Vec<crate::archetype::ArchetypeId> {
        step_query(&mut self.world).matched_archetypes()
    }

    /// The component types the step's query probes per row, for tests.
    pub(crate) fn step_probed(
        &mut self,
    ) -> Vec<(crate::archetype::ComponentId, crate::query::seal::Role)> {
        step_query(&mut self.world).probed()
    }

    /// Whether the step query probes every row, for tests.
    pub(crate) fn step_row_probes(&mut self) -> bool {
        step_query(&mut self.world).row_probes()
    }

    /// The number of archetypes in the world, for tests.
    pub(crate) fn archetype_count(&self) -> usize {
        self.world.archetype_count()
    }
}

impl Workload for HybridSimpleIter {
    type Spec = SimpleIter;
    const BACKEND: &'static str = "hybrid";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            world.spawn_with(rules::spawn(&prng, i));
        }
        Self { world }
    }

    fn step(&mut self) {
        step_query(&mut self.world)
            .for_each(|(position, velocity)| rules::update(position, velocity));
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        self.world
            .query::<(&Position, &Velocity)>()
            .for_each(|(position, velocity)| {
                sum = sum.wrapping_add(rules::hash(position, velocity));
            });
        sum
    }
}
