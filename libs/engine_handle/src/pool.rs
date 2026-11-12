//! [`HandlePool`]: dense storage addressed by generational handles.

use alloc::vec::Vec;
use core::fmt;
use core::ops::{Index, IndexMut};

use alloc::collections::TryReserveError;

use crate::error::CapacityError;
use crate::handle::Handle;
#[cfg(test)]
use crate::slots::MAX_SLOTS;
#[cfg(any(test, feature = "test-invariants"))]
use crate::slots::SlotState;
use crate::slots::{SlotTable, to_u32};
#[cfg(test)]
use core::num::NonZeroU32;

mod iter;

pub use iter::{Drain, IntoIter, Iter, IterMut};

/// Dense storage addressed by generational [`Handle`]s: a "sparse set with generations".
///
/// Values live in one contiguous array, so iteration is `O(len)` over contiguous
/// memory, and [`values`](Self::values) exposes them as a slice. Each slot
/// records its value's dense position, so [`get`](Self::get) costs two array
/// accesses (slot, then dense position).
///
/// # Semantics
///
/// - New slots start at generation 1. [`remove`](Self::remove) bumps the slot's
///   generation, so every outstanding handle to it becomes stale
///   ([`get`](Self::get) returns `None`).
/// - Freed slots are reused **LIFO**, so allocation order is deterministic for a
///   given operation sequence.
/// - **Generation exhaustion:** if bumping would overflow `u32::MAX`, the slot is
///   permanently retired. That costs one slot per 2³²−1 reuses and guarantees a
///   stale handle can never become valid again.
/// - [`clear`](Self::clear) makes every outstanding handle stale and returns all
///   non-retired slots to the free list in ascending index order, so reuse after
///   `clear` is deterministic too.
/// - [`remove`](Self::remove) **swap-removes**: the last dense element moves into
///   the hole. Iteration order is therefore dense order: fully deterministic (a pure
///   function of the operation sequence) but not ascending by slot index.
///   Inserting a, b, c and removing a gives the order c, b.
/// - At most 2³¹−2 (`0x7FFF_FFFE`) slots exist, because a slot packs its state into 32 bits; [`insert`](Self::insert) panics beyond that
///   and [`try_insert`](Self::try_insert) reports it.
///
/// # Complexity and memory
///
/// `insert`, `get`, `remove`, and `contains` are `O(1)`. Iteration, `retain`,
/// `values`, and `drain` are `O(len)` over contiguous memory. Memory is 8 bytes per
/// slot ever used (slots never shrink, because indices must stay stable), plus 8 bytes
/// per live value (its dense `Handle`), plus the values themselves.
/// [`shrink_to_fit`](Self::shrink_to_fit) releases dense capacity after a spike.
///
/// # Limitation
///
/// A handle used on a *different* pool than the one that issued it can resolve to an
/// unrelated element; see the [crate docs](crate#known-limitation).
///
/// # Examples
///
/// ```
/// use engine_handle::HandlePool;
///
/// let mut pool = HandlePool::new();
/// let a = pool.insert("a");
/// let b = pool.insert("b");
///
/// assert_eq!(pool[a], "a");
/// assert_eq!(pool.remove(a), Some("a"));
/// assert!(pool.get(a).is_none()); // `a` is stale now
/// assert_eq!(pool.values(), &["b"]);
/// assert_eq!(pool[b], "b");
/// ```
#[derive(Clone)]
pub struct HandlePool<T> {
    slots: SlotTable,
    handles: Vec<Handle<T>>,
    values: Vec<T>,
}

