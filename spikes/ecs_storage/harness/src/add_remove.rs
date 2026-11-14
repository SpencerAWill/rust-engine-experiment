//! `add_remove`: structural change in bulk. Every step adds one component to every entity, then
//! removes it again (spec §4.4.4).

use engine::prelude::Prng;

use crate::{Fnv1a, Spec};

/// The `add_remove` workload's identity.
#[derive(Clone, Copy, Debug)]
pub struct AddRemove;

impl Spec for AddRemove {
    const NAME: &'static str = "add_remove";
    const SIZES: &'static [usize] = &[1_000, 10_000, 100_000];
}

/// The component every entity has.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct A(pub f32);

/// The component the step adds and removes. No entity has it after `setup` or after a step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct B(pub f32);

/// Entity `i`'s `A`: one draw in `0..1`.
#[must_use]
pub fn spawn(prng: &Prng, i: u64) -> A {
    A(prng.stream(i).range(0.0_f32..1.0))
}

/// The `B` the step inserts for an entity with `a`: `B(a.0)`.
#[inline]
#[must_use]
pub fn make_b(a: &A) -> B {
    B(a.0)
}

/// One entity's term of the checksum: `f32(a)`, then `u8(0)` without `B`, or `u8(1)` and `f32(b)`
/// with it. A backend that leaves a `B` behind changes the checksum.
#[must_use]
pub fn hash(a: &A, b: Option<&B>) -> u64 {
    let h = Fnv1a::new().f32(a.0);
    match b {
        None => h.u8(0),
        Some(b) => h.u8(1).f32(b.0),
    }
    .finish()
}
