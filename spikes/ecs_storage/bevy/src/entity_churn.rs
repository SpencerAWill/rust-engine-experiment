//! The `entity_churn` workload on a `bevy_ecs` `World`.

use bevy_ecs::entity::Entity;
use bevy_ecs::query::QueryState;
use bevy_ecs::world::World;
use engine::prelude::Prng;
use spike_ecs_harness::entity_churn::{
    self as rules, EntityChurn, Lifetime, Position, SpawnId, Velocity,
};
use spike_ecs_harness::{Params, Workload};

use crate::Table;

/// The step's query.
type StepQuery = QueryState<(
    Entity,
    &'static mut Table<Position>,
    &'static Table<Velocity>,
    &'static mut Table<Lifetime>,
)>;

/// `entity_churn` on a `bevy_ecs` [`World`]: expired entities are collected during the query,
/// despawned after it, and replaced with `spawn` until the target count is alive again.
///
/// The count is the backend's own, because a bevy `World` holds an entity of its own.
#[derive(Debug)]
pub struct BevyEntityChurn {
    world: World,
    step: StepQuery,
    expired: Vec<Entity>,
    prng: Prng,
    next_id: u64,
    alive: usize,
    count: usize,
}

#[cfg(test)]
impl BevyEntityChurn {
    /// The world, for tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }
}

impl BevyEntityChurn {
    /// Spawns entities until `count` are alive.
    fn respawn(&mut self) {
        while self.alive < self.count {
            let (id, position, velocity, lifetime) = rules::spawn(&self.prng, self.next_id);
            self.next_id += 1;
            self.world
                .spawn((Table(id), Table(position), Table(velocity), Table(lifetime)));
            self.alive += 1;
        }
    }
}

impl Workload for BevyEntityChurn {
    type Spec = EntityChurn;
    const BACKEND: &'static str = "bevy_ecs";

    fn setup(params: Params) -> Self {
        let mut world = World::new();
        let step = world.query();
        let mut w = Self {
            world,
            step,
            expired: Vec::with_capacity(params.count),
            prng: Prng::from_seed(params.seed),
            next_id: 0,
            alive: 0,
            count: params.count,
        };
        w.respawn();
        w
    }

    fn step(&mut self) {
        self.expired.clear();
        let expired = &mut self.expired;
        for (e, mut position, velocity, mut lifetime) in self.step.iter_mut(&mut self.world) {
            rules::update(&mut position.0, &velocity.0, &mut lifetime.0);
            if rules::is_expired(&lifetime.0) {
                expired.push(e);
            }
        }
        for &e in &self.expired {
            self.world.despawn(e);
        }
        self.alive -= self.expired.len();
        self.respawn();
        self.world.clear_trackers();
    }

    fn checksum(&self) -> u64 {
        let Some(query) = self.world.try_query::<(
            &Table<SpawnId>,
            &Table<Position>,
            &Table<Velocity>,
            &Table<Lifetime>,
        )>() else {
            return 0;
        };
        query
            .iter_manual(&self.world)
            .fold(0_u64, |sum, (id, position, velocity, lifetime)| {
                sum.wrapping_add(rules::hash(&id.0, &position.0, &velocity.0, &lifetime.0))
            })
    }
}
