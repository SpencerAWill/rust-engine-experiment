//! The `entity_churn` workload on a `hecs` `World`.

use engine::prelude::Prng;
use hecs::{CommandBuffer, Entity, World};
use spike_ecs_harness::entity_churn::{
    self as rules, EntityChurn, Lifetime, Position, SpawnId, Velocity,
};
use spike_ecs_harness::{Params, Workload};

/// `entity_churn` on a `hecs` [`World`]: expired entities are despawned through a command buffer
/// and replaced with `spawn` until the target count is alive again.
pub struct HecsEntityChurn {
    world: World,
    commands: CommandBuffer,
    prng: Prng,
    next_id: u64,
    count: usize,
}

impl HecsEntityChurn {
    /// Spawns entities until `count` are alive.
    fn respawn(&mut self) {
        while (self.world.len() as usize) < self.count {
            let bundle = rules::spawn(&self.prng, self.next_id);
            self.next_id += 1;
            self.world.spawn(bundle);
        }
    }
}

impl core::fmt::Debug for HecsEntityChurn {
    /// `hecs::World` has no `Debug`, so this shows the entity count only.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HecsEntityChurn")
            .field("entities", &self.world.len())
            .finish_non_exhaustive()
    }
}

impl Workload for HecsEntityChurn {
    type Spec = EntityChurn;
    const BACKEND: &'static str = "hecs";

    fn setup(params: Params) -> Self {
        let mut w = Self {
            world: World::new(),
            commands: CommandBuffer::new(),
            prng: Prng::from_seed(params.seed),
            next_id: 0,
            count: params.count,
        };
        w.respawn();
        w
    }

    fn step(&mut self) {
        let commands = &mut self.commands;
        self.world
            .query_mut::<(Entity, &mut Position, &Velocity, &mut Lifetime)>()
            .into_iter()
            .for_each(|(e, position, velocity, lifetime)| {
                rules::update(position, velocity, lifetime);
                if rules::is_expired(lifetime) {
                    commands.despawn(e);
                }
            });
        self.commands.run_on(&mut self.world);
        self.respawn();
    }

    fn checksum(&self) -> u64 {
        self.world
            .query::<(&SpawnId, &Position, &Velocity, &Lifetime)>()
            .iter()
            .fold(0_u64, |sum, (id, position, velocity, lifetime)| {
                sum.wrapping_add(rules::hash(id, position, velocity, lifetime))
            })
    }
}
