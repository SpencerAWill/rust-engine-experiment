//! [`Tick`]: the simulation step counter.

/// A monotonic simulation step counter.
///
/// A `Tick` names a whole simulation step (not wall-clock time): tick 0 is the first step the
/// simulation takes. At 60 Hz a `u64` lasts about 9.7 billion years, so overflow is a bug
/// rather than a condition to plan for, but it is handled explicitly all the same.
///
/// # Examples
///
/// ```
/// use engine_time::Tick;
///
/// let t = Tick::ZERO;
/// assert_eq!(t.next(), Tick::new(1));
/// assert_eq!(t.next().get(), 1);
/// assert_eq!(Tick::new(u64::MAX).checked_next(), None);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(pub(crate) u64);

impl Tick {
    /// The first tick.
    pub const ZERO: Self = Self(0);

    /// Creates a tick from a raw step count, for example when restoring a snapshot.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::Tick;
    /// assert_eq!(Tick::new(7).get(), 7);
    /// ```
    #[inline]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The tick after this one.
    ///
    /// # Panics
    ///
    /// Panics if the counter would overflow `u64`. Use [`checked_next`](Self::checked_next)
    /// to handle that without panicking.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::Tick;
    /// assert_eq!(Tick::new(41).next(), Tick::new(42));
    /// ```
    #[inline]
    #[must_use]
    #[track_caller]
    pub const fn next(self) -> Self {
        match self.0.checked_add(1) {
            Some(value) => Self(value),
            None => panic!(
                "Tick::next: the tick counter overflowed u64; use checked_next to handle this without panicking"
            ),
        }
    }

    /// Fallible twin of [`next`](Self::next): `None` on overflow.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::Tick;
    /// assert_eq!(Tick::ZERO.checked_next(), Some(Tick::new(1)));
    /// assert_eq!(Tick::new(u64::MAX).checked_next(), None);
    /// ```
    #[inline]
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// The raw step count.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_time::Tick;
    /// assert_eq!(Tick::new(3).get(), 3);
    /// ```
    #[inline]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_next_at_max_is_none() {
        assert_eq!(Tick::new(u64::MAX).checked_next(), None);
        assert_eq!(Tick::new(4).checked_next(), Some(Tick::new(5)));
    }

    #[test]
    fn next_increments() {
        assert_eq!(Tick::ZERO.next(), Tick::new(1));
        assert_eq!(Tick::default(), Tick::ZERO);
        assert_eq!(Tick::new(9).get(), 9);
    }
}
