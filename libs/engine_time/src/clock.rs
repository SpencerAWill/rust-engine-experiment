//! [`TimeSource`] and its implementations.

#[cfg(all(
    feature = "std",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
extern crate std;

use core::time::Duration;

use crate::{NonMonotonicError, Timestamp};

/// Something that can say what time it is, as a monotonic [`Timestamp`].
///
/// Simulation code takes a `TimeSource` parameter instead of reading the wall clock, so tests
/// and replays can substitute a [`ManualClock`] (ADR 0003). The trait is object-safe
/// (`&dyn TimeSource` works), and it is implemented for `&T` and `&mut T`, so an
/// `impl TimeSource` parameter accepts a borrowed clock.
///
/// # Examples
///
/// ```
/// use core::time::Duration;
/// use engine_time::{ManualClock, TimeSource, Timestamp};
///
/// fn uptime(clock: impl TimeSource) -> Duration {
///     clock.now().as_duration()
/// }
///
/// let clock = ManualClock::starting_at(Timestamp::from_duration(Duration::from_secs(3)));
/// assert_eq!(uptime(&clock), Duration::from_secs(3));
/// ```
pub trait TimeSource {
    /// The current time. Successive calls never go backwards.
    fn now(&self) -> Timestamp;
}

impl<T: TimeSource + ?Sized> TimeSource for &T {
    #[inline]
    fn now(&self) -> Timestamp {
        (**self).now()
    }
}

impl<T: TimeSource + ?Sized> TimeSource for &mut T {
    #[inline]
    fn now(&self) -> Timestamp {
        (**self).now()
    }
}

/// A clock that only moves when told to: the deterministic [`TimeSource`] for tests, replays,
/// and headless simulation.
///
/// # Examples
///
/// ```
/// use core::time::Duration;
/// use engine_time::{ManualClock, TimeSource};
///
/// let mut clock = ManualClock::new();
/// clock.advance(Duration::from_millis(16));
/// assert_eq!(clock.now().as_duration(), Duration::from_millis(16));
/// ```
#[derive(Clone, Debug, Default)]
pub struct ManualClock {
    now: Timestamp,
}

impl ManualClock {
    /// A clock at the origin.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::{ManualClock, TimeSource};
    /// assert_eq!(ManualClock::new().now().as_duration(), core::time::Duration::ZERO);
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            now: Timestamp(Duration::ZERO),
        }
    }

    /// A clock starting at `now`, for restoring from a snapshot.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::{ManualClock, TimeSource, Timestamp};
    /// let t = Timestamp::from_duration(core::time::Duration::from_secs(9));
    /// assert_eq!(ManualClock::starting_at(t).now(), t);
    /// ```
    #[inline]
    pub const fn starting_at(now: Timestamp) -> Self {
        Self { now }
    }

    /// Moves the clock forward by `by`.
    ///
    /// Saturates at the maximum [`Duration`] instead of panicking, so it never fails.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::{ManualClock, TimeSource};
    /// # use core::time::Duration;
    /// let mut clock = ManualClock::new();
    /// clock.advance(Duration::MAX);
    /// clock.advance(Duration::MAX); // saturates
    /// assert_eq!(clock.now().as_duration(), Duration::MAX);
    /// ```
    #[inline]
    pub fn advance(&mut self, by: Duration) {
        self.now = Timestamp(self.now.0.saturating_add(by));
    }

    /// Sets the clock to `to`.
    ///
    /// # Panics
    ///
    /// Panics if `to` is earlier than the current time (clocks are monotonic). Use
    /// [`try_set`](Self::try_set) to handle that without panicking.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::{ManualClock, TimeSource, Timestamp};
    /// # use core::time::Duration;
    /// let mut clock = ManualClock::new();
    /// clock.set(Timestamp::from_duration(Duration::from_secs(1)));
    /// assert_eq!(clock.now().as_duration(), Duration::from_secs(1));
    /// ```
    #[track_caller]
    pub fn set(&mut self, to: Timestamp) {
        if let Err(error) = self.try_set(to) {
            panic!(
                "ManualClock::set: requested {:?} is earlier than current {:?}; clocks are \
                 monotonic; use try_set to handle this without panicking",
                error.requested, error.current
            );
        }
    }

    /// Fallible twin of [`set`](Self::set).
    ///
    /// # Errors
    ///
    /// Returns [`NonMonotonicError`] if `to` is earlier than the current time. The clock is
    /// left unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::{ManualClock, Timestamp};
    /// let mut clock = ManualClock::new();
    /// assert!(clock.try_set(Timestamp::default()).is_ok());
    /// ```
    pub fn try_set(&mut self, to: Timestamp) -> Result<(), NonMonotonicError> {
        if to < self.now {
            return Err(NonMonotonicError {
                current: self.now,
                requested: to,
            });
        }
        self.now = to;
        Ok(())
    }
}

