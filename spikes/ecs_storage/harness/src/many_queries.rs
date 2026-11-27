//! `many_queries`: eight component types in eight archetypes of four types each, and every
//! ordered pair of types run as its own query each step (M3.22 spec §4.2 and §4.3).
//!
//! The rules here are the M3.22 spec §4.2; the common rules of M3.2 §4.4 apply too. Entity `i` is
//! in group `i % 8` and has the four types `C(g)` to `C(g+3)`, indices modulo 8.

use engine::prelude::Prng;

use crate::{Fnv1a, Spec};

/// The `many_queries` workload's identity.
#[derive(Clone, Copy, Debug)]
pub struct ManyQueries;

impl Spec for ManyQueries {
    const NAME: &'static str = "many_queries";
    const SIZES: &'static [usize] = &[100, 1_000, 10_000];
}

/// The number of groups, and of component types: entity `i` is in group `i % GROUPS`.
pub const GROUPS: u8 = 8;

/// Component type 0 of the eight a `many_queries` entity draws four of; one `f32`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct C0(pub f32);

/// Component type 1 of the eight a `many_queries` entity draws four of; one `f32`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct C1(pub f32);

/// Component type 2 of the eight a `many_queries` entity draws four of; one `f32`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct C2(pub f32);

/// Component type 3 of the eight a `many_queries` entity draws four of; one `f32`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct C3(pub f32);

/// Component type 4 of the eight a `many_queries` entity draws four of; one `f32`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct C4(pub f32);

/// Component type 5 of the eight a `many_queries` entity draws four of; one `f32`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct C5(pub f32);

/// Component type 6 of the eight a `many_queries` entity draws four of; one `f32`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct C6(pub f32);

/// Component type 7 of the eight a `many_queries` entity draws four of; one `f32`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct C7(pub f32);

/// One entity's initial state: its group and the values of its four components.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spawn {
    /// The group, `i % 8`; the entity has types `C(group)` to `C(group + 3)`, modulo 8.
    pub group: u8,
    /// The values in spawn order: `values[k]` is the value of `C((group + k) % 8)`.
    pub values: [f32; 4],
}

/// Entity `i`'s initial state: group `i % 8`, then four draws in `0.0_f32..1.0` from
/// `prng.stream(i)`, in the order `C(g)`, `C(g+1)`, `C(g+2)`, `C(g+3)`.
#[must_use]
pub fn spawn(prng: &Prng, i: u64) -> Spawn {
    let mut r = prng.stream(i);
    let group = (i % u64::from(GROUPS)).to_le_bytes()[0];
    let values = [
        r.range(0.0_f32..1.0),
        r.range(0.0_f32..1.0),
        r.range(0.0_f32..1.0),
        r.range(0.0_f32..1.0),
    ];
    Spawn { group, values }
}

/// The rule of one query on one entity: `x = write + read * 0.5 + 0.25` (the multiplication, then
/// the additions left to right, not fused), then `write = x - 1.0` if `x >= 1.0`, else `x`.
#[inline]
pub fn update(write: &mut f32, read: f32) {
    let x = *write + read * 0.5 + 0.25;
    *write = if x >= 1.0 { x - 1.0 } else { x };
}

/// One entity's term of the checksum: `u8(group)`, then `f32` of each value in spawn order.
#[must_use]
pub fn hash(group: u8, values: [f32; 4]) -> u64 {
    let [a, b, c, d] = values;
    Fnv1a::new().u8(group).f32(a).f32(b).f32(c).f32(d).finish()
}

/// The 56 queries of a step, in order: every ordered pair `(write, read)` of distinct type
/// indices, write index major, read index minor.
pub const QUERIES: [(u8, u8); 56] = [
    (0, 1),
    (0, 2),
    (0, 3),
    (0, 4),
    (0, 5),
    (0, 6),
    (0, 7),
    (1, 0),
    (1, 2),
    (1, 3),
    (1, 4),
    (1, 5),
    (1, 6),
    (1, 7),
    (2, 0),
    (2, 1),
    (2, 3),
    (2, 4),
    (2, 5),
    (2, 6),
    (2, 7),
    (3, 0),
    (3, 1),
    (3, 2),
    (3, 4),
    (3, 5),
    (3, 6),
    (3, 7),
    (4, 0),
    (4, 1),
    (4, 2),
    (4, 3),
    (4, 5),
    (4, 6),
    (4, 7),
    (5, 0),
    (5, 1),
    (5, 2),
    (5, 3),
    (5, 4),
    (5, 6),
    (5, 7),
    (6, 0),
    (6, 1),
    (6, 2),
    (6, 3),
    (6, 4),
    (6, 5),
    (6, 7),
    (7, 0),
    (7, 1),
    (7, 2),
    (7, 3),
    (7, 4),
    (7, 5),
    (7, 6),
];

