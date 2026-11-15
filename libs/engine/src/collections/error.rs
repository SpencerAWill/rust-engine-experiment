//! Error types.

use core::error::Error;
use core::fmt;

/// Returned by `try_reserve` when the capacity overflows or the allocator fails.
///
/// # Examples
///
/// ```
/// use engine::collections::IndexMap;
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

/// Returned by `BitArray::try_insert` and `try_remove` when the index is past the end.
///
/// # Examples
///
/// ```
/// use engine::collections::BitArray;
///
/// let mut bits = BitArray::<1>::new();
/// let error = bits.try_insert(70).unwrap_err();
/// assert_eq!(error.index(), 70);
/// assert_eq!(error.capacity(), 64);
/// assert_eq!(
///     error.to_string(),
///     "bit index 70 is out of range: this set holds bits 0 to 63"
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BitIndexError {
    index: usize,
    capacity: usize,
}

impl BitIndexError {
    pub(crate) const fn new(index: usize, capacity: usize) -> Self {
        Self { index, capacity }
    }

    /// The index that was out of range.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// The number of bits the set holds: valid indices are `0..capacity`.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }
}

impl fmt::Display for BitIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bit index {} is out of range: ", self.index)?;
        match self.capacity.checked_sub(1) {
            Some(last) => write!(f, "this set holds bits 0 to {last}"),
            None => f.write_str("this set holds no bits"),
        }
    }
}

impl Error for BitIndexError {}
