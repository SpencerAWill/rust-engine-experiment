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
