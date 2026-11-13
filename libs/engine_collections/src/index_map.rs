//! `IndexMap` and its iterator and entry types.

use core::borrow::Borrow;
use core::fmt;
use core::hash::Hash;
use core::iter::FromIterator;
use core::ops::{Index, IndexMut};

use crate::TryReserveError;
use crate::hasher::{Hasher, default_hasher};

/// The backend, always with the fixed hasher.
#[expect(
    clippy::disallowed_types,
    reason = "the one sanctioned use: wrapped with a fixed hasher"
)]
type Inner<K, V> = indexmap::IndexMap<K, V, Hasher>;

mod entry;
mod iter;

pub use entry::Entry;
pub use iter::{IntoIter, IntoKeys, IntoValues, Iter, IterMut, Keys, Values, ValuesMut};

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
    pub(crate) fn with_test_hasher(hasher: Hasher) -> Self {
        Self {
            inner: Inner::with_hasher(hasher),
        }
    }
}

impl<K, V> IndexMap<K, V> {
    /// The entry at position `index`, or `None` if out of range.
    pub fn get_index(&self, index: usize) -> Option<(&K, &V)> {
        self.inner.get_index(index)
    }

    /// Like [`get_index`](Self::get_index), with mutable access to the value.
    pub fn get_index_mut(&mut self, index: usize) -> Option<(&K, &mut V)> {
        self.inner.get_index_mut(index)
    }

    /// The first entry in iteration order.
    pub fn first(&self) -> Option<(&K, &V)> {
        self.inner.first()
    }

    /// Like [`first`](Self::first), with mutable access to the value.
    pub fn first_mut(&mut self) -> Option<(&K, &mut V)> {
        self.inner.first_mut().map(|e| (e.0, e.1))
    }

    /// The last entry in iteration order.
    pub fn last(&self) -> Option<(&K, &V)> {
        self.inner.last()
    }

    /// Like [`last`](Self::last), with mutable access to the value.
    pub fn last_mut(&mut self) -> Option<(&K, &mut V)> {
        self.inner.last_mut().map(|e| (e.0, e.1))
    }

    /// Removes the last entry and returns it. Every other entry keeps its position.
    pub fn pop(&mut self) -> Option<(K, V)> {
        self.inner.pop()
    }

    /// Removes the entry at `index` in O(1) by moving the last entry into its place.
    ///
    /// Every entry other than the removed one and the last keeps its position.
    pub fn swap_remove_index(&mut self, index: usize) -> Option<(K, V)> {
        self.inner.swap_remove_index(index)
    }

    /// Removes the entry at `index` in O(n); later entries move up one position and the
    /// relative order of the rest is kept.
    pub fn shift_remove_index(&mut self, index: usize) -> Option<(K, V)> {
        self.inner.shift_remove_index(index)
    }

    /// Keeps only the entries for which `keep` returns `true`, in their relative order.
    pub fn retain(&mut self, keep: impl FnMut(&K, &mut V) -> bool) {
        self.inner.retain(keep);
    }

    /// Sorts the entries by key.
    ///
    /// Keys are unique, so the result is fully determined, provided `Ord` agrees with `Eq`
    /// (keys that are equal under `Ord` but distinct under `Eq` end up in an unspecified
    /// order). It does not allocate.
    pub fn sort_keys(&mut self)
    where
        K: Ord,
    {
        self.inner.sort_unstable_keys();
    }

    /// An iterator over the entries, in order.
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            inner: self.inner.iter(),
        }
    }

    /// A mutable iterator over the entries, in order. Keys are not mutable.
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut {
            inner: self.inner.iter_mut(),
        }
    }

    /// An iterator over the values, in order.
    pub fn values(&self) -> Values<'_, K, V> {
        Values {
            inner: self.inner.values(),
        }
    }

    /// A mutable iterator over the values, in order.
    pub fn values_mut(&mut self) -> ValuesMut<'_, K, V> {
        ValuesMut {
            inner: self.inner.values_mut(),
        }
    }

    /// An owning iterator over the keys, in order.
    pub fn into_keys(self) -> IntoKeys<K, V> {
        IntoKeys {
            inner: self.inner.into_keys(),
        }
    }

    /// An owning iterator over the values, in order.
    pub fn into_values(self) -> IntoValues<K, V> {
        IntoValues {
            inner: self.inner.into_values(),
        }
    }

    /// An iterator over the keys, in order.
    pub fn keys(&self) -> Keys<'_, K, V> {
        Keys {
            inner: self.inner.keys(),
        }
    }
}

