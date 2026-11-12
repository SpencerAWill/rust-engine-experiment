//! [`HandleAllocator`]: handle allocation without value storage or a live-handle list.

use alloc::collections::TryReserveError;
use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;

use crate::handle::{Handle, write_short_type_name};
use crate::slots::{SlotTable, VACANT_BIT};

/// Lean handle allocation: it hands out [`Handle`]s and tracks which are alive, and nothing more.
///
/// This is the shape of an ECS entity allocator whose data lives elsewhere. It keeps one 8-byte
/// slot per slot ever used and a free list, so [`allocate`](Self::allocate),
/// [`free`](Self::free) and [`is_alive`](Self::is_alive) touch a single array. It does **not**
/// list its live handles; use [`DenseHandleAllocator`](crate::DenseHandleAllocator) when you need
/// to iterate them (at the cost of a second array maintained on every allocate and free).
///
/// Handles follow the same rules as [`HandlePool`](crate::HandlePool) and
/// [`DenseHandleAllocator`](crate::DenseHandleAllocator), and for the same sequence of
/// `allocate`, `free` and `clear` calls all three issue identical handles: new slots start at
/// generation 1; freeing bumps the generation (or retires the slot on exhaustion); freed slots
/// are reused last-freed-first; [`clear`](Self::clear) returns slots in ascending index order.
///
/// # Limitation
///
/// A handle used on a *different* allocator than the one that issued it can look alive; see the
/// [crate docs](crate#known-limitation).
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
    slots: SlotTable,
    live: usize,
    _marker: PhantomData<fn() -> T>,
}

