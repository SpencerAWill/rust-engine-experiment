//! Named iterator types for [`GenIdMap`](super::GenIdMap).
//!
//! Every iterator walks the dense arrays in dense order and is
//! `ExactSizeIterator + FusedIterator`: the remaining live count is known exactly.

use alloc::vec;
use core::fmt;
use core::iter::FusedIterator;
use core::slice;

use crate::id::GenId;

macro_rules! debug_remaining {
    ($name:literal, $ty:ident) => {
        impl<T> fmt::Debug for $ty<'_, T> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct($name)
                    .field("remaining", &self.len())
                    .finish_non_exhaustive()
            }
        }
    };
}

/// Iterator over `(GenId<T>, &T)` in dense order; returned by [`GenIdMap::iter`](super::GenIdMap::iter).
pub struct Iter<'a, T> {
    pub(super) handles: slice::Iter<'a, GenId<T>>,
    pub(super) values: slice::Iter<'a, T>,
}

impl<T> Clone for Iter<'_, T> {
    fn clone(&self) -> Self {
        Self {
            handles: self.handles.clone(),
            values: self.values.clone(),
        }
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (GenId<T>, &'a T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        Some((*self.handles.next()?, self.values.next()?))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.values.size_hint()
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {}
impl<T> FusedIterator for Iter<'_, T> {}
debug_remaining!("Iter", Iter);

/// Iterator over `(GenId<T>, &mut T)` in dense order; returned by [`GenIdMap::iter_mut`](super::GenIdMap::iter_mut).
pub struct IterMut<'a, T> {
    pub(super) handles: slice::Iter<'a, GenId<T>>,
    pub(super) values: slice::IterMut<'a, T>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (GenId<T>, &'a mut T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        Some((*self.handles.next()?, self.values.next()?))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.values.size_hint()
    }
}

impl<T> ExactSizeIterator for IterMut<'_, T> {}
impl<T> FusedIterator for IterMut<'_, T> {}
debug_remaining!("IterMut", IterMut);

/// Draining iterator over `(GenId<T>, T)` in dense order; returned by [`GenIdMap::drain`](super::GenIdMap::drain).
///
/// The pool is already in the state [`clear`](super::GenIdMap::clear) would
/// produce when the `Drain` is created, so dropping it (consumed or not), or even
/// leaking it, leaves the pool consistent. Values not yet yielded are dropped
/// when the `Drain` is dropped.
pub struct Drain<'a, T> {
    pub(super) handles: vec::Drain<'a, GenId<T>>,
    pub(super) values: vec::Drain<'a, T>,
}

impl<T> Iterator for Drain<'_, T> {
    type Item = (GenId<T>, T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        Some((self.handles.next()?, self.values.next()?))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.values.size_hint()
    }
}

impl<T> ExactSizeIterator for Drain<'_, T> {}
impl<T> FusedIterator for Drain<'_, T> {}
debug_remaining!("Drain", Drain);

/// Owning iterator over `(GenId<T>, T)` in dense order; returned by `GenIdMap::into_iter`.
pub struct IntoIter<T> {
    pub(super) handles: vec::IntoIter<GenId<T>>,
    pub(super) values: vec::IntoIter<T>,
}

impl<T> Iterator for IntoIter<T> {
    type Item = (GenId<T>, T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        Some((self.handles.next()?, self.values.next()?))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.values.size_hint()
    }
}

impl<T> ExactSizeIterator for IntoIter<T> {}
impl<T> FusedIterator for IntoIter<T> {}

impl<T> fmt::Debug for IntoIter<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntoIter")
            .field("remaining", &self.len())
            .finish_non_exhaustive()
    }
}