impl<T> HandlePool<T> {
    /// Creates an empty pool. Does not allocate.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let pool = HandlePool::<u8>::new();
    /// assert!(pool.is_empty());
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            slots: SlotTable::new(),
            handles: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Creates an empty pool with room for `capacity` values before reallocating.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let pool = HandlePool::<u8>::with_capacity(16);
    /// assert!(pool.capacity() >= 16);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: SlotTable::with_capacity(capacity),
            handles: Vec::with_capacity(capacity),
            values: Vec::with_capacity(capacity),
        }
    }

    /// Reserves room for at least `additional` more values without reallocating.
    ///
    /// The room is counted from the live values now: after `reserve(n)`, the next `n`
    /// inserts do not reallocate.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity overflows `usize`, or if the allocator fails, exactly like
    /// `Vec::reserve`. Use [`try_reserve`](Self::try_reserve) to handle that without panicking.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut x = HandlePool::<u64>::new();
    /// x.reserve(1000);
    /// ```
    #[track_caller]
    pub fn reserve(&mut self, additional: usize) {
        if let Err(error) = self.try_reserve(additional) {
            panic!(
                "HandlePool::reserve: cannot reserve room for {additional} more values ({error}); \
                 use try_reserve to handle this without panicking"
            );
        }
    }

    /// Fallible twin of [`reserve`](Self::reserve).
    ///
    /// # Errors
    ///
    /// Returns [`TryReserveError`] if the capacity overflows or the allocator fails. On error
    /// some of the internal arrays may already have grown, which is harmless.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut x = HandlePool::<u64>::new();
    /// assert!(x.try_reserve(16).is_ok());
    /// assert!(x.try_reserve(usize::MAX).is_err());
    /// ```
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.handles.try_reserve(additional)?;
        self.values.try_reserve(additional)?;
        self.slots
            .try_reserve_slots(self.len().saturating_add(additional))
    }

    /// Slots the pool can hold without reallocating its slot array.
    #[cfg(test)]
    pub(crate) fn slot_capacity(&self) -> usize {
        self.slots.capacity()
    }

    /// A pool that holds at most `limit` slots, for testing the full-pool paths.
    #[cfg(test)]
    pub(crate) fn with_slot_limit(limit: u32) -> Self {
        Self {
            slots: SlotTable::with_limit(limit),
            ..Self::new()
        }
    }

    /// Inserts `value` and returns the handle that names it.
    ///
    /// # Panics
    ///
    /// Panics if 2³¹−2 slots are already in use or retired. Use
    /// [`try_insert`](Self::try_insert) to handle that without panicking.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut pool = HandlePool::new();
    /// let h = pool.insert(42);
    /// assert_eq!(pool[h], 42);
    /// ```
    #[must_use = "dropping the handle makes the value unreachable except by iteration"]
    #[track_caller]
    pub fn insert(&mut self, value: T) -> Handle<T> {
        let Ok(handle) = self.try_insert(value) else {
            panic!(
                "HandlePool::insert: pool is full ({} slots in use or retired); \
                 use try_insert to handle this without panicking",
                self.slots.limit()
            )
        };
        handle
    }

    /// Fallible twin of [`insert`](Self::insert). On failure, ownership of `value`
    /// is returned in the error.
    ///
    /// # Errors
    ///
    /// Returns [`CapacityError`] if 2³¹−2 slots are already in use or retired.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut pool = HandlePool::new();
    /// let h = pool.try_insert("x").unwrap();
    /// assert_eq!(pool[h], "x");
    /// ```
    pub fn try_insert(&mut self, value: T) -> Result<Handle<T>, CapacityError<T>> {
        let Some((index, generation)) = self.slots.acquire(to_u32(self.handles.len())) else {
            return Err(CapacityError(value));
        };
        let handle = Handle::new(index, generation);
        self.handles.push(handle);
        self.values.push(value);
        Ok(handle)
    }

    /// Dense position of the value `handle` names, if the handle is live.
    #[inline]
    fn resolve(&self, handle: Handle<T>) -> Option<usize> {
        let dense = self.slots.lookup(handle.index(), handle.generation())? as usize;
        // Occupied slots have the top bit clear, so the link is the dense position. Vacant and
        // retired links are at least `VACANT_BIT`, which exceeds any dense length, so one range
        // check against the dense length replaces a separate state test.
        (dense < self.values.len()).then_some(dense)
    }

    /// Returns a reference to the value `handle` names, or `None` if it is stale.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut pool = HandlePool::new();
    /// let h = pool.insert(1);
    /// assert_eq!(pool.get(h), Some(&1));
    /// pool.remove(h);
    /// assert_eq!(pool.get(h), None);
    /// ```
    #[inline]
    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        self.resolve(handle).map(|dense| &self.values[dense])
    }

    /// Returns a mutable reference to the value `handle` names, or `None` if it is stale.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut pool = HandlePool::new();
    /// let h = pool.insert(1);
    /// *pool.get_mut(h).unwrap() += 1;
    /// assert_eq!(pool[h], 2);
    /// ```
    #[inline]
    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        self.resolve(handle).map(|dense| &mut self.values[dense])
    }

    /// Returns `true` if `handle` names a live value in this pool.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut pool = HandlePool::new();
    /// let h = pool.insert(());
    /// assert!(pool.contains(h));
    /// pool.remove(h);
    /// assert!(!pool.contains(h));
    /// ```
    #[inline]
    pub fn contains(&self, handle: Handle<T>) -> bool {
        self.resolve(handle).is_some()
    }

    /// Removes the value `handle` names and returns it, or `None` if the handle is stale.
    ///
    /// The last dense element moves into the vacated position (swap-remove), so
    /// [`values`](Self::values) changes order; every other handle stays valid.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut pool = HandlePool::new();
    /// let a = pool.insert('a');
    /// let b = pool.insert('b');
    /// let c = pool.insert('c');
    /// assert_eq!(pool.remove(a), Some('a'));
    /// assert_eq!(pool.values(), &['c', 'b']); // `c` moved into `a`'s place
    /// assert_eq!(pool[c], 'c');
    /// assert_eq!(pool.remove(a), None);
    /// # let _ = b;
    /// ```
    pub fn remove(&mut self, handle: Handle<T>) -> Option<T> {
        let dense = self.resolve(handle)?;
        Some(self.remove_at(dense))
    }

    /// Swap-removes the element at dense position `dense`, keeping every slot consistent.
    fn remove_at(&mut self, dense: usize) -> T {
        let handle = self.handles[dense];
        self.slots.release(handle.index());
        self.handles.swap_remove(dense);
        let value = self.values.swap_remove(dense);
        if let Some(moved) = self.handles.get(dense) {
            self.slots.set_link(moved.index(), to_u32(dense));
        }
        value
    }

    /// Number of live values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut pool = HandlePool::new();
    /// let _ = pool.insert(1);
    /// assert_eq!(pool.len(), 1);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns `true` if the pool holds no live values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// assert!(HandlePool::<u8>::new().is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Number of values the pool can hold without reallocating its dense value
    /// storage (`Vec::capacity` semantics).
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// assert!(HandlePool::<u8>::with_capacity(8).capacity() >= 8);
    /// ```
    pub fn capacity(&self) -> usize {
        self.values.capacity()
    }

    /// Shrinks the dense value and handle storage to fit [`len`](Self::len).
    ///
    /// The slot array cannot shrink, because indices must stay stable; it costs
    /// 8 bytes per slot ever used.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut pool = HandlePool::with_capacity(100);
    /// let _ = pool.insert(1);
    /// pool.shrink_to_fit();
    /// assert!(pool.capacity() < 100);
    /// ```
    pub fn shrink_to_fit(&mut self) {
        self.handles.shrink_to_fit();
        self.values.shrink_to_fit();
    }

    /// All live values as one contiguous slice, in dense order.
    ///
    /// The fastest way to process every value when handles are not needed.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut pool = HandlePool::new();
    /// let _ = pool.insert(1);
    /// let _ = pool.insert(2);
    /// assert_eq!(pool.values().iter().sum::<i32>(), 3);
    /// ```
    #[inline]
    pub fn values(&self) -> &[T] {
        &self.values
    }

    /// Mutable view of [`values`](Self::values).
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut pool = HandlePool::new();
    /// let h = pool.insert(1);
    /// for v in pool.values_mut() {
    ///     *v *= 10;
    /// }
    /// assert_eq!(pool[h], 10);
    /// ```
    #[inline]
    pub fn values_mut(&mut self) -> &mut [T] {
        &mut self.values
    }

    /// Keeps only the elements for which `keep` returns `true`, visiting in dense order.
    ///
    /// Removed elements behave exactly like [`remove`](Self::remove), including
    /// swap-removal. The pool is consistent after every removal, so if `keep`
    /// panics the pool is left valid (elements not yet visited are kept).
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut pool = HandlePool::new();
    /// for i in 0..6 {
    ///     let _ = pool.insert(i);
    /// }
    /// pool.retain(|_, v| *v % 2 == 0);
    /// assert_eq!(pool.len(), 3);
    /// ```
    pub fn retain(&mut self, mut keep: impl FnMut(Handle<T>, &mut T) -> bool) {
        let mut dense = 0;
        while dense < self.values.len() {
            let handle = self.handles[dense];
            if keep(handle, &mut self.values[dense]) {
                dense += 1;
            } else {
                // The last element is swapped into `dense`, so it is visited next.
                drop(self.remove_at(dense));
            }
        }
    }

    /// Mutable access to several distinct elements at once (for example both
    /// bodies in a collision), without `RefCell` or `unsafe`.
    ///
    /// Returns `None` if any handle is stale or two handles refer to the same slot.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut pool = HandlePool::new();
    /// let a = pool.insert(1);
    /// let b = pool.insert(2);
    /// let [x, y] = pool.get_disjoint_mut([a, b]).unwrap();
    /// std::mem::swap(x, y);
    /// assert_eq!((pool[a], pool[b]), (2, 1));
    /// assert!(pool.get_disjoint_mut([a, a]).is_none());
    /// ```
    pub fn get_disjoint_mut<const N: usize>(
        &mut self,
        handles: [Handle<T>; N],
    ) -> Option<[&mut T; N]> {
        let mut positions = [0_usize; N];
        for (position, handle) in positions.iter_mut().zip(handles) {
            *position = self.resolve(handle)?;
        }
        self.values.get_disjoint_mut(positions).ok()
    }

    /// Iterates `(handle, &value)` pairs in dense order.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut pool = HandlePool::new();
    /// let a = pool.insert('a');
    /// assert_eq!(pool.iter().next(), Some((a, &'a')));
    /// ```
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            handles: self.handles.iter(),
            values: self.values.iter(),
        }
    }

    /// Iterates `(handle, &mut value)` pairs in dense order.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut pool = HandlePool::new();
    /// let a = pool.insert(1);
    /// for (_, v) in pool.iter_mut() {
    ///     *v += 1;
    /// }
    /// assert_eq!(pool[a], 2);
    /// ```
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            handles: self.handles.iter(),
            values: self.values.iter_mut(),
        }
    }

    /// Removes all values, yielding `(handle, value)` in dense order.
    ///
    /// When the [`Drain`] is dropped (fully consumed or not) the pool is in
    /// exactly the state [`clear`](Self::clear) would produce.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut pool = HandlePool::new();
    /// let a = pool.insert(1);
    /// let drained: Vec<_> = pool.drain().collect();
    /// assert_eq!(drained, vec![(a, 1)]);
    /// assert!(pool.is_empty() && !pool.contains(a));
    /// ```
    pub fn drain(&mut self) -> Drain<'_, T> {
        // Release the slots first: from here on the pool already looks cleared,
        // even if the `Drain` is leaked.
        self.slots.release_all();
        Drain {
            handles: self.handles.drain(..),
            values: self.values.drain(..),
        }
    }

    /// Removes every value. All outstanding handles become stale, and reuse of
    /// slots then proceeds in ascending index order.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandlePool;
    /// let mut pool = HandlePool::new();
    /// let h = pool.insert(1);
    /// pool.clear();
    /// assert!(pool.is_empty() && !pool.contains(h));
    /// ```
    pub fn clear(&mut self) {
        self.slots.release_all();
        self.handles.clear();
        self.values.clear();
    }

    /// Checks every internal invariant, panicking with a description of the first violation.
    ///
    /// Available in tests and with the `test-invariants` feature; not part of the stable API.
    ///
    /// # Panics
    ///
    /// Panics if any invariant is violated, which would be a bug in this crate.
    #[cfg(any(test, feature = "test-invariants"))]
    #[track_caller]
    pub fn assert_invariants(&self) {
        assert_eq!(
            self.handles.len(),
            self.values.len(),
            "dense arrays differ in length"
        );
        let occupied = self.slots.assert_invariants();
        assert_eq!(
            occupied,
            self.handles.len(),
            "occupied slots != dense length"
        );
        for (i, handle) in self.handles.iter().enumerate() {
            assert!(
                matches!(
                    self.slots.state(handle.index()),
                    SlotState::Occupied { dense } if dense as usize == i
                ),
                "slot of {handle:?} does not point back at dense position {i}"
            );
            assert_eq!(
                self.slots.generation(handle.index()),
                handle.generation(),
                "generation mismatch for {handle:?}"
            );
        }
    }

    /// Overwrites a slot's generation (and its dense handle), for exhaustion tests.
    #[cfg(test)]
    fn set_generation_for_test(&mut self, index: u32, generation: u32) {
        let generation = NonZeroU32::new(generation).unwrap();
        self.slots.set_generation(index, generation);
        if let SlotState::Occupied { dense } = self.slots.state(index) {
            self.handles[dense as usize] = Handle::new(index, generation);
        }
    }
}

