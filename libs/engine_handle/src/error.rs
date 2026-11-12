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
pub struct CapacityError<T>(pub(crate) T);

impl<T> CapacityError<T> {
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
        self.0
    }
}

impl<T> fmt::Display for CapacityError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HandlePool is full: all {} slot indices are in use or retired; remove values or use another pool",
            crate::slots::MAX_SLOTS
        )
    }
}

impl<T: fmt::Debug> Error for CapacityError<T> {}
