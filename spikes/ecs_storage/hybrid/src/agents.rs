//! The `agents` workload on the hybrid `World`.

use engine::prelude::{Prng, Transform2d, Vec2};
use spike_ecs_harness::agents::{self as rules, Agent, Agents, Input, Seeker};
use spike_ecs_harness::{Params, Workload, dt};

use crate::component::components;
use crate::{Commands, Entity, QueryMut, World};

/// An agent's spawn counter. A newtype because the harness uses a bare `u64`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentId(pub u64);

/// An agent's velocity. A newtype because the harness uses a bare `Vec2`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Velocity(pub Vec2);

/// Ticks an agent has left. A newtype because the harness uses a bare `u32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ttl(pub u32);

components!(TableStorage: AgentId, Velocity, Ttl);

/// `agents` on the hybrid [`World`].
///
/// Agents spawn as a 4- or 5-element bundle (without or with `Seeker`), whose `Seeker` goes to
/// its sparse set, so all agents share one table. `Seeker` comes and goes while an agent
/// lives, so changes are recorded in a [`Commands`] buffer during the step and applied with
/// the despawns; none moves a row.
#[derive(Debug)]
pub struct HybridAgents {
    world: World,
    commands: Commands,
    prng: Prng,
    next_id: u64,
    tick: u64,
    target: usize,
    dt: f32,
}

/// The step query's parameters.
type StepParams<'a> = (
    Entity,
    (
        &'a mut Transform2d,
        &'a mut Velocity,
        &'a mut Ttl,
        Option<&'a mut Seeker>,
    ),
);

/// The step's query, shared with the test hook so the two cannot drift.
fn step_query(world: &mut World) -> QueryMut<'_, StepParams<'_>> {
    world.query_mut()
}

#[cfg(test)]
impl HybridAgents {
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

    /// The number of archetypes in the world, for tests.
    pub(crate) fn archetype_count(&self) -> usize {
        self.world.archetype_count()
    }

    /// The world, for invariant checks in tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }
}

impl HybridAgents {
    /// Spawns bundles until `target` entities are alive.
    fn respawn(&mut self) {
        while self.world.len() < self.target {
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
                    self.world
                        .spawn_with((id, transform, velocity, ttl, seeker));
                }
                None => {
                    self.world.spawn_with((id, transform, velocity, ttl));
                }
            }
        }
    }
}

impl Workload for HybridAgents {
    type Spec = Agents;
    const BACKEND: &'static str = "hybrid";

    fn setup(params: Params) -> Self {
        let mut w = Self {
            world: World::new(),
            commands: Commands::new(),
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
        let commands = &mut self.commands;
        step_query(&mut self.world).for_each(|(e, (transform, velocity, ttl, seeker))| {
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
            // expired in the same tick has its `remove` run first and the despawn last.
            match (before, local, seeker) {
                (Some(_), Some(new), Some(slot)) => *slot = new,
                (Some(_), None, _) => commands.remove::<Seeker>(e),
                (None, Some(new), _) => commands.insert(e, new),
                _ => {}
            }
            if ttl.0 == 0 {
                commands.despawn(e);
            }
        });
        self.world.apply(&mut self.commands);
        self.respawn();
        self.tick += 1;
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        self.world
            .query::<(&AgentId, &Transform2d, (&Velocity, &Ttl, Option<&Seeker>))>()
            .for_each(|(id, transform, (velocity, ttl, seeker))| {
                let agent = Agent {
                    id: id.0,
                    transform: *transform,
                    velocity: velocity.0,
                    ttl: ttl.0,
                    seeker: seeker.copied(),
                };
                sum = sum.wrapping_add(rules::agent_hash(&agent));
            });
        sum
    }
}
