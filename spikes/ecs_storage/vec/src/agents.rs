//! The `agents` workload: `agents_2d::Sim` as the array-of-structs backend, and a
//! struct-of-arrays variant.

use agents_2d::{Sim, SimConfig};
use engine::prelude::{Prng, Transform2d, Vec2};
use spike_ecs_harness::agents::{self as rules, Agent, Agents, Input, Seeker};
use spike_ecs_harness::{Params, Workload, dt};

/// `agents` as `agents_2d::Sim`: one `Vec<Agent>`, the M3.0 baseline.
#[derive(Clone, Debug)]
pub struct VecAgents(Sim);

impl Workload for VecAgents {
    type Spec = Agents;
    const BACKEND: &'static str = "vec_aos";

    fn setup(params: Params) -> Self {
        Self(Sim::new(&SimConfig {
            agents: params.count,
            seed: params.seed,
        }))
    }

    fn step(&mut self) {
        let input = Input::scripted(self.0.tick());
        self.0.step(&input);
    }

    fn checksum(&self) -> u64 {
        self.0.checksum()
    }
}

/// `agents` as one `Vec` per field. Applies M3.0 §4.3 itself through [`rules::update`].
#[derive(Clone, Debug)]
pub struct SoaAgents {
    pub(crate) ids: Vec<u64>,
    pub(crate) transforms: Vec<Transform2d>,
    pub(crate) velocities: Vec<Vec2>,
    pub(crate) ttls: Vec<u32>,
    pub(crate) seekers: Vec<Option<Seeker>>,
    prng: Prng,
    next_id: u64,
    tick: u64,
    target: usize,
    dt: f32,
}

impl SoaAgents {
    fn spawn_next(&mut self) {
        let agent = rules::spawn_agent(&self.prng, self.next_id);
        self.next_id += 1;
        self.ids.push(agent.id);
        self.transforms.push(agent.transform);
        self.velocities.push(agent.velocity);
        self.ttls.push(agent.ttl);
        self.seekers.push(agent.seeker);
    }

    /// Removes agent `i` from every column with the same operation.
    fn remove(&mut self, i: usize) {
        self.ids.swap_remove(i);
        self.transforms.swap_remove(i);
        self.velocities.swap_remove(i);
        self.ttls.swap_remove(i);
        self.seekers.swap_remove(i);
    }
}

impl Workload for SoaAgents {
    type Spec = Agents;
    const BACKEND: &'static str = "vec_soa";

    fn setup(params: Params) -> Self {
        let n = params.count;
        let mut w = Self {
            ids: Vec::with_capacity(n),
            transforms: Vec::with_capacity(n),
            velocities: Vec::with_capacity(n),
            ttls: Vec::with_capacity(n),
            seekers: Vec::with_capacity(n),
            prng: Prng::from_seed(params.seed),
            next_id: 0,
            tick: 0,
            target: n,
            dt: dt(),
        };
        for _ in 0..n {
            w.spawn_next();
        }
        w
    }

    fn step(&mut self) {
        let cursor = Input::scripted(self.tick).cursor;
        for i in 0..self.ids.len() {
            rules::update(
                &mut self.transforms[i],
                &mut self.velocities[i],
                &mut self.ttls[i],
                &mut self.seekers[i],
                cursor,
                self.dt,
            );
        }
        // Walking from the end, the element swapped into `i` was already examined and kept.
        for i in (0..self.ids.len()).rev() {
            if self.ttls[i] == 0 {
                self.remove(i);
            }
        }
        while self.ids.len() < self.target {
            self.spawn_next();
        }
        self.tick += 1;
    }

    fn checksum(&self) -> u64 {
        (0..self.ids.len()).fold(0_u64, |sum, i| {
            let agent = Agent {
                id: self.ids[i],
                transform: self.transforms[i],
                velocity: self.velocities[i],
                ttl: self.ttls[i],
                seeker: self.seekers[i],
            };
            sum.wrapping_add(rules::agent_hash(&agent))
        })
    }
}
