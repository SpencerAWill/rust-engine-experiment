//! The `entity_churn` workload on the hybrid `World`.

use engine::prelude::Prng;
use spike_ecs_harness::entity_churn::{
    self as rules, EntityChurn, Lifetime, Position, SpawnId, Velocity,
};
use spike_ecs_harness::{Params, Workload};

use crate::{Commands, Entity, QueryMut, World};

/// `entity_churn` on the hybrid [`World`]: expired entities are despawned through a command
/// buffer and replaced with `spawn_with` until the target count is alive again.
#[derive(Debug)]
pub struct HybridEntityChurn {
    world: World,
    commands: Commands,
    prng: Prng,
    next_id: u64,
    count: usize,
}

/// The step's query, shared with the test hook so the two cannot drift.
fn step_query(
    world: &mut World,
) -> QueryMut<'_, (Entity, (&mut Position, &Velocity, &mut Lifetime))> {
    world.query_mut()
}

#[cfg(test)]
impl HybridEntityChurn {
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

    /// The world, for invariant checks in tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }
}

impl HybridEntityChurn {
    /// Spawns bundles until `count` entities are alive.
    fn respawn(&mut self) {
        while self.world.len() < self.count {
            let bundle = rules::spawn(&self.prng, self.next_id);
            self.next_id += 1;
            self.world.spawn_with(bundle);
        }
    }
}

impl Workload for HybridEntityChurn {
    type Spec = EntityChurn;
    const BACKEND: &'static str = "hybrid";

    fn setup(params: Params) -> Self {
        let mut w = Self {
            world: World::new(),
            commands: Commands::new(),
            prng: Prng::from_seed(params.seed),
            next_id: 0,
            count: params.count,
        };
        w.respawn();
        w
    }

    fn step(&mut self) {
        let commands = &mut self.commands;
        step_query(&mut self.world).for_each(|(e, (position, velocity, lifetime))| {
            rules::update(position, velocity, lifetime);
            if rules::is_expired(lifetime) {
                commands.despawn(e);
            }
        });
        self.world.apply(&mut self.commands);
        self.respawn();
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        self.world
            .query::<(&SpawnId, &Position, &Velocity, &Lifetime)>()
            .for_each(|(id, position, velocity, lifetime)| {
                sum = sum.wrapping_add(rules::hash(id, position, velocity, lifetime));
            });
        sum
    }
}
