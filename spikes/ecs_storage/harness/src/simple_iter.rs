//! `simple_iter`: every entity has four components and the step moves each position by its
//! velocity (spec §4.4.1).

use engine::prelude::{Mat4, Prng, Vec3};

use crate::{Fnv1a, Spec};

/// The `simple_iter` workload's identity.
#[derive(Clone, Copy, Debug)]
pub struct SimpleIter;

impl Spec for SimpleIter {
    const NAME: &'static str = "simple_iter";
    const SIZES: &'static [usize] = &[1_000, 10_000, 100_000];
}

/// A transform matrix. Present to make the entity 100 bytes; the step never touches it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix(pub Mat4);

/// A position in space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position(pub Vec3);

/// A rotation, as three components. The step never reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rotation(pub Vec3);

/// A velocity, added to the position each step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Velocity(pub Vec3);

/// Entity `i`'s components: position (3 draws in `-100..100`), rotation and velocity (3 draws
/// each in `-1..1`), and an identity matrix.
#[must_use]
pub fn spawn(prng: &Prng, i: u64) -> (Matrix, Position, Rotation, Velocity) {
    let mut r = prng.stream(i);
    let position = Vec3::new(
        r.range(-100.0_f32..100.0),
        r.range(-100.0_f32..100.0),
        r.range(-100.0_f32..100.0),
    );
    let rotation = Vec3::new(
        r.range(-1.0_f32..1.0),
        r.range(-1.0_f32..1.0),
        r.range(-1.0_f32..1.0),
    );
    let velocity = Vec3::new(
        r.range(-1.0_f32..1.0),
        r.range(-1.0_f32..1.0),
        r.range(-1.0_f32..1.0),
    );
    (
        Matrix(Mat4::IDENTITY),
        Position(position),
        Rotation(rotation),
        Velocity(velocity),
    )
}

/// The rule for one entity: `position = position + velocity`.
#[inline]
pub fn update(position: &mut Position, velocity: &Velocity) {
    position.0 += velocity.0;
}

/// One entity's term of the checksum: `vec3(position)`, `vec3(velocity)`.
#[must_use]
pub fn hash(position: &Position, velocity: &Velocity) -> u64 {
    Fnv1a::new().vec3(position.0).vec3(velocity.0).finish()
}
