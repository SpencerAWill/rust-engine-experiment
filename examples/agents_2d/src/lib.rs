#![forbid(unsafe_code)]
//! A deterministic 2D agent simulation: the plain-`Vec` baseline for the M3 storage decision.
//!
//! Thousands of agents wander a 320 by 180 meter world, bounce off its edges, gain and lose an
//! optional [`Seeker`] part, die and respawn. The simulation is a plain state machine, [`Sim`],
//! that a host steps with [`Input`]; it never sees a window, a clock or an event loop.
//!
//! The rules are written down in the M3.0 spec (§4.3) precisely enough that a different storage
//! model can reproduce [`Sim::checksum`], which does not depend on the order agents are stored
//! in.
//!
//! # Examples
//!
//! ```
//! use agents_2d::{Sim, SimConfig};
//!
//! let sim = Sim::new(&SimConfig { agents: 1_000, ..SimConfig::default() });
//! assert_eq!(sim.summary().agents, 1_000);
//! assert_eq!(sim.tick(), 0);
//! ```

use core::num::NonZeroU32;

use engine::prelude::*;

/// World width in meters (+X is right, the origin is the bottom-left corner).
pub const WORLD_WIDTH: f32 = 320.0;
/// World height in meters (+Y is up).
pub const WORLD_HEIGHT: f32 = 180.0;
/// Simulation rate, in steps per second.
pub const TICK_HZ: u32 = 60;
/// Spawn velocity components are drawn from `-SPAWN_SPEED..SPAWN_SPEED`, in m/s.
pub const SPAWN_SPEED: f32 = 20.0;
/// Shortest lifetime of a spawned agent, in ticks.
pub const TTL_MIN: u32 = 60;
/// Longest lifetime of a spawned agent, in ticks (inclusive).
pub const TTL_MAX: u32 = 600;
/// Probability that a spawned agent is a seeker.
pub const SEEKER_CHANCE: f64 = 0.25;
/// Lowest acceleration of a spawned seeker, in m/s².
pub const SEEKER_ACCELERATION_MIN: f32 = 40.0;
/// Highest acceleration (exclusive) of a spawned seeker, in m/s².
pub const SEEKER_ACCELERATION_MAX: f32 = 80.0;
/// Acceleration of a seeker gained by bouncing off a world edge, in m/s².
pub const GAINED_ACCELERATION: f32 = 60.0;
/// Maximum speed a seeker can steer up to, in m/s.
pub const MAX_SPEED: f32 = 30.0;
/// A seeker this close (in meters) to the cursor has caught it and stops seeking.
pub const CATCH_RADIUS: f32 = 4.0;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Configuration of a simulation.
///
/// # Examples
///
/// ```
/// use agents_2d::SimConfig;
///
/// let config = SimConfig::default();
/// assert_eq!((config.seed, config.agents), (42, 10_000));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimConfig {
    /// Seed of the generator that spawned agents are derived from.
    pub seed: u64,
    /// Target population: the number of agents at the start and after every step.
    pub agents: usize,
}

impl Default for SimConfig {
    /// Seed 42 and 10,000 agents.
    fn default() -> Self {
        Self {
            seed: 42,
            agents: 10_000,
        }
    }
}

/// The optional part that makes an agent steer toward the cursor.
///
/// # Examples
///
/// ```
/// use agents_2d::Seeker;
///
/// let part = Seeker { acceleration: 60.0 };
/// assert_eq!(part.acceleration, 60.0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seeker {
    /// Acceleration toward the cursor, in m/s².
    pub acceleration: f32,
}

/// One simulated agent.
///
/// # Examples
///
/// ```
/// use agents_2d::{Agent, Seeker};
/// use engine::prelude::*;
///
/// let agent = Agent {
///     id: 0,
///     transform: Transform2d::IDENTITY,
///     velocity: Vec2::ZERO,
///     ttl: 60,
///     seeker: Some(Seeker { acceleration: 40.0 }),
/// };
/// assert!(agent.seeker.is_some());
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Agent {
    /// Spawn counter, unique for the life of a [`Sim`].
    pub id: u64,
    /// The translation is the position, in meters. Rotation and scale stay at identity.
    pub transform: Transform2d,
    /// Velocity, in m/s.
    pub velocity: Vec2,
    /// Ticks left to live.
    pub ttl: u32,
    /// The seeker part, if the agent has it.
    pub seeker: Option<Seeker>,
}

/// A compact description of a simulation's state.
///
/// # Examples
///
/// ```
/// use agents_2d::{Sim, SimConfig};
///
/// let summary = Sim::new(&SimConfig { agents: 10, ..SimConfig::default() }).summary();
/// assert_eq!((summary.tick, summary.agents), (0, 10));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimSummary {
    /// Number of steps executed so far.
    pub tick: u64,
    /// Number of live agents.
    pub agents: usize,
    /// Number of agents that currently have the seeker part.
    pub seekers: usize,
    /// The order-independent checksum, see [`Sim::checksum`].
    pub checksum: u64,
}