impl<K: Hash + Eq, V> IndexMap<K, V> {
    /// Inserts a key and value.
    ///
    /// A new key goes at the end. For an existing key the value is replaced and the old
    /// one returned; the entry keeps its position and its original key.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.inner.insert(key, value)
    }

    /// The entry for `key`, for in-place insertion or update.
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        Entry {
            inner: self.inner.entry(key),
        }
    }

    /// The value for `key`.
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.get(key)
    }

    /// The value for `key`, mutably.
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.get_mut(key)
    }

    /// Whether `key` is in the map.
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.contains_key(key)
    }

    /// The position of `key`.
    ///
    /// A position is valid until the next removal or sort.
    pub fn get_index_of<Q>(&self, key: &Q) -> Option<usize>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.get_index_of(key)
    }

    /// Removes `key` in O(1) by moving the last entry into its place; returns its value.
    ///
    /// Every entry other than the removed one and the last keeps its position.
    pub fn swap_remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.swap_remove(key)
    }

    /// Removes `key` in O(n); later entries move up one position and the relative order
    /// of the rest is kept. Returns its value.
    pub fn shift_remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.shift_remove(key)
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for IndexMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<'a, K, V> IntoIterator for &'a IndexMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    fn into_iter(self) -> Iter<'a, K, V> {
        self.iter()
    }
}

impl<'a, K, V> IntoIterator for &'a mut IndexMap<K, V> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;

    fn into_iter(self) -> IterMut<'a, K, V> {
        self.iter_mut()
    }
}

impl<K, V> IntoIterator for IndexMap<K, V> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V>;

    fn into_iter(self) -> IntoIter<K, V> {
        IntoIter {
            inner: self.inner.into_iter(),
        }
    }
}

impl<K: Clone, V: Clone> Clone for IndexMap<K, V> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// Equal only if the entries are equal **in the same order**.
impl<K: PartialEq, V: PartialEq> PartialEq for IndexMap<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

impl<K: Eq, V: Eq> Eq for IndexMap<K, V> {}

impl<K: Hash, V: Hash> Hash for IndexMap<K, V> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_usize(self.len());
        for (key, value) in self {
            key.hash(state);
            value.hash(state);
        }
    }
}

impl<K: Hash + Eq, V> FromIterator<(K, V)> for IndexMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut map = Self::new();
        map.extend(iter);
        map
    }
}

impl<K: Hash + Eq, V> Extend<(K, V)> for IndexMap<K, V> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        self.inner.extend(iter);
    }
}

impl<K: Hash + Eq, V, const N: usize> From<[(K, V); N]> for IndexMap<K, V> {
    fn from(entries: [(K, V); N]) -> Self {
        Self::from_iter(entries)
    }
}

#[cold]
#[track_caller]
fn missing_key() -> ! {
    panic!("IndexMap: no entry for this key; use get (or get_mut) to handle a missing key")
}

/// # Panics
///
/// Panics if the key is not in the map. Use [`get`](IndexMap::get) to handle that.
impl<K, V, Q> Index<&Q> for IndexMap<K, V>
where
    K: Hash + Eq + Borrow<Q>,
    Q: Hash + Eq + ?Sized,
{
    type Output = V;

    #[track_caller]
    fn index(&self, key: &Q) -> &V {
        match self.get(key) {
            Some(value) => value,
            None => missing_key(),
        }
    }
}

