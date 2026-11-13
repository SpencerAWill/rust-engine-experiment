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
//! use agents_2d::{Input, Sim, SimConfig};
//!
//! let mut sim = Sim::new(&SimConfig { agents: 1_000, ..SimConfig::default() });
//! for _ in 0..120 {
//!     let input = Input::scripted(sim.tick());
//!     sim.step(&input);
//! }
//! let summary = sim.summary();
//! assert_eq!(summary.agents, 1_000);
//! assert_eq!(summary.tick, 120);
//! ```

use core::num::NonZeroU32;

use engine::prelude::*;

pub mod render;

pub use render::{Frame, View, render};

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

/// What the host tells the simulation each step.
///
/// # Examples
///
/// ```
/// use agents_2d::Input;
/// use engine::prelude::*;
///
/// let input = Input { cursor: Some(Vec2::new(10.0, 20.0)) };
/// assert!(input.cursor.is_some());
/// assert_eq!(Input::default().cursor, None);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    /// The cursor in world space, or `None` when it is not inside the world.
    pub cursor: Option<Vec2>,
}

impl Input {
    /// The scripted cursor used by headless runs, tests and benchmarks.
    ///
    /// One period in three (`(tick / 600) % 3 == 2`) has no cursor. Otherwise the cursor moves
    /// along the world's diagonal and back every 600 ticks.
    ///
    /// # Examples
    ///
    /// ```
    /// use agents_2d::Input;
    /// use engine::prelude::*;
    ///
    /// assert_eq!(Input::scripted(150).cursor, Some(Vec2::new(160.0, 90.0)));
    /// assert_eq!(Input::scripted(1200).cursor, None);
    /// ```
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "phase is below 600, which f32 represents exactly"
    )]
    pub fn scripted(tick: u64) -> Self {
        if (tick / 600) % 3 == 2 {
            return Self { cursor: None };
        }
        let phase = tick % 600;
        let f = if phase < 300 {
            phase as f32 / 300.0
        } else {
            (600 - phase) as f32 / 300.0
        };
        Self {
            cursor: Some(Vec2::new(WORLD_WIDTH * f, WORLD_HEIGHT * f)),
        }
    }
}

/// Applies rules 1 to 4 of the spec (§4.3) to one agent, written component by component in the
/// spec's operation order so any storage can reproduce the result bit for bit.
fn update(agent: &mut Agent, cursor: Option<Vec2>, dt: f32) {
    let mut p = agent.transform.translation;
    let mut v = agent.velocity;

    // 1. Steer or catch.
    if let (Some(seeker), Some(c)) = (agent.seeker, cursor) {
        let dx = c.x - p.x;
        let dy = c.y - p.y;
        let len2 = dx * dx + dy * dy;
        if len2 <= CATCH_RADIUS * CATCH_RADIUS {
            agent.seeker = None;
        } else {
            let len = len2.sqrt();
            let k = seeker.acceleration * dt;
            v.x += (dx / len) * k;
            v.y += (dy / len) * k;
            let s2 = v.x * v.x + v.y * v.y;
            if s2 > MAX_SPEED * MAX_SPEED {
                let f = MAX_SPEED / s2.sqrt();
                v.x *= f;
                v.y *= f;
            }
        }
    }

    // 2. Move.
    p.x += v.x * dt;
    p.y += v.y * dt;

    // 3. Bounce. The comparisons are strict: an agent exactly on an edge does not bounce.
    let mut bounced = false;
    if p.x < 0.0 {
        p.x = 0.0;
        v.x = v.x.abs();
        bounced = true;
    } else if p.x > WORLD_WIDTH {
        p.x = WORLD_WIDTH;
        v.x = -v.x.abs();
        bounced = true;
    }
    if p.y < 0.0 {
        p.y = 0.0;
        v.y = v.y.abs();
        bounced = true;
    } else if p.y > WORLD_HEIGHT {
        p.y = WORLD_HEIGHT;
        v.y = -v.y.abs();
        bounced = true;
    }
    if bounced && agent.seeker.is_none() {
        agent.seeker = Some(Seeker {
            acceleration: GAINED_ACCELERATION,
        });
    }

    // 4. Age.
    agent.ttl = agent.ttl.saturating_sub(1);

    agent.transform.translation = p;
    agent.velocity = v;
}

