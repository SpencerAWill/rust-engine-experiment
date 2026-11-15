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
//! # Iteration
//!
//! [`Iter`] yields the members in ascending order, and from the back in descending order. It
//! costs O(1) per member plus O(1) per zero word.

mod array;
mod iter;
mod words;

use alloc::vec::Vec;
use core::fmt;
use core::hash::{Hash, Hasher};

pub use array::BitArray;
pub use iter::{IntoIter, Iter};

use crate::collections::error::TryReserveError;
use words::WORD_BITS;

/// The number of bits in `words` words, saturating at [`BitSet::MAX_BITS`].
///
/// A plain `words * 64` overflows `usize` on a 32-bit target.
fn bits_for_words(words: usize) -> usize {
    words.saturating_mul(WORD_BITS).min(BitSet::MAX_BITS)
}

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
    pub const MAX_BITS: usize = u32::MAX as usize;

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
    pub fn len(&self) -> usize {
        words::count(&self.words)
    }

    /// Whether the set has no members. This takes O(words) time.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|&w| w == 0)
    }

    /// The number of bits that fit without allocating: 64 per word allocated, up to `MAX_BITS`.
    #[must_use]
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
        if bits > Self::MAX_BITS {
            return Err(TryReserveError::capacity_overflow());
        }
        let needed = bits.div_ceil(WORD_BITS);
        if needed > self.words.capacity() {
            self.words
                .try_reserve_exact(needed - self.words.len())
                .map_err(TryReserveError::from_alloc)?;
        }
        Ok(())
    }

    /// Drops trailing zero words, then shrinks the allocation to fit.
    pub fn shrink_to_fit(&mut self) {
        let keep = words::trimmed(&self.words).len();
        self.words.truncate(keep);
        self.words.shrink_to_fit();
    }

    /// Removes every member and keeps the allocation.
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
    pub fn try_insert(&mut self, index: usize) -> Result<bool, TryReserveError> {
        if index >= Self::MAX_BITS {
            return Err(TryReserveError::capacity_overflow());
        }
        let (word, mask) = words::locate(index);
        if word >= self.words.len() {
            self.words
                .try_reserve(word + 1 - self.words.len())
                .map_err(TryReserveError::from_alloc)?;
            self.words.resize(word + 1, 0);
        }
        let added = self.words[word] & mask == 0;
        self.words[word] |= mask;
        Ok(added)
    }

    /// Removes `index`, returning `true` if it was a member. It never allocates.
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
    pub fn contains(&self, index: usize) -> bool {
        let (word, mask) = words::locate(index);
        self.words.get(word).is_some_and(|w| w & mask != 0)
    }

    /// The smallest member.
    #[must_use]
    pub fn first(&self) -> Option<usize> {
        words::first(&self.words)
    }

    /// The largest member.
    #[must_use]
    pub fn last(&self) -> Option<usize> {
        words::last(&self.words)
    }

    /// An iterator over the members in ascending order.
    #[must_use]
    pub fn iter(&self) -> Iter<'_> {
        Iter::new(&self.words)
    }

    /// The words in use, in the layout of the [module documentation](self).
    ///
    /// It may end in zero words, after removals or [`intersect_with`](Self::intersect_with);
    /// use [`as_trimmed_words`](Self::as_trimmed_words) for a canonical form.
    #[must_use]
    pub fn as_words(&self) -> &[u64] {
        &self.words
    }

    /// [`as_words`](Self::as_words) without its trailing zero words. Equal sets return equal
    /// slices.
    #[must_use]
    pub fn as_trimmed_words(&self) -> &[u64] {
        words::trimmed(&self.words)
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
        set.extend(iter);
        set
    }
}

impl Extend<usize> for BitSet {
    /// # Panics
    ///
    /// Panics if an index is `>= MAX_BITS`. Use [`BitSet::try_extend`] to handle that.
    #[track_caller]
    fn extend<I: IntoIterator<Item = usize>>(&mut self, iter: I) {
        for index in iter {
            if self.try_insert(index).is_err() {
                limit_exceeded("extend", format_args!("bit index {index}"), "try_extend");
            }
        }
    }
}

impl<'a> Extend<&'a usize> for BitSet {
    /// # Panics
    ///
    /// Panics if an index is `>= MAX_BITS`. Use [`BitSet::try_extend`] to handle that.
    #[track_caller]
    fn extend<I: IntoIterator<Item = &'a usize>>(&mut self, iter: I) {
        self.extend(iter.into_iter().copied());
    }
}

impl<const N: usize> From<[usize; N]> for BitSet {
    /// # Panics
    ///
    /// Panics if an index is `>= MAX_BITS`. Use [`BitSet::try_extend`] on a new set to handle
    /// that.
    #[track_caller]
    fn from(indices: [usize; N]) -> Self {
        Self::from_iter(indices)
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
    use alloc::format;
    use alloc::vec::Vec;

    use super::*;

    #[test]
    fn word_boundary_indices() {
        let mut s = BitSet::new();
        for i in [0, 63, 64, 127, 128] {
            assert!(s.insert(i));
        }
        assert_eq!(s.as_words(), [1 | 1 << 63, 1 | 1 << 63, 1]);
        assert_eq!(s.iter().collect::<Vec<_>>(), [0, 63, 64, 127, 128]);
        assert_eq!(s.iter().rev().collect::<Vec<_>>(), [128, 127, 64, 63, 0]);
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
        assert_eq!(s.iter().collect::<Vec<_>>(), [1, 2]);
    }

    #[test]
    fn exceeds_max_bits_boundary() {
        assert!(!exceeds_max_bits(u64::from(u32::MAX) - 1));
        assert!(exceeds_max_bits(u64::from(u32::MAX)));
    }

    #[test]
    fn debug_format() {
        let mut s = BitSet::new();
        assert_eq!(format!("{s:?}"), "{}");
        s.insert(130);
        s.insert(3);
        assert_eq!(format!("{s:?}"), "{3, 130}");
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
