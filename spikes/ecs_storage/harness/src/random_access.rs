//! `random_access`: every entity holds the handle of a target and each step reads the target's
//! position through that handle to set its own velocity (M3.21 spec §4.2).
//!
//! The rules here are the M3.21 spec §4.2; the common rules of M3.2 §4.4 apply too. Every entity
//! has exactly one slot, and the backend keeps a roster of the handle in each slot.

use engine::prelude::{Prng, Vec2};

use crate::{Fnv1a, Spec};

/// The `random_access` workload's identity.
#[derive(Clone, Copy, Debug)]
pub struct RandomAccess;

impl Spec for RandomAccess {
    const NAME: &'static str = "random_access";
    const SIZES: &'static [usize] = &[10_000, 100_000, 1_000_000];
}

/// The factor of [`steer`]: the fraction of the distance to the target the velocity covers.
pub const GAIN: f32 = 0.5;

/// The spawn index the entity was created with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnId(pub u64);

/// The entity's own slot, `0..count`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot(pub u32);

/// A position in the 320 by 180 world. The step moves it; nothing wraps or bounces it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position(pub Vec2);

/// A velocity, set from the target's position each step and added to the position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Velocity(pub Vec2);

/// Ticks left to live. The entity expires when it reaches 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lifetime(pub u32);

/// The entity's target: the slot it follows and the handle it last resolved there.
///
/// `E` is the backend's entity handle, so every backend stores this type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target<E> {
    /// The target's slot, fixed at spawn.
    pub slot: u32,
    /// The handle last resolved in that slot; stale once the occupant has been replaced.
    pub entity: E,
}

/// Entity `i`'s initial values for a workload of `count` slots: `SpawnId(i)`; position x in
/// `0.0..320.0` then y in `0.0..180.0`; velocity zero (no draw); lifetime in `60..=600`; then the
/// target slot in `0..count` (which may be the entity's own).
///
/// The backend builds [`Slot`] and [`Target`] from the returned slot and its roster.
///
/// # Panics
///
/// Panics if `count` is 0 or exceeds `u32::MAX`.
#[must_use]
#[track_caller]
pub fn spawn(prng: &Prng, i: u64, count: usize) -> (SpawnId, Position, Velocity, Lifetime, u32) {
    let count = u32::try_from(count).expect("count must fit in u32");
    assert!(count > 0, "random_access needs at least one slot to target");
    let mut r = prng.stream(i);
    let position = Vec2::new(r.range(0.0_f32..320.0), r.range(0.0_f32..180.0));
    let lifetime = r.range(60_u32..=600);
    let target = r.range(0_u32..count);
    (
        SpawnId(i),
        Position(position),
        Velocity(Vec2::ZERO),
        Lifetime(lifetime),
        target,
    )
}

/// The new velocity of an entity at `position` with a target at `target`:
/// `(target - position) * GAIN`, as `Vec2` subtraction then scalar multiplication.
#[inline]
#[must_use]
pub fn steer(position: &Position, target: &Position) -> Velocity {
    Velocity((target.0 - position.0) * GAIN)
}

/// The rule for one living entity after its velocity is set: `position = position + velocity *
/// dt` (scalar multiplication, then addition), then `lifetime = lifetime - 1`.
///
/// `dt` is [`dt()`](crate::dt), which the backend computes once in `setup`.
///
/// A living entity has a lifetime of at least 1, so the subtraction cannot underflow.
#[inline]
pub fn integrate(position: &mut Position, velocity: &Velocity, lifetime: &mut Lifetime, dt: f32) {
    position.0 += velocity.0 * dt;
    lifetime.0 -= 1;
}

/// Whether the entity expires after this step: its lifetime is 0.
#[inline]
#[must_use]
pub fn is_expired(lifetime: &Lifetime) -> bool {
    lifetime.0 == 0
}

/// One entity's term of the checksum: `u64(spawn id)`, `u32(slot)`, `vec2(position)`,
/// `vec2(velocity)`, `u32(lifetime)`, `u32(target slot)`. The handle is not hashed: it differs
/// between backends.
#[must_use]
pub fn hash<E>(
    id: &SpawnId,
    slot: &Slot,
    position: &Position,
    velocity: &Velocity,
    lifetime: &Lifetime,
    target: &Target<E>,
) -> u64 {
    Fnv1a::new()
        .u64(id.0)
        .u32(slot.0)
        .vec2(position.0)
        .vec2(velocity.0)
        .u32(lifetime.0)
        .u32(target.slot)
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::prelude::{Prng, Vec2};

    #[test]
    fn spawn_follows_the_draw_order() {
        let prng = Prng::from_seed(42);
        for i in [0_u64, 1, 999] {
            let mut r = prng.stream(i);
            let x = r.range(0.0_f32..320.0);
            let y = r.range(0.0_f32..180.0);
            let lifetime = r.range(60_u32..=600);
            let target = r.range(0_u32..1_000);
            let (id, position, velocity, life, slot) = spawn(&prng, i, 1_000);
            assert_eq!(id, SpawnId(i));
            assert_eq!(position, Position(Vec2::new(x, y)));
            assert_eq!(velocity, Velocity(Vec2::ZERO));
            assert_eq!(life, Lifetime(lifetime));
            assert_eq!(slot, target);
        }
    }

    #[test]
    fn steer_and_integrate_follow_the_spec_order() {
        let position = Position(Vec2::new(10.0, 20.0));
        let target = Position(Vec2::new(14.0, 10.0));
        let velocity = steer(&position, &target);
        // (14 - 10) * 0.5 and (10 - 20) * 0.5, both exact.
        assert_eq!(velocity, Velocity(Vec2::new(2.0, -5.0)));
        let dt = crate::dt();
        let mut p = position;
        let mut life = Lifetime(2);
        integrate(&mut p, &velocity, &mut life, dt);
        assert_eq!(p, Position(Vec2::new(10.0 + 2.0 * dt, 20.0 + -5.0 * dt)));
        assert_eq!(life, Lifetime(1));
        assert!(!is_expired(&life));
        integrate(&mut p, &velocity, &mut life, dt);
        assert!(is_expired(&life));
    }
}
