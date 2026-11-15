//! `BitArray`: a set of small indices stored inline.

use core::fmt;

use super::iter::Iter;
use super::words::{self, WORD_BITS};
use crate::collections::error::BitIndexError;

/// A set of the indices `0..WORDS * 64`, stored inline in `WORDS` words, with no heap use.
///
/// The size is a number of words because stable Rust cannot yet size an array from a bit count.
/// `BitArray<1>` holds bits 0 to 63 and `BitArray<2>` holds 0 to 127. `BitArray<0>` is allowed
/// and holds nothing. Inserting past the end is a bug, so [`insert`](Self::insert) panics, and
/// [`try_insert`](Self::try_insert) is the fallible version. For a set that grows, use
/// [`BitSet`](super::BitSet).
///
/// Bit `i` is bit `i % 64` of word `i / 64`; see the [module documentation](super).
///
/// # Examples
///
/// ```
/// use engine::collections::BitArray;
///
/// let mut free_bins = BitArray::<1>::new();
/// free_bins.insert(5);
/// free_bins.insert(17);
/// assert_eq!(free_bins.first(), Some(5)); // lowest free bin
/// assert!(free_bins.try_insert(64).is_err()); // BitArray<1> holds 0..64
/// assert_eq!(BitArray::<1>::CAPACITY, 64);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct BitArray<const WORDS: usize> {
    words: [u64; WORDS],
}

impl<const WORDS: usize> BitArray<WORDS> {
    /// The number of bits the set holds: valid indices are `0..CAPACITY`.
    pub const CAPACITY: usize = WORDS * WORD_BITS;

    /// An empty set.
    #[must_use]
    pub const fn new() -> Self {
        Self { words: [0; WORDS] }
    }

    /// A set from its words, in the layout of the [module documentation](super).
    ///
    /// # Examples
    ///
    /// ```
    /// use engine::collections::BitArray;
    ///
    /// let bits = BitArray::from_words([0b101, 1]);
    /// assert_eq!(bits.iter().collect::<Vec<_>>(), [0, 2, 64]);
    /// ```
    #[must_use]
    pub const fn from_words(words: [u64; WORDS]) -> Self {
        Self { words }
    }

    /// The words of the set.
    #[must_use]
    pub const fn as_words(&self) -> &[u64; WORDS] {
        &self.words
    }

    /// The number of members. This takes O(`WORDS`) time.
    #[must_use]
    pub fn len(&self) -> usize {
        words::count(&self.words)
    }

    /// Whether the set has no members.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|&w| w == 0)
    }

    /// Removes every member.
    pub fn clear(&mut self) {
        self.words = [0; WORDS];
    }

    /// Adds `index`, returning `true` if it was not already a member.
    ///
    /// # Panics
    ///
    /// Panics if `index >= CAPACITY`. Use [`try_insert`](Self::try_insert) to handle that.
    ///
    /// # Examples
    ///
    /// ```
    /// use engine::collections::BitArray;
    ///
    /// let mut bits = BitArray::<1>::new();
    /// assert!(bits.insert(3));
    /// assert!(!bits.insert(3));
    /// ```
    #[track_caller]
    pub fn insert(&mut self, index: usize) -> bool {
        match self.try_insert(index) {
            Ok(added) => added,
            Err(_) => out_of_range("insert", index, Self::CAPACITY, WORDS),
        }
    }

    /// Adds `index`, returning `true` if it was not already a member, or an error if
    /// `index >= CAPACITY`.
    ///
    /// # Errors
    ///
    /// Returns [`BitIndexError`] if `index >= CAPACITY`; the set is unchanged.
    pub fn try_insert(&mut self, index: usize) -> Result<bool, BitIndexError> {
        if index >= Self::CAPACITY {
            return Err(BitIndexError::new(index, Self::CAPACITY));
        }
        let (word, mask) = words::locate(index);
        let added = self.words[word] & mask == 0;
        self.words[word] |= mask;
        Ok(added)
    }

    /// Removes `index`, returning `true` if it was a member.
    ///
    /// # Panics
    ///
    /// Panics if `index >= CAPACITY`. Use [`try_remove`](Self::try_remove) to handle that, or
    /// [`contains`](Self::contains), which never panics.
    #[track_caller]
    pub fn remove(&mut self, index: usize) -> bool {
        match self.try_remove(index) {
            Ok(removed) => removed,
            Err(_) => out_of_range("remove", index, Self::CAPACITY, WORDS),
        }
    }

    /// Removes `index`, returning `true` if it was a member, or an error if
    /// `index >= CAPACITY`.
    ///
    /// # Errors
    ///
    /// Returns [`BitIndexError`] if `index >= CAPACITY`; the set is unchanged.
    pub fn try_remove(&mut self, index: usize) -> Result<bool, BitIndexError> {
        if index >= Self::CAPACITY {
            return Err(BitIndexError::new(index, Self::CAPACITY));
        }
        let (word, mask) = words::locate(index);
        let removed = self.words[word] & mask != 0;
        self.words[word] &= !mask;
        Ok(removed)
    }

    /// Whether `index` is a member. An index at or above `CAPACITY` is not.
    #[must_use]
    pub const fn contains(&self, index: usize) -> bool {
        if index >= Self::CAPACITY {
            return false;
        }
        let (word, mask) = words::locate(index);
        self.words[word] & mask != 0
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
}

