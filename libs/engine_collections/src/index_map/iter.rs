//! Iterators over an [`IndexMap`](crate::IndexMap).

use core::fmt;
use core::iter::FusedIterator;

/// Implements the iterator traits of spec §4.4 for a wrapper around an `indexmap` iterator.
macro_rules! iterator {
    ($name:ident<$($lt:lifetime,)? $($g:ident),*> => $item:ty) => {
        impl<$($lt,)? $($g),*> Iterator for $name<$($lt,)? $($g),*> {
            type Item = $item;

            fn next(&mut self) -> Option<Self::Item> {
                self.inner.next()
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                self.inner.size_hint()
            }
        }

        impl<$($lt,)? $($g),*> DoubleEndedIterator for $name<$($lt,)? $($g),*> {
            fn next_back(&mut self) -> Option<Self::Item> {
                self.inner.next_back()
            }
        }

        impl<$($lt,)? $($g),*> ExactSizeIterator for $name<$($lt,)? $($g),*> {
            fn len(&self) -> usize {
                self.inner.len()
            }
        }

        impl<$($lt,)? $($g),*> FusedIterator for $name<$($lt,)? $($g),*> {}
    };
}

/// An iterator over the entries of an [`IndexMap`](crate::IndexMap), in order.
///
/// Created by [`IndexMap::iter`](crate::IndexMap::iter).
///
/// # Examples
///
/// ```
/// use engine_collections::IndexMap;
///
/// let mut map = IndexMap::new();
/// map.insert(1, 'a');
/// map.insert(2, 'b');
/// assert_eq!(map.iter().rev().next(), Some((&2, &'b')));
/// ```
pub struct Iter<'a, K, V> {
    pub(super) inner: indexmap::map::Iter<'a, K, V>,
}

iterator!(Iter<'a, K, V> => (&'a K, &'a V));

impl<K, V> Clone for Iter<'_, K, V> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for Iter<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// An iterator over the keys of an [`IndexMap`](crate::IndexMap), in order.
///
/// Created by [`IndexMap::keys`](crate::IndexMap::keys).
///
/// # Examples
///
/// ```
/// use engine_collections::IndexMap;
///
/// let mut map = IndexMap::new();
/// map.insert(1, 'a');
/// map.insert(2, 'b');
/// assert_eq!(map.keys().copied().collect::<Vec<_>>(), [1, 2]);
/// ```
pub struct Keys<'a, K, V> {
    pub(super) inner: indexmap::map::Keys<'a, K, V>,
}

iterator!(Keys<'a, K, V> => &'a K);

impl<K, V> Clone for Keys<'_, K, V> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<K: fmt::Debug, V> fmt::Debug for Keys<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// A mutable iterator over the entries of an [`IndexMap`](crate::IndexMap), in order.
///
/// Created by [`IndexMap::iter_mut`](crate::IndexMap::iter_mut). Keys are not mutable.
///
/// # Examples
///
/// ```
/// use engine_collections::IndexMap;
///
/// let mut map = IndexMap::new();
/// map.insert(1, 10);
/// for (_, v) in map.iter_mut() {
///     *v += 1;
/// }
/// assert_eq!(map.get(&1), Some(&11));
/// ```
pub struct IterMut<'a, K, V> {
    pub(super) inner: indexmap::map::IterMut<'a, K, V>,
}

iterator!(IterMut<'a, K, V> => (&'a K, &'a mut V));

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for IterMut<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

/// An iterator over the values of an [`IndexMap`](crate::IndexMap), in order.
///
/// Created by [`IndexMap::values`](crate::IndexMap::values).
///
/// # Examples
///
/// ```
/// use engine_collections::IndexMap;
///
/// let mut map = IndexMap::new();
/// map.insert(1, 'a');
/// assert_eq!(map.values().copied().collect::<Vec<_>>(), ['a']);
/// ```
pub struct Values<'a, K, V> {
    pub(super) inner: indexmap::map::Values<'a, K, V>,
}

iterator!(Values<'a, K, V> => &'a V);

impl<K, V> Clone for Values<'_, K, V> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<K, V: fmt::Debug> fmt::Debug for Values<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// A mutable iterator over the values of an [`IndexMap`](crate::IndexMap), in order.
///
/// Created by [`IndexMap::values_mut`](crate::IndexMap::values_mut).
///
/// # Examples
///
/// ```
/// use engine_collections::IndexMap;
///
/// let mut map = IndexMap::new();
/// map.insert(1, 10);
/// for v in map.values_mut() {
///     *v *= 2;
/// }
/// assert_eq!(map.get(&1), Some(&20));
/// ```
pub struct ValuesMut<'a, K, V> {
    pub(super) inner: indexmap::map::ValuesMut<'a, K, V>,
}

iterator!(ValuesMut<'a, K, V> => &'a mut V);

impl<K, V: fmt::Debug> fmt::Debug for ValuesMut<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

/// An owning iterator over the entries of an [`IndexMap`](crate::IndexMap), in order.
///
/// Created by [`IntoIterator::into_iter`] on an `IndexMap`.
///
/// # Examples
///
/// ```
/// use engine_collections::IndexMap;
///
/// let mut map = IndexMap::new();
/// map.insert(1, 'a');
/// assert_eq!(map.into_iter().collect::<Vec<_>>(), [(1, 'a')]);
/// ```
pub struct IntoIter<K, V> {
    pub(super) inner: indexmap::map::IntoIter<K, V>,
}

iterator!(IntoIter<K, V> => (K, V));

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for IntoIter<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

/// An owning iterator over the keys of an [`IndexMap`](crate::IndexMap), in order.
///
/// Created by [`IndexMap::into_keys`](crate::IndexMap::into_keys).
///
/// # Examples
///
/// ```
/// use engine_collections::IndexMap;
///
/// let mut map = IndexMap::new();
/// map.insert(1, 'a');
/// assert_eq!(map.into_keys().collect::<Vec<_>>(), [1]);
/// ```
pub struct IntoKeys<K, V> {
    pub(super) inner: indexmap::map::IntoKeys<K, V>,
}

iterator!(IntoKeys<K, V> => K);

impl<K: fmt::Debug, V> fmt::Debug for IntoKeys<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

/// An owning iterator over the values of an [`IndexMap`](crate::IndexMap), in order.
///
/// Created by [`IndexMap::into_values`](crate::IndexMap::into_values).
///
/// # Examples
///
/// ```
/// use engine_collections::IndexMap;
///
/// let mut map = IndexMap::new();
/// map.insert(1, 'a');
/// assert_eq!(map.into_values().collect::<Vec<_>>(), ['a']);
/// ```
pub struct IntoValues<K, V> {
    pub(super) inner: indexmap::map::IntoValues<K, V>,
}

iterator!(IntoValues<K, V> => V);

impl<K, V: fmt::Debug> fmt::Debug for IntoValues<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}
