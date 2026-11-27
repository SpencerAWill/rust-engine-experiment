//! The `rare_components` workload on a `hecs` `World`.
//!
//! An entity is spawned from a one- or two-element tuple bundle, so the rare entities share an
//! archetype of their own and the step's query matches only that one.

use engine::prelude::Prng;
use hecs::World;
use spike_ecs_harness::rare_components::{self as rules, Common, Rare, RareComponents};
use spike_ecs_harness::{Params, Workload};

/// `rare_components` on a `hecs` [`World`]: the step walks only the entities that hold a
/// `Rare`.
pub struct HecsRareComponents {
    world: World,
}

impl core::fmt::Debug for HecsRareComponents {
    /// `hecs::World` has no `Debug`, so this shows the entity count only.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HecsRareComponents")
            .field("entities", &self.world.len())
            .finish_non_exhaustive()
    }
}

impl Workload for HecsRareComponents {
    type Spec = RareComponents;
    const BACKEND: &'static str = "hecs";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            match rules::spawn(&prng, i) {
                (common, None) => world.spawn((common,)),
                (common, Some(rare)) => world.spawn((common, rare)),
            };
        }
        Self { world }
    }

    fn step(&mut self) {
        self.world
            .query_mut::<(&mut Common, &Rare)>()
            .into_iter()
            .for_each(|(common, rare)| rules::update(common, rare));
    }

    fn checksum(&self) -> u64 {
        self.world
            .query::<(&Common, Option<&Rare>)>()
            .iter()
            .fold(0_u64, |sum, (common, rare)| {
                sum.wrapping_add(rules::hash(common, rare))
            })
    }
}
