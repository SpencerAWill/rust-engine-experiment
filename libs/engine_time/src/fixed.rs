//! [`FixedTimestep`]: variable elapsed time in, whole fixed simulation steps out.

use core::iter::FusedIterator;
use core::num::NonZeroU32;
use core::time::Duration;

use crate::Tick;

const NANOS_PER_SEC: u128 = 1_000_000_000;

/// Builds a `Duration` from a nanosecond count that is known to fit (it never exceeds the
/// nanoseconds of a `Duration`, because it was derived from one).
#[expect(
    clippy::cast_possible_truncation,
    reason = "seconds fit u64 and the sub-second part fits u32 for any in-range Duration"
)]
fn duration_from_nanos(nanos: u128) -> Duration {
    Duration::new(
        (nanos / NANOS_PER_SEC) as u64,
        (nanos % NANOS_PER_SEC) as u32,
    )
}

/// Converts variable elapsed time into a whole number of fixed simulation steps.
///
/// This is the classic fixed-timestep accumulator: each frame the host reports how much real
/// time passed, and the timestep says how many fixed-size simulation steps to run. The
/// leftover fraction of a step is kept for next time and exposed as [`alpha`](Self::alpha) for
/// render interpolation.
///
/// Arithmetic uses integer nanoseconds, so accumulation is exact. Splitting `elapsed`
/// differently across [`accumulate`](Self::accumulate) calls yields the same sequence of ticks
/// as long as the step cap is not hit (property-tested).
///
/// To avoid the "spiral of death" (a slow frame demanding more steps, which makes the next
/// frame slower), at most [`max_steps_per_update`](Self::with_max_steps_per_update) steps are
/// produced per call; time beyond that cap is discarded and tallied in
/// [`dropped_time`](Self::dropped_time).
///
/// # Examples
///
/// ```
/// use core::num::NonZeroU32;
/// use core::time::Duration;
/// use engine_time::FixedTimestep;
///
/// let mut timestep = FixedTimestep::from_hz(NonZeroU32::new(60).unwrap());
///
/// // A 40 ms frame at 60 Hz runs two whole steps and keeps the remainder.
/// for tick in timestep.accumulate(Duration::from_millis(40)) {
///     // simulate(tick);
///     # let _ = tick;
/// }
/// assert_eq!(timestep.current_tick().get(), 2);
/// assert!(timestep.alpha() < 1.0);
/// ```
#[derive(Clone, Debug)]
pub struct FixedTimestep {
    step: Duration,
    accumulator: Duration,
    max_steps: NonZeroU32,
    tick: Tick,
    dropped: Duration,
}

impl FixedTimestep {
    /// The default cap on steps produced by one [`accumulate`](Self::accumulate) call: 8.
    pub const DEFAULT_MAX_STEPS_PER_UPDATE: NonZeroU32 = match NonZeroU32::new(8) {
        Some(n) => n,
        None => panic!("8 is non-zero"),
    };

    /// A timestep of `step` per simulation step.
    ///
    /// # Panics
    ///
    /// Panics if `step` is zero. Use [`try_new`](Self::try_new) to handle that without
    /// panicking.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::FixedTimestep;
    /// # use core::time::Duration;
    /// let t = FixedTimestep::new(Duration::from_millis(10));
    /// assert_eq!(t.step(), Duration::from_millis(10));
    /// ```
    #[track_caller]
    pub fn new(step: Duration) -> Self {
        let Some(timestep) = Self::try_new(step) else {
            panic!(
                "FixedTimestep::new: step must be non-zero, got {step:?}; use try_new to handle this without panicking"
            )
        };
        timestep
    }