/// # Panics
///
/// Panics if the key is not in the map. Use [`get_mut`](IndexMap::get_mut) to handle that.
impl<K, V, Q> IndexMut<&Q> for IndexMap<K, V>
where
    K: Hash + Eq + Borrow<Q>,
    Q: Hash + Eq + ?Sized,
{
    #[track_caller]
    fn index_mut(&mut self, key: &Q) -> &mut V {
        match self.get_mut(key) {
            Some(value) => value,
            None => missing_key(),
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
    use std::string::String;
    use std::vec::Vec;

    use crate::IndexMap;

    #[test]
    fn empty_map_operations_are_noops() {
        let mut map = IndexMap::<u8, u8>::new();
        assert_eq!(map.pop(), None);
        assert_eq!(map.first(), None);
        assert_eq!(map.last(), None);
        assert_eq!(map.first_mut(), None);
        assert_eq!(map.last_mut(), None);
        assert_eq!(map.swap_remove_index(0), None);
        assert_eq!(map.shift_remove_index(0), None);
        map.sort_keys();
        map.retain(|_, _| true);
        assert!(map.is_empty());
        assert_eq!(std::format!("{map:?}"), "{}");
    }

    #[test]
    fn string_keys_are_looked_up_by_str() {
        let mut map = IndexMap::new();
        map.insert(std::string::String::from("walk"), 1_u32);
        map.insert(std::string::String::from("run"), 2);
        assert_eq!(map.get("run"), Some(&2));
        assert!(map.contains_key("walk"));
        assert_eq!(map.get_index_of("run"), Some(1));
        assert_eq!(map.swap_remove("walk"), Some(1));
        assert_eq!(map.get("walk"), None);
    }

    #[test]
    fn swap_remove_of_the_last_entry_moves_nothing() {
        let mut map: IndexMap<u8, u8> = IndexMap::new();
        map.insert(1, 10);
        map.insert(2, 20);
        map.insert(3, 30);
        assert_eq!(map.swap_remove(&3), Some(30));
        assert_eq!(
            map.iter().map(|(k, _)| *k).collect::<std::vec::Vec<_>>(),
            [1, 2]
        );
        assert_eq!(map.swap_remove(&1), Some(10));
        assert_eq!(map.swap_remove(&2), Some(20));
        assert!(map.is_empty());
    }

    #[test]
    fn vacant_entry_index_is_len() {
        let mut map: IndexMap<u8, u8> = IndexMap::new();
        map.insert(1, 10);
        map.insert(2, 20);
        let entry = map.entry(9);
        assert_eq!(entry.index(), 2);
        entry.or_insert(90);
        assert_eq!(map.get_index_of(&9), Some(2));
        assert_eq!(map.entry(1).index(), 0);
    }

    #[test]
    fn insert_existing_keeps_position_and_returns_old() {
        let mut map: IndexMap<char, u8> = IndexMap::new();
        map.insert('a', 1);
        map.insert('b', 2);
        map.insert('c', 3);
        assert_eq!(map.insert('b', 9), Some(2));
        assert_eq!(
            map.keys().copied().collect::<std::vec::Vec<_>>(),
            ['a', 'b', 'c']
        );
        assert_eq!(map.get(&'b'), Some(&9));
    }

    #[test]
    fn from_iter_keeps_first_position_last_value() {
        let want = [(1, 'c'), (2, 'b')];
        let items = [(1, 'a'), (2, 'b'), (1, 'c')];
        let collected: IndexMap<i32, char> = items.into_iter().collect();
        assert_eq!(
            collected.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(),
            want
        );
        let mut extended = IndexMap::new();
        extended.extend(items);
        assert_eq!(
            extended.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(),
            want
        );
        let from = IndexMap::from(items);
        assert_eq!(from.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(), want);
    }

    #[test]
    fn debug_format() {
        let map = IndexMap::from([(1, 'a'), (2, 'b')]);
        assert_eq!(format!("{map:?}"), "{1: 'a', 2: 'b'}");
        assert_eq!(format!("{:?}", IndexMap::<u8, u8>::new()), "{}");
    }

    #[test]
    fn iterators_are_double_ended_and_exact() {
        let mut map = IndexMap::from([(1, 10), (2, 20), (3, 30)]);
        assert_eq!(map.iter().len(), 3);
        assert_eq!(
            map.iter().rev().map(|(k, _)| *k).collect::<Vec<_>>(),
            [3, 2, 1]
        );
        assert_eq!(map.keys().rev().copied().collect::<Vec<_>>(), [3, 2, 1]);
        assert_eq!(map.values().len(), 3);
        assert_eq!(
            map.values().rev().copied().collect::<Vec<_>>(),
            [30, 20, 10]
        );
        assert_eq!(map.iter_mut().len(), 3);
        assert_eq!(
            map.iter_mut().rev().map(|(k, _)| *k).collect::<Vec<_>>(),
            [3, 2, 1]
        );
        assert_eq!(map.values_mut().len(), 3);
        for v in map.values_mut().rev() {
            *v += 1;
        }
        assert_eq!(map.values().copied().collect::<Vec<_>>(), [11, 21, 31]);
        assert_eq!(map.clone().into_keys().rev().collect::<Vec<_>>(), [3, 2, 1]);
        assert_eq!(map.clone().into_keys().len(), 3);
        assert_eq!(
            map.clone().into_values().rev().collect::<Vec<_>>(),
            [31, 21, 11]
        );
        assert_eq!(map.clone().into_values().len(), 3);
        let owned = map.clone().into_iter();
        assert_eq!(owned.len(), 3);
        assert_eq!(owned.rev().map(|(k, _)| k).collect::<Vec<_>>(), [3, 2, 1]);
        let mut seen = 0;
        for (_, v) in &mut map {
            *v = 0;
            seen += 1;
        }
        assert_eq!(seen, 3);
        assert_eq!((&map).into_iter().count(), 3);
    }

    #[test]
    fn index_by_str() {
        let mut map = IndexMap::new();
        map.insert(String::from("walk"), 1.0_f32);
        assert!((map["walk"] - 1.0).abs() < f32::EPSILON);
        map["walk"] += 1.0;
        assert!((map["walk"] - 2.0).abs() < f32::EPSILON);
    }

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