impl<T> Default for HandlePool<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: fmt::Debug> fmt::Debug for HandlePool<T> {
    /// Prints as a map of live handles to values, in dense order.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<'a, T> IntoIterator for &'a HandlePool<T> {
    type Item = (Handle<T>, &'a T);
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut HandlePool<T> {
    type Item = (Handle<T>, &'a mut T);
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> IterMut<'a, T> {
        self.iter_mut()
    }
}

impl<T> IntoIterator for HandlePool<T> {
    type Item = (Handle<T>, T);
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> IntoIter<T> {
        IntoIter {
            handles: self.handles.into_iter(),
            values: self.values.into_iter(),
        }
    }
}

impl<T> Index<Handle<T>> for HandlePool<T> {
    type Output = T;

    /// # Panics
    ///
    /// Panics if `handle` is stale. Use [`HandlePool::get`] to handle that without panicking.
    #[inline]
    #[track_caller]
    fn index(&self, handle: Handle<T>) -> &T {
        match self.get(handle) {
            Some(value) => value,
            None => stale_handle(handle),
        }
    }
}

impl<T> IndexMut<Handle<T>> for HandlePool<T> {
    /// # Panics
    ///
    /// Panics if `handle` is stale. Use [`HandlePool::get_mut`] to handle that without panicking.
    #[inline]
    #[track_caller]
    fn index_mut(&mut self, handle: Handle<T>) -> &mut T {
        match self.get_mut(handle) {
            Some(value) => value,
            None => stale_handle(handle),
        }
    }
}

#[cold]
#[track_caller]
fn stale_handle<T>(handle: Handle<T>) -> ! {
    panic!(
        "HandlePool index: stale handle {handle:?} (its value was removed, or the pool was \
         cleared); use get() to handle this without panicking"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn insert_get_remove_roundtrip() {
        let mut p = HandlePool::new();
        let h = p.insert(5);
        assert_eq!(p.get(h), Some(&5));
        assert_eq!(p.remove(h), Some(5));
        assert_eq!(p.get(h), None);
        assert_eq!(p.len(), 0);
        assert!(p.is_empty());
        p.assert_invariants();
    }

    #[test]
    fn removed_handle_is_stale_and_slot_reused_lifo() {
        let mut p = HandlePool::new();
        let a = p.insert('a');
        let b = p.insert('b');
        p.remove(a);
        p.remove(b);
        let c = p.insert('c'); // LIFO: reuses b's slot
        assert_eq!(c.index(), b.index());
        assert_eq!(c.generation().get(), b.generation().get() + 1);
        assert!(!p.contains(b));
        p.assert_invariants();
    }

    #[test]
    fn generation_exhaustion_retires_slot() {
        let mut p = HandlePool::new();
        let h = p.insert(1);
        p.set_generation_for_test(h.index(), u32::MAX);
        let h = Handle::new(h.index(), NonZeroU32::new(u32::MAX).unwrap());
        assert_eq!(p.remove(h), Some(1));
        let n = p.insert(2);
        assert_ne!(n.index(), h.index()); // retired, never reused
        p.remove(n);
        let m = p.insert(3);
        assert_ne!(m.index(), h.index()); // still never reused after more churn
        p.assert_invariants();
    }

    #[test]
    fn clear_staleness_and_ascending_reuse() {
        let mut p = HandlePool::new();
        let old: Vec<_> = (0..3).map(|i| p.insert(i)).collect();
        p.clear();
        assert!(old.iter().all(|h| !p.contains(*h)));
        assert!(p.is_empty());
        let new: Vec<_> = (0..3).map(|i| p.insert(i)).collect();
        assert_eq!(new.iter().map(|h| h.index()).collect::<Vec<_>>(), [0, 1, 2]);
        assert!(new.iter().all(|h| h.generation().get() == 2));
        p.assert_invariants();
    }

    #[test]
    fn remove_swaps_last_into_hole() {
        let mut p = HandlePool::new();
        let a = p.insert('a');
        let b = p.insert('b');
        let c = p.insert('c');
        assert_eq!(p.remove(a), Some('a'));
        assert_eq!(p.values(), &['c', 'b']); // dense order after swap-remove
        assert_eq!(p[c], 'c');
        assert_eq!(p[b], 'b'); // moved element still resolves
        p.assert_invariants();
    }

    #[test]
    fn values_mut_and_shrink_to_fit() {
        let mut p = HandlePool::new();
        let hs: Vec<_> = (0..1000).map(|i| p.insert(i)).collect();
        for h in &hs[10..] {
            p.remove(*h);
        }
        for v in p.values_mut() {
            *v += 1;
        }
        p.shrink_to_fit();
        assert!(p.capacity() < 1000);
        for (i, h) in hs[..10].iter().enumerate() {
            assert_eq!(p[*h], i + 1);
        }
        p.assert_invariants();
    }

    #[test]
    fn forged_handle_to_vacant_slot_is_stale_not_a_panic() {
        let mut p = HandlePool::new();
        let h = p.insert(1);
        p.remove(h);
        // A vacant slot stores the generation its next occupant will get; forge exactly that.
        let forged = Handle::<i32>::from_bits(h.to_bits() + 1).unwrap();
        assert_eq!(p.get(forged), None);
        assert_eq!(p.get_mut(forged), None);
        assert!(!p.contains(forged));
        assert_eq!(p.remove(forged), None);
    }

    #[test]
    fn try_insert_returns_value_when_full() {
        let mut p = HandlePool::with_slot_limit(1);
        let _ = p.insert(1);
        assert_eq!(p.try_insert(2).unwrap_err().into_inner(), 2);
    }

    #[test]
    #[should_panic(expected = "try_insert")]
    fn insert_past_limit_panics_with_hint() {
        let mut p = HandlePool::with_slot_limit(1);
        let _ = p.insert(1);
        let _ = p.insert(2);
    }

    #[test]
    fn reserve_then_insert_does_not_reallocate_any_array() {
        let mut p = HandlePool::<u64>::new();
        p.reserve(100);
        let (values, slots) = (p.capacity(), p.slot_capacity());
        assert!(values >= 100 && slots >= 100);
        for i in 0..100 {
            let _ = p.insert(i);
        }
        assert_eq!((p.capacity(), p.slot_capacity()), (values, slots));
    }

    #[test]
    fn reserve_counts_live_values() {
        let mut p = HandlePool::new();
        for i in 0..50 {
            let _ = p.insert(i);
        }
        p.reserve(50);
        assert!(p.capacity() >= 100 && p.slot_capacity() >= 100);
    }

    #[test]
    #[should_panic(expected = "try_reserve")]
    fn reserve_panics_on_capacity_overflow() {
        HandlePool::<u8>::new().reserve(usize::MAX);
    }

    #[test]
    fn try_reserve_reports_overflow_instead_of_panicking() {
        let mut p = HandlePool::<u8>::new();
        assert!(p.try_reserve(usize::MAX).is_err());
        assert!(p.try_reserve(16).is_ok());
    }

    #[test]
    fn zero_sized_values_work() {
        let mut p = HandlePool::<()>::new();
        let a = p.insert(());
        let b = p.insert(());
        let c = p.insert(());
        assert_eq!(p.remove(b), Some(()));
        assert_eq!(p.len(), 2);
        assert!(p.contains(a) && p.contains(c) && !p.contains(b));
        assert_eq!(p.values().len(), 2);
        p.assert_invariants();
    }

    #[test]
    fn iteration_is_dense_and_exact_size() {
        let mut p = HandlePool::new();
        let hs: Vec<_> = (0..5).map(|i| p.insert(i)).collect();
        p.remove(hs[1]);
        p.remove(hs[3]);
        assert_eq!(p.values(), &[0, 4, 2]); // 4 moved into 1's hole, then 3 was last
        let mut it = p.iter();
        assert_eq!(it.len(), 3);
        let seen: Vec<_> = it.by_ref().map(|(_, v)| *v).collect();
        assert_eq!(seen, [0, 4, 2]);
        assert_eq!(it.len(), 0);
        assert!(it.next().is_none());
        assert!(it.next().is_none()); // fused
        let mut it = p.iter();
        let _ = it.next();
        assert_eq!(it.len(), 2);
    }

    #[test]
    fn iter_mut_and_into_iter_cover_every_element() {
        let mut p = HandlePool::new();
        let hs: Vec<_> = (0..4).map(|i| p.insert(i)).collect();
        for (h, v) in &mut p {
            *v += i32::try_from(h.index()).unwrap() * 10;
        }
        assert_eq!((&p).into_iter().count(), 4);
        assert_eq!(p[hs[3]], 33);
        let owned: Vec<_> = p.into_iter().collect();
        assert_eq!(owned.iter().map(|(h, _)| *h).collect::<Vec<_>>(), hs);
        assert_eq!(
            owned.iter().map(|(_, v)| *v).collect::<Vec<_>>(),
            [0, 11, 22, 33]
        );
    }

    #[test]
    fn drain_dropped_early_equals_clear() {
        let fill = |p: &mut HandlePool<i32>| (0..4).map(|i| p.insert(i)).collect::<Vec<_>>();
        let mut a = HandlePool::new();
        let old = fill(&mut a);
        {
            let mut d = a.drain();
            assert_eq!(d.len(), 4);
            assert_eq!(d.next().map(|(_, v)| v), Some(0));
        }
        let mut b = HandlePool::new();
        fill(&mut b);
        b.clear();
        assert!(a.is_empty());
        assert!(old.iter().all(|h| !a.contains(*h)));
        a.assert_invariants();
        let from_a = fill(&mut a);
        let from_b = fill(&mut b);
        assert_eq!(from_a, from_b);
    }

    #[test]
    fn drain_yields_dense_order_and_forgotten_drain_is_safe() {
        let mut p = HandlePool::new();
        let hs: Vec<_> = (0..3).map(|i| p.insert(i)).collect();
        p.remove(hs[0]);
        let drained: Vec<_> = p.drain().map(|(_, v)| v).collect();
        assert_eq!(drained, [2, 1]);
        let mut q = HandlePool::new();
        let _ = q.insert(1);
        core::mem::forget(q.drain());
        assert!(q.is_empty());
        q.assert_invariants();
    }

    #[test]
    fn retain_visits_dense_order_and_swaps_correctly() {
        let mut p = HandlePool::new();
        let hs: Vec<_> = (0..6).map(|i| p.insert(i)).collect();
        let mut visited = Vec::new();
        p.retain(|_, v| {
            visited.push(*v);
            *v % 2 == 0
        });
        // Removing 1 swaps 5 into its place, and removing 5 swaps 4 in: each element is
        // visited exactly once, swapped-in elements immediately after the hole.
        assert_eq!(visited, [0, 1, 5, 4, 2, 3]);
        let mut kept: Vec<_> = p.values().to_vec();
        kept.sort_unstable();
        assert_eq!(kept, [0, 2, 4]);
        assert!(p.contains(hs[0]) && !p.contains(hs[1]) && p.contains(hs[4]));
        p.assert_invariants();
    }

    #[test]
    fn get_disjoint_mut_rules() {
        let mut pool = HandlePool::new();
        let first = pool.insert(1);
        let second = pool.insert(2);
        let gone = pool.insert(3);
        pool.remove(gone);
        {
            let [left, right] = pool.get_disjoint_mut([first, second]).unwrap();
            core::mem::swap(left, right);
        }
        assert_eq!((pool[first], pool[second]), (2, 1)); // mutation through both is visible
        assert!(pool.get_disjoint_mut([first, first]).is_none());
        assert!(pool.get_disjoint_mut([first, gone]).is_none());
    }

    #[test]
    fn debug_prints_as_map() {
        let mut p = HandlePool::new();
        let _ = p.insert(7_u8);
        let b = p.insert(8_u8);
        let _ = p.insert(9_u8);
        p.remove(b); // 9 swaps into dense position 1
        assert_eq!(
            alloc::format!("{p:?}"),
            "{Handle<u8>(0v1): 7, Handle<u8>(2v1): 9}"
        );
    }

    // Panics abort on WASI, so unwinding tests run on native targets only (spec §4.7).
    #[cfg(not(target_os = "wasi"))]
    #[test]
    fn retain_panic_leaves_pool_consistent() {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        let mut p = HandlePool::new();
        let hs: Vec<_> = (0..10).map(|i| p.insert(i)).collect();
        let result = catch_unwind(AssertUnwindSafe(|| {
            p.retain(|_, v| {
                assert_ne!(*v, 5, "boom");
                *v % 2 == 0
            });
        }));
        assert!(result.is_err());
        p.assert_invariants();
        for (h, v) in &p {
            assert_eq!(p[h], *v);
        }
        assert_eq!(hs.iter().filter(|h| p.contains(**h)).count(), p.len());
    }

    #[test]
    fn slot_limit_leaves_room_for_the_tag_bit_and_sentinels() {
        assert_eq!(MAX_SLOTS, 0x7FFF_FFFE);
        assert_eq!(HandlePool::<u8>::new().slots.limit(), MAX_SLOTS);
    }
}