#[cold]
#[track_caller]
fn out_of_range(operation: &str, index: usize, capacity: usize, words: usize) -> ! {
    match capacity.checked_sub(1) {
        Some(last) => panic!(
            "BitArray::{operation}: bit index {index} is out of range for BitArray<{words}>, \
             which holds bits 0 to {last}; use a larger WORDS, or BitSet if the size is not \
             known at compile time"
        ),
        None => panic!(
            "BitArray::{operation}: bit index {index} is out of range for BitArray<{words}>, \
             which holds no bits; use a larger WORDS, or BitSet if the size is not known at \
             compile time"
        ),
    }
}

impl<const WORDS: usize> Default for BitArray<WORDS> {
    /// An empty set. Written by hand because `[u64; N]: Default` holds only for `N <= 32`.
    fn default() -> Self {
        Self::new()
    }
}

impl<const WORDS: usize> fmt::Debug for BitArray<WORDS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

impl<'a, const WORDS: usize> IntoIterator for &'a BitArray<WORDS> {
    type Item = usize;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use alloc::format;
    use alloc::string::ToString;
    use alloc::vec::Vec;

    use super::*;

    #[test]
    fn word_boundary_indices() {
        let mut a = BitArray::<3>::new();
        for i in [0, 63, 64, 127, 128] {
            assert!(a.insert(i));
        }
        assert_eq!(a.as_words(), &[1 | 1 << 63, 1 | 1 << 63, 1]);
        assert_eq!(a.iter().collect::<Vec<_>>(), [0, 63, 64, 127, 128]);
        assert_eq!(a.iter().rev().collect::<Vec<_>>(), [128, 127, 64, 63, 0]);
        assert_eq!((a.first(), a.last()), (Some(0), Some(128)));
        a.remove(0);
        a.remove(128);
        assert_eq!((a.first(), a.last()), (Some(63), Some(127)));
    }

    fn alternate<const W: usize>(members: &[usize]) {
        let mut a = BitArray::<W>::new();
        for &m in members {
            a.insert(m);
        }
        let mut iter = a.iter();
        let mut seen = Vec::new();
        for turn in 0.. {
            let item = if turn % 2 == 0 {
                iter.next()
            } else {
                iter.next_back()
            };
            match item {
                Some(i) => seen.push(i),
                None => break,
            }
        }
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next_back(), None);
        seen.sort_unstable();
        assert_eq!(seen, members);
    }

    #[test]
    fn double_ended_meets_once() {
        alternate::<1>(&[1, 5, 9, 63]);
        alternate::<3>(&[1, 70, 140]);
        alternate::<3>(&[64, 65, 66, 67]);
        alternate::<2>(&[]);
        alternate::<0>(&[]);
    }

    #[test]
    fn debug_format() {
        let mut a = BitArray::<2>::new();
        assert_eq!(format!("{a:?}"), "{}");
        a.insert(3);
        a.insert(64);
        assert_eq!(format!("{a:?}"), "{3, 64}");
    }

    #[test]
    fn default_is_empty_for_large_words() {
        assert!(BitArray::<40>::default().is_empty());
    }

    #[test]
    fn bit_index_error_display() {
        let mut a = BitArray::<1>::new();
        assert_eq!(
            a.try_insert(70).unwrap_err().to_string(),
            "bit index 70 is out of range: this set holds bits 0 to 63"
        );
        assert_eq!(
            BitArray::<0>::new().try_remove(0).unwrap_err().to_string(),
            "bit index 0 is out of range: this set holds no bits"
        );
    }

    #[test]
    fn contains_is_const() {
        const A: BitArray<1> = BitArray::from_words([1 << 5]);
        const { assert!(A.contains(5)) };
        assert!(!A.contains(64));
    }
}