    /// Fallible twin of [`new`](Self::new): `None` if `step` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::FixedTimestep;
    /// # use core::time::Duration;
    /// assert!(FixedTimestep::try_new(Duration::ZERO).is_none());
    /// assert!(FixedTimestep::try_new(Duration::from_millis(1)).is_some());
    /// ```
    pub fn try_new(step: Duration) -> Option<Self> {
        (!step.is_zero()).then_some(Self {
            step,
            accumulator: Duration::ZERO,
            max_steps: Self::DEFAULT_MAX_STEPS_PER_UPDATE,
            tick: Tick::ZERO,
            dropped: Duration::ZERO,
        })
    }

    /// A timestep running at `hz` steps per second.
    ///
    /// The step is exactly `round(1e9 / hz)` nanoseconds, computed in integer arithmetic as
    /// `(1_000_000_000 + hz / 2) / hz` (60 Hz is 16,666,667 ns), clamped to at least 1 ns for
    /// rates above 2 GHz. Never panics.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::FixedTimestep;
    /// # use core::{num::NonZeroU32, time::Duration};
    /// let t = FixedTimestep::from_hz(NonZeroU32::new(60).unwrap());
    /// assert_eq!(t.step(), Duration::from_nanos(16_666_667));
    /// ```
    pub fn from_hz(hz: NonZeroU32) -> Self {
        let hz = u64::from(hz.get());
        let nanos = ((1_000_000_000 + hz / 2) / hz).max(1);
        Self {
            step: Duration::from_nanos(nanos),
            accumulator: Duration::ZERO,
            max_steps: Self::DEFAULT_MAX_STEPS_PER_UPDATE,
            tick: Tick::ZERO,
            dropped: Duration::ZERO,
        }
    }

    /// Sets the cap on steps per [`accumulate`](Self::accumulate) call.
    ///
    /// `NonZeroU32` makes a zero cap unrepresentable instead of a runtime panic.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::FixedTimestep;
    /// # use core::{num::NonZeroU32, time::Duration};
    /// let mut t = FixedTimestep::new(Duration::from_millis(10))
    ///     .with_max_steps_per_update(NonZeroU32::new(2).unwrap());
    /// assert_eq!(t.accumulate(Duration::from_millis(100)).len(), 2);
    /// ```
    #[must_use]
    pub const fn with_max_steps_per_update(mut self, max_steps: NonZeroU32) -> Self {
        self.max_steps = max_steps;
        self
    }

