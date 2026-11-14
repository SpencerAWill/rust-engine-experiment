//! `entity_churn`: entities live a few ticks, die and are replaced, so storage sees constant
//! spawning and despawning (spec §4.4.5).

use engine::prelude::{Prng, Vec2};

use crate::{Fnv1a, Spec};

/// The `entity_churn` workload's identity.
#[derive(Clone, Copy, Debug)]
pub struct EntityChurn;

impl Spec for EntityChurn {
    const NAME: &'static str = "entity_churn";
    const SIZES: &'static [usize] = &[1_000, 10_000, 100_000];
}

/// The spawn index the entity was created with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnId(pub u64);

/// A position in the 320 by 180 world. The step moves it; nothing wraps or bounces it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position(pub Vec2);

/// A velocity, added to the position each step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Velocity(pub Vec2);

/// Ticks left to live. The entity despawns when it reaches 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lifetime(pub u32);

/// Entity `i`'s components: `SpawnId(i)`; position x in `0..320` then y in `0..180`; velocity x
/// then y in `-1..1`; lifetime in `1..=30`.
#[must_use]
pub fn spawn(prng: &Prng, i: u64) -> (SpawnId, Position, Velocity, Lifetime) {
    let mut r = prng.stream(i);
    let position = Vec2::new(r.range(0.0_f32..320.0), r.range(0.0_f32..180.0));
    let velocity = Vec2::new(r.range(-1.0_f32..1.0), r.range(-1.0_f32..1.0));
    let lifetime = r.range(1_u32..=30);
    (
        SpawnId(i),
        Position(position),
        Velocity(velocity),
        Lifetime(lifetime),
    )
}

/// The rule for one living entity: `position = position + velocity`, then `lifetime = lifetime - 1`.
///
/// A living entity has a lifetime of at least 1, so the subtraction cannot underflow.
#[inline]
pub fn update(position: &mut Position, velocity: &Velocity, lifetime: &mut Lifetime) {
    position.0 += velocity.0;
    lifetime.0 -= 1;
}

/// Whether the entity despawns after this step: its lifetime is 0.
#[inline]
#[must_use]
pub fn is_expired(lifetime: &Lifetime) -> bool {
    lifetime.0 == 0
}

/// One entity's term of the checksum: `u64(spawn id)`, `vec2(position)`, `vec2(velocity)`,
/// `u32(lifetime)`.
#[must_use]
pub fn hash(id: &SpawnId, position: &Position, velocity: &Velocity, lifetime: &Lifetime) -> u64 {
    Fnv1a::new()
        .u64(id.0)
        .vec2(position.0)
        .vec2(velocity.0)
        .u32(lifetime.0)
        .finish()
}
