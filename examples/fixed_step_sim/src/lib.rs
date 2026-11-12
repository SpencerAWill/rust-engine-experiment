#![forbid(unsafe_code)]
//! A headless, deterministic particle simulation that exercises the M1 crates together.
//!
//! Particles live in a [`HandlePool`], are respawned from per-handle [`Prng`] streams, and are
//! stepped by a [`FixedTimestep`] driven from a [`ManualClock`] with jittery frame times. The
//! whole simulation is plain data: it can be cloned mid-run and continued, and it produces the
//! same [`checksum`](Sim::checksum) on every architecture, because it uses only basic IEEE `f32`
//! operations on `Vec3` (no transcendental functions, no SIMD-backed glam types).
//!
//! # Rules (these define what the checksum means)
//!
//! - **Spawn:** insert a zero particle, derive a stream from the new handle's bits
//!   (`prng.stream(handle.to_bits())`), then draw position (x, y, z in `[-100, 100)`), velocity
//!   (x, y, z in `[-5, 5)`), and `ttl` (`60..=600`), in that order.
//! - **Step:** `velocity.y -= 9.81 * dt`, `position += velocity * dt`, `ttl -= 1`; then remove
//!   expired particles with `retain` and respawn until the population is back to its target.
//! - **Frames:** a separate jitter generator picks each frame's length in `[4 ms, 40 ms]`, so the
//!   split of time into frames never consumes the simulation's own random numbers.
//!
//! # Examples
//!
//! ```
//! use fixed_step_sim::{SimConfig, run};
//!
//! let config = SimConfig { ticks: 60, particles: 100, ..SimConfig::default() };
//! let summary = run(&config);
//! assert_eq!(summary.tick, 60);
//! assert_eq!(summary.live, 100);
//! assert_eq!(summary, run(&config)); // deterministic
//! ```

use core::num::NonZeroU32;
use core::time::Duration;

use engine::prelude::*;
use engine::time::Steps;

/// Configuration for a simulation run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimConfig {
    /// Seed of the simulation's own generator (spawn values).
    pub seed: u64,
    /// Seed of the frame-time jitter generator. It must not affect the checksum.
    pub jitter_seed: u64,
    /// Number of fixed steps to run.
    pub ticks: u64,
    /// Target particle population.
    pub particles: usize,
}

impl Default for SimConfig {
    /// Seed 42, jitter seed 7, 600 ticks, 10,000 particles.
    fn default() -> Self {
        Self {
            seed: 42,
            jitter_seed: 7,
            ticks: 600,
            particles: 10_000,
        }
    }
}

/// One simulated particle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    /// Position, in meters.
    pub position: Vec3,
    /// Velocity, in meters per second.
    pub velocity: Vec3,
    /// Remaining lifetime, in ticks.
    pub ttl: u32,
}

impl Particle {
    /// A particle at rest at the origin, used as a placeholder before its values are drawn.
    pub const ZERO: Self = Self {
        position: Vec3::ZERO,
        velocity: Vec3::ZERO,
        ttl: 0,
    };
}

/// A compact description of a simulation's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimSummary {
    /// Number of fixed steps executed so far.
    pub tick: u64,
    /// Number of live particles.
    pub live: usize,
    /// Capacity of the particle storage, in values.
    pub capacity: usize,
    /// FNV-1a checksum of the particle state (see [`Sim::checksum`]).
    pub checksum: u64,
}

/// The complete simulation state. Plain data: cloning it snapshots the simulation.
#[derive(Clone, Debug)]
pub struct Sim {
    pool: HandlePool<Particle>,
    prng: Prng,
    jitter_prng: Prng,
    timestep: FixedTimestep,
    clock: ManualClock,
    last: Timestamp,
    pending: Steps,
    particles: usize,
    dt: f32,
}

