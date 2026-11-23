//! The `agents` workload on a `bevy_ecs` `World`.

use bevy_ecs::entity::Entity;
use bevy_ecs::query::QueryState;
use bevy_ecs::world::World;
use engine::prelude::{Prng, Transform2d, Vec2};
use spike_ecs_harness::agents::{self as rules, Agent, Agents, Input, Seeker};
use spike_ecs_harness::{Params, Workload, dt};

use crate::{Sparse, Table};

/// An agent's spawn counter. A newtype because the harness uses a bare `u64`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentId(pub u64);

/// An agent's velocity. A newtype because the harness uses a bare `Vec2`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Velocity(pub Vec2);

/// Ticks an agent has left. A newtype because the harness uses a bare `u32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ttl(pub u32);

/// The step's query.
type StepQuery = QueryState<(
    Entity,
    &'static mut Table<Transform2d>,
    &'static mut Table<Velocity>,
    &'static mut Table<Ttl>,
    Option<&'static mut Table<Seeker>>,
)>;

/// A structural change decided inside the query and applied after it.
#[derive(Clone, Copy, Debug)]
enum Change {
    /// Remove `Seeker` from the entity.
    RemoveSeeker(Entity),
    /// Insert `Seeker` on the entity.
    InsertSeeker(Entity, Seeker),
    /// Despawn the entity.
    Despawn(Entity),
}

/// `agents` on a `bevy_ecs` [`World`].
///
/// Agents spawn into the 4- or 5-element archetype (without or with `Seeker`). `Seeker` comes
/// and goes while an agent lives, so changes are recorded in a reused `Vec` during the step and
/// applied, in recorded order, after the iteration.
#[derive(Debug)]
pub struct BevyAgents {
    world: World,
    step: StepQuery,
    changes: Vec<Change>,
    prng: Prng,
    next_id: u64,
    tick: u64,
    alive: usize,
    target: usize,
    dt: f32,
    /// Seeker removals applied so far, for tests.
    #[cfg(test)]
    seeker_removals: usize,
}

#[cfg(test)]
impl BevyAgents {
    /// The number of agents spawned so far, for tests.
    pub(crate) fn spawned(&self) -> u64 {
        self.next_id
    }

    /// The number of `Seeker` removals applied so far, for tests.
    pub(crate) fn seeker_removals(&self) -> usize {
        self.seeker_removals
    }

    /// The world, for tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }
}

impl BevyAgents {
    /// Spawns agents until `target` are alive.
    fn respawn(&mut self) {
        while self.alive < self.target {
            let agent = rules::spawn_agent(&self.prng, self.next_id);
            self.next_id += 1;
            let (id, transform, velocity, ttl) = (
                Table(AgentId(agent.id)),
                Table(agent.transform),
                Table(Velocity(agent.velocity)),
                Table(Ttl(agent.ttl)),
            );
            match agent.seeker {
                Some(seeker) => {
                    self.world
                        .spawn((id, transform, velocity, ttl, Table(seeker)));
                }
                None => {
                    self.world.spawn((id, transform, velocity, ttl));
                }
            }
            self.alive += 1;
        }
    }
}

impl Workload for BevyAgents {
    type Spec = Agents;
    const BACKEND: &'static str = "bevy_ecs";

    fn setup(params: Params) -> Self {
        let mut world = World::new();
        let step = world.query();
        let mut w = Self {
            world,
            step,
            changes: Vec::with_capacity(params.count),
            prng: Prng::from_seed(params.seed),
            next_id: 0,
            tick: 0,
            alive: 0,
            target: params.count,
            dt: dt(),
            #[cfg(test)]
            seeker_removals: 0,
        };
        w.respawn();
        w
    }

    fn step(&mut self) {
        let cursor = Input::scripted(self.tick).cursor;
        let dt = self.dt;
        self.changes.clear();
        let changes = &mut self.changes;
        self.step.iter_mut(&mut self.world).for_each(
            |(e, mut transform, mut velocity, mut ttl, seeker)| {
                let before = seeker.as_deref().map(|s| s.0);
                let mut local = before;
                rules::update(
                    &mut transform.0,
                    &mut velocity.0.0,
                    &mut ttl.0.0,
                    &mut local,
                    cursor,
                    dt,
                );
                // The seeker change is recorded before the despawn, so an agent caught and
                // expired in the same tick has its removal applied first and the despawn last.
                match (before, local, seeker) {
                    (Some(_), Some(new), Some(mut slot)) => slot.0 = new,
                    (Some(_), None, _) => changes.push(Change::RemoveSeeker(e)),
                    (None, Some(new), _) => changes.push(Change::InsertSeeker(e, new)),
                    _ => {}
                }
                if ttl.0.0 == 0 {
                    changes.push(Change::Despawn(e));
                }
            },
        );
        for &change in &self.changes {
            match change {
                Change::RemoveSeeker(e) => {
                    self.world.entity_mut(e).remove::<Table<Seeker>>();
                    #[cfg(test)]
                    {
                        self.seeker_removals += 1;
                    }
                }
                Change::InsertSeeker(e, seeker) => {
                    self.world.entity_mut(e).insert(Table(seeker));
                }
                Change::Despawn(e) => {
                    self.world.despawn(e);
                    self.alive -= 1;
                }
            }
        }
        self.respawn();
        self.tick += 1;
        self.world.clear_trackers();
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        if let Some(query) = self.world.try_query::<(
            &Table<AgentId>,
            &Table<Transform2d>,
            &Table<Velocity>,
            &Table<Ttl>,
            Option<&Table<Seeker>>,
        )>() {
            for (id, transform, velocity, ttl, seeker) in query.iter_manual(&self.world) {
                let agent = Agent {
                    id: id.0.0,
                    transform: transform.0,
                    velocity: velocity.0.0,
                    ttl: ttl.0.0,
                    seeker: seeker.map(|s| s.0),
                };
                sum = sum.wrapping_add(rules::agent_hash(&agent));
            }
        }
        sum
    }
}