/// Creates a [`Sim`] from `config`, steps it `ticks` times with [`Input::scripted`], and returns
/// its summary.
///
/// # Examples
///
/// ```
/// use agents_2d::{SimConfig, run};
///
/// let config = SimConfig { agents: 100, ..SimConfig::default() };
/// let summary = run(&config, 60);
/// assert_eq!(summary.tick, 60);
/// assert_eq!(summary, run(&config, 60));
/// ```
#[must_use]
pub fn run(config: &SimConfig, ticks: u64) -> SimSummary {
    let mut sim = Sim::new(config);
    for _ in 0..ticks {
        let input = Input::scripted(sim.tick());
        sim.step(&input);
    }
    sim.summary()
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

    /// Advances the simulation by one fixed step.
    ///
    /// Every agent is updated by the rules of spec §4.3 (steer or catch, move, bounce, age); then
    /// agents whose time to live ran out are removed, new agents are spawned until the
    /// population is back at its target, and the tick counter increases. Agents spawned during a
    /// step are not updated in that step, and the storage order afterwards is unspecified.
    pub fn step(&mut self, input: &Input) {
        let dt = self.dt;
        for agent in &mut self.agents {
            update(agent, input.cursor, dt);
        }
        self.agents.retain(|a| a.ttl > 0);
        while self.agents.len() < self.target {
            self.agents.push(spawn_agent(&self.prng, self.next_id));
            self.next_id += 1;
        }
        self.tick += 1;
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

    fn dt() -> f32 {
        Sim::new(&SimConfig {
            agents: 0,
            ..SimConfig::default()
        })
        .dt()
    }

    fn agent(x: f32, y: f32, vx: f32, vy: f32, seeker: Option<f32>) -> Agent {
        Agent {
            id: 0,
            transform: Transform2d::IDENTITY.with_translation(Vec2::new(x, y)),
            velocity: Vec2::new(vx, vy),
            ttl: 100,
            seeker: seeker.map(|acceleration| Seeker { acceleration }),
        }
    }

    #[test]
    fn seeker_steers_toward_the_cursor() {
        let dt = dt();
        let mut a = agent(100.0, 90.0, 0.0, 0.0, Some(60.0));
        update(&mut a, Some(Vec2::new(200.0, 90.0)), dt);
        assert!(a.velocity.x > 0.0);
        assert_eq!(a.velocity.y, 0.0);
        assert_eq!(a.velocity.x, 60.0 * dt);
    }

    #[test]
    fn steering_clamps_speed() {
        let mut a = agent(100.0, 90.0, 30.0, 0.0, Some(60.0));
        update(&mut a, Some(Vec2::new(200.0, 90.0)), dt());
        let (vx, vy) = (a.velocity.x, a.velocity.y);
        assert!(vx * vx + vy * vy <= MAX_SPEED * MAX_SPEED * (1.0 + 1e-6));
        assert!(vx > 29.0, "still moving toward the cursor");
    }

    #[test]
    fn exactly_at_the_catch_radius_catches() {
        let mut a = agent(100.0, 90.0, 1.0, 2.0, Some(60.0));
        update(&mut a, Some(Vec2::new(100.0 + CATCH_RADIUS, 90.0)), dt());
        assert_eq!(a.seeker, None);
        assert_eq!(a.velocity, Vec2::new(1.0, 2.0));
    }

    #[test]
    fn cursor_on_the_agent_catches_without_nan() {
        let mut a = agent(100.0, 90.0, 1.0, 2.0, Some(60.0));
        update(&mut a, Some(Vec2::new(100.0, 90.0)), dt());
        assert_eq!(a.seeker, None);
        assert_eq!(a.velocity, Vec2::new(1.0, 2.0));
        assert!(a.transform.translation.is_finite());
    }

    #[test]
    fn wanderer_ignores_the_cursor() {
        let mut a = agent(100.0, 90.0, 1.0, 2.0, None);
        update(&mut a, Some(Vec2::new(200.0, 90.0)), dt());
        assert_eq!(a.velocity, Vec2::new(1.0, 2.0));
        assert_eq!(a.seeker, None);
    }

    #[test]
    fn each_edge_clamps_and_reflects() {
        // (x, y, vx, vy) moving out of the world, and the expected (x, y, vx, vy) after one tick.
        let cases = [
            ((0.5, 90.0, -60.0, 7.0), (0.0, 90.0 + 7.0 * dt(), 60.0, 7.0)),
            (
                (319.5, 90.0, 60.0, 7.0),
                (320.0, 90.0 + 7.0 * dt(), -60.0, 7.0),
            ),
            (
                (100.0, 0.5, 7.0, -60.0),
                (100.0 + 7.0 * dt(), 0.0, 7.0, 60.0),
            ),
            (
                (100.0, 179.5, 7.0, 60.0),
                (100.0 + 7.0 * dt(), 180.0, 7.0, -60.0),
            ),
        ];
        for ((x, y, vx, vy), (ex, ey, evx, evy)) in cases {
            let mut w = agent(x, y, vx, vy, None);
            update(&mut w, None, dt());
            assert_eq!(
                w.transform.translation,
                Vec2::new(ex, ey),
                "wanderer {x},{y}"
            );
            assert_eq!(w.velocity, Vec2::new(evx, evy), "wanderer {x},{y}");
            assert_eq!(
                w.seeker,
                Some(Seeker {
                    acceleration: GAINED_ACCELERATION
                })
            );

            let mut s = agent(x, y, vx, vy, Some(50.0));
            update(&mut s, None, dt());
            assert_eq!(
                s.seeker,
                Some(Seeker { acceleration: 50.0 }),
                "seeker {x},{y}"
            );
        }
    }

    #[test]
    fn exactly_on_the_edge_does_not_bounce() {
        for (x, y) in [(0.0, 90.0), (320.0, 90.0), (100.0, 0.0), (100.0, 180.0)] {
            let mut a = agent(x, y, 0.0, 0.0, None);
            update(&mut a, None, dt());
            assert_eq!(a.transform.translation, Vec2::new(x, y));
            assert_eq!(a.seeker, None, "bounced at {x},{y}");
        }
    }

    #[test]
    fn caught_then_bounced_is_a_seeker_again() {
        let mut a = agent(0.5, 90.0, -60.0, 0.0, Some(50.0));
        update(&mut a, Some(Vec2::new(2.0, 90.0)), dt());
        assert_eq!(
            a.seeker,
            Some(Seeker {
                acceleration: GAINED_ACCELERATION
            })
        );
        assert_eq!(a.transform.translation.x, 0.0);
    }

    #[test]
    fn update_ages_the_agent() {
        let mut a = agent(100.0, 90.0, 0.0, 0.0, None);
        update(&mut a, None, dt());
        assert_eq!(a.ttl, 99);
    }

    #[test]
    fn expired_agents_are_replaced_with_new_ids() {
        let mut sim = Sim::new(&SimConfig {
            agents: 10,
            ..SimConfig::default()
        });
        for a in &mut sim.agents {
            a.ttl = 1;
        }
        sim.step(&Input::default());
        assert_eq!(sim.agents().len(), 10);
        let mut ids: Vec<u64> = sim.agents().iter().map(|a| a.id).collect();
        ids.sort_unstable();
        assert_eq!(ids, (10..20).collect::<Vec<u64>>());
        assert_eq!(sim.tick(), 1);
    }

    #[test]
    fn step_ignores_storage_order() {
        let mut a = Sim::new(&SimConfig {
            agents: 1_000,
            ..SimConfig::default()
        });
        for _ in 0..50 {
            a.step(&Input::scripted(a.tick()));
        }
        let mut b = a.clone();
        b.agents.reverse();
        assert_eq!(a.checksum(), b.checksum());
        while a.tick() < 300 {
            let input = Input::scripted(a.tick());
            a.step(&input);
            b.step(&input);
            assert_eq!(a.checksum(), b.checksum(), "tick {}", a.tick());
        }
    }

    #[test]
    fn transforms_keep_identity_rotation_and_scale() {
        let mut sim = Sim::new(&SimConfig {
            agents: 500,
            ..SimConfig::default()
        });
        for _ in 0..600 {
            sim.step(&Input::scripted(sim.tick()));
        }
        for a in sim.agents() {
            assert_eq!(a.transform.rotation, 0.0);
            assert_eq!(a.transform.scale, Vec2::ONE);
        }
    }

    #[test]
    fn scripted_input_follows_the_spec() {
        let at = |t| Input::scripted(t).cursor;
        assert_eq!(at(0), Some(Vec2::new(0.0, 0.0)));
        assert_eq!(at(150), Some(Vec2::new(160.0, 90.0)));
        assert_eq!(at(300), Some(Vec2::new(320.0, 180.0)));
        assert_eq!(at(450), Some(Vec2::new(160.0, 90.0)));
        let f = 1.0_f32 / 300.0;
        assert_eq!(at(599), Some(Vec2::new(WORLD_WIDTH * f, WORLD_HEIGHT * f)));
        assert_eq!(at(1200), None);
        assert_eq!(at(1799), None);
    }
}
