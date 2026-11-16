//! Sets of small non-negative integers, stored one bit per index.
//!
//! [`BitSet`] lives on the heap and grows to fit any index it is given; [`BitArray`] is
//! inline, never allocates, and holds `WORDS × 64` bits. Both are our own code, with the same
//! layout, and neither is in the prelude.
//!
//! # Layout
//!
//! A word is a `u64` on every target, never a `usize`. Bit `i` is bit `i % 64`, counted from the
//! least significant bit, of word `i / 64`. The `as_words` methods expose this layout, so it is
//! part of the API: a change to it is a breaking change.
//!
//! # Hierarchical layers
//!
//! [`HierarchicalBitSet`] keeps layer 0 in the layout above and adds five layers over it, so that
//! a range with no members can be skipped. For `k` in 1 to 5, bit `b` of word `j` of layer `k` is
//! set if and only if word `j × 64 + b` of layer `k - 1` exists and is non-zero. Layer 5 is a
//! single word, `top`, of which only bits 0 to 3 are ever set, because the largest index,
//! [`HierarchicalBitSet::MAX_BITS`] `- 1` (`u32::MAX - 1`), has a layer-4 word index of 3. One
//! bit of layer 1 covers 64 indices, of layer 2 4,096, of layer 3 262,144, and so on. The layers
//! are the same on every target: nothing is sized from the pointer width.
//!
//! **Space.** Layer 0 is dense: it holds at least `ceil((highest index + 1) / 64)` words, however
//! few members there are. Each upper layer holds at most 1/64 of the layer below, plus one word.
//! A set with one member at index `i` therefore takes about `i / 8` bytes, and one at
//! `MAX_BITS - 1` about 512 MiB. A sparse variant that stores only occupied blocks would not
//! have that cost; it is not part of this module.
//!
//! # Iteration
//!
//! [`Iter`] yields the members in ascending order, and from the back in descending order. It
//! costs O(1) per member plus O(1) per zero word.

mod array;
mod descent;
mod hierarchical;
mod iter;
mod join;
mod words;

use alloc::vec::Vec;
use core::fmt;
use core::hash::{Hash, Hasher};

pub use array::BitArray;
pub use hierarchical::{HierarchicalBitSet, HierarchicalIter};
pub use iter::{IntoIter, Iter};
pub use join::Join;

use crate::collections::error::TryReserveError;
use words::{bits_for_words, words_for_bits};

/// Whether a set bit at `highest` breaks the invariant of [`BitSet`].
fn exceeds_max_bits(highest: u64) -> bool {
    highest >= BitSet::MAX_BITS as u64
}

/// A growable set of small non-negative integers, stored one bit per index on the heap.
///
/// [`insert`](Self::insert) grows the set to fit the index, and asking about an index past the
/// end answers `false` without allocating. Indices are `usize`, and valid indices are
/// `0..`[`MAX_BITS`](Self::MAX_BITS) on every target. For a set whose size is known at compile
/// time, with no heap use, see [`BitArray`].
///
/// Equality and [`Hash`] depend only on the members, never on the capacity or on how the set got
/// there. Iteration is in ascending order.
///
/// Bit `i` is bit `i % 64` of word `i / 64`; see the [module documentation](self).
///
/// # Examples
///
/// ```
/// use engine::collections::BitSet;
///
/// let mut dirty = BitSet::new();
/// dirty.insert(3);
/// dirty.insert(130); // grows to fit
/// assert!(dirty.contains(130));
/// assert!(!dirty.contains(10_000)); // past the end: not a member, no allocation
///
/// assert_eq!(dirty.iter().collect::<Vec<_>>(), [3, 130]); // always ascending
/// assert!(dirty.remove(3));
/// assert_eq!(dirty.len(), 1);
/// ```
///
/// Sets with the same members are equal however they got there:
///
/// ```
/// use engine::collections::BitSet;
///
/// let mut grown = BitSet::with_capacity(4_096);
/// grown.insert(1);
/// grown.insert(4_000);
/// grown.remove(4_000);
///
/// let small: BitSet = [1].into_iter().collect();
/// assert_eq!(grown, small); // equal, and their hashes are equal
/// ```
#[derive(Clone, Default)]
pub struct BitSet {
    words: Vec<u64>,
}

