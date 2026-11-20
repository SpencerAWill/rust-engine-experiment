//! The `entity_churn` workload on the sparse-set `World`, ungrouped and with an owning group.

use engine::prelude::Prng;
use spike_ecs_harness::entity_churn::{
    self as rules, EntityChurn, Lifetime, Position, SpawnId, Velocity,
};
use spike_ecs_harness::{Params, Workload};

use crate::{Commands, Entity, World};

/// `entity_churn` on a [`World`]; `GROUPED` declares the `(Position, Velocity, Lifetime)` group.
#[derive(Debug)]
pub struct EntityChurnOn<const GROUPED: bool> {
    world: World,
    commands: Commands,
    prng: Prng,
    next_id: u64,
    count: usize,
}

/// `entity_churn` on a sparse-set `World` without groups.
pub type SparseSetEntityChurn = EntityChurnOn<false>;
/// `entity_churn` on a sparse-set `World` with the `(Position, Velocity, Lifetime)` group.
pub type GroupedEntityChurn = EntityChurnOn<true>;

#[cfg(test)]
impl<const GROUPED: bool> EntityChurnOn<GROUPED> {
    /// Whether the step's query takes the group path, for tests.
    pub(crate) fn uses_group(&self) -> bool {
        self.world
            .query::<(Entity, (&Position, &Velocity, &Lifetime))>()
            .uses_group()
    }

    /// The world, for invariant checks in tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }
}

impl<const GROUPED: bool> EntityChurnOn<GROUPED> {
    /// Spawns with direct inserts until `count` entities are alive.
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

impl<const GROUPED: bool> Workload for EntityChurnOn<GROUPED> {
    type Spec = EntityChurn;
    const BACKEND: &'static str = if GROUPED {
        "sparse_set_grouped"
    } else {
        "sparse_set"
    };

    fn setup(params: Params) -> Self {
        let world = if GROUPED {
            World::builder()
                .group::<(Position, Velocity, Lifetime)>()
                .build()
        } else {
            World::new()
        };
        let mut w = Self {
            world,
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
        self.world
            .query_mut::<(Entity, (&mut Position, &Velocity, &mut Lifetime))>()
            .for_each(|(e, (position, velocity, lifetime))| {
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
