//! [`HandleAllocator`]: handle allocation without value storage or a live-handle list.

use alloc::collections::TryReserveError;
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
    /// [`Vec::reserve`]. Use [`try_reserve`](Self::try_reserve) to handle that without panicking.
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

    /// Allocates exactly `n` handles at once, appending them to `out`.
    ///
    /// The handles are exactly the ones `n` calls to [`allocate`](Self::allocate) would issue, in
    /// the same order, but a batch is several times faster than a loop of single allocations.
    ///
    /// # Panics
    ///
    /// Panics if the allocator fills up before `n` handles are allocated (2³¹−2 slots in use or
    /// retired). The handles allocated before that stay allocated and in `out`. Use
    /// [`try_allocate_many`](Self::try_allocate_many) to handle that without panicking.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandleAllocator;
    /// let mut alloc = HandleAllocator::<u8>::new();
    /// let mut handles = Vec::new();
    /// alloc.allocate_many(100, &mut handles);
    /// assert_eq!(handles.len(), 100);
    /// ```
    #[track_caller]
    pub fn allocate_many(&mut self, n: usize, out: &mut alloc::vec::Vec<Handle<T>>) {
        let got = self.try_allocate_many(n, out);
        assert!(
            got == n,
            "HandleAllocator::allocate_many: allocator is full (allocated {got} of {n}); \
             use try_allocate_many to handle this without panicking"
        );
    }

    /// Allocates up to `n` handles at once, appending them to `out`, and returns how many it
    /// allocated.
    ///
    /// The handles are exactly the ones `n` calls to [`allocate`](Self::allocate) would issue, in
    /// the same order. It allocates fewer than `n` only when the allocator fills up.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::HandleAllocator;
    /// let mut alloc = HandleAllocator::<u8>::new();
    /// let mut handles = Vec::new();
    /// assert_eq!(alloc.try_allocate_many(3, &mut handles), 3);
    /// assert_eq!(handles.len(), 3);
    /// assert!(handles.iter().all(|h| alloc.is_alive(*h)));
    /// ```
    pub fn try_allocate_many(&mut self, n: usize, out: &mut alloc::vec::Vec<Handle<T>>) -> usize {
        out.reserve(n.min(self.slots.limit() as usize));
        let got = self.slots.acquire_many(n, OCCUPIED, |index, generation| {
            out.push(Handle::new(index, generation));
        });
        self.live += got;
        got
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
    pub fn len(&self) -> usize {
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
    pub fn is_empty(&self) -> bool {
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
            let mut out = alloc::vec::Vec::new();
            assert_eq!(batch.try_allocate_many(n, &mut out), n);
            let expected: alloc::vec::Vec<_> = (0..n).map(|_| looped.allocate()).collect();
            assert_eq!(out, expected, "n = {n}");
            assert_eq!(batch.len(), looped.len());
            batch.assert_invariants();
            // And the two continue identically afterwards.
            assert_eq!(batch.allocate(), looped.allocate());
        }
    }

    #[test]
    fn allocate_many_appends_exactly_n_handles() {
        let mut a = HandleAllocator::<u8>::new();
        let mut out = alloc::vec::Vec::new();
        a.allocate_many(4, &mut out);
        a.allocate_many(2, &mut out);
        assert_eq!(out.len(), 6);
        assert_eq!(a.len(), 6);
        assert!(out.iter().all(|h| a.is_alive(*h)));
    }

    #[test]
    #[should_panic(expected = "try_allocate_many")]
    fn allocate_many_past_the_limit_panics_with_hint() {
        let mut a = HandleAllocator::<u8>::with_slot_limit(2);
        a.allocate_many(3, &mut alloc::vec::Vec::new());
    }

    #[test]
    fn allocate_many_stops_at_the_slot_limit_and_reports_how_many() {
        let mut a = HandleAllocator::<u8>::with_slot_limit(5);
        let mut out = alloc::vec::Vec::new();
        assert_eq!(a.try_allocate_many(8, &mut out), 5);
        assert_eq!(out.len(), 5);
        assert_eq!(a.len(), 5);
        assert_eq!(a.try_allocate_many(1, &mut out), 0);
        a.assert_invariants();
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