impl BitSet {
    /// The number of valid indices: the largest index is `MAX_BITS - 1`.
    ///
    /// It is `u32::MAX` on every target, so a program behaves the same on 32-bit and 64-bit
    /// targets.
    pub const MAX_BITS: usize = words::MAX_BITS;

    /// An empty set. It does not allocate.
    #[must_use]
    pub const fn new() -> Self {
        Self { words: Vec::new() }
    }

    /// An empty set with room for the indices `0..bits`, so inserting them does not allocate.
    ///
    /// # Panics
    ///
    /// Panics if `bits > MAX_BITS`, or if the allocation fails. Use
    /// [`try_with_capacity`](Self::try_with_capacity) to handle that.
    #[must_use]
    #[track_caller]
    pub fn with_capacity(bits: usize) -> Self {
        match Self::try_with_capacity(bits) {
            Ok(set) => set,
            Err(_) => limit_exceeded(
                "with_capacity",
                format_args!("a capacity of {bits} bits"),
                "try_with_capacity",
            ),
        }
    }

    /// An empty set with room for the indices `0..bits`, or an error.
    ///
    /// # Errors
    ///
    /// Returns [`TryReserveError`] if `bits > MAX_BITS` or the allocation fails.
    pub fn try_with_capacity(bits: usize) -> Result<Self, TryReserveError> {
        let mut set = Self::new();
        set.try_ensure_capacity(bits)?;
        Ok(set)
    }

    /// A set from its words, in the layout of the [module documentation](self). The words are
    /// copied.
    ///
    /// # Panics
    ///
    /// Panics if a bit at or above `MAX_BITS` is set, or if the allocation fails. Use
    /// [`try_from_words`](Self::try_from_words) to handle that.
    #[must_use]
    #[track_caller]
    pub fn from_words(words: &[u64]) -> Self {
        match Self::try_from_words(words) {
            Ok(set) => set,
            Err(_) => limit_exceeded(
                "from_words",
                format_args!("these {} words", words.len()),
                "try_from_words",
            ),
        }
    }

    /// A set from its words, or an error.
    ///
    /// # Errors
    ///
    /// Returns [`TryReserveError`] if a bit at or above `MAX_BITS` is set or the allocation
    /// fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use engine::collections::BitSet;
    ///
    /// let set = BitSet::try_from_words(&[0b101, 1])?;
    /// assert_eq!(set.iter().collect::<Vec<_>>(), [0, 2, 64]);
    /// # Ok::<(), engine::collections::TryReserveError>(())
    /// ```
    pub fn try_from_words(words: &[u64]) -> Result<Self, TryReserveError> {
        if words::highest_bit_u64(words).is_some_and(exceeds_max_bits) {
            return Err(TryReserveError::capacity_overflow());
        }
        let mut vec = Vec::new();
        vec.try_reserve_exact(words.len())
            .map_err(TryReserveError::from_alloc)?;
        vec.extend_from_slice(words);
        Ok(Self { words: vec })
    }

