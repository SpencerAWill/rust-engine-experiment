//! The [`Prng`] generator.

use crate::splitmix::{SplitMix64, mix64};

/// `2^-24`, exactly representable: scales a 24-bit integer into `[0, 1)`.
const F32_UNIT: f32 = 1.0 / 16_777_216.0;
/// `2^-53`, exactly representable: scales a 53-bit integer into `[0, 1)`.
const F64_UNIT: f64 = 1.0 / 9_007_199_254_740_992.0;

/// A fast, deterministic pseudorandom number generator (xoshiro256++, seeded by `SplitMix64`).
///
/// The same seed produces the same sequence on every target, and every method's output
/// stream is frozen for all of 0.x (see the [crate docs](crate)).
///
/// **Not cryptographically secure.** Never use it for secrets, tokens, or anything an
/// adversary should not predict.
///
/// # Deliberately not `Copy` and not `Default`
///
/// - **Not `Copy`:** an implicit copy silently duplicates the generator, so two code paths
///   would draw identical "random" values. Duplicating must be explicit: [`clone`](Clone::clone),
///   or [`stream`](Self::stream) for an independent sequence.
/// - **Not `Default`:** a default would hide the seed, which is the one input that determines
///   everything. Pick a seed on purpose.
///
/// ```compile_fail,E0382
/// use engine_random::Prng;
///
/// let a = Prng::from_seed(1);
/// let b = a; // a move, not a copy
/// let _ = a.clone(); // error: `a` was moved
/// # let _ = b;
/// ```
///
/// ```compile_fail,E0599
/// use engine_random::Prng;
///
/// let _ = Prng::default(); // there is no default seed
/// ```
///
/// # Examples
///
/// ```
/// use engine_random::Prng;
///
/// let mut rng = Prng::from_seed(42);
/// let a = rng.next_u64();
/// let mut again = Prng::from_seed(42);
/// assert_eq!(a, again.next_u64()); // deterministic
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Prng {
    state: [u64; 4],
}

impl Prng {
    /// Expands `seed` into 256 bits of state using four successive `SplitMix64` outputs.
    ///
    /// Any `u64` is a valid seed, including 0. The resulting state is never all zeros
    /// (the one invalid xoshiro state): `SplitMix64`'s output function is a bijection, so
    /// at most one of four consecutive outputs can be zero.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_random::Prng;
    /// let _ = Prng::from_seed(0);
    /// ```
    #[must_use]
    pub const fn from_seed(seed: u64) -> Self {
        let mut sm = SplitMix64::new(seed);
        Self {
            state: [sm.next_u64(), sm.next_u64(), sm.next_u64(), sm.next_u64()],
        }
    }

