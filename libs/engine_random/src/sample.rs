//! Bounded sampling on top of [`Prng`]: ranges, `chance`, `shuffle`, `choose`, `fill_bytes`.
//!
//! Every algorithm here is normative: the output streams are frozen for all of 0.x
//! (see the [crate docs](crate)), so these definitions are part of the contract.

use core::fmt::Debug;
use core::ops::{Range, RangeInclusive};

use crate::prng::Prng;

mod sealed {
    /// Prevents implementations outside this crate.
    pub trait Sealed {}

    /// An argument no code outside this crate can construct, so the sampling method of
    /// [`SampleRange`](super::SampleRange) cannot be called (or implemented) from outside.
    #[derive(Debug)]
    pub struct Token(pub(super) ());
}

/// A range [`Prng::range`] can sample from.
///
/// Implemented for [`Range`] and [`RangeInclusive`] over `u8`, `u16`, `u32`, `u64`, `usize`,
/// `i8`, `i16`, `i32`, `i64`, `isize`, `f32`, and `f64`. The trait is sealed: it cannot be
/// implemented outside this crate, and it has no public methods.
///
/// # Examples
///
/// ```
/// use engine_random::Prng;
///
/// let mut rng = Prng::from_seed(1);
/// let die = rng.range(1..=6); // RangeInclusive<i32>
/// let angle = rng.range(0.0_f32..360.0); // Range<f32>
/// # let _ = (die, angle);
/// ```
pub trait SampleRange: sealed::Sealed + Debug {
    /// The type of the sampled value.
    type Output;

    #[doc(hidden)]
    fn sample(&self, prng: &mut Prng, token: sealed::Token) -> Option<Self::Output>;
}

/// Uniform sample of `[lo, hi]` over wrapped 64-bit arithmetic (`lo <= hi` in the caller's
/// ordering). The full 64-bit span is the one case that cannot go through `bounded_u64`.
#[inline]
fn sample_inclusive(prng: &mut Prng, lo: u64, hi: u64) -> u64 {
    let span = hi.wrapping_sub(lo);
    if span == u64::MAX {
        lo.wrapping_add(prng.next_u64())
    } else {
        lo.wrapping_add(prng.bounded_u64(span + 1))
    }
}

/// Integer ranges. Both bounds widen to 64 bits (unsigned via zero-extension, signed via
/// sign-extension then reinterpretation), so every integer type, `usize` and `isize`
/// included, takes the same 64-bit path on every target.
macro_rules! impl_int_range {
    ($($t:ty => |$v:ident| $widen:expr),* $(,)?) => {$(
        impl sealed::Sealed for Range<$t> {}
        impl sealed::Sealed for RangeInclusive<$t> {}

        impl SampleRange for Range<$t> {
            type Output = $t;

            #[inline]
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_possible_wrap,
                clippy::cast_sign_loss,
                clippy::unnecessary_cast,
                reason = "widening to u64 and narrowing back are the documented algorithm"
            )]
            fn sample(&self, prng: &mut Prng, _: sealed::Token) -> Option<$t> {
                if self.start >= self.end {
                    return None;
                }
                let lo = { let $v = self.start; $widen };
                let hi = { let $v = self.end - 1; $widen };
                Some(sample_inclusive(prng, lo, hi) as $t)
            }
        }

        impl SampleRange for RangeInclusive<$t> {
            type Output = $t;

            #[inline]
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_possible_wrap,
                clippy::cast_sign_loss,
                clippy::unnecessary_cast,
                reason = "widening to u64 and narrowing back are the documented algorithm"
            )]
            fn sample(&self, prng: &mut Prng, _: sealed::Token) -> Option<$t> {
                if self.start() > self.end() {
                    return None;
                }
                let lo = { let $v = *self.start(); $widen };
                let hi = { let $v = *self.end(); $widen };
                Some(sample_inclusive(prng, lo, hi) as $t)
            }
        }
    )*};
}

