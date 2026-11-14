//! `agents`: the M3.0 agent workload (spec §4.4.7).
//!
//! The rules, the spawn function and the hash are M3.0 §4.3's, from `agents_2d`. [`update`] is a
//! second implementation of rules 1 to 4 for one agent's components, so a backend that stores
//! them in columns need not build an [`Agent`] to step it. The `vec_soa` conformance test against
//! `agents_2d::Sim` is what proves it matches.

use agents_2d::{CATCH_RADIUS, GAINED_ACCELERATION, MAX_SPEED, WORLD_HEIGHT, WORLD_WIDTH};
use engine::prelude::{Transform2d, Vec2};

pub use agents_2d::{Agent, Input, Seeker, agent_hash, spawn_agent};

use crate::Spec;

/// The `agents` workload's identity.
#[derive(Clone, Copy, Debug)]
pub struct Agents;

impl Spec for Agents {
    const NAME: &'static str = "agents";
    const SIZES: &'static [usize] = &[10_000, 100_000];
}

/// Rules 1 to 4 of M3.0 §4.3 for one agent: steer or catch, move, bounce, age. Removal of expired
/// agents and respawn are the backend's.
///
/// The floating-point operations are in the spec's order, so any storage reproduces the result
/// bit for bit.
#[inline]
pub fn update(
    transform: &mut Transform2d,
    velocity: &mut Vec2,
    ttl: &mut u32,
    seeker: &mut Option<Seeker>,
    cursor: Option<Vec2>,
    dt: f32,
) {
    let mut p = transform.translation;
    let mut v = *velocity;

    // 1. Steer or catch.
    if let (Some(s), Some(c)) = (*seeker, cursor) {
        let dx = c.x - p.x;
        let dy = c.y - p.y;
        let len2 = dx * dx + dy * dy;
        if len2 <= CATCH_RADIUS * CATCH_RADIUS {
            *seeker = None;
        } else {
            let len = len2.sqrt();
            let k = s.acceleration * dt;
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

    // 3. Bounce, per axis, with strict comparisons.
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
    if bounced && seeker.is_none() {
        *seeker = Some(Seeker {
            acceleration: GAINED_ACCELERATION,
        });
    }

    // 4. Age. A live agent has at least one tick left.
    *ttl -= 1;

    transform.translation = p;
    *velocity = v;
}
