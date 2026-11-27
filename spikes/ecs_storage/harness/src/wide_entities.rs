//! `wide_entities`: 24 component types of mixed sizes on every entity, and a step that updates
//! four of them, then adds and removes a 25th (spec M3.23 §4.2).

use engine::prelude::Prng;

use crate::{Fnv1a, Spec};

/// The `wide_entities` workload's identity.
#[derive(Clone, Copy, Debug)]
pub struct WideEntities;

impl Spec for WideEntities {
    const NAME: &'static str = "wide_entities";
    const SIZES: &'static [usize] = &[1_000, 10_000, 100_000];
}

/// Defines the 24 wide types and [`Wide`], and the `spawn` and `hash` that walk them in order.
macro_rules! wide {
    ($($name:ident $field:ident [$n:literal] $bytes:literal),+ $(,)?) => {
        $(
            #[doc = concat!(
                "The wide component `", stringify!($name), "`: ", stringify!($n),
                " `f32` elements, ", stringify!($bytes), " bytes."
            )]
            #[derive(Clone, Copy, Debug, PartialEq)]
            pub struct $name(pub [f32; $n]);
        )+

        /// One entity's 24 wide values, one field per type, `w0` to `w23`.
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct Wide {
            $(
                #[doc = concat!("The entity's `", stringify!($name), "`.")]
                pub $field: $name,
            )+
        }

        /// Entity `i`'s 24 values: one draw in `0.0..1.0` from `prng.stream(i)` per element,
        /// `W0`'s elements in index order first, then `W1`'s, through `W23`'s (136 draws).
        #[must_use]
        pub fn spawn(prng: &Prng, i: u64) -> Wide {
            let mut r = prng.stream(i);
            Wide {
                $($field: $name(core::array::from_fn(|_| r.range(0.0_f32..1.0))),)+
            }
        }

        /// One entity's term of the checksum: `f32` of each of the 136 values in spawn order,
        /// then `u8(0)` without `Extra`, or `u8(1)` and `f32(extra)` with it. A backend that
        /// drops a column, leaves a stale value or leaves an `Extra` behind changes the checksum.
        #[must_use]
        pub fn hash(wide: &Wide, extra: Option<&Extra>) -> u64 {
            let mut h = Fnv1a::new();
            $(
                for x in wide.$field.0 {
                    h = h.f32(x);
                }
            )+
            match extra {
                None => h.u8(0),
                Some(e) => h.u8(1).f32(e.0),
            }
            .finish()
        }
    };
}

wide!(
    W0 w0 [1] 4,
    W1 w1 [2] 8,
    W2 w2 [3] 12,
    W3 w3 [4] 16,
    W4 w4 [8] 32,
    W5 w5 [16] 64,
    W6 w6 [1] 4,
    W7 w7 [2] 8,
    W8 w8 [3] 12,
    W9 w9 [4] 16,
    W10 w10 [8] 32,
    W11 w11 [16] 64,
    W12 w12 [1] 4,
    W13 w13 [2] 8,
    W14 w14 [3] 12,
    W15 w15 [4] 16,
    W16 w16 [8] 32,
    W17 w17 [16] 64,
    W18 w18 [1] 4,
    W19 w19 [2] 8,
    W20 w20 [3] 12,
    W21 w21 [4] 16,
    W22 w22 [8] 32,
    W23 w23 [16] 64,
);

/// The component the step adds and removes: 4 bytes. No entity has it after `setup` or after a
/// step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extra(pub f32);

/// The rule for one element: `y = x + 0.618_034`, then `y - 1.0` if `y >= 1.0`, else `y`.
#[inline]
fn rotate(x: &mut f32) {
    let y = *x + 0.618_034;
    *x = if y >= 1.0 { y - 1.0 } else { y };
}

/// The update pass's rule for one entity: every element of `W0`, `W7`, `W14` and `W21` (1 + 2 +
/// 3 + 4 elements) is rotated by the amended `frag_iter` rule (M3.26 §4.2). The other 20 types
/// are not touched. Values stay in `0.0..1.0`.
#[inline]
pub fn update(a: &mut W0, b: &mut W7, c: &mut W14, d: &mut W21) {
    a.0.iter_mut().for_each(rotate);
    b.0.iter_mut().for_each(rotate);
    c.0.iter_mut().for_each(rotate);
    d.0.iter_mut().for_each(rotate);
}

