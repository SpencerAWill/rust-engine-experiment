//! The `entity_churn` workload on the dense `World`.

use engine::prelude::Prng;
use spike_ecs_harness::entity_churn::{
    self as rules, EntityChurn, Lifetime, Position, SpawnId, Velocity,
};
use spike_ecs_harness::{Params, Workload};

use crate::{Commands, Entity, QueryMut, World};

/// `entity_churn` on a dense-array [`World`].
#[derive(Debug)]
pub struct DenseEntityChurn {
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
impl DenseEntityChurn {
    /// The world, for tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }

    /// The sets the step's query joins (required, excluded), for tests.
    pub(crate) fn step_joined(&mut self) -> crate::query::JoinedSets {
        step_query(&mut self.world).joined()
    }
}

impl DenseEntityChurn {
    /// Spawns, with one insert per component, until `count` entities are alive.
    fn respawn(&mut self) {
        while self.world.len() < self.count {
            let (id, position, velocity, lifetime) = rules::spawn(&self.prng, self.next_id);
            self.next_id += 1;
            let e = self.world.spawn();
            self.world.insert::<SpawnId>(e, id);
            self.world.insert::<Position>(e, position);
            self.world.insert::<Velocity>(e, velocity);
            self.world.insert::<Lifetime>(e, lifetime);
        }
    }
}

impl Workload for DenseEntityChurn {
    type Spec = EntityChurn;
    const BACKEND: &'static str = "dense";

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
