//! [`HandleAllocator`]: handle allocation without value storage.

use core::fmt;
use core::iter::FusedIterator;
use core::marker::PhantomData;

use crate::handle::{Handle, write_short_type_name};
use crate::pool::{HandlePool, Iter};

/// Re-types a handle. Internal only: the public API never exposes a cast (see [`Handle`]).
#[inline]
fn retype<A, B>(handle: Handle<A>) -> Handle<B> {
    Handle::new(handle.index(), handle.generation())
}

/// Handle allocation without value storage: the shape of an ECS entity allocator.
///
/// It hands out [`Handle`]s and tracks which are alive, but stores no values.
/// Semantics are identical to [`HandlePool`], by construction: it wraps a
/// `HandlePool<()>`. In particular, new slots start at generation 1; freeing bumps
/// the generation (or retires the slot on exhaustion); freed slots are reused LIFO;
/// [`clear`](Self::clear) returns slots in ascending index order; iteration is in
/// dense order (swap-removal on free).
///
/// # Examples
///
/// ```
/// use engine_handle::HandleAllocator;
///
/// struct Entity;
///
/// let mut entities = HandleAllocator::<Entity>::new();
/// let a = entities.allocate();
/// assert!(entities.is_alive(a));
/// assert!(entities.free(a));
/// assert!(!entities.is_alive(a)); // stale
/// let b = entities.allocate(); // same slot, next generation
/// assert_eq!((b.index(), b.generation().get()), (a.index(), 2));
/// ```
pub struct HandleAllocator<T> {
    pool: HandlePool<()>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> HandleAllocator<T> {
    /// Creates an empty allocator. Does not allocate.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandleAllocator;
    /// assert!(HandleAllocator::<u8>::new().is_empty());
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            pool: HandlePool::new(),
            _marker: PhantomData,
        }
    }

    /// Creates an empty allocator with room for `capacity` live handles.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandleAllocator;
    /// let _ = HandleAllocator::<u8>::with_capacity(64);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            pool: HandlePool::with_capacity(capacity),
            _marker: PhantomData,
        }
    }

    /// An allocator that holds at most `limit` slots, for testing the full path.
    #[cfg(test)]
    pub(crate) fn with_slot_limit(limit: u32) -> Self {
        Self {
            pool: HandlePool::with_slot_limit(limit),
            _marker: PhantomData,
        }
    }

    /// Allocates a fresh handle.
    ///
    /// # Panics
    ///
    /// Panics if 2³¹−2 slots are in use or retired. Use
    /// [`try_allocate`](Self::try_allocate) to handle that without panicking.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandleAllocator;
    /// let mut alloc = HandleAllocator::<u8>::new();
    /// let h = alloc.allocate();
    /// assert!(alloc.is_alive(h));
    /// ```
    #[must_use = "dropping the handle leaks a live slot until clear() or retain()"]
    #[track_caller]
    pub fn allocate(&mut self) -> Handle<T> {
        let Some(handle) = self.try_allocate() else {
            panic!(
                "HandleAllocator::allocate: allocator is full (2^31-2 slots in use or retired); \
                 use try_allocate to handle this without panicking"
            )
        };
        handle
    }

    /// Fallible twin of [`allocate`](Self::allocate): `None` if the allocator is full.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandleAllocator;
    /// let mut alloc = HandleAllocator::<u8>::new();
    /// assert!(alloc.try_allocate().is_some());
    /// ```
    pub fn try_allocate(&mut self) -> Option<Handle<T>> {
        self.pool.try_insert(()).ok().map(retype)
    }

    /// Frees `handle`, making it and every copy of it stale. Returns `false` if it
    /// was already stale.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandleAllocator;
    /// let mut alloc = HandleAllocator::<u8>::new();
    /// let h = alloc.allocate();
    /// assert!(alloc.free(h));
    /// assert!(!alloc.free(h));
    /// ```
    pub fn free(&mut self, handle: Handle<T>) -> bool {
        self.pool.remove(retype(handle)).is_some()
    }

    /// Returns `true` if `handle` has been allocated and not yet freed.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandleAllocator;
    /// let mut alloc = HandleAllocator::<u8>::new();
    /// let h = alloc.allocate();
    /// assert!(alloc.is_alive(h));
    /// ```
    #[inline]
    pub fn is_alive(&self, handle: Handle<T>) -> bool {
        self.pool.contains(retype(handle))
    }

    /// Number of live handles.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandleAllocator;
    /// let mut alloc = HandleAllocator::<u8>::new();
    /// let _ = alloc.allocate();
    /// assert_eq!(alloc.len(), 1);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.pool.len()
    }

    /// Returns `true` if no handle is alive.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandleAllocator;
    /// assert!(HandleAllocator::<u8>::new().is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pool.is_empty()
    }

    /// Frees every handle. Slots are then reused in ascending index order.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandleAllocator;
    /// let mut alloc = HandleAllocator::<u8>::new();
    /// let h = alloc.allocate();
    /// alloc.clear();
    /// assert!(!alloc.is_alive(h));
    /// ```
    pub fn clear(&mut self) {
        self.pool.clear();
    }

    /// Iterates the live handles in dense order.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandleAllocator;
    /// let mut alloc = HandleAllocator::<u8>::new();
    /// let a = alloc.allocate();
    /// assert_eq!(alloc.iter().next(), Some(a));
    /// ```
    pub fn iter(&self) -> AllocIter<'_, T> {
        AllocIter {
            inner: self.pool.iter(),
            _marker: PhantomData,
        }
    }

    /// Keeps only the handles for which `keep` returns `true`, visiting in dense order.
    /// Removed handles behave exactly like [`free`](Self::free).
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandleAllocator;
    /// let mut alloc = HandleAllocator::<u8>::new();
    /// for _ in 0..4 {
    ///     let _ = alloc.allocate();
    /// }
    /// alloc.retain(|h| h.index() < 2);
    /// assert_eq!(alloc.len(), 2);
    /// ```
    pub fn retain(&mut self, mut keep: impl FnMut(Handle<T>) -> bool) {
        self.pool.retain(|handle, ()| keep(retype(handle)));
    }

    /// Checks every internal invariant, panicking on the first violation.
    ///
    /// Available in tests and with the `test-invariants` feature; not part of the stable API.
    ///
    /// # Panics
    ///
    /// Panics if any invariant is violated, which would be a bug in this crate.
    #[cfg(any(test, feature = "test-invariants"))]
    #[track_caller]
    pub fn assert_invariants(&self) {
        self.pool.assert_invariants();
    }
}

