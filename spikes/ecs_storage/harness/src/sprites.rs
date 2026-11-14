//! `sprites`: 2D sprites that move, wrap around the world, and are collected into a draw list
//! each step, with some hidden (spec §4.4.6).

use agents_2d::{WORLD_HEIGHT, WORLD_WIDTH};
use engine::prelude::{Prng, Transform2d, Vec2};

use crate::{Fnv1a, Spec};

/// The `sprites` workload's identity.
#[derive(Clone, Copy, Debug)]
pub struct Sprites;

impl Spec for Sprites {
    const NAME: &'static str = "sprites";
    const SIZES: &'static [usize] = &[100_000];
}

/// A velocity in world units per second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Velocity2d(pub Vec2);

/// How a sprite looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sprite {
    /// A 24-bit color.
    pub color: u32,
    /// Half the side length of the square.
    pub half_extent: f32,
}

/// The tag of a sprite that is not drawn. It carries no data and never changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hidden;

/// One entry of the draw list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawItem {
    /// The sprite's position.
    pub position: Vec2,
    /// The sprite's half extent.
    pub half_extent: f32,
    /// The sprite's color.
    pub color: u32,
}

/// Entity `i`'s components.
///
/// Draws, in order: translation x in `0..320`, then y in `0..180`; velocity x, then y, each in
/// `-20..20`; color `next_u32() & 0x00FF_FFFF`; half extent in `0.25..2.0`; hidden with
/// `chance(0.1)`. Rotation and scale stay at identity.
#[must_use]
pub fn spawn(prng: &Prng, i: u64) -> (Transform2d, Velocity2d, Sprite, Option<Hidden>) {
    let mut r = prng.stream(i);
    let translation = Vec2::new(
        r.range(0.0_f32..WORLD_WIDTH),
        r.range(0.0_f32..WORLD_HEIGHT),
    );
    let velocity = Vec2::new(r.range(-20.0_f32..20.0), r.range(-20.0_f32..20.0));
    let color = r.next_u32() & 0x00FF_FFFF;
    let half_extent = r.range(0.25_f32..2.0);
    let hidden = r.chance(0.1).then_some(Hidden);
    (
        Transform2d::IDENTITY.with_translation(translation),
        Velocity2d(velocity),
        Sprite { color, half_extent },
        hidden,
    )
}

/// Wraps one coordinate into the world: below 0 adds `bound`, at or above `bound` subtracts it.
///
/// The result is in the closed range `0..=bound`: for a tiny negative `p`, `p + bound` rounds to
/// `bound` exactly.
#[inline]
#[must_use]
pub fn wrap(p: f32, bound: f32) -> f32 {
    if p < 0.0 {
        p + bound
    } else if p >= bound {
        p - bound
    } else {
        p
    }
}

/// The rule for one entity, hidden or not: `t = t + v * dt`, then wrap each axis.
#[inline]
pub fn update(transform: &mut Transform2d, velocity: &Velocity2d, dt: f32) {
    let t = transform.translation + velocity.0 * dt;
    transform.translation = Vec2::new(wrap(t.x, WORLD_WIDTH), wrap(t.y, WORLD_HEIGHT));
}

/// The draw-list entry of a visible sprite.
#[inline]
#[must_use]
pub fn draw_item(transform: &Transform2d, sprite: &Sprite) -> DrawItem {
    DrawItem {
        position: transform.translation,
        half_extent: sprite.half_extent,
        color: sprite.color,
    }
}

/// One entity's term of the checksum: `vec2(translation)`, `vec2(velocity)`, `u32(color)`,
/// `f32(half_extent)`, `u8(hidden)`.
#[must_use]
pub fn hash_entity(
    transform: &Transform2d,
    velocity: &Velocity2d,
    sprite: &Sprite,
    hidden: bool,
) -> u64 {
    Fnv1a::new()
        .vec2(transform.translation)
        .vec2(velocity.0)
        .u32(sprite.color)
        .f32(sprite.half_extent)
        .u8(u8::from(hidden))
        .finish()
}

/// One draw item's term of the checksum: `vec2(position)`, `f32(half_extent)`, `u32(color)`.
#[must_use]
pub fn hash_draw_item(item: &DrawItem) -> u64 {
    Fnv1a::new()
        .vec2(item.position)
        .f32(item.half_extent)
        .u32(item.color)
        .finish()
}
