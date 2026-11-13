//! `IndexSet` and its iterator types.

use core::borrow::Borrow;
use core::fmt;
use core::hash::Hash;
use core::iter::FromIterator;

use crate::TryReserveError;
use crate::hasher::{Hasher, default_hasher};

mod iter;

pub use iter::{IntoIter, Iter};

/// The backend, always with the fixed hasher.
#[expect(
    clippy::disallowed_types,
    reason = "the one sanctioned use: wrapped with a fixed hasher"
)]
type Inner<T> = indexmap::IndexSet<T, Hasher>;

/// A hash set that iterates in insertion order.
///
/// Two sets are equal only if they hold the same items **in the same order**. To use a set
/// as an order-independent key, [`sort`](Self::sort) it first.
///
/// # Examples
///
/// ```
/// use engine_collections::IndexSet;
///
/// let ab: IndexSet<u32> = [1, 2].into();
/// let ba: IndexSet<u32> = [2, 1].into();
/// assert_ne!(ab, ba);
///
/// // To use a set as an order-independent key, sort it first.
/// let (mut ab, mut ba) = (ab, ba);
/// ab.sort();
/// ba.sort();
/// assert_eq!(ab, ba);
/// ```
pub struct IndexSet<T> {
    inner: Inner<T>,
}

impl<T> IndexSet<T> {
    /// Creates an empty set. Does not allocate.
    pub const fn new() -> Self {
        Self {
            inner: Inner::with_hasher(default_hasher()),
        }
    }

    /// Creates an empty set with room for at least `capacity` items.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Inner::with_capacity_and_hasher(capacity, default_hasher()),
        }
    }

    /// The number of items.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the set has no items.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// The number of items the set can hold without allocating.
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// Reserves room for at least `additional` more items.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity overflows `usize`, or if the allocator fails, exactly like
    /// `Vec::reserve`. Use [`try_reserve`](Self::try_reserve) to handle that without panicking.
    #[track_caller]
    pub fn reserve(&mut self, additional: usize) {
        if let Err(error) = self.try_reserve(additional) {
            panic!(
                "IndexSet::reserve: cannot reserve room for {additional} more items ({error}); \
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

    /// Shrinks the capacity as far as the items allow.
    pub fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit();
    }

    /// Removes every item, keeping the capacity.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// The item at position `index`, or `None` if out of range.
    pub fn get_index(&self, index: usize) -> Option<&T> {
        self.inner.get_index(index)
    }

    /// The first item in iteration order.
    pub fn first(&self) -> Option<&T> {
        self.inner.first()
    }

    /// The last item in iteration order.
    pub fn last(&self) -> Option<&T> {
        self.inner.last()
    }

    /// Removes the last item and returns it. Every other item keeps its position.
    pub fn pop(&mut self) -> Option<T> {
        self.inner.pop()
    }

    /// Removes the item at `index` in O(1) by moving the last item into its place.
    ///
    /// Every item other than the removed one and the last keeps its position.
    pub fn swap_remove_index(&mut self, index: usize) -> Option<T> {
        self.inner.swap_remove_index(index)
    }

    /// Removes the item at `index` in O(n); later items move up one position and the
    /// relative order of the rest is kept.
    pub fn shift_remove_index(&mut self, index: usize) -> Option<T> {
        self.inner.shift_remove_index(index)
    }

    /// Keeps only the items for which `keep` returns `true`, in their relative order.
    pub fn retain(&mut self, keep: impl FnMut(&T) -> bool) {
        self.inner.retain(keep);
    }

    /// Sorts the items.
    ///
    /// Items are unique, so the result is fully determined. It does not allocate.
    pub fn sort(&mut self)
    where
        T: Ord,
    {
        self.inner.sort_unstable();
    }

    /// An iterator over the items, in order.
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            inner: self.inner.iter(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_test_hasher(hasher: Hasher) -> Self {
        Self {
            inner: Inner::with_hasher(hasher),
        }
    }
}

impl<T: Hash + Eq> IndexSet<T> {
    /// Inserts `item` at the end if it is not present; returns whether it was inserted.
    ///
    /// An item that is already present changes nothing, including its position.
    pub fn insert(&mut self, item: T) -> bool {
        self.inner.insert(item)
    }

    /// Whether `item` is in the set.
    pub fn contains<Q>(&self, item: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.contains(item)
    }

    /// The stored item equal to `item`.
    pub fn get<Q>(&self, item: &Q) -> Option<&T>
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.get(item)
    }

    /// The position of `item`.
    ///
    /// A position is valid until the next removal or sort.
    pub fn get_index_of<Q>(&self, item: &Q) -> Option<usize>
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.get_index_of(item)
    }

    /// Removes `item` in O(1) by moving the last item into its place; returns whether it
    /// was present.
    ///
    /// Every item other than the removed one and the last keeps its position.
    pub fn swap_remove<Q>(&mut self, item: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.swap_remove(item)
    }

    /// Removes `item` in O(n); later items move up one position and the relative order of
    /// the rest is kept. Returns whether it was present.
    pub fn shift_remove<Q>(&mut self, item: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.shift_remove(item)
    }
}