    /// The number of members. This takes O(words) time.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        words::count(&self.words)
    }

    /// Whether the set has no members. This takes O(words) time.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|&w| w == 0)
    }

    /// The number of bits that fit without allocating: 64 per word allocated, up to `MAX_BITS`.
    #[must_use]
    #[inline]
    pub fn capacity(&self) -> usize {
        bits_for_words(self.words.capacity())
    }

    /// Makes [`capacity`](Self::capacity) at least `bits`.
    ///
    /// It takes a total bit count, not an additional count as `Vec::reserve` does.
    ///
    /// # Panics
    ///
    /// Panics if `bits > MAX_BITS`, or if the allocation fails. Use
    /// [`try_ensure_capacity`](Self::try_ensure_capacity) to handle that.
    #[track_caller]
    pub fn ensure_capacity(&mut self, bits: usize) {
        if self.try_ensure_capacity(bits).is_err() {
            limit_exceeded(
                "ensure_capacity",
                format_args!("a capacity of {bits} bits"),
                "try_ensure_capacity",
            );
        }
    }

    /// Makes [`capacity`](Self::capacity) at least `bits`, or returns an error.
    ///
    /// # Errors
    ///
    /// Returns [`TryReserveError`] if `bits > MAX_BITS` or the allocation fails.
    pub fn try_ensure_capacity(&mut self, bits: usize) -> Result<(), TryReserveError> {
        let words = words_for_bits(bits).ok_or_else(TryReserveError::capacity_overflow)?;
        // Reserving for `len + additional` words is a no-op when the capacity already covers it.
        let additional = words.saturating_sub(self.words.len());
        self.words
            .try_reserve_exact(additional)
            .map_err(TryReserveError::from_alloc)?;
        Ok(())
    }

    /// Drops trailing zero words, then shrinks the allocation to fit.
    pub fn shrink_to_fit(&mut self) {
        let keep = words::trimmed(&self.words).len();
        self.words.truncate(keep);
        self.words.shrink_to_fit();
    }

    /// Removes every member and keeps the allocation.
    #[inline]
    pub fn clear(&mut self) {
        self.words.clear();
    }

    /// Adds `index`, returning `true` if it was not already a member. The set grows to fit.
    ///
    /// # Panics
    ///
    /// Panics if `index >= MAX_BITS`, or if growing fails. Use [`try_insert`](Self::try_insert)
    /// to handle that.
    #[track_caller]
    #[inline]
    pub fn insert(&mut self, index: usize) -> bool {
        match self.try_insert(index) {
            Ok(added) => added,
            Err(_) => limit_exceeded("insert", format_args!("bit index {index}"), "try_insert"),
        }
    }

    /// Adds `index`, returning `true` if it was not already a member, or an error.
    ///
    /// # Errors
    ///
    /// Returns [`TryReserveError`] if `index >= MAX_BITS` or growing fails; the set is
    /// unchanged.
    #[inline]
    pub fn try_insert(&mut self, index: usize) -> Result<bool, TryReserveError> {
        if index >= Self::MAX_BITS {
            return Err(TryReserveError::capacity_overflow());
        }
        let (word, mask) = words::locate(index);
        // The common case: the word exists already.
        if let Some(w) = self.words.get_mut(word) {
            let added = *w & mask == 0;
            *w |= mask;
            return Ok(added);
        }
        self.grow_to_hold(word + 1)?;
        self.words[word] |= mask;
        Ok(true)
    }

    /// Grows the storage to `needed` words (more than it has), filled with zeros.
    #[cold]
    #[inline(never)]
    fn grow_to_hold(&mut self, needed: usize) -> Result<(), TryReserveError> {
        self.words
            .try_reserve(needed - self.words.len())
            .map_err(TryReserveError::from_alloc)?;
        self.words.resize(needed, 0);
        Ok(())
    }

    /// Removes `index`, returning `true` if it was a member. It never allocates.
    #[inline]
    pub fn remove(&mut self, index: usize) -> bool {
        let (word, mask) = words::locate(index);
        match self.words.get_mut(word) {
            Some(w) => {
                let removed = *w & mask != 0;
                *w &= !mask;
                removed
            }
            None => false,
        }
    }

    /// Whether `index` is a member. An index past the end is not, and nothing is allocated.
    #[must_use]
    #[inline]
    pub fn contains(&self, index: usize) -> bool {
        let (word, mask) = words::locate(index);
        self.words.get(word).is_some_and(|w| w & mask != 0)
    }

    /// The smallest member.
    #[must_use]
    #[inline]
    pub fn first(&self) -> Option<usize> {
        words::first(&self.words)
    }

    /// The largest member.
    #[must_use]
    #[inline]
    pub fn last(&self) -> Option<usize> {
        words::last(&self.words)
    }

    /// An iterator over the members in ascending order.
    #[must_use]
    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        Iter::new(&self.words)
    }

    /// The words in use, in the layout of the [module documentation](self).
    ///
    /// It may end in zero words, after removals or [`intersect_with`](Self::intersect_with);
    /// use [`as_trimmed_words`](Self::as_trimmed_words) for a canonical form.
    #[must_use]
    #[inline]
    pub fn as_words(&self) -> &[u64] {
        &self.words
    }

    /// [`as_words`](Self::as_words) without its trailing zero words. Equal sets return equal
    /// slices.
    #[must_use]
    #[inline]
    pub fn as_trimmed_words(&self) -> &[u64] {
        words::trimmed(&self.words)
    }

    /// Adds every member of `other`. The set grows to hold `other`'s highest member.
    #[inline]
    pub fn union_with(&mut self, other: &Self) {
        let src = words::trimmed(&other.words);
        if src.len() > self.words.len() {
            self.words.resize(src.len(), 0);
        }
        words::union_into(&mut self.words, src);
    }

    /// Keeps only the members that are also in `other`. It never allocates or shrinks the
    /// storage.
    #[inline]
    pub fn intersect_with(&mut self, other: &Self) {
        words::intersect_into(&mut self.words, &other.words);
    }

    /// Removes the members that are also in `other`. It never allocates or shrinks the storage.
    #[inline]
    pub fn difference_with(&mut self, other: &Self) {
        words::difference_into(&mut self.words, &other.words);
    }

    /// Keeps the members that are in exactly one of the two sets. The set grows to hold
    /// `other`'s highest member.
    #[inline]
    pub fn symmetric_difference_with(&mut self, other: &Self) {
        let src = words::trimmed(&other.words);
        if src.len() > self.words.len() {
            self.words.resize(src.len(), 0);
        }
        words::xor_into(&mut self.words, src);
    }

    /// Whether every member is also in `other`. Capacity and trailing zero words do not matter.
    ///
    /// # Examples
    ///
    /// ```
    /// use engine::collections::BitSet;
    ///
    /// let archetype: BitSet = [0, 2, 5, 9].into_iter().collect();
    /// let query: BitSet = [2, 5].into_iter().collect();
    /// let excluded: BitSet = [7].into_iter().collect();
    ///
    /// assert!(query.is_subset(&archetype));
    /// assert!(excluded.is_disjoint(&archetype));
    ///
    /// let mut shared = archetype.clone();
    /// shared.intersect_with(&query);
    /// assert_eq!(shared, query);
    /// ```
    #[must_use]
    #[inline]
    pub fn is_subset(&self, other: &Self) -> bool {
        words::is_subset(&self.words, &other.words)
    }

    /// Whether every member of `other` is also in this set.
    #[must_use]
    #[inline]
    pub fn is_superset(&self, other: &Self) -> bool {
        other.is_subset(self)
    }

    /// Whether the two sets have no member in common.
    #[must_use]
    #[inline]
    pub fn is_disjoint(&self, other: &Self) -> bool {
        words::is_disjoint(&self.words, &other.words)
    }

    /// Adds every index from `indices`.
    ///
    /// This is the fallible twin of the [`FromIterator`], [`Extend`] and `From` implementations.
    ///
    /// # Errors
    ///
    /// Returns [`TryReserveError`] at the first index that is `>= MAX_BITS` or whose growth
    /// fails. The indices before it have been inserted.
    ///
    /// # Examples
    ///
    /// ```
    /// use engine::collections::BitSet;
    ///
    /// let mut set = BitSet::new();
    /// assert!(set.try_extend([1, 2, usize::MAX, 4]).is_err());
    /// assert_eq!(set.iter().collect::<Vec<_>>(), [1, 2]);
    /// ```
    pub fn try_extend(
        &mut self,
        indices: impl IntoIterator<Item = usize>,
    ) -> Result<(), TryReserveError> {
        for index in indices {
            self.try_insert(index)?;
        }
        Ok(())
    }
}

