//! The `agents` workload on a `hecs` `World`.

use engine::prelude::{Prng, Transform2d, Vec2};
use hecs::{Entity, World};
use spike_ecs_harness::agents::{self as rules, Agent, Agents, Input, Seeker};
use spike_ecs_harness::{Params, Workload, dt};

/// An agent's spawn counter. A newtype because the harness uses a bare `u64`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentId(pub u64);

/// An agent's velocity. A newtype because the harness uses a bare `Vec2`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Velocity(pub Vec2);

/// Ticks an agent has left. A newtype because the harness uses a bare `u32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ttl(pub u32);

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

/// `agents` on a `hecs` [`World`].
///
/// Agents spawn into the 4- or 5-element archetype (without or with `Seeker`). `Seeker` comes
/// and goes while an agent lives, so changes are recorded in a reused `Vec` during the step and
/// applied through `World`, in recorded order, after the iteration.
pub struct HecsAgents {
    world: World,
    changes: Vec<Change>,
    prng: Prng,
    next_id: u64,
    tick: u64,
    target: usize,
    dt: f32,
}

impl HecsAgents {
    /// Spawns agents until `target` are alive.
    fn respawn(&mut self) {
        while (self.world.len() as usize) < self.target {
            let agent = rules::spawn_agent(&self.prng, self.next_id);
            self.next_id += 1;
            let (id, transform, velocity, ttl) = (
                AgentId(agent.id),
                agent.transform,
                Velocity(agent.velocity),
                Ttl(agent.ttl),
            );
            match agent.seeker {
                Some(seeker) => {
                    self.world.spawn((id, transform, velocity, ttl, seeker));
                }
                None => {
                    self.world.spawn((id, transform, velocity, ttl));
                }
            }
        }
    }
}

impl core::fmt::Debug for HecsAgents {
    /// `hecs::World` has no `Debug`, so this shows the entity count only.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HecsAgents")
            .field("entities", &self.world.len())
            .finish_non_exhaustive()
    }
}

impl Workload for HecsAgents {
    type Spec = Agents;
    const BACKEND: &'static str = "hecs";

    fn setup(params: Params) -> Self {
        let mut w = Self {
            world: World::new(),
            changes: Vec::with_capacity(params.count),
            prng: Prng::from_seed(params.seed),
            next_id: 0,
            tick: 0,
            target: params.count,
            dt: dt(),
        };
        w.respawn();
        w
    }

    fn step(&mut self) {
        let cursor = Input::scripted(self.tick).cursor;
        let dt = self.dt;
        self.changes.clear();
        let changes = &mut self.changes;
        self.world
            .query_mut::<(
                Entity,
                &mut Transform2d,
                &mut Velocity,
                &mut Ttl,
                Option<&mut Seeker>,
            )>()
            .into_iter()
            .for_each(|(e, transform, velocity, ttl, seeker)| {
                let before = seeker.as_deref().copied();
                let mut local = before;
                rules::update(
                    transform,
                    &mut velocity.0,
                    &mut ttl.0,
                    &mut local,
                    cursor,
                    dt,
                );
                // The seeker change is recorded before the despawn, so an agent caught and
                // expired in the same tick has its removal applied first and the despawn last.
                match (before, local, seeker) {
                    (Some(_), Some(new), Some(slot)) => *slot = new,
                    (Some(_), None, _) => changes.push(Change::RemoveSeeker(e)),
                    (None, Some(new), _) => changes.push(Change::InsertSeeker(e, new)),
                    _ => {}
                }
                if ttl.0 == 0 {
                    changes.push(Change::Despawn(e));
                }
            });
        for &change in &self.changes {
            match change {
                Change::RemoveSeeker(e) => {
                    let _ = self.world.remove_one::<Seeker>(e);
                }
                Change::InsertSeeker(e, seeker) => {
                    let _ = self.world.insert_one(e, seeker);
                }
                Change::Despawn(e) => {
                    let _ = self.world.despawn(e);
                }
            }
        }
        self.respawn();
        self.tick += 1;
    }

    fn checksum(&self) -> u64 {
        self.world
            .query::<(&AgentId, &Transform2d, &Velocity, &Ttl, Option<&Seeker>)>()
            .iter()
            .fold(0_u64, |sum, (id, transform, velocity, ttl, seeker)| {
                let agent = Agent {
                    id: id.0,
                    transform: *transform,
                    velocity: velocity.0,
                    ttl: ttl.0,
                    seeker: seeker.copied(),
                };
                sum.wrapping_add(rules::agent_hash(&agent))
            })
    }
}