impl<T> Default for HandleAllocator<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for HandleAllocator<T> {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> fmt::Debug for HandleAllocator<T> {
    /// Prints `HandleAllocator<T> [Handle<T>(0v1), ...]`, listing the live handles.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HandleAllocator<")?;
        write_short_type_name::<T>(f)?;
        f.write_str("> ")?;
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<'a, T> IntoIterator for &'a HandleAllocator<T> {
    type Item = Handle<T>;
    type IntoIter = AllocIter<'a, T>;

    fn into_iter(self) -> AllocIter<'a, T> {
        self.iter()
    }
}

/// Iterator over the live handles of a [`HandleAllocator`], in dense order.
///
/// Returned by [`HandleAllocator::iter`]; it is `ExactSizeIterator + FusedIterator`.
pub struct AllocIter<'a, T> {
    inner: Iter<'a, ()>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for AllocIter<'_, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> Iterator for AllocIter<'_, T> {
    type Item = Handle<T>;

    #[inline]
    fn next(&mut self) -> Option<Handle<T>> {
        self.inner.next().map(|(handle, ())| retype(handle))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<T> ExactSizeIterator for AllocIter<'_, T> {}
impl<T> FusedIterator for AllocIter<'_, T> {}

impl<T> fmt::Debug for AllocIter<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AllocIter")
            .field("remaining", &self.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn free_returns_false_for_stale_handle() {
        let mut a = HandleAllocator::<u8>::new();
        let h = a.allocate();
        assert!(a.is_alive(h));
        assert!(a.free(h));
        assert!(!a.free(h));
        assert!(!a.is_alive(h));
        a.assert_invariants();
    }

    #[test]
    fn allocate_after_free_reuses_slot_lifo_with_next_generation() {
        let mut a = HandleAllocator::<u8>::new();
        let first = a.allocate();
        let second = a.allocate();
        a.free(first);
        a.free(second);
        let third = a.allocate(); // LIFO: second's slot
        assert_eq!(third.index(), second.index());
        assert_eq!(third.generation().get(), second.generation().get() + 1);
        let fourth = a.allocate();
        assert_eq!(fourth.index(), first.index());
        a.assert_invariants();
    }

    #[test]
    fn try_allocate_returns_none_at_the_slot_limit() {
        let mut a = HandleAllocator::<u8>::with_slot_limit(2);
        assert!(a.try_allocate().is_some());
        assert!(a.try_allocate().is_some());
        assert!(a.try_allocate().is_none());
    }

    #[test]
    #[should_panic(expected = "try_allocate")]
    fn allocate_past_limit_panics_with_hint() {
        let mut a = HandleAllocator::<u8>::with_slot_limit(1);
        let _ = a.allocate();
        let _ = a.allocate();
    }

    #[test]
    fn iter_retain_clear_and_len() {
        let mut a = HandleAllocator::<u8>::new();
        let hs: Vec<_> = (0..6).map(|_| a.allocate()).collect();
        assert_eq!(a.len(), 6);
        assert_eq!(a.iter().len(), 6);
        a.retain(|h| h.index() % 2 == 0);
        assert_eq!(a.len(), 3);
        assert_eq!(a.iter().count(), 3);
        assert!(a.is_alive(hs[0]) && !a.is_alive(hs[1]));
        a.clear();
        assert!(a.is_empty() && !a.is_alive(hs[0]));
        let again = a.allocate();
        assert_eq!((again.index(), again.generation().get()), (0, 2));
        a.assert_invariants();
    }

    #[test]
    fn debug_lists_live_handles() {
        let mut a = HandleAllocator::<u8>::new();
        let _ = a.allocate();
        let _ = a.allocate();
        assert_eq!(
            alloc::format!("{a:?}"),
            "HandleAllocator<u8> [Handle<u8>(0v1), Handle<u8>(1v1)]"
        );
    }
}