    /// Returns the next 64 random bits (one xoshiro256++ step).
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_random::Prng;
    /// let mut rng = Prng::from_seed(1);
    /// assert_ne!(rng.next_u64(), rng.next_u64());
    /// ```
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let [s0, s1, s2, s3] = &mut self.state;
        let result = s0.wrapping_add(*s3).rotate_left(23).wrapping_add(*s0);
        let t = *s1 << 17;
        *s2 ^= *s0;
        *s3 ^= *s1;
        *s1 ^= *s2;
        *s0 ^= *s3;
        *s2 ^= t;
        *s3 = s3.rotate_left(45);
        result
    }

    /// Returns the upper 32 bits of [`next_u64`](Self::next_u64).
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_random::Prng;
    /// let mut a = Prng::from_seed(3);
    /// let mut b = a.clone();
    /// assert_eq!(u64::from(a.next_u32()), b.next_u64() >> 32);
    /// ```
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Returns a uniform `f32` in `[0, 1)`: `(next_u64() >> 40) as f32 * 2^-24`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_random::Prng;
    /// let x = Prng::from_seed(4).next_f32();
    /// assert!((0.0..1.0).contains(&x));
    /// ```
    #[inline]
    #[expect(
        clippy::cast_precision_loss,
        reason = "a 24-bit integer is exactly representable in f32"
    )]
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 * F32_UNIT
    }

    /// Returns a uniform `f64` in `[0, 1)`: `(next_u64() >> 11) as f64 * 2^-53`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_random::Prng;
    /// let x = Prng::from_seed(5).next_f64();
    /// assert!((0.0..1.0).contains(&x));
    /// ```
    #[inline]
    #[expect(
        clippy::cast_precision_loss,
        reason = "a 53-bit integer is exactly representable in f64"
    )]
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * F64_UNIT
    }

    /// Returns a fair coin flip: the top bit of [`next_u64`](Self::next_u64).
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_random::Prng;
    /// let _heads: bool = Prng::from_seed(6).next_bool();
    /// ```
    #[inline]
    pub fn next_bool(&mut self) -> bool {
        self.next_u64() >> 63 == 1
    }

    /// Derives an independent generator that is a pure function of this generator's
    /// current state and `id`. Does **not** advance `self`.
    ///
    /// The typical use is a per-entity stream: `world_prng.stream(entity.to_bits())`.
    /// The same entity then gets the same sequence regardless of system execution order or
    /// thread assignment. Definition (frozen):
    /// `Prng::from_seed(mix(s0 ^ s1.rotate_left(17) ^ s2.rotate_left(31) ^ s3.rotate_left(47) ^ mix(id)))`,
    /// where `mix` is the `SplitMix64` finalizer.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_random::Prng;
    /// let world = Prng::from_seed(7);
    /// let mut a = world.stream(1);
    /// let mut b = world.stream(1);
    /// assert_eq!(a.next_u64(), b.next_u64()); // same id, same stream
    /// assert_ne!(world.stream(1), world.stream(2));
    /// ```
    #[must_use]
    pub fn stream(&self, id: u64) -> Self {
        let [s0, s1, s2, s3] = self.state;
        Self::from_seed(mix64(
            s0 ^ s1.rotate_left(17) ^ s2.rotate_left(31) ^ s3.rotate_left(47) ^ mix64(id),
        ))
    }

    /// The full generator state, for save/load, replays, and rollback.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_random::Prng;
    /// let mut rng = Prng::from_seed(8);
    /// let saved = rng.to_state();
    /// let next = rng.next_u64();
    /// let mut restored = Prng::from_state(saved).unwrap();
    /// assert_eq!(restored.next_u64(), next);
    /// ```
    #[must_use]
    pub const fn to_state(&self) -> [u64; 4] {
        self.state
    }

    /// Restores a generator from a state returned by [`to_state`](Self::to_state).
    ///
    /// Returns `None` if and only if all four words are zero, the one invalid xoshiro256++
    /// state. [`from_seed`](Self::from_seed) can never produce it.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_random::Prng;
    /// assert!(Prng::from_state([0; 4]).is_none());
    /// assert!(Prng::from_state([0, 0, 0, 1]).is_some());
    /// ```
    #[must_use]
    pub const fn from_state(state: [u64; 4]) -> Option<Self> {
        if state[0] | state[1] | state[2] | state[3] == 0 {
            None
        } else {
            Some(Self { state })
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::*;

    #[test]
    fn from_state_rejects_all_zero() {
        assert_eq!(Prng::from_state([0; 4]), None);
        assert!(Prng::from_state([0, 0, 0, 1]).is_some());
    }

    #[test]
    fn from_state_continues_identically() {
        let mut original = Prng::from_seed(11);
        for _ in 0..10 {
            original.next_u64();
        }
        let mut restored = Prng::from_state(original.to_state()).unwrap();
        for _ in 0..100 {
            assert_eq!(original.next_u64(), restored.next_u64());
        }
    }

    #[test]
    fn stream_does_not_advance_self_and_ids_differ() {
        let rng = Prng::from_seed(12);
        let before = rng.clone();
        let one = rng.stream(1);
        let two = rng.stream(2);
        assert_eq!(rng, before);
        assert_ne!(one, two);
        assert_eq!(rng.stream(1), one); // pure function of (state, id)
    }

    #[test]
    fn floats_lie_in_the_unit_interval() {
        let mut rng = Prng::from_seed(13);
        for _ in 0..10_000 {
            assert!((0.0..1.0).contains(&rng.next_f32()));
            assert!((0.0..1.0).contains(&rng.next_f64()));
        }
    }

    #[test]
    fn debug_shows_state() {
        let rng = Prng::from_state([1, 2, 3, 4]).unwrap();
        assert_eq!(alloc::format!("{rng:?}"), "Prng { state: [1, 2, 3, 4] }");
    }

    #[test]
    fn from_seed_never_yields_the_zero_state() {
        for seed in 0..1000 {
            assert_ne!(Prng::from_seed(seed).to_state(), [0; 4]);
        }
    }
}
