//! Error types.

use core::error::Error;
use core::fmt;

use crate::Timestamp;

/// Returned by [`ManualClock::try_set`](crate::ManualClock::try_set) when the requested time is
/// earlier than the clock's current time. Clocks are monotonic.
///
/// # Examples
///
/// ```
/// use core::time::Duration;
/// use engine_time::{ManualClock, Timestamp};
///
/// let mut clock = ManualClock::starting_at(Timestamp::from_duration(Duration::from_secs(10)));
/// let err = clock.try_set(Timestamp::default()).unwrap_err();
/// assert_eq!(err.current, Timestamp::from_duration(Duration::from_secs(10)));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct NonMonotonicError {
    /// The clock's time, which was left unchanged.
    pub current: Timestamp,
    /// The earlier time that was rejected.
    pub requested: Timestamp,
}

impl fmt::Display for NonMonotonicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ManualClock::try_set: requested {:?} is earlier than current {:?}; clocks are monotonic",
            self.requested, self.current
        )
    }
}

impl Error for NonMonotonicError {}