    /// Adds `elapsed` and commits up to the cap's worth of whole steps, returning their ticks.
    ///
    /// Steps are committed when this returns, whether or not the returned [`Steps`] is
    /// consumed. If the cap is hit, the time beyond `max_steps * step` is discarded (only the
    /// fractional remainder is kept) and added to [`dropped_time`](Self::dropped_time).
    /// Saturates instead of panicking on huge `elapsed` values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::{FixedTimestep, Tick};
    /// # use core::time::Duration;
    /// let mut t = FixedTimestep::new(Duration::from_millis(10));
    /// assert_eq!(t.accumulate(Duration::from_millis(6)).len(), 0); // not a whole step yet
    /// let ticks: Vec<Tick> = t.accumulate(Duration::from_millis(6)).collect();
    /// assert_eq!(ticks, [Tick::new(0)]); // 12 ms accumulated: one step, 2 ms left
    /// ```
    #[expect(
        clippy::cast_possible_truncation,
        reason = "n is at most max_steps, which is a u32"
    )]
    pub fn accumulate(&mut self, elapsed: Duration) -> Steps {
        let step_ns = self.step.as_nanos();
        let total_ns = self.accumulator.saturating_add(elapsed).as_nanos();
        let cap = u128::from(self.max_steps.get());

        let steps = (total_ns / step_ns).min(cap);
        let mut rest = total_ns - steps * step_ns;
        if steps == cap && rest >= step_ns {
            // Capped: keep only the fractional part of a step, and tally the rest.
            let excess = rest - rest % step_ns;
            self.dropped = self.dropped.saturating_add(duration_from_nanos(excess));
            rest -= excess;
        }
        self.accumulator = duration_from_nanos(rest);

        let remaining = steps as u32;
        let committed = Steps {
            next: self.tick,
            remaining,
        };
        self.tick = Tick(self.tick.0.saturating_add(u64::from(remaining)));
        committed
    }

    /// The leftover fraction of a step in `[0, 1)`, for render interpolation.
    ///
    /// Computed as `(accumulator_ns as f64 / step_ns as f64) as f32`, then clamped to
    /// `1.0_f32.next_down()` (rounding to `f32` alone can produce exactly 1.0). This is the
    /// only floating-point output, and it is never fed back into the simulation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::FixedTimestep;
    /// # use core::time::Duration;
    /// let mut t = FixedTimestep::new(Duration::from_millis(10));
    /// let _ = t.accumulate(Duration::from_millis(5));
    /// assert!((t.alpha() - 0.5).abs() < 1e-6);
    /// ```
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        reason = "alpha is a display-only approximation"
    )]
    pub fn alpha(&self) -> f32 {
        let fraction = self.accumulator.as_nanos() as f64 / self.step.as_nanos() as f64;
        (fraction as f32).min(1.0_f32.next_down())
    }

    /// The fixed step length.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::FixedTimestep;
    /// # use core::time::Duration;
    /// assert_eq!(FixedTimestep::new(Duration::from_millis(4)).step(), Duration::from_millis(4));
    /// ```
    pub const fn step(&self) -> Duration {
        self.step
    }

    /// The tick of the next step to be produced.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::{FixedTimestep, Tick};
    /// # use core::time::Duration;
    /// assert_eq!(FixedTimestep::new(Duration::from_millis(4)).current_tick(), Tick::ZERO);
    /// ```
    pub const fn current_tick(&self) -> Tick {
        self.tick
    }

    /// Total time discarded by the step cap so far.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::FixedTimestep;
    /// # use core::{num::NonZeroU32, time::Duration};
    /// let mut t = FixedTimestep::new(Duration::from_millis(10))
    ///     .with_max_steps_per_update(NonZeroU32::MIN);
    /// let _ = t.accumulate(Duration::from_millis(35));
    /// assert_eq!(t.dropped_time(), Duration::from_millis(20));
    /// ```
    pub const fn dropped_time(&self) -> Duration {
        self.dropped
    }

    /// The complete state, for save/load, replays, and rollback.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::FixedTimestep;
    /// # use core::time::Duration;
    /// let t = FixedTimestep::new(Duration::from_millis(10));
    /// let restored = FixedTimestep::from_state(t.to_state()).unwrap();
    /// assert_eq!(restored.to_state(), t.to_state());
    /// ```
    pub const fn to_state(&self) -> FixedTimestepState {
        FixedTimestepState {
            step: self.step,
            accumulator: self.accumulator,
            max_steps_per_update: self.max_steps,
            next_tick: self.tick,
            dropped: self.dropped,
        }
    }

    /// Restores a timestep from [`to_state`](Self::to_state).
    ///
    /// Returns `None` if `step` is zero or `accumulator >= step`, states that normal use can
    /// never produce.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::FixedTimestep;
    /// # use core::time::Duration;
    /// let mut state = FixedTimestep::new(Duration::from_millis(10)).to_state();
    /// state.accumulator = state.step; // not a reachable state
    /// assert!(FixedTimestep::from_state(state).is_none());
    /// ```
    pub fn from_state(state: FixedTimestepState) -> Option<Self> {
        (!state.step.is_zero() && state.accumulator < state.step).then_some(Self {
            step: state.step,
            accumulator: state.accumulator,
            max_steps: state.max_steps_per_update,
            tick: state.next_tick,
            dropped: state.dropped,
        })
    }
}

