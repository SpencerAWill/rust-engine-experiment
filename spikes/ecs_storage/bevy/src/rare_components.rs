//! The `rare_components` workload on a `bevy_ecs` `World`.
//!
//! Both types are in table storage. An entity is spawned from a one- or two-element tuple
//! bundle, so the rare entities share an archetype of their own and the step's query matches
//! only that one.

use bevy_ecs::query::QueryState;
use bevy_ecs::world::World;
use engine::prelude::Prng;
use spike_ecs_harness::rare_components::{self as rules, Common, Rare, RareComponents};
use spike_ecs_harness::{Params, Workload};

use crate::Table;

/// The step's query.
type StepQuery = QueryState<(&'static mut Table<Common>, &'static Table<Rare>)>;

/// `rare_components` on a `bevy_ecs` [`World`]: the step walks only the entities that hold a
/// `Rare`.
#[derive(Debug)]
pub struct BevyRareComponents {
    world: World,
    step_query: StepQuery,
}

#[cfg(test)]
impl BevyRareComponents {
    /// The world, for tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }
}

impl Workload for BevyRareComponents {
    type Spec = RareComponents;
    const BACKEND: &'static str = "bevy_ecs";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            match rules::spawn(&prng, i) {
                (common, None) => world.spawn((Table(common),)),
                (common, Some(rare)) => world.spawn((Table(common), Table(rare))),
            };
        }
        let step_query = world.query();
        Self { world, step_query }
    }

    fn step(&mut self) {
        self.step_query
            .iter_mut(&mut self.world)
            .for_each(|(mut common, rare)| rules::update(&mut common.0, &rare.0));
        self.world.clear_trackers();
    }

    fn checksum(&self) -> u64 {
        let Some(query) = self
            .world
            .try_query::<(&Table<Common>, Option<&Table<Rare>>)>()
        else {
            return 0;
        };
        query
            .iter_manual(&self.world)
            .fold(0_u64, |sum, (common, rare)| {
                sum.wrapping_add(rules::hash(&common.0, rare.map(|r| &r.0)))
            })
    }
}
