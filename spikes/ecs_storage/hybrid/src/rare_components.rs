//! The `rare_components` workload on the hybrid `World`.

use engine::prelude::Prng;
use spike_ecs_harness::rare_components::{self as rules, Common, Rare, RareComponents};
use spike_ecs_harness::{Params, Workload};

use crate::{QueryMut, World};

/// `rare_components` on a hybrid [`World`]: the rare entities live in a `{Common, Rare}` table
/// of their own (both types are table types), so the step's query walks only that table and never
/// sees the others' rows.
#[derive(Debug)]
pub struct HybridRareComponents {
    world: World,
}

/// The step's query, shared with the test hook so the two cannot drift.
fn step_query(world: &mut World) -> QueryMut<'_, (&mut Common, &Rare)> {
    world.query_mut()
}

#[cfg(test)]
impl HybridRareComponents {
    /// The archetypes the step's query matches, for tests.
    pub(crate) fn step_archetypes(&mut self) -> Vec<crate::archetype::ArchetypeId> {
        step_query(&mut self.world).matched_archetypes()
    }

    /// The number of archetypes in the world, the empty one included, for tests.
    pub(crate) fn archetype_count(&self) -> usize {
        self.world.archetype_count()
    }

    /// The world, for invariant checks in tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }
}

impl Workload for HybridRareComponents {
    type Spec = RareComponents;
    const BACKEND: &'static str = "hybrid";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            match rules::spawn(&prng, i) {
                (common, None) => world.spawn_with((common,)),
                (common, Some(rare)) => world.spawn_with((common, rare)),
            };
        }
        Self { world }
    }

    fn step(&mut self) {
        step_query(&mut self.world).for_each(|(common, rare)| rules::update(common, rare));
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        self.world
            .query::<(&Common, Option<&Rare>)>()
            .for_each(|(common, rare)| sum = sum.wrapping_add(rules::hash(common, rare)));
        sum
    }
}