impl<T: fmt::Debug> fmt::Debug for IndexSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

impl<T> Default for IndexSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> Clone for IndexSet<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// Equal only if the items are equal **in the same order**.
impl<T: PartialEq> PartialEq for IndexSet<T> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

impl<T: Eq> Eq for IndexSet<T> {}

impl<T: Hash> Hash for IndexSet<T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_usize(self.len());
        for item in self {
            item.hash(state);
        }
    }
}

impl<'a, T> IntoIterator for &'a IndexSet<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

impl<T> IntoIterator for IndexSet<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> IntoIter<T> {
        IntoIter {
            inner: self.inner.into_iter(),
        }
    }
}

impl<T: Hash + Eq> FromIterator<T> for IndexSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut set = Self::new();
        set.extend(iter);
        set
    }
}

impl<T: Hash + Eq> Extend<T> for IndexSet<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.inner.extend(iter);
    }
}

impl<T: Hash + Eq, const N: usize> From<[T; N]> for IndexSet<T> {
    fn from(items: [T; N]) -> Self {
        Self::from_iter(items)
    }
}

#[cfg(test)]
mod tests {
    use std::vec::Vec;

    use crate::IndexSet;

    #[test]
    fn empty_set_operations_are_noops() {
        let mut set = IndexSet::<u8>::new();
        assert_eq!(set.pop(), None);
        assert_eq!(set.first(), None);
        assert_eq!(set.last(), None);
        assert_eq!(set.swap_remove_index(0), None);
        assert_eq!(set.shift_remove_index(0), None);
        set.sort();
        set.retain(|_| true);
        assert!(set.is_empty());
        assert_eq!(format!("{set:?}"), "{}");
    }

    #[test]
    fn debug_format() {
        let set = IndexSet::from([1, 2]);
        assert_eq!(format!("{set:?}"), "{1, 2}");
    }

    #[test]
    fn insert_existing_returns_false_and_keeps_position() {
        let mut set = IndexSet::new();
        assert!(set.insert('a'));
        assert!(set.insert('b'));
        assert!(!set.insert('a'));
        assert_eq!(set.iter().copied().collect::<Vec<_>>(), ['a', 'b']);
    }

    #[test]
    fn from_iter_extend_and_from_keep_first_position() {
        let want = [3, 1, 2];
        let items = [3, 1, 3, 2, 1];
        let collected: IndexSet<i32> = items.into_iter().collect();
        assert_eq!(collected.iter().copied().collect::<Vec<_>>(), want);
        let mut extended = IndexSet::new();
        extended.extend(items);
        assert_eq!(extended.iter().copied().collect::<Vec<_>>(), want);
        let mut from = IndexSet::from([3, 1, 3, 2, 1]);
        assert_eq!(from.iter().copied().collect::<Vec<_>>(), want);
        assert_eq!(from.iter().len(), 3);
        assert_eq!(from.iter().rev().copied().collect::<Vec<_>>(), [2, 1, 3]);
        assert_eq!(from.try_reserve(8), Ok(()));
        assert_eq!(
            from.clone().into_iter().rev().collect::<Vec<_>>(),
            [2, 1, 3]
        );
        assert_eq!((&from).into_iter().count(), 3);
    }
}
