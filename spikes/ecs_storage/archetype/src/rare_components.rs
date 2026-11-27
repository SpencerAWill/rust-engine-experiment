//! The `rare_components` workload on the archetype `World`.

use engine::prelude::Prng;
use spike_ecs_harness::rare_components::{self as rules, Common, Rare, RareComponents};
use spike_ecs_harness::{Params, Workload};

use crate::{QueryMut, World};

/// `rare_components` on an archetype [`World`]: the rare entities live in a `{Common, Rare}` table
/// of their own, so the step's query walks only that table and never sees the others' rows.
#[derive(Debug)]
pub struct ArchetypeRareComponents {
    world: World,
}

/// The step's query, shared with the test hook so the two cannot drift.
fn step_query(world: &mut World) -> QueryMut<'_, (&mut Common, &Rare)> {
    world.query_mut()
}

#[cfg(test)]
impl ArchetypeRareComponents {
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

impl Workload for ArchetypeRareComponents {
    type Spec = RareComponents;
    const BACKEND: &'static str = "archetype";

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