/// Creates the agent with the given `id`, drawing from `prng.stream(id)` in the spec's order:
/// x, y, velocity x, velocity y, time to live, whether it is a seeker, and (only for a seeker)
/// its acceleration.
///
/// An agent's values depend on its `id` and the generator's seed alone, never on where it is
/// stored. Storage prototypes reuse this function so they spawn the same agents.
///
/// # Examples
///
/// ```
/// use agents_2d::spawn_agent;
/// use engine::prelude::*;
///
/// let prng = Prng::from_seed(42);
/// let agent = spawn_agent(&prng, 7);
/// assert_eq!(agent.id, 7);
/// assert_eq!(agent, spawn_agent(&prng, 7));
/// ```
#[must_use]
pub fn spawn_agent(prng: &Prng, id: u64) -> Agent {
    let mut r = prng.stream(id);
    let x = r.range(0.0_f32..WORLD_WIDTH);
    let y = r.range(0.0_f32..WORLD_HEIGHT);
    let vx = r.range(-SPAWN_SPEED..SPAWN_SPEED);
    let vy = r.range(-SPAWN_SPEED..SPAWN_SPEED);
    let ttl = r.range(TTL_MIN..=TTL_MAX);
    let seeker = if r.chance(SEEKER_CHANCE) {
        Some(Seeker {
            acceleration: r.range(SEEKER_ACCELERATION_MIN..SEEKER_ACCELERATION_MAX),
        })
    } else {
        None
    };
    Agent {
        id,
        transform: Transform2d::IDENTITY.with_translation(Vec2::new(x, y)),
        velocity: Vec2::new(vx, vy),
        ttl,
        seeker,
    }
}

fn fnv_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        hash = (hash ^ u64::from(b)).wrapping_mul(FNV_PRIME);
    }
    hash
}

/// One agent's term of the checksum: a 64-bit FNV-1a hash over the little-endian bytes of the
/// id, translation x and y, velocity x and y (as `f32` bits), the time to live, and then a `0`
/// byte for a wanderer or a `1` byte and the acceleration bits for a seeker.
///
/// Rotation and scale are not hashed: no rule changes them.
///
/// # Examples
///
/// ```
/// use agents_2d::{Sim, SimConfig, agent_hash};
///
/// let sim = Sim::new(&SimConfig { agents: 5, ..SimConfig::default() });
/// let sum = sim.agents().iter().fold(0_u64, |s, a| s.wrapping_add(agent_hash(a)));
/// assert_eq!(sum, sim.checksum());
/// ```
#[must_use]
pub fn agent_hash(agent: &Agent) -> u64 {
    let mut h = fnv_bytes(FNV_OFFSET, &agent.id.to_le_bytes());
    for f in [
        agent.transform.translation.x,
        agent.transform.translation.y,
        agent.velocity.x,
        agent.velocity.y,
    ] {
        h = fnv_bytes(h, &f.to_bits().to_le_bytes());
    }
    h = fnv_bytes(h, &agent.ttl.to_le_bytes());
    match agent.seeker {
        None => fnv_bytes(h, &[0]),
        Some(s) => fnv_bytes(fnv_bytes(h, &[1]), &s.acceleration.to_bits().to_le_bytes()),
    }
}

/// The complete simulation state: plain data, so cloning it snapshots the simulation.
///
/// # Examples
///
/// ```
/// use agents_2d::{Sim, SimConfig};
///
/// let sim = Sim::new(&SimConfig { agents: 50, ..SimConfig::default() });
/// let snapshot = sim.clone();
/// assert_eq!(sim.checksum(), snapshot.checksum());
/// ```
#[derive(Clone, Debug)]
#[expect(dead_code, reason = "read by `step`, added in the next commit")]
pub struct Sim {
    agents: Vec<Agent>,
    prng: Prng,
    next_id: u64,
    tick: u64,
    target: usize,
    dt: f32,
}

impl Sim {
    /// Creates a simulation with `config.agents` agents, whose ids are `0..config.agents`.
    ///
    /// # Panics
    ///
    /// Never in practice: 60 Hz is a valid rate.
    #[must_use]
    pub fn new(config: &SimConfig) -> Self {
        let hz = NonZeroU32::new(TICK_HZ).expect("TICK_HZ is non-zero");
        let dt = FixedTimestep::from_hz(hz).step().as_secs_f32();
        let prng = Prng::from_seed(config.seed);
        let mut agents = Vec::with_capacity(config.agents);
        let mut next_id = 0_u64;
        for _ in 0..config.agents {
            agents.push(spawn_agent(&prng, next_id));
            next_id += 1;
        }
        Self {
            agents,
            prng,
            next_id,
            tick: 0,
            target: config.agents,
            dt,
        }
    }