impl TimeSource for ManualClock {
    #[inline]
    fn now(&self) -> Timestamp {
        self.now
    }
}

/// The wall-clock [`TimeSource`], backed by [`std::time::Instant`].
///
/// This is the one sanctioned reader of ambient time (ADR 0003): its origin is the moment
/// it was created, so [`now`](TimeSource::now) reports the time elapsed since then.
///
/// Not available on `wasm32-unknown-unknown`, where `std::time::Instant::now()` panics.
/// Platforms without a std clock supply their own [`TimeSource`] (for example one backed by
/// `performance.now()`).
///
/// # Examples
///
/// ```
/// use engine_time::{SystemClock, TimeSource};
///
/// let clock = SystemClock::new();
/// let first = clock.now();
/// assert!(clock.now() >= first);
/// ```
#[cfg(all(
    feature = "std",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
#[derive(Clone, Debug)]
pub struct SystemClock {
    origin: std::time::Instant,
}

#[cfg(all(
    feature = "std",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
impl SystemClock {
    /// Creates a clock whose origin is now.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::SystemClock;
    /// let _clock = SystemClock::new();
    /// ```
    #[expect(
        clippy::disallowed_methods,
        reason = "SystemClock is the sanctioned wall-clock TimeSource (ADR 0003)"
    )]
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: std::time::Instant::now(),
        }
    }
}

#[cfg(all(
    feature = "std",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(
    feature = "std",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
impl TimeSource for SystemClock {
    #[inline]
    fn now(&self) -> Timestamp {
        Timestamp(self.origin.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NonMonotonicError, Timestamp};
    use core::time::Duration;

    fn read(source: impl TimeSource) -> Timestamp {
        source.now()
    }

    #[test]
    fn advance_saturates_instead_of_panicking() {
        let mut clock = ManualClock::new();
        clock.advance(Duration::MAX);
        clock.advance(Duration::MAX);
        assert_eq!(clock.now().as_duration(), Duration::MAX);
    }

    #[test]
    fn try_set_backwards_errors_and_leaves_clock_unchanged() {
        let mut clock = ManualClock::starting_at(Timestamp::from_duration(Duration::from_secs(10)));
        let earlier = Timestamp::from_duration(Duration::from_secs(5));
        let err = clock.try_set(earlier).unwrap_err();
        assert_eq!(
            err.current,
            Timestamp::from_duration(Duration::from_secs(10))
        );
        assert_eq!(err.requested, earlier);
        assert_eq!(
            clock.now(),
            Timestamp::from_duration(Duration::from_secs(10))
        );
        assert!(matches!(err, NonMonotonicError { .. }));
    }

    #[test]
    fn set_forward_and_equal_succeed() {
        let mut clock = ManualClock::new();
        clock.set(Timestamp::from_duration(Duration::from_secs(2)));
        clock.set(Timestamp::from_duration(Duration::from_secs(2)));
        assert_eq!(clock.now().as_duration(), Duration::from_secs(2));
    }

    #[test]
    fn time_source_works_through_references() {
        let mut clock = ManualClock::starting_at(Timestamp::from_duration(Duration::from_secs(1)));
        assert_eq!(read(&clock).as_duration(), Duration::from_secs(1));
        assert_eq!(read(&mut clock).as_duration(), Duration::from_secs(1));
        let dynamic: &dyn TimeSource = &clock;
        assert_eq!(dynamic.now().as_duration(), Duration::from_secs(1));
    }

    #[test]
    fn error_message_names_both_timestamps_and_the_remedy() {
        extern crate alloc;
        let mut clock = ManualClock::starting_at(Timestamp::from_duration(Duration::from_secs(10)));
        let err = clock.try_set(Timestamp::default()).unwrap_err();
        let text = alloc::format!("{err}");
        assert!(
            text.starts_with("ManualClock::try_set: requested"),
            "{text}"
        );
        assert!(text.contains("is earlier than current"), "{text}");
        assert!(text.contains("clocks are monotonic"), "{text}");
    }

    #[cfg(all(
        feature = "std",
        not(all(target_arch = "wasm32", target_os = "unknown"))
    ))]
    #[test]
    fn system_clock_is_monotonic() {
        let clock = SystemClock::new();
        let mut previous = clock.now();
        for _ in 0..1000 {
            let current = clock.now();
            assert!(current >= previous);
            previous = current;
        }
        let _ = SystemClock::default();
    }
}