/// The link stored in an occupied slot. The lean allocator keeps no per-handle data, and any
/// value below [`VACANT_BIT`] means "occupied".
const OCCUPIED: u32 = 0;

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
            slots: SlotTable::new(),
            live: 0,
            _marker: PhantomData,
        }
    }

    /// Creates an empty allocator with room for `capacity` slots before reallocating.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandleAllocator;
    /// let _ = HandleAllocator::<u8>::with_capacity(64);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: SlotTable::with_capacity(capacity),
            live: 0,
            _marker: PhantomData,
        }
    }

    /// Reserves room for at least `additional` more live handles without reallocating.
    ///
    /// The room is counted from the live handles now: after `reserve(n)`, the next `n`
    /// allocations do not reallocate.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity overflows `usize`, or if the allocator fails, exactly like
    /// `Vec::reserve`. Use [`try_reserve`](Self::try_reserve) to handle that without panicking.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandleAllocator;
    /// let mut x = HandleAllocator::<u8>::new();
    /// x.reserve(1000);
    /// ```
    #[track_caller]
    pub fn reserve(&mut self, additional: usize) {
        if let Err(error) = self.try_reserve(additional) {
            panic!(
                "HandleAllocator::reserve: cannot reserve room for {additional} more handles ({error}); \
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
    /// # use engine_handle::HandleAllocator;
    /// let mut x = HandleAllocator::<u8>::new();
    /// assert!(x.try_reserve(16).is_ok());
    /// assert!(x.try_reserve(usize::MAX).is_err());
    /// ```
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.slots
            .try_reserve_slots(self.live.saturating_add(additional))
    }

    /// An allocator that holds at most `limit` slots, for testing the full path.
    #[cfg(test)]
    pub(crate) fn with_slot_limit(limit: u32) -> Self {
        Self {
            slots: SlotTable::with_limit(limit),
            ..Self::new()
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
    #[must_use = "dropping the handle leaks a live slot until clear()"]
    #[track_caller]
    pub fn allocate(&mut self) -> Handle<T> {
        let Some(handle) = self.try_allocate() else {
            panic!(
                "HandleAllocator::allocate: allocator is full ({} slots in use or retired); \
                 use try_allocate to handle this without panicking",
                self.slots.limit()
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
    #[inline]
    pub fn try_allocate(&mut self) -> Option<Handle<T>> {
        let (index, generation) = self.slots.acquire(OCCUPIED)?;
        self.live += 1;
        Some(Handle::new(index, generation))
    }

    /// Allocates `n` handles at once and returns them.
    ///
    /// The handles are exactly the ones `n` calls to [`allocate`](Self::allocate) would issue, in
    /// the same order. A batch also reserves all the room it needs up front, which makes it about
    /// 2.5x faster than a loop when it has to create new slots, and no faster when it only reuses
    /// freed ones (see `docs/benchmarks.md`).
    ///
    /// # Panics
    ///
    /// Panics if fewer than `n` slots are available (the limit is 2³¹−2 slots in use or
    /// retired); nothing is allocated in that case. Use
    /// [`try_allocate_many`](Self::try_allocate_many) to handle that without panicking.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandleAllocator;
    /// let mut alloc = HandleAllocator::<u8>::new();
    /// let handles = alloc.allocate_many(100);
    /// assert_eq!(handles.len(), 100);
    /// assert!(handles.iter().all(|h| alloc.is_alive(*h)));
    /// ```
    #[must_use = "dropping the handles leaks live slots until clear()"]
    #[track_caller]
    pub fn allocate_many(&mut self, n: usize) -> Vec<Handle<T>> {
        let Some(handles) = self.try_allocate_many(n) else {
            panic!(
                "HandleAllocator::allocate_many: cannot allocate {n} handles; use \
                 try_allocate_many to handle this without panicking"
            )
        };
        handles
    }

    /// Fallible twin of [`allocate_many`](Self::allocate_many): `None`, with nothing allocated,
    /// if fewer than `n` slots are available.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandleAllocator;
    /// let mut alloc = HandleAllocator::<u8>::new();
    /// assert_eq!(alloc.try_allocate_many(3).map(|hs| hs.len()), Some(3));
    /// ```
    pub fn try_allocate_many(&mut self, n: usize) -> Option<Vec<Handle<T>>> {
        if n > self.slots.obtainable(self.live) {
            return None;
        }
        let mut handles = Vec::with_capacity(n);
        self.slots
            .acquire_exactly(n, OCCUPIED, |index, generation| {
                handles.push(Handle::new(index, generation));
            });
        self.live += n;
        Some(handles)
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
    #[inline]
    pub fn free(&mut self, handle: Handle<T>) -> bool {
        if self.is_alive(handle) {
            self.slots.release(handle.index());
            self.live -= 1;
            true
        } else {
            false
        }
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
        // Vacant and retired links have the top bit set; a matching generation alone is not
        // enough, because a vacant slot stores the generation its next occupant will get.
        self.slots
            .lookup(handle.index(), handle.generation())
            .is_some_and(|link| link & VACANT_BIT == 0)
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
    pub const fn len(&self) -> usize {
        self.live
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
    pub const fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Frees every handle. Slots are then reused in ascending index order.
    ///
    /// This walks every slot ever used, so it is `O(slots)` and not `O(len)`.
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
        self.slots.release_all();
        self.live = 0;
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
        let occupied = self.slots.assert_invariants();
        assert_eq!(occupied, self.live, "occupied slots != live count");
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
            slots: self.slots.clone(),
            live: self.live,
            _marker: PhantomData,
        }
    }
}

impl<T> fmt::Debug for HandleAllocator<T> {
    /// Prints `HandleAllocator<T> { live: n }`. The allocator does not list its handles.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HandleAllocator<")?;
        write_short_type_name::<T>(f)?;
        write!(f, "> {{ live: {} }}", self.live)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds an allocator with some live handles and a free list in a known order.
    fn churned() -> HandleAllocator<u8> {
        let mut a = HandleAllocator::<u8>::new();
        let hs: alloc::vec::Vec<_> = (0..10).map(|_| a.allocate()).collect();
        for i in [3, 7, 1, 8] {
            a.free(hs[i]);
        }
        a
    }

    #[test]
    fn allocate_many_issues_the_same_handles_as_repeated_allocate() {
        for n in [0, 1, 3, 4, 5, 50] {
            let mut batch = churned();
            let mut looped = churned();
            let out = batch.try_allocate_many(n).unwrap();
            let expected: alloc::vec::Vec<_> = (0..n).map(|_| looped.allocate()).collect();
            assert_eq!(out, expected, "n = {n}");
            assert_eq!(batch.len(), looped.len());
            batch.assert_invariants();
            // And the two continue identically afterwards.
            assert_eq!(batch.allocate(), looped.allocate());
        }
    }

    #[test]
    fn allocate_many_returns_n_live_handles() {
        let mut a = HandleAllocator::<u8>::new();
        let first = a.allocate_many(4);
        let second = a.allocate_many(2);
        assert_eq!((first.len(), second.len(), a.len()), (4, 2, 6));
        assert!(first.iter().chain(&second).all(|h| a.is_alive(*h)));
    }

    #[test]
    fn try_allocate_many_is_all_or_nothing() {
        let mut a = HandleAllocator::<u8>::with_slot_limit(5);
        let kept = a.allocate();
        assert!(a.try_allocate_many(5).is_none()); // only 4 left: nothing is allocated
        assert_eq!(a.len(), 1);
        a.assert_invariants();
        let rest = a.try_allocate_many(4).unwrap();
        assert_eq!(rest.len(), 4);
        assert!(a.try_allocate_many(1).is_none());
        assert!(a.is_alive(kept));
        a.assert_invariants();
    }

    #[test]
    fn try_allocate_many_never_counts_retired_slots() {
        let mut a = HandleAllocator::<u8>::with_slot_limit(3);
        let h = a.allocate();
        a.slots
            .set_generation(h.index(), core::num::NonZeroU32::MAX);
        let last = Handle::<u8>::new(h.index(), core::num::NonZeroU32::MAX);
        assert!(a.free(last)); // retires the slot: 2 usable slots remain
        assert!(a.try_allocate_many(3).is_none());
        assert_eq!(a.try_allocate_many(2).unwrap().len(), 2);
        a.assert_invariants();
    }

    #[test]
    #[should_panic(expected = "try_allocate_many")]
    fn allocate_many_past_the_limit_panics_with_hint() {
        let mut a = HandleAllocator::<u8>::with_slot_limit(2);
        let _ = a.allocate_many(3);
    }

    #[test]
    fn reserve_then_allocate_does_not_reallocate() {
        let mut a = HandleAllocator::<u8>::new();
        a.reserve(100);
        let capacity = a.slots.capacity();
        assert!(capacity >= 100);
        for _ in 0..100 {
            let _ = a.allocate();
        }
        assert_eq!(a.slots.capacity(), capacity);
    }

    #[test]
    fn reserve_counts_live_handles() {
        let mut a = HandleAllocator::<u8>::new();
        for _ in 0..50 {
            let _ = a.allocate();
        }
        a.reserve(50);
        assert!(a.slots.capacity() >= 100);
        assert!(a.try_reserve(10).is_ok());
    }

    #[test]
    #[should_panic(expected = "try_reserve")]
    fn reserve_panics_on_capacity_overflow() {
        HandleAllocator::<u8>::new().reserve(usize::MAX);
    }

    #[test]
    fn try_reserve_reports_overflow_instead_of_panicking() {
        assert!(
            HandleAllocator::<u8>::new()
                .try_reserve(usize::MAX)
                .is_err()
        );
    }

    #[test]
    #[allow(
        clippy::len_zero,
        reason = "the point is to call `len` in a const context"
    )]
    fn length_queries_are_const() {
        const _: () = {
            let a = HandleAllocator::<u8>::new();
            assert!(a.is_empty());
            assert!(a.len() == 0);
            core::mem::forget(a); // a const context cannot run `Drop`
        };
    }

    #[test]
    fn free_returns_false_for_stale_handle() {
        let mut a = HandleAllocator::<u8>::new();
        let h = a.allocate();
        assert!(a.free(h));
        assert!(!a.free(h));
        assert!(!a.is_alive(h));
        a.assert_invariants();
    }

    #[test]
    fn a_forged_handle_to_a_vacant_slot_is_not_alive() {
        let mut a = HandleAllocator::<u8>::new();
        let h = a.allocate();
        assert!(a.free(h));
        // A vacant slot stores the generation its next occupant will get; forge exactly that.
        let forged = Handle::<u8>::from_bits(h.to_bits() + 1).unwrap();
        assert!(!a.is_alive(forged));
        assert!(!a.free(forged));
        assert_eq!(a.len(), 0);
        a.assert_invariants();
    }

    #[test]
    fn reuse_is_lifo_with_the_next_generation() {
        let mut a = HandleAllocator::<u8>::new();
        let first = a.allocate();
        let second = a.allocate();
        a.free(first);
        a.free(second);
        let third = a.allocate();
        assert_eq!(third.index(), second.index());
        assert_eq!(third.generation().get(), second.generation().get() + 1);
        assert_eq!(a.allocate().index(), first.index());
        a.assert_invariants();
    }

    #[test]
    fn try_allocate_returns_none_at_the_slot_limit() {
        let mut a = HandleAllocator::<u8>::with_slot_limit(2);
        assert!(a.try_allocate().is_some());
        assert!(a.try_allocate().is_some());
        assert!(a.try_allocate().is_none());
        assert_eq!(a.len(), 2);
    }

    #[test]
    #[should_panic(expected = "try_allocate")]
    fn allocate_past_limit_panics_with_hint() {
        let mut a = HandleAllocator::<u8>::with_slot_limit(1);
        let _ = a.allocate();
        let _ = a.allocate();
    }

    #[test]
    fn exhausted_generation_retires_the_slot() {
        let mut a = HandleAllocator::<u8>::with_slot_limit(1);
        let h = a.allocate();
        a.slots
            .set_generation(h.index(), core::num::NonZeroU32::MAX);
        let last = Handle::<u8>::new(h.index(), core::num::NonZeroU32::MAX);
        assert!(a.free(last));
        assert!(!a.is_alive(last));
        // The only slot is retired, so the allocator is now full.
        assert!(a.try_allocate().is_none());
        a.assert_invariants();
    }

    #[test]
    fn clear_makes_handles_stale_and_reuses_in_ascending_order() {
        let mut a = HandleAllocator::<u8>::new();
        let hs: alloc::vec::Vec<_> = (0..4).map(|_| a.allocate()).collect();
        a.free(hs[2]);
        a.clear();
        assert!(a.is_empty() && hs.iter().all(|h| !a.is_alive(*h)));
        let again: alloc::vec::Vec<_> = (0..4).map(|_| a.allocate().index()).collect();
        assert_eq!(again, [0, 1, 2, 3]);
        a.assert_invariants();
    }

    #[test]
    fn debug_shows_the_live_count() {
        let mut a = HandleAllocator::<u8>::new();
        let _ = a.allocate();
        assert_eq!(alloc::format!("{a:?}"), "HandleAllocator<u8> { live: 1 }");
    }

    #[test]
    fn slot_is_eight_bytes_per_slot_ever_used() {
        assert_eq!(size_of::<crate::slots::Slot>(), 8);
    }
}