    /// Number of steps executed so far.
    #[must_use]
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// The step length in seconds, `FixedTimestep::from_hz(60).step()` as `f32`.
    #[must_use]
    pub fn dt(&self) -> f32 {
        self.dt
    }

    /// The live agents, in an unspecified order.
    #[must_use]
    pub fn agents(&self) -> &[Agent] {
        &self.agents
    }

    /// The wrapping sum of [`agent_hash`] over every agent.
    ///
    /// Addition is commutative, so the result does not depend on storage order.
    #[must_use]
    pub fn checksum(&self) -> u64 {
        self.agents
            .iter()
            .fold(0_u64, |sum, a| sum.wrapping_add(agent_hash(a)))
    }

    /// A summary of the current state.
    #[must_use]
    pub fn summary(&self) -> SimSummary {
        SimSummary {
            tick: self.tick,
            agents: self.agents.len(),
            seekers: self.agents.iter().filter(|a| a.seeker.is_some()).count(),
            checksum: self.checksum(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

    fn fnv(bytes: &[u8]) -> u64 {
        bytes.iter().fold(FNV_OFFSET, |h, &b| {
            (h ^ u64::from(b)).wrapping_mul(FNV_PRIME)
        })
    }

    fn expected_spawn(prng: &Prng, id: u64) -> Agent {
        let mut r = prng.stream(id);
        let x = r.range(0.0_f32..320.0);
        let y = r.range(0.0_f32..180.0);
        let vx = r.range(-20.0_f32..20.0);
        let vy = r.range(-20.0_f32..20.0);
        let ttl = r.range(60_u32..=600);
        let seeker = if r.chance(0.25) {
            Some(Seeker {
                acceleration: r.range(40.0_f32..80.0),
            })
        } else {
            None
        };
        Agent {
            id,
            transform: Transform2d::IDENTITY.with_translation(Vec2::new(x, y)),
            velocity: Vec2::new(vx, vy),
            ttl,
            seeker,
        }
    }

    #[test]
    fn spawn_draws_in_spec_order() {
        let prng = Prng::from_seed(42);
        for id in [0, 1, 12345] {
            assert_eq!(spawn_agent(&prng, id), expected_spawn(&prng, id), "id {id}");
        }
    }

    #[test]
    fn new_spawns_ids_in_order() {
        let sim = Sim::new(&SimConfig {
            agents: 100,
            ..SimConfig::default()
        });
        assert_eq!(sim.tick(), 0);
        assert_eq!(sim.agents().len(), 100);
        for (i, a) in sim.agents().iter().enumerate() {
            assert_eq!(a.id, i as u64);
            assert_eq!(a.transform.rotation, 0.0);
            assert_eq!(a.transform.scale, Vec2::ONE);
        }
    }

    #[test]
    fn agent_hash_matches_hand_computed_fnv() {
        let wanderer = Agent {
            id: 0x0102_0304_0506_0708,
            transform: Transform2d::IDENTITY.with_translation(Vec2::new(1.5, -2.25)),
            velocity: Vec2::new(3.0, 4.5),
            ttl: 77,
            seeker: None,
        };
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&wanderer.id.to_le_bytes());
        for f in [1.5_f32, -2.25, 3.0, 4.5] {
            bytes.extend_from_slice(&f.to_bits().to_le_bytes());
        }
        bytes.extend_from_slice(&77_u32.to_le_bytes());
        bytes.push(0);
        assert_eq!(agent_hash(&wanderer), fnv(&bytes));

        let seeker = Agent {
            seeker: Some(Seeker { acceleration: 55.5 }),
            ..wanderer
        };
        bytes.pop();
        bytes.push(1);
        bytes.extend_from_slice(&55.5_f32.to_bits().to_le_bytes());
        assert_eq!(agent_hash(&seeker), fnv(&bytes));
    }

    #[test]
    fn checksum_ignores_storage_order() {
        let sim = Sim::new(&SimConfig {
            agents: 1_000,
            ..SimConfig::default()
        });
        let mut reversed = sim.clone();
        reversed.agents.reverse();
        assert_ne!(sim.agents(), reversed.agents());
        assert_eq!(sim.checksum(), reversed.checksum());
    }

    #[test]
    fn empty_sim_has_zero_checksum() {
        let sim = Sim::new(&SimConfig {
            agents: 0,
            ..SimConfig::default()
        });
        assert_eq!(sim.checksum(), 0);
        assert_eq!(sim.summary().agents, 0);
    }
}