impl BitSet {
    /// Adds every index, or panics naming `operation`, the public item the caller used.
    #[track_caller]
    fn extend_named(&mut self, operation: &str, indices: impl IntoIterator<Item = usize>) {
        for index in indices {
            if self.try_insert(index).is_err() {
                limit_exceeded(operation, format_args!("bit index {index}"), "try_extend");
            }
        }
    }
}

#[cold]
#[track_caller]
fn limit_exceeded(operation: &str, subject: fmt::Arguments<'_>, twin: &str) -> ! {
    panic!(
        "BitSet::{operation}: {subject} is not allowed: indices and capacities are limited to \
         BitSet::MAX_BITS ({max}) and the allocation must succeed; use {twin} to handle it",
        max = BitSet::MAX_BITS
    );
}

impl PartialEq for BitSet {
    fn eq(&self, other: &Self) -> bool {
        self.as_trimmed_words() == other.as_trimmed_words()
    }
}

impl Eq for BitSet {}

impl Hash for BitSet {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_trimmed_words().hash(state);
    }
}

impl fmt::Debug for BitSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

impl FromIterator<usize> for BitSet {
    /// # Panics
    ///
    /// Panics if an index is `>= MAX_BITS`. Use [`BitSet::try_extend`] on a new set to handle
    /// that.
    #[track_caller]
    fn from_iter<I: IntoIterator<Item = usize>>(iter: I) -> Self {
        let mut set = Self::new();
        set.extend_named("from_iter", iter);
        set
    }
}