impl Sim {
    /// Creates a simulation with `config.particles` particles spawned.
    ///
    /// # Panics
    ///
    /// Never in practice: 60 Hz is a valid rate.
    #[must_use]
    pub fn new(config: &SimConfig) -> Self {
        let mut timestep = FixedTimestep::from_hz(NonZeroU32::new(60).expect("60 is non-zero"));
        let pending = timestep.accumulate(Duration::ZERO); // an empty `Steps` to start from
        let mut sim = Self {
            pool: HandlePool::with_capacity(config.particles),
            prng: Prng::from_seed(config.seed),
            jitter_prng: Prng::from_seed(config.jitter_seed),
            dt: timestep.step().as_secs_f32(),
            timestep,
            clock: ManualClock::new(),
            last: Timestamp::default(),
            pending,
            particles: config.particles,
        };
        for _ in 0..config.particles {
            sim.spawn();
        }
        sim
    }

    /// Inserts a particle with values drawn from a stream keyed by its handle.
    fn spawn(&mut self) {
        let handle = self.pool.insert(Particle::ZERO);
        let mut r = self.prng.stream(handle.to_bits());
        let position = Vec3::new(
            r.range(-100.0_f32..100.0),
            r.range(-100.0_f32..100.0),
            r.range(-100.0_f32..100.0),
        );
        let velocity = Vec3::new(
            r.range(-5.0_f32..5.0),
            r.range(-5.0_f32..5.0),
            r.range(-5.0_f32..5.0),
        );
        let ttl = r.range(60_u32..=600);
        self.pool[handle] = Particle {
            position,
            velocity,
            ttl,
        };
    }

    /// One fixed simulation step.
    fn step(&mut self) {
        let dt = self.dt;
        for (_, p) in &mut self.pool {
            p.velocity.y -= 9.81 * dt;
            p.position += p.velocity * dt;
            p.ttl -= 1;
        }
        self.pool.retain(|_, p| p.ttl > 0);
        while self.pool.len() < self.particles {
            self.spawn();
        }
    }

    /// Runs `n` more fixed steps, driving the timestep from the manual clock with jittery frames.
    ///
    /// Steps committed by the timestep but not yet executed stay pending for the next call, so
    /// `run_ticks(300)` followed by `run_ticks(300)` equals `run_ticks(600)`.
    pub fn run_ticks(&mut self, n: u64) {
        let mut done = 0;
        loop {
            while done < n && self.pending.next().is_some() {
                self.step();
                done += 1;
            }
            if done == n {
                return;
            }
            let frame = self.jitter_prng.range(4_000_000_u64..=40_000_000);
            self.clock.advance(Duration::from_nanos(frame));
            let now = self.clock.now();
            self.pending = self
                .timestep
                .accumulate(now.saturating_duration_since(self.last));
            self.last = now;
        }
    }

    /// FNV-1a 64-bit checksum over every live particle in iteration order: the handle's bits,
    /// then position and velocity components as little-endian `f32` bit patterns.
    #[must_use]
    pub fn checksum(&self) -> u64 {
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        let mut hash = OFFSET;
        let mut feed = |bytes: &[u8]| {
            for byte in bytes {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(PRIME);
            }
        };
        for (handle, p) in &self.pool {
            feed(&handle.to_bits().to_le_bytes());
            for value in [
                p.position.x,
                p.position.y,
                p.position.z,
                p.velocity.x,
                p.velocity.y,
                p.velocity.z,
            ] {
                feed(&value.to_bits().to_le_bytes());
            }
        }
        hash
    }

    /// A compact summary of the current state.
    #[must_use]
    pub fn summary(&self) -> SimSummary {
        SimSummary {
            // Steps committed by the timestep minus those still waiting to run.
            tick: self.timestep.current_tick().get() - self.pending.len() as u64,
            live: self.pool.len(),
            capacity: self.pool.capacity(),
            checksum: self.checksum(),
        }
    }

    /// The simulation's generator, for raw-state snapshot tests.
    #[must_use]
    pub fn prng(&self) -> &Prng {
        &self.prng
    }

    /// The fixed timestep, for raw-state snapshot tests.
    #[must_use]
    pub fn timestep(&self) -> &FixedTimestep {
        &self.timestep
    }

    /// The manual clock, for raw-state snapshot tests.
    #[must_use]
    pub fn clock(&self) -> &ManualClock {
        &self.clock
    }
}

/// Runs a whole simulation and returns its summary.
#[must_use]
pub fn run(config: &SimConfig) -> SimSummary {
    let mut sim = Sim::new(config);
    sim.run_ticks(config.ticks);
    sim.summary()
}
