//! The entry API of an [`IndexMap`](crate::IndexMap).

use core::fmt;

/// A view into one key of an [`IndexMap`](crate::IndexMap), present or not.
///
/// Created by [`IndexMap::entry`](crate::IndexMap::entry).
///
/// # Examples
///
/// ```
/// use engine_collections::IndexMap;
///
/// let mut counts = IndexMap::new();
/// for word in ["a", "b", "a"] {
///     *counts.entry(word).or_insert(0) += 1;
/// }
/// assert_eq!(counts.get("a"), Some(&2));
/// ```
pub struct Entry<'a, K, V> {
    pub(super) inner: indexmap::map::Entry<'a, K, V>,
}

impl<'a, K, V> Entry<'a, K, V> {
    /// The key this entry is for.
    pub fn key(&self) -> &K {
        self.inner.key()
    }

    /// The position the entry has, or will have once inserted (the map's length, for a
    /// key that is not present yet).
    pub fn index(&self) -> usize {
        self.inner.index()
    }

    /// Inserts `value` at the end if the key is absent; returns the value either way.
    pub fn or_insert(self, value: V) -> &'a mut V {
        self.inner.or_insert(value)
    }

    /// Like [`or_insert`](Self::or_insert), computing the value only if it is needed.
    pub fn or_insert_with(self, default: impl FnOnce() -> V) -> &'a mut V {
        self.inner.or_insert_with(default)
    }

    /// Like [`or_insert`](Self::or_insert), inserting `V::default()`.
    pub fn or_default(self) -> &'a mut V
    where
        V: Default,
    {
        self.inner.or_default()
    }

    /// Calls `f` on the value if the key is present; returns the entry for chaining.
    #[must_use]
    pub fn and_modify(self, f: impl FnOnce(&mut V)) -> Self {
        Self {
            inner: self.inner.and_modify(f),
        }
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for Entry<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}
