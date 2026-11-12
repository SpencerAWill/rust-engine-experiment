//! [`Timestamp`]: a monotonic instant relative to a source-defined origin.

use core::time::Duration;

/// A monotonic instant relative to an arbitrary, source-defined origin.
///
/// Timestamps from the same [`TimeSource`](crate::TimeSource) are comparable. Timestamps from
/// different sources are not meaningful to compare, because their origins differ.
///
/// # Examples
///
/// ```
/// use core::time::Duration;
/// use engine_time::Timestamp;
///
/// let a = Timestamp::from_duration(Duration::from_secs(1));
/// let b = Timestamp::from_duration(Duration::from_secs(3));
/// assert_eq!(b.saturating_duration_since(a), Duration::from_secs(2));
/// assert_eq!(a.saturating_duration_since(b), Duration::ZERO);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(pub(crate) Duration);

impl Timestamp {
    /// A timestamp `since_origin` after the source's origin.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::Timestamp;
    /// # use core::time::Duration;
    /// let t = Timestamp::from_duration(Duration::from_millis(5));
    /// assert_eq!(t.as_duration(), Duration::from_millis(5));
    /// ```
    #[inline]
    pub const fn from_duration(since_origin: Duration) -> Self {
        Self(since_origin)
    }

    /// The time elapsed since the source's origin.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::Timestamp;
    /// assert_eq!(Timestamp::default().as_duration(), core::time::Duration::ZERO);
    /// ```
    #[inline]
    pub const fn as_duration(self) -> Duration {
        self.0
    }

    /// The time from `earlier` to `self`, or zero if `earlier` is later.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::Timestamp;
    /// # use core::time::Duration;
    /// let t = Timestamp::from_duration(Duration::from_secs(2));
    /// assert_eq!(t.saturating_duration_since(Timestamp::default()), Duration::from_secs(2));
    /// ```
    #[inline]
    pub fn saturating_duration_since(self, earlier: Self) -> Duration {
        self.0.saturating_sub(earlier.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::time::Duration;

    #[test]
    fn saturating_duration_since_clamps_to_zero() {
        let early = Timestamp::from_duration(Duration::from_secs(1));
        let late = Timestamp::from_duration(Duration::from_secs(3));
        assert_eq!(
            late.saturating_duration_since(early),
            Duration::from_secs(2)
        );
        assert_eq!(early.saturating_duration_since(late), Duration::ZERO);
    }

    #[test]
    fn roundtrips_and_orders() {
        let t = Timestamp::from_duration(Duration::from_millis(5));
        assert_eq!(t.as_duration(), Duration::from_millis(5));
        assert!(Timestamp::default() < t);
    }
}