/// The step's query of the sparse variant.
type SparseStepQuery = QueryState<(
    Entity,
    &'static mut Table<Transform2d>,
    &'static mut Table<Velocity>,
    &'static mut Table<Ttl>,
    Option<&'static mut Sparse<Seeker>>,
)>;

/// `agents` on a `bevy_ecs` [`World`] with `Seeker` in `SparseSet` storage.
///
/// Agents spawn into the 4- or 5-element archetype (without or with `Seeker`). `Seeker` comes
/// and goes while an agent lives, so changes are recorded in a reused `Vec` during the step and
/// applied, in recorded order, after the iteration.
#[derive(Debug)]
pub struct BevySparseAgents {
    world: World,
    step: SparseStepQuery,
    changes: Vec<Change>,
    prng: Prng,
    next_id: u64,
    tick: u64,
    alive: usize,
    target: usize,
    dt: f32,
    /// Seeker removals applied so far, for tests.
    #[cfg(test)]
    seeker_removals: usize,
}

#[cfg(test)]
impl BevySparseAgents {
    /// The number of agents spawned so far, for tests.
    pub(crate) fn spawned(&self) -> u64 {
        self.next_id
    }

    /// The number of `Seeker` removals applied so far, for tests.
    pub(crate) fn seeker_removals(&self) -> usize {
        self.seeker_removals
    }

    /// The world, for tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }
}

impl BevySparseAgents {
    /// Spawns agents until `target` are alive.
    fn respawn(&mut self) {
        while self.alive < self.target {
            let agent = rules::spawn_agent(&self.prng, self.next_id);
            self.next_id += 1;
            let (id, transform, velocity, ttl) = (
                Table(AgentId(agent.id)),
                Table(agent.transform),
                Table(Velocity(agent.velocity)),
                Table(Ttl(agent.ttl)),
            );
            match agent.seeker {
                Some(seeker) => {
                    self.world
                        .spawn((id, transform, velocity, ttl, Sparse(seeker)));
                }
                None => {
                    self.world.spawn((id, transform, velocity, ttl));
                }
            }
            self.alive += 1;
        }
    }
}

impl Workload for BevySparseAgents {
    type Spec = Agents;
    const BACKEND: &'static str = "bevy_ecs_sparse";

    fn setup(params: Params) -> Self {
        let mut world = World::new();
        let step = world.query();
        let mut w = Self {
            world,
            step,
            changes: Vec::with_capacity(params.count),
            prng: Prng::from_seed(params.seed),
            next_id: 0,
            tick: 0,
            alive: 0,
            target: params.count,
            dt: dt(),
            #[cfg(test)]
            seeker_removals: 0,
        };
        w.respawn();
        w
    }

    fn step(&mut self) {
        let cursor = Input::scripted(self.tick).cursor;
        let dt = self.dt;
        self.changes.clear();
        let changes = &mut self.changes;
        self.step.iter_mut(&mut self.world).for_each(
            |(e, mut transform, mut velocity, mut ttl, seeker)| {
                let before = seeker.as_deref().map(|s| s.0);
                let mut local = before;
                rules::update(
                    &mut transform.0,
                    &mut velocity.0.0,
                    &mut ttl.0.0,
                    &mut local,
                    cursor,
                    dt,
                );
                // The seeker change is recorded before the despawn, so an agent caught and
                // expired in the same tick has its removal applied first and the despawn last.
                match (before, local, seeker) {
                    (Some(_), Some(new), Some(mut slot)) => slot.0 = new,
                    (Some(_), None, _) => changes.push(Change::RemoveSeeker(e)),
                    (None, Some(new), _) => changes.push(Change::InsertSeeker(e, new)),
                    _ => {}
                }
                if ttl.0.0 == 0 {
                    changes.push(Change::Despawn(e));
                }
            },
        );
        for &change in &self.changes {
            match change {
                Change::RemoveSeeker(e) => {
                    self.world.entity_mut(e).remove::<Sparse<Seeker>>();
                    #[cfg(test)]
                    {
                        self.seeker_removals += 1;
                    }
                }
                Change::InsertSeeker(e, seeker) => {
                    self.world.entity_mut(e).insert(Sparse(seeker));
                }
                Change::Despawn(e) => {
                    self.world.despawn(e);
                    self.alive -= 1;
                }
            }
        }
        self.respawn();
        self.tick += 1;
        self.world.clear_trackers();
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        if let Some(query) = self.world.try_query::<(
            &Table<AgentId>,
            &Table<Transform2d>,
            &Table<Velocity>,
            &Table<Ttl>,
            Option<&Sparse<Seeker>>,
        )>() {
            for (id, transform, velocity, ttl, seeker) in query.iter_manual(&self.world) {
                let agent = Agent {
                    id: id.0.0,
                    transform: transform.0,
                    velocity: velocity.0.0,
                    ttl: ttl.0.0,
                    seeker: seeker.map(|s| s.0),
                };
                sum = sum.wrapping_add(rules::agent_hash(&agent));
            }
        }
        sum
    }
}