impl_int_range! {
    u8 => |v| u64::from(v),
    u16 => |v| u64::from(v),
    u32 => |v| u64::from(v),
    u64 => |v| v,
    usize => |v| v as u64,
    i8 => |v| i64::from(v) as u64,
    i16 => |v| i64::from(v) as u64,
    i32 => |v| i64::from(v) as u64,
    i64 => |v| v as u64,
    isize => |v| v as i64 as u64,
}

/// Float ranges. `lo..hi` redraws until the result is below `hi`; `lo..=hi` maps a uniform
/// value over `[0, 1]` inclusive and clamps. Both reject non-finite bounds and spans.
macro_rules! impl_float_range {
    ($t:ty, $next:ident, $shift:literal, $denominator:literal) => {
        impl sealed::Sealed for Range<$t> {}
        impl sealed::Sealed for RangeInclusive<$t> {}

        impl SampleRange for Range<$t> {
            type Output = $t;

            fn sample(&self, prng: &mut Prng, _: sealed::Token) -> Option<$t> {
                let (lo, hi) = (self.start, self.end);
                if !(lo.is_finite() && hi.is_finite() && lo < hi && (hi - lo).is_finite()) {
                    return None;
                }
                let span = hi - lo;
                // `u = 0` gives `lo < hi`, so this terminates with probability 1.
                loop {
                    let x = lo + span * prng.$next();
                    if x < hi {
                        return Some(x);
                    }
                }
            }
        }

        impl SampleRange for RangeInclusive<$t> {
            type Output = $t;

            #[allow(
                clippy::cast_precision_loss,
                reason = "the shifted value fits the mantissa exactly"
            )]
            fn sample(&self, prng: &mut Prng, _: sealed::Token) -> Option<$t> {
                let (lo, hi) = (*self.start(), *self.end());
                if !(lo.is_finite() && hi.is_finite() && lo <= hi && (hi - lo).is_finite()) {
                    return None;
                }
                let u = (prng.next_u64() >> $shift) as $t / $denominator;
                Some((lo + (hi - lo) * u).min(hi))
            }
        }
    };
}

impl_float_range!(f32, next_f32, 40, 16_777_215.0); // 2^24 - 1
impl_float_range!(f64, next_f64, 11, 9_007_199_254_740_991.0); // 2^53 - 1

