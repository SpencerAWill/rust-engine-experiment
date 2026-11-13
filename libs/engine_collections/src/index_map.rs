//! `IndexMap` and its iterator and entry types.

use crate::TryReserveError;
use crate::hasher::{Hasher, default_hasher};

/// The backend, always with the fixed hasher.
#[expect(
    clippy::disallowed_types,
    reason = "the one sanctioned use: wrapped with a fixed hasher"
)]
type Inner<K, V> = indexmap::IndexMap<K, V, Hasher>;

/// A hash map that iterates in insertion order.
///
/// # Examples
///
/// ```
/// use engine_collections::IndexMap;
///
/// let map = IndexMap::<&str, u32>::with_capacity(4);
/// assert!(map.is_empty());
/// ```
#[derive(Debug)]
pub struct IndexMap<K, V> {
    inner: Inner<K, V>,
}

impl<K, V> IndexMap<K, V> {
    /// Creates an empty map. Does not allocate.
    pub const fn new() -> Self {
        Self {
            inner: Inner::with_hasher(default_hasher()),
        }
    }

    /// Creates an empty map with room for at least `capacity` entries.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Inner::with_capacity_and_hasher(capacity, default_hasher()),
        }
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the map has no entries.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// The number of entries the map can hold without allocating.
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// Reserves room for at least `additional` more entries.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity overflows `usize`, or if the allocator fails, exactly like
    /// `Vec::reserve`. Use [`try_reserve`](Self::try_reserve) to handle that without panicking.
    #[track_caller]
    pub fn reserve(&mut self, additional: usize) {
        if let Err(error) = self.try_reserve(additional) {
            panic!(
                "IndexMap::reserve: cannot reserve room for {additional} more entries ({error}); \
                 use try_reserve to handle this without panicking"
            );
        }
    }

    /// Fallible twin of [`reserve`](Self::reserve).
    ///
    /// # Errors
    ///
    /// Returns [`TryReserveError`] if the capacity overflows or the allocator fails.
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.inner
            .try_reserve(additional)
            .map_err(TryReserveError::from_inner)
    }

    /// Shrinks the capacity as far as the entries allow.
    pub fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit();
    }

    /// Removes every entry, keeping the capacity.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    #[cfg(test)]
    #[allow(dead_code, reason = "used by the model tests from the next commit")]
    pub(crate) fn with_test_hasher(hasher: Hasher) -> Self {
        Self {
            inner: Inner::with_hasher(hasher),
        }
    }
}

impl<K, V> Default for IndexMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::IndexMap;

    #[test]
    fn new_map_is_empty() {
        let map = IndexMap::<u32, u32>::new();
        assert_eq!(map.len(), 0);
        assert!(map.is_empty());
        assert_eq!(map.capacity(), 0);
    }

    #[test]
    fn with_capacity_reserves() {
        let mut map = IndexMap::<u32, u32>::with_capacity(10);
        assert!(map.capacity() >= 10);
        map.reserve(100);
        assert!(map.capacity() >= 100);
    }

    #[test]
    fn try_reserve_overflow_is_an_error() {
        let mut map = IndexMap::<u32, u32>::new();
        let error = map.try_reserve(usize::MAX).unwrap_err();
        assert_ne!(std::string::ToString::to_string(&error), "");
        assert!(map.try_reserve(4).is_ok());
        assert_eq!(map.len(), 0);
    }
}
