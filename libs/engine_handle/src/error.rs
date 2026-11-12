//! Error types.

use core::error::Error;
use core::fmt;

/// Returned by [`HandlePool::try_insert`](crate::HandlePool::try_insert) when
/// the pool is full. Carries the rejected value back, so nothing is lost.
///
/// # Examples
///
/// ```
/// use engine_handle::CapacityError;
///
/// fn describe(e: CapacityError<String>) -> (String, String) {
///     (e.to_string(), e.into_inner())
/// }
/// # let _ = describe;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CapacityError<T> {
    value: T,
    limit: u32,
}

impl<T> CapacityError<T> {
    pub(crate) const fn new(value: T, limit: u32) -> Self {
        Self { value, limit }
    }

    /// The most slots the pool that rejected the value can hold.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut pool = HandlePool::new();
    /// if let Err(e) = pool.try_insert(1) {
    ///     assert!(e.limit() > 0);
    /// }
    /// ```
    pub const fn limit(&self) -> u32 {
        self.limit
    }

    /// Returns the value that could not be inserted.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// // A pool only fills up after 2³¹−2 slots, so this just shows the API shape:
    /// let mut pool = HandlePool::new();
    /// match pool.try_insert(7) {
    ///     Ok(handle) => assert_eq!(pool[handle], 7),
    ///     Err(e) => assert_eq!(e.into_inner(), 7),
    /// }
    /// ```
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> fmt::Display for CapacityError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HandlePool is full: all {} slot indices are in use or retired; remove values or use another pool",
            self.limit
        )
    }
}

impl<T: fmt::Debug> Error for CapacityError<T> {}

/// Returned by `Handle::try_from(u64)` when the generation (low) half of the bits is zero, which
/// no handle ever has.
///
/// # Examples
///
/// ```
/// use engine_handle::Handle;
///
/// let err = Handle::<u8>::try_from(5_u64 << 32).unwrap_err();
/// assert_eq!(err.bits(), 5_u64 << 32);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct InvalidHandleBits {
    bits: u64,
}

impl InvalidHandleBits {
    pub(crate) const fn new(bits: u64) -> Self {
        Self { bits }
    }

    /// The rejected bits.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::Handle;
    /// assert_eq!(Handle::<u8>::try_from(0_u64).unwrap_err().bits(), 0);
    /// ```
    pub const fn bits(&self) -> u64 {
        self.bits
    }
}

impl fmt::Display for InvalidHandleBits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:#018x} is not a handle: its generation (low 32 bits) is zero",
            self.bits
        )
    }
}

impl Error for InvalidHandleBits {}