impl Extend<usize> for BitSet {
    /// # Panics
    ///
    /// Panics if an index is `>= MAX_BITS`. Use [`BitSet::try_extend`] to handle that.
    #[track_caller]
    fn extend<I: IntoIterator<Item = usize>>(&mut self, iter: I) {
        self.extend_named("extend", iter);
    }
}

impl<'a> Extend<&'a usize> for BitSet {
    /// # Panics
    ///
    /// Panics if an index is `>= MAX_BITS`. Use [`BitSet::try_extend`] to handle that.
    #[track_caller]
    fn extend<I: IntoIterator<Item = &'a usize>>(&mut self, iter: I) {
        self.extend_named("extend", iter.into_iter().copied());
    }
}

impl<const N: usize> From<[usize; N]> for BitSet {
    /// # Panics
    ///
    /// Panics if an index is `>= MAX_BITS`. Use [`BitSet::try_extend`] on a new set to handle
    /// that.
    #[track_caller]
    fn from(indices: [usize; N]) -> Self {
        let mut set = Self::new();
        set.extend_named("from", indices);
        set
    }
}

impl IntoIterator for BitSet {
    type Item = usize;
    type IntoIter = IntoIter;

    fn into_iter(self) -> IntoIter {
        IntoIter::new(self.words)
    }
}

impl<'a> IntoIterator for &'a BitSet {
    type Item = usize;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::collections::tests::{bounded, debug_bounded};

    #[test]
    fn word_boundary_indices() {
        let mut s = BitSet::new();
        for i in [0, 63, 64, 127, 128] {
            assert!(s.insert(i));
        }
        assert_eq!(s.as_words(), [1 | 1 << 63, 1 | 1 << 63, 1]);
        assert_eq!(bounded(s.iter(), 6), [0, 63, 64, 127, 128]);
        assert_eq!(bounded(s.iter().rev(), 6), [128, 127, 64, 63, 0]);
        assert_eq!((s.first(), s.last()), (Some(0), Some(128)));
    }

    #[test]
    fn bits_for_words_saturates_at_max_bits() {
        assert_eq!(bits_for_words(3), 192);
        assert_eq!(bits_for_words(usize::MAX / 8), BitSet::MAX_BITS);
    }

    #[test]
    fn try_insert_at_the_limit_is_an_error() {
        for index in [usize::MAX, BitSet::MAX_BITS] {
            let mut s = BitSet::new();
            assert!(s.try_insert(index).is_err());
            assert_eq!(s.len(), 0);
            assert_eq!(s.as_words(), []);
        }
    }

    #[test]
    fn max_bits_is_u32_max() {
        assert_eq!(BitSet::MAX_BITS as u64, u64::from(u32::MAX));
    }

    #[test]
    fn contains_past_the_end_is_false() {
        let s: BitSet = [3].into_iter().collect();
        assert!(!s.contains(10_000));
        assert!(!s.contains(usize::MAX));
        assert_eq!(s.as_words().len(), 1);
    }

    #[test]
    fn try_extend_stops_at_the_failing_index() {
        let mut s = BitSet::new();
        assert!(s.try_extend([1, 2, usize::MAX, 4]).is_err());
        assert_eq!(bounded(s.iter(), 3), [1, 2]);
    }

