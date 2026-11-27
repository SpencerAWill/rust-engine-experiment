//! The `heavy_compute` workload on the archetype `World`.

use engine::prelude::Prng;
use spike_ecs_harness::heavy_compute::{
    self as rules, HeavyCompute, Matrix, Position, Rotation, Velocity,
};
use spike_ecs_harness::{Params, Workload};

use crate::{QueryMut, World};

/// `heavy_compute` on an archetype [`World`]: one table holding every entity.
#[derive(Debug)]
pub struct ArchetypeHeavyCompute {
    world: World,
}

/// The step's query, shared with the test hook so the two cannot drift.
fn step_query(
    world: &mut World,
) -> QueryMut<'_, (&mut Matrix, &mut Position, &Rotation, &Velocity)> {
    world.query_mut()
}

#[cfg(test)]
impl ArchetypeHeavyCompute {
    /// The archetypes the step's query matches, for tests.
    pub(crate) fn step_archetypes(&mut self) -> Vec<crate::archetype::ArchetypeId> {
        step_query(&mut self.world).matched_archetypes()
    }

    /// The number of archetypes in the world, for tests.
    pub(crate) fn archetype_count(&self) -> usize {
        self.world.archetype_count()
    }
}

impl Workload for ArchetypeHeavyCompute {
    type Spec = HeavyCompute;
    const BACKEND: &'static str = "archetype";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            world.spawn_with(rules::spawn(&prng, i));
        }
        Self { world }
    }

    fn step(&mut self) {
        step_query(&mut self.world).for_each(|(matrix, position, rotation, velocity)| {
            rules::update(matrix, position, rotation, velocity);
        });
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        self.world
            .query::<(&Position, &Matrix)>()
            .for_each(|(position, matrix)| {
                sum = sum.wrapping_add(rules::hash(position, matrix));
            });
        sum
    }
}