/// The complete, plain-data state of a [`FixedTimestep`].
///
/// # Examples
///
/// ```
/// use engine_time::{FixedTimestep, FixedTimestepState};
/// use core::time::Duration;
///
/// let state: FixedTimestepState = FixedTimestep::new(Duration::from_millis(10)).to_state();
/// assert_eq!(state.step, Duration::from_millis(10));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FixedTimestepState {
    /// The fixed step length.
    pub step: Duration,
    /// Time carried over toward the next step; less than `step`.
    pub accumulator: Duration,
    /// The cap on steps per update.
    pub max_steps_per_update: NonZeroU32,
    /// The tick of the next step to be produced.
    pub next_tick: Tick,
    /// Total time discarded by the step cap.
    pub dropped: Duration,
}

/// The ticks committed by one [`FixedTimestep::accumulate`] call.
///
/// Steps are committed when `accumulate` returns, whether or not this iterator is consumed. It
/// owns its data, so the timestep can be used again while a `Steps` is still being iterated.
///
/// # Examples
///
/// ```
/// use engine_time::{FixedTimestep, Tick};
/// use core::time::Duration;
///
/// let mut t = FixedTimestep::new(Duration::from_millis(10));
/// let steps = t.accumulate(Duration::from_millis(30));
/// assert_eq!(steps.len(), 3);
/// assert_eq!(steps.rev().next(), Some(Tick::new(2)));
/// ```
#[derive(Clone, Debug)]
pub struct Steps {
    next: Tick,
    remaining: u32,
}

impl Iterator for Steps {
    type Item = Tick;

    #[inline]
    fn next(&mut self) -> Option<Tick> {
        if self.remaining == 0 {
            return None;
        }
        let tick = self.next;
        self.next = Tick(tick.0.saturating_add(1));
        self.remaining -= 1;
        Some(tick)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.remaining as usize;
        (remaining, Some(remaining))
    }
}

impl DoubleEndedIterator for Steps {
    #[inline]
    fn next_back(&mut self) -> Option<Tick> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        Some(Tick(self.next.0.saturating_add(u64::from(self.remaining))))
    }
}

