//! Iterators over an [`IndexSet`](crate::IndexSet).

use core::fmt;
use core::iter::FusedIterator;

/// An iterator over the items of an [`IndexSet`](crate::IndexSet), in order.
///
/// Created by [`IndexSet::iter`](crate::IndexSet::iter).
///
/// # Examples
///
/// ```
/// use engine_collections::IndexSet;
///
/// let mut set = IndexSet::new();
/// set.insert(2);
/// set.insert(1);
/// assert_eq!(set.iter().copied().collect::<Vec<_>>(), [2, 1]);
/// ```
pub struct Iter<'a, T> {
    pub(super) inner: indexmap::set::Iter<'a, T>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<T> DoubleEndedIterator for Iter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back()
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<T> FusedIterator for Iter<'_, T> {}

impl<T> Clone for Iter<'_, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for Iter<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// An owning iterator over the items of an [`IndexSet`](crate::IndexSet), in order.
///
/// Created by [`IntoIterator::into_iter`] on an `IndexSet`.
///
/// # Examples
///
/// ```
/// use engine_collections::IndexSet;
///
/// let mut set = IndexSet::new();
/// set.insert('x');
/// assert_eq!(set.into_iter().collect::<Vec<_>>(), ['x']);
/// ```
pub struct IntoIter<T> {
    pub(super) inner: indexmap::set::IntoIter<T>,
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<T> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<T> {
        self.inner.next_back()
    }
}

impl<T> ExactSizeIterator for IntoIter<T> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<T> FusedIterator for IntoIter<T> {}

impl<T: fmt::Debug> fmt::Debug for IntoIter<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}