/// The `Extra` the step inserts for an entity whose `W0` is `w0` (read after the update pass):
/// `Extra(w0.0[0])`.
#[inline]
#[must_use]
pub fn make_extra(w0: &W0) -> Extra {
    Extra(w0.0[0])
}

#[cfg(test)]
mod tests {
    use core::mem::size_of;

    use engine::prelude::Prng;

    use super::{Extra, W0, W7, W14, W21, Wide, hash, make_extra, spawn, update};

    /// Calls `$callback!` with the 24 `(index, field)` pairs.
    macro_rules! each_field {
        ($callback:ident, $($args:tt)*) => {
            $callback!($($args)* 0 w0 1 w1 2 w2 3 w3 4 w4 5 w5 6 w6 7 w7 8 w8 9 w9 10 w10 11 w11
                12 w12 13 w13 14 w14 15 w15 16 w16 17 w17 18 w18 19 w19 20 w20 21 w21
                22 w22 23 w23)
        };
    }

    /// The 136 values of `wide` in type then element order.
    fn values(wide: &Wide) -> Vec<f32> {
        let mut out = Vec::new();
        macro_rules! push {
            ($($k:literal $f:ident)+) => { $(out.extend_from_slice(&wide.$f.0);)+ };
        }
        each_field!(push,);
        out
    }

    /// The first element of the type with index `k`, mutable.
    fn first_element(wide: &mut Wide, k: usize) -> &mut f32 {
        macro_rules! pick {
            ($w:ident, $k:ident, $($i:literal $f:ident)+) => {
                match $k { $($i => &mut $w.$f.0[0],)+ _ => unreachable!() }
            };
        }
        each_field!(pick, wide, k,)
    }

    #[test]
    fn sizes_follow_the_cycle() {
        let wide = spawn(&Prng::from_seed(1), 0);
        let mut total = 0;
        macro_rules! check {
            ($($k:literal $f:ident)+) => {$({
                let wanted = 4 * [1_usize, 2, 3, 4, 8, 16][$k % 6];
                let size = size_of_val(&wide.$f);
                assert_eq!(size, wanted, "type {} is {size} bytes", $k);
                total += size;
            })+};
        }
        each_field!(check,);
        assert_eq!(total, 544);
        assert_eq!(size_of::<Wide>(), 544);
    }

    #[test]
    fn spawn_follows_the_draw_order() {
        let prng = Prng::from_seed(42);
        for i in [0_u64, 1, 999] {
            let mut r = prng.stream(i);
            let expected: Vec<f32> = (0..136).map(|_| r.range(0.0_f32..1.0)).collect();
            assert_eq!(values(&spawn(&prng, i)), expected, "entity {i}");
        }
    }

    #[test]
    fn update_wraps_and_keeps_the_order() {
        let step = |x: f32| {
            let (mut a, mut b, mut c, mut d) = (W0([x]), W7([x; 2]), W14([x; 3]), W21([x; 4]));
            update(&mut a, &mut b, &mut c, &mut d);
            (a.0.to_vec(), b.0.to_vec(), c.0.to_vec(), d.0.to_vec())
        };
        let (a, b, c, d) = step(0.25);
        let plain = 0.25_f32 + 0.618_034;
        assert_eq!(plain, 0.868_034);
        assert_eq!(a, vec![plain; 1]);
        assert_eq!(b, vec![plain; 2]);
        assert_eq!(c, vec![plain; 3]);
        assert_eq!(d, vec![plain; 4]);
        let (a, b, c, d) = step(0.5);
        let wrapped = 0.5_f32 + 0.618_034 - 1.0;
        assert_eq!(a, vec![wrapped; 1]);
        assert_eq!(b, vec![wrapped; 2]);
        assert_eq!(c, vec![wrapped; 3]);
        assert_eq!(d, vec![wrapped; 4]);
    }

    #[test]
    fn make_extra_reads_w0() {
        assert_eq!(make_extra(&W0([0.375])), Extra(0.375));
    }

    #[test]
    fn hash_sees_every_type() {
        let prng = Prng::from_seed(7);
        let base = spawn(&prng, 3);
        let base_hash = hash(&base, None);
        for k in 0..24 {
            let mut changed = base;
            *first_element(&mut changed, k) += 0.5;
            assert_ne!(hash(&changed, None), base_hash, "type {k} is not hashed");
        }
        assert_ne!(hash(&base, Some(&Extra(0.5))), base_hash);
        assert_ne!(
            hash(&base, Some(&Extra(0.5))),
            hash(&base, Some(&Extra(0.25)))
        );
    }
}