    #[test]
    fn exceeds_max_bits_boundary() {
        assert!(!exceeds_max_bits(u64::from(u32::MAX) - 1));
        assert!(exceeds_max_bits(u64::from(u32::MAX)));
    }

    #[test]
    fn debug_format() {
        let mut s = BitSet::new();
        assert_eq!(debug_bounded(&s, 64), "{}");
        s.insert(130);
        s.insert(3);
        assert_eq!(debug_bounded(&s, 64), "{3, 130}");
    }

    #[test]
    fn intersect_with_shorter_clears_tail() {
        let mut a: BitSet = [1, 200].into_iter().collect();
        let b: BitSet = [1].into_iter().collect();
        a.intersect_with(&b);
        assert_eq!(bounded(a.iter(), 2), [1]);
        assert_eq!(a.as_words().len(), 4);
    }

    #[test]
    fn difference_with_shorter_keeps_tail() {
        let mut a: BitSet = [1, 200].into_iter().collect();
        let b: BitSet = [1].into_iter().collect();
        a.difference_with(&b);
        assert_eq!(bounded(a.iter(), 2), [200]);
    }

    #[test]
    fn subset_ignores_trailing_zeros() {
        let mut roomy = BitSet::with_capacity(4_096);
        roomy.insert(1);
        roomy.insert(4_000);
        roomy.remove(4_000);
        let small: BitSet = [1].into_iter().collect();
        assert!(roomy.is_subset(&small));
        assert!(small.is_subset(&roomy));
        assert!(roomy.is_superset(&small));
        assert!(!roomy.is_disjoint(&small));
    }

    #[test]
    fn words_for_bits_accepts_max_bits_and_rejects_one_more() {
        // No allocation, so this runs on every target and in any memory limit.
        assert_eq!(words_for_bits(0), Some(0));
        assert_eq!(words_for_bits(1), Some(1));
        assert_eq!(words_for_bits(64), Some(1));
        assert_eq!(words_for_bits(65), Some(2));
        assert_eq!(words_for_bits(BitSet::MAX_BITS), Some(1 << 26));
        // One past the limit does not fit in a `usize` on a 32-bit target.
        if let Some(over) = BitSet::MAX_BITS.checked_add(1) {
            assert_eq!(words_for_bits(over), None);
            assert!(BitSet::try_with_capacity(over).is_err()); // refused before allocating
        }
    }

    #[test]
    fn hash_depends_on_the_members() {
        use core::hash::{Hash, Hasher};

        /// FNV-1a, so the test needs no hasher from outside the crate.
        struct Fnv(u64);
        impl Hasher for Fnv {
            fn finish(&self) -> u64 {
                self.0
            }
            fn write(&mut self, bytes: &[u8]) {
                for &b in bytes {
                    self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
                }
            }
        }
        fn hash(set: &BitSet) -> u64 {
            let mut hasher = Fnv(0xcbf2_9ce4_8422_2325);
            set.hash(&mut hasher);
            hasher.finish()
        }

        let sets = [
            BitSet::new(),
            BitSet::from([0]),
            BitSet::from([1]),
            BitSet::from([64]),
            BitSet::from([0, 64]),
        ];
        for (i, a) in sets.iter().enumerate() {
            for b in &sets[i + 1..] {
                assert_ne!(hash(a), hash(b), "{a:?} and {b:?}");
            }
        }
    }

    #[test]
    fn extend_from_references_and_from_array_add_the_indices() {
        let mut s = BitSet::new();
        s.extend(&[1_usize, 70]);
        assert_eq!(bounded(s.iter(), 3), [1, 70]);
        s.extend([2_usize, 200]);
        assert_eq!(bounded(s.iter(), 5), [1, 2, 70, 200]);
        let t = BitSet::from([5, 130]);
        assert_eq!(bounded(t.iter(), 3), [5, 130]);
    }

    #[test]
    fn shrink_to_fit_drops_trailing_zero_words() {
        let mut s = BitSet::with_capacity(4_096);
        s.insert(1);
        s.insert(200);
        s.remove(200);
        assert_eq!(s.as_words().len(), 4);
        s.shrink_to_fit();
        assert_eq!(s.as_words().len(), 1);
        assert_eq!(s.capacity(), 64);
    }
}