impl ExactSizeIterator for Steps {}
impl FusedIterator for Steps {}

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::NonZeroU32;
    use core::time::Duration;

    extern crate alloc;
    use alloc::vec::Vec;

    const MS10: Duration = Duration::from_millis(10);

    fn nz(n: u32) -> NonZeroU32 {
        NonZeroU32::new(n).unwrap()
    }

    #[test]
    fn from_hz_60_is_exact() {
        assert_eq!(
            FixedTimestep::from_hz(nz(60)).step(),
            Duration::from_nanos(16_666_667)
        );
    }

    #[test]
    fn from_hz_extremes_never_panic_or_yield_a_zero_step() {
        assert_eq!(FixedTimestep::from_hz(nz(1)).step(), Duration::from_secs(1));
        assert_eq!(
            FixedTimestep::from_hz(nz(1_000_000_000)).step(),
            Duration::from_nanos(1)
        );
        assert_eq!(
            FixedTimestep::from_hz(NonZeroU32::MAX).step(),
            Duration::from_nanos(1)
        );
    }

    #[test]
    fn alpha_stays_below_one() {
        for step in [Duration::from_nanos(16_666_667), Duration::from_millis(100)] {
            let mut t = FixedTimestep::new(step);
            let _ = t.accumulate(step.saturating_sub(Duration::from_nanos(1)));
            assert!(t.alpha() < 1.0, "alpha {} for step {step:?}", t.alpha());
        }
    }

    #[test]
    fn alpha_tracks_the_leftover_fraction() {
        let mut t = FixedTimestep::new(MS10);
        assert_eq!(t.alpha().to_bits(), 0.0_f32.to_bits());
        let _ = t.accumulate(Duration::from_millis(5));
        assert!((t.alpha() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn cap_drops_excess_keeps_fraction() {
        let mut t = FixedTimestep::new(MS10).with_max_steps_per_update(nz(2));
        let ticks: Vec<Tick> = t.accumulate(Duration::from_millis(35)).collect();
        assert_eq!(ticks, [Tick::new(0), Tick::new(1)]);
        assert_eq!(t.dropped_time(), MS10);
        assert!((t.alpha() - 0.5).abs() < 1e-6);
        assert_eq!(t.current_tick(), Tick::new(2));
    }

    #[test]
    fn hitting_the_cap_exactly_drops_nothing() {
        let mut t = FixedTimestep::new(MS10).with_max_steps_per_update(nz(2));
        assert_eq!(t.accumulate(Duration::from_millis(25)).len(), 2);
        assert_eq!(t.dropped_time(), Duration::ZERO);
        assert!((t.alpha() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn accumulate_huge_elapsed_saturates_without_panic() {
        let mut t = FixedTimestep::new(Duration::from_millis(10));
        assert_eq!(t.accumulate(Duration::MAX).len(), 8);
        assert_eq!(t.accumulate(Duration::MAX).len(), 8);
        assert!(t.alpha() < 1.0);
    }

    #[test]
    fn remainder_carries_between_calls() {
        let mut t = FixedTimestep::new(MS10);
        assert_eq!(t.accumulate(Duration::from_millis(6)).len(), 0);
        let ticks: Vec<Tick> = t.accumulate(Duration::from_millis(6)).collect();
        assert_eq!(ticks, [Tick::new(0)]);
        assert!((t.alpha() - 0.2).abs() < 1e-6);
    }

    #[test]
    fn steps_are_committed_even_if_not_consumed() {
        let mut t = FixedTimestep::new(MS10);
        let _ = t.accumulate(Duration::from_millis(25));
        assert_eq!(t.current_tick(), Tick::new(2));
    }

    #[test]
    fn steps_iterator_is_exact_double_ended_and_fused() {
        let mut t = FixedTimestep::new(MS10);
        let mut steps = t.accumulate(Duration::from_millis(35)); // 3 steps: ticks 0, 1, 2
        assert_eq!(steps.len(), 3);
        assert_eq!(steps.next_back(), Some(Tick::new(2)));
        assert_eq!(steps.next(), Some(Tick::new(0)));
        assert_eq!(steps.len(), 1);
        assert_eq!(steps.next(), Some(Tick::new(1)));
        assert_eq!(steps.next(), None);
        assert_eq!(steps.next(), None); // fused
        assert_eq!(steps.next_back(), None);
    }

    #[test]
    fn state_roundtrip_and_validation() {
        let mut t = FixedTimestep::new(MS10).with_max_steps_per_update(nz(3));
        let _ = t.accumulate(Duration::from_millis(47));
        let mut restored = FixedTimestep::from_state(t.to_state()).unwrap();
        assert_eq!(restored.to_state(), t.to_state());
        let a: Vec<Tick> = t.accumulate(Duration::from_millis(33)).collect();
        let b: Vec<Tick> = restored.accumulate(Duration::from_millis(33)).collect();
        assert_eq!(a, b);
        assert_eq!(t.to_state(), restored.to_state());

        let good = t.to_state();
        assert!(
            FixedTimestep::from_state(FixedTimestepState {
                step: Duration::ZERO,
                ..good
            })
            .is_none()
        );
        assert!(
            FixedTimestep::from_state(FixedTimestepState {
                accumulator: good.step,
                ..good
            })
            .is_none()
        );
    }

    #[test]
    fn try_new_zero_is_none() {
        assert!(FixedTimestep::try_new(Duration::ZERO).is_none());
        assert!(FixedTimestep::try_new(Duration::from_nanos(1)).is_some());
    }

    #[test]
    fn defaults() {
        assert_eq!(FixedTimestep::DEFAULT_MAX_STEPS_PER_UPDATE.get(), 8);
        assert_eq!(
            FixedTimestep::new(MS10)
                .to_state()
                .max_steps_per_update
                .get(),
            8
        );
        assert_eq!(FixedTimestep::new(MS10).current_tick(), Tick::ZERO);
    }
}
