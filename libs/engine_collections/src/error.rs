//! Error types.

use core::error::Error;
use core::fmt;

/// Returned by `try_reserve` when the capacity overflows or the allocator fails.
///
/// # Examples
///
/// ```
/// use engine_collections::IndexMap;
///
/// let mut map = IndexMap::<u32, u32>::new();
/// let error = map.try_reserve(usize::MAX).unwrap_err();
/// assert!(error.to_string().contains("allocation failed"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TryReserveError(());

impl TryReserveError {
    pub(crate) fn from_inner(_inner: indexmap::TryReserveError) -> Self {
        Self(())
    }
}

impl fmt::Display for TryReserveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            "memory allocation failed: the capacity overflowed or the allocator reported an error",
        )
    }
}

impl Error for TryReserveError {}