/// Calls `$m!(Cw, Cr)` for each of the 56 queries, in [`QUERIES`] order, with the harness's
/// types as paths, so a backend can wrap them (`Table<$write>`) without importing them.
///
/// Every ECS backend runs its step's queries through this macro, so none can drop or reorder one.
///
/// ```
/// use core::any::TypeId;
/// use spike_ecs_harness::many_queries::{QUERIES, C0, C1, C2, C3, C4, C5, C6, C7};
///
/// let ids = [
///     TypeId::of::<C0>(),
///     TypeId::of::<C1>(),
///     TypeId::of::<C2>(),
///     TypeId::of::<C3>(),
///     TypeId::of::<C4>(),
///     TypeId::of::<C5>(),
///     TypeId::of::<C6>(),
///     TypeId::of::<C7>(),
/// ];
/// let index_of = |id: TypeId| ids.iter().position(|&x| x == id).unwrap();
/// let mut seen: Vec<(usize, usize)> = Vec::new();
/// macro_rules! collect {
///     ($w:ty, $r:ty) => {
///         seen.push((index_of(TypeId::of::<$w>()), index_of(TypeId::of::<$r>())));
///     };
/// }
/// spike_ecs_harness::for_each_query!(collect);
/// assert_eq!(seen.len(), 56);
/// assert_eq!(seen[0], (0, 1));
/// assert_eq!(seen[55], (7, 6));
/// let expected: Vec<(usize, usize)> =
///     QUERIES.iter().map(|&(w, r)| (usize::from(w), usize::from(r))).collect();
/// assert_eq!(seen, expected);
/// ```
#[macro_export]
macro_rules! for_each_query {
    ($m:ident) => {
        $m!($crate::many_queries::C0, $crate::many_queries::C1);
        $m!($crate::many_queries::C0, $crate::many_queries::C2);
        $m!($crate::many_queries::C0, $crate::many_queries::C3);
        $m!($crate::many_queries::C0, $crate::many_queries::C4);
        $m!($crate::many_queries::C0, $crate::many_queries::C5);
        $m!($crate::many_queries::C0, $crate::many_queries::C6);
        $m!($crate::many_queries::C0, $crate::many_queries::C7);
        $m!($crate::many_queries::C1, $crate::many_queries::C0);
        $m!($crate::many_queries::C1, $crate::many_queries::C2);
        $m!($crate::many_queries::C1, $crate::many_queries::C3);
        $m!($crate::many_queries::C1, $crate::many_queries::C4);
        $m!($crate::many_queries::C1, $crate::many_queries::C5);
        $m!($crate::many_queries::C1, $crate::many_queries::C6);
        $m!($crate::many_queries::C1, $crate::many_queries::C7);
        $m!($crate::many_queries::C2, $crate::many_queries::C0);
        $m!($crate::many_queries::C2, $crate::many_queries::C1);
        $m!($crate::many_queries::C2, $crate::many_queries::C3);
        $m!($crate::many_queries::C2, $crate::many_queries::C4);
        $m!($crate::many_queries::C2, $crate::many_queries::C5);
        $m!($crate::many_queries::C2, $crate::many_queries::C6);
        $m!($crate::many_queries::C2, $crate::many_queries::C7);
        $m!($crate::many_queries::C3, $crate::many_queries::C0);
        $m!($crate::many_queries::C3, $crate::many_queries::C1);
        $m!($crate::many_queries::C3, $crate::many_queries::C2);
        $m!($crate::many_queries::C3, $crate::many_queries::C4);
        $m!($crate::many_queries::C3, $crate::many_queries::C5);
        $m!($crate::many_queries::C3, $crate::many_queries::C6);
        $m!($crate::many_queries::C3, $crate::many_queries::C7);
        $m!($crate::many_queries::C4, $crate::many_queries::C0);
        $m!($crate::many_queries::C4, $crate::many_queries::C1);
        $m!($crate::many_queries::C4, $crate::many_queries::C2);
        $m!($crate::many_queries::C4, $crate::many_queries::C3);
        $m!($crate::many_queries::C4, $crate::many_queries::C5);
        $m!($crate::many_queries::C4, $crate::many_queries::C6);
        $m!($crate::many_queries::C4, $crate::many_queries::C7);
        $m!($crate::many_queries::C5, $crate::many_queries::C0);
        $m!($crate::many_queries::C5, $crate::many_queries::C1);
        $m!($crate::many_queries::C5, $crate::many_queries::C2);
        $m!($crate::many_queries::C5, $crate::many_queries::C3);
        $m!($crate::many_queries::C5, $crate::many_queries::C4);
        $m!($crate::many_queries::C5, $crate::many_queries::C6);
        $m!($crate::many_queries::C5, $crate::many_queries::C7);
        $m!($crate::many_queries::C6, $crate::many_queries::C0);
        $m!($crate::many_queries::C6, $crate::many_queries::C1);
        $m!($crate::many_queries::C6, $crate::many_queries::C2);
        $m!($crate::many_queries::C6, $crate::many_queries::C3);
        $m!($crate::many_queries::C6, $crate::many_queries::C4);
        $m!($crate::many_queries::C6, $crate::many_queries::C5);
        $m!($crate::many_queries::C6, $crate::many_queries::C7);
        $m!($crate::many_queries::C7, $crate::many_queries::C0);
        $m!($crate::many_queries::C7, $crate::many_queries::C1);
        $m!($crate::many_queries::C7, $crate::many_queries::C2);
        $m!($crate::many_queries::C7, $crate::many_queries::C3);
        $m!($crate::many_queries::C7, $crate::many_queries::C4);
        $m!($crate::many_queries::C7, $crate::many_queries::C5);
        $m!($crate::many_queries::C7, $crate::many_queries::C6);
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::prelude::Prng;

    #[test]
    fn spawn_follows_the_draw_order() {
        let prng = Prng::from_seed(42);
        for i in [0_u64, 1, 7, 8, 999] {
            let mut r = prng.stream(i);
            let values = [
                r.range(0.0_f32..1.0),
                r.range(0.0_f32..1.0),
                r.range(0.0_f32..1.0),
                r.range(0.0_f32..1.0),
            ];
            let s = spawn(&prng, i);
            assert_eq!(u64::from(s.group), i % 8);
            assert_eq!(s.values, values);
        }
    }

    #[test]
    fn update_wraps_and_keeps_the_order() {
        let mut w = 0.5;
        update(&mut w, 0.25);
        assert_eq!(w, 0.875);
        let mut w = 0.75;
        update(&mut w, 0.5);
        assert_eq!(w, 0.25);
        let mut w = 0.0;
        update(&mut w, 0.0);
        assert_eq!(w, 0.25);
    }

    #[test]
    fn queries_are_every_ordered_pair() {
        let mut expected = Vec::new();
        for w in 0..8_u8 {
            for r in 0..8_u8 {
                if w != r {
                    expected.push((w, r));
                }
            }
        }
        assert_eq!(expected.len(), 56);
        assert_eq!(QUERIES.to_vec(), expected);
    }

    #[test]
    fn the_macro_expands_in_query_order() {
        use core::any::TypeId;
        fn index_of<T: 'static>(ids: &[TypeId; 8]) -> u8 {
            let i = ids.iter().position(|&x| x == TypeId::of::<T>()).unwrap();
            u8::try_from(i).unwrap()
        }
        let ids = [
            TypeId::of::<C0>(),
            TypeId::of::<C1>(),
            TypeId::of::<C2>(),
            TypeId::of::<C3>(),
            TypeId::of::<C4>(),
            TypeId::of::<C5>(),
            TypeId::of::<C6>(),
            TypeId::of::<C7>(),
        ];
        let mut seen: Vec<(u8, u8)> = Vec::new();
        macro_rules! collect {
            ($w:ty, $r:ty) => {
                seen.push((index_of::<$w>(&ids), index_of::<$r>(&ids)));
            };
        }
        for_each_query!(collect);
        assert_eq!(seen, QUERIES.to_vec());
    }
}