impl Prng {
    /// Uniform value in `[0, n)` for `n >= 1`, by Lemire's nearly-divisionless method in
    /// 64 bits. `n == 0` is a caller bug (debug-asserted).
    #[inline]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "taking the low and high 64-bit halves is the algorithm"
    )]
    pub(crate) fn bounded_u64(&mut self, n: u64) -> u64 {
        debug_assert!(n >= 1, "bounded_u64 needs n >= 1");
        let mut m = u128::from(self.next_u64()) * u128::from(n);
        let mut lo = m as u64;
        if lo < n {
            let threshold = n.wrapping_neg() % n;
            while lo < threshold {
                m = u128::from(self.next_u64()) * u128::from(n);
                lo = m as u64;
            }
        }
        (m >> 64) as u64
    }

    /// Returns a uniform value in `range`.
    ///
    /// Works for `Range` and `RangeInclusive` over every primitive integer and float
    /// ([`SampleRange`]). Integer ranges use Lemire's nearly-divisionless method in 64 bits
    /// on every target, so results never depend on pointer width.
    ///
    /// # Panics
    ///
    /// Panics if the range is empty or invalid: `start >= end` for a `Range`, `start > end`
    /// for a `RangeInclusive`, or a float range with a non-finite bound or span. Use
    /// [`try_range`](Self::try_range) to handle that without panicking.
    ///
    /// # Examples
    ///
    /// ```
    /// use engine_random::Prng;
    ///
    /// let mut rng = Prng::from_seed(9);
    /// let d6 = rng.range(1..=6);
    /// assert!((1..=6).contains(&d6));
    /// let x = rng.range(-1.0_f32..1.0);
    /// assert!((-1.0..1.0).contains(&x));
    /// ```
    ///
    /// Only the primitive numeric types are supported:
    ///
    /// ```compile_fail
    /// use engine_random::Prng;
    ///
    /// let mut p = Prng::from_seed(0);
    /// let _ = p.range(0u128..5);
    /// ```
    #[allow(
        clippy::needless_pass_by_value,
        reason = "ranges are passed by value so `rng.range(0..10)` reads naturally (spec §5.2)"
    )]
    #[track_caller]
    pub fn range<R: SampleRange>(&mut self, range: R) -> R::Output {
        match range.sample(self, sealed::Token(())) {
            Some(value) => value,
            None => panic!(
                "Prng::range: empty range {range:?} (a Range needs start < end, a RangeInclusive \
                 needs start <= end, and float bounds must be finite with a finite span); \
                 use try_range to handle this without panicking"
            ),
        }
    }

    /// Fallible twin of [`range`](Self::range): `None` for an empty or invalid range.
    /// Draws nothing when it returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_random::Prng;
    /// let mut rng = Prng::from_seed(9);
    /// assert_eq!(rng.try_range(5..5), None);
    /// assert!(rng.try_range(0..=1).is_some());
    /// ```
    #[allow(
        clippy::needless_pass_by_value,
        reason = "ranges are passed by value so `rng.range(0..10)` reads naturally (spec §5.2)"
    )]
    pub fn try_range<R: SampleRange>(&mut self, range: R) -> Option<R::Output> {
        range.sample(self, sealed::Token(()))
    }

    /// Returns `true` with probability `p`: `next_f64() < p`.
    ///
    /// `p <= 0.0` is never true, `p >= 1.0` is always true, and `NaN` is always false.
    /// It never panics, and it always consumes one draw.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_random::Prng;
    /// let mut rng = Prng::from_seed(10);
    /// assert!(rng.chance(1.0));
    /// assert!(!rng.chance(0.0));
    /// ```
    #[inline]
    pub fn chance(&mut self, p: f64) -> bool {
        self.next_f64() < p
    }

    /// Shuffles `slice` in place (Fisher–Yates, descending).
    ///
    /// Slices of length 0 or 1 draw nothing.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_random::Prng;
    /// let mut deck = [1, 2, 3, 4, 5];
    /// Prng::from_seed(11).shuffle(&mut deck);
    /// deck.sort_unstable();
    /// assert_eq!(deck, [1, 2, 3, 4, 5]); // still a permutation
    /// ```
    #[allow(
        clippy::cast_possible_truncation,
        reason = "j <= i, which is a valid usize"
    )]
    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        for i in (1..slice.len()).rev() {
            let j = self.bounded_u64(i as u64 + 1) as usize;
            slice.swap(i, j);
        }
    }

    /// Returns a uniformly chosen element, or `None` for an empty slice (which draws nothing).
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_random::Prng;
    /// let mut rng = Prng::from_seed(12);
    /// assert!(rng.choose(&["a", "b", "c"]).is_some());
    /// assert_eq!(rng.choose::<u8>(&[]), None);
    /// ```
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the result is below slice.len()"
    )]
    pub fn choose<'a, T>(&mut self, slice: &'a [T]) -> Option<&'a T> {
        if slice.is_empty() {
            return None;
        }
        slice.get(self.bounded_u64(slice.len() as u64) as usize)
    }

    /// Fills `dest` with random bytes.
    ///
    /// Bytes come in 8-byte chunks of `next_u64().to_le_bytes()`. A final partial chunk of
    /// `k` bytes takes the first `k` bytes of one more word and discards the rest. The
    /// layout is little-endian on every target.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_random::Prng;
    /// let mut buf = [0_u8; 13];
    /// Prng::from_seed(13).fill_bytes(&mut buf);
    /// assert_ne!(buf, [0; 13]);
    /// ```
    pub fn fill_bytes(&mut self, dest: &mut [u8]) {
        let (chunks, rest) = dest.as_chunks_mut::<8>();
        for chunk in chunks {
            *chunk = self.next_u64().to_le_bytes();
        }
        if !rest.is_empty() {
            let word = self.next_u64().to_le_bytes();
            rest.copy_from_slice(&word[..rest.len()]);
        }
    }
}
