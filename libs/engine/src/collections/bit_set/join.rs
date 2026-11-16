//! `Join`: the indices in every one of several hierarchical bitsets and in none of others.

use core::fmt;
use core::iter::FusedIterator;

use super::HierarchicalBitSet;
use super::descent::{LAYERS, WordCursor, WordSource};
use super::words::WORD_BITS;
use crate::collections::error::EmptyJoinError;

/// The sets a join reads, seen as one set of words.
#[derive(Clone, Copy)]
pub(super) struct JoinSource<'a> {
    pub(super) required: &'a [&'a HierarchicalBitSet],
    pub(super) excluded: &'a [&'a HierarchicalBitSet],
}

impl WordSource for JoinSource<'_> {
    #[inline]
    fn word(&self, layer: usize, index: usize) -> u64 {
        let mut word = !0_u64;
        for set in self.required {
            word &= set.word(layer, index);
            if word == 0 {
                return 0;
            }
        }
        // An upper bit says only that something below is set, so excluded sets apply to
        // layer 0 alone.
        if layer == 0 {
            for set in self.excluded {
                word &= !set.word(0, index);
            }
        }
        word
    }
}

/// An iterator over the indices that are in every required [`HierarchicalBitSet`] and in no
/// excluded one, in ascending order.
///
/// Created by [`Join::new`] or [`Join::try_new`]. At each layer it ANDs the required sets'
/// words (a word past a set's end counts as zero) and descends only under bits set in all of
/// them; the excluded sets are applied to layer 0 only. It never allocates. A set may appear
/// more than once, and in both lists (the join is then empty).
///
/// # Examples
///
/// ```
/// use engine::collections::{HierarchicalBitSet, Join};
///
/// let position: HierarchicalBitSet = [1, 5, 9, 700].into_iter().collect();
/// let velocity: HierarchicalBitSet = [5, 9, 700, 800].into_iter().collect();
/// let frozen: HierarchicalBitSet = [9].into_iter().collect();
///
/// // Entities with Position and Velocity, without Frozen.
/// let moving: Vec<usize> = Join::new(&[&position, &velocity], &[&frozen]).collect();
/// assert_eq!(moving, [5, 700]);
/// ```
#[derive(Clone)]
pub struct Join<'a> {
    source: JoinSource<'a>,
    cursor: WordCursor,
    /// The index of the first bit of the current layer-0 word.
    base: usize,
    /// The bits of the current layer-0 word not yet yielded.
    bits: u64,
}

impl<'a> Join<'a> {
    /// A join of the indices in every set of `required` and in no set of `excluded`.
    ///
    /// # Panics
    ///
    /// Panics if `required` is empty, because the result would be every index not excluded.
    /// Use [`try_new`](Self::try_new) to get an error instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use engine::collections::{HierarchicalBitSet, Join};
    ///
    /// let alive: HierarchicalBitSet = [1, 2, 3].into_iter().collect();
    /// let hidden: HierarchicalBitSet = [2].into_iter().collect();
    /// assert_eq!(Join::new(&[&alive], &[&hidden]).collect::<Vec<_>>(), [1, 3]);
    /// ```
    #[must_use]
    #[track_caller]
    pub fn new(
        required: &'a [&'a HierarchicalBitSet],
        excluded: &'a [&'a HierarchicalBitSet],
    ) -> Self {
        let Ok(join) = Self::try_new(required, excluded) else {
            panic!(
                "Join::new: no required sets were given; a join needs at least one set to draw \
                 indices from (to iterate everything not excluded, pass the set of live indices)"
            )
        };
        join
    }

    /// Like [`new`](Self::new), but returns an error if `required` is empty.
    ///
    /// # Errors
    ///
    /// Returns [`EmptyJoinError`] if `required` is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use engine::collections::{HierarchicalBitSet, Join};
    ///
    /// assert!(Join::try_new(&[], &[]).is_err());
    /// let set = HierarchicalBitSet::new();
    /// assert!(Join::try_new(&[&set], &[]).is_ok());
    /// ```
    pub fn try_new(
        required: &'a [&'a HierarchicalBitSet],
        excluded: &'a [&'a HierarchicalBitSet],
    ) -> Result<Self, EmptyJoinError> {
        if required.is_empty() {
            return Err(EmptyJoinError::new());
        }
        let source = JoinSource { required, excluded };
        Ok(Self {
            cursor: WordCursor::new(source.word(LAYERS - 1, 0)),
            source,
            base: 0,
            bits: 0,
        })
    }
}

impl Iterator for Join<'_> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<usize> {
        while self.bits == 0 {
            let (word, bits) = self.cursor.next_word(&self.source)?;
            self.base = word * WORD_BITS;
            self.bits = bits;
        }
        let bit = self.bits.trailing_zeros() as usize;
        self.bits &= self.bits - 1; // clear the lowest set bit
        Some(self.base + bit)
    }
}

impl FusedIterator for Join<'_> {}

impl fmt::Debug for Join<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Join")
            .field("required", &self.source.required.len())
            .field("excluded", &self.source.excluded.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collections::tests::{bounded, debug_bounded};
    use std::string::ToString;

    fn set(items: &[usize]) -> HierarchicalBitSet {
        items.iter().copied().collect()
    }

    #[test]
    fn join_short_required_set_ends_the_join() {
        let long = set(&[1, 5, 1_000_000]);
        let short = set(&[1, 5]);
        let got = bounded(Join::new(&[&long, &short], &[]), 5);
        assert_eq!(got, [1, 5]);
        let got = bounded(Join::new(&[&short, &long], &[]), 5);
        assert_eq!(got, [1, 5]);
    }

    #[test]
    fn join_set_in_both_lists_is_empty() {
        let a = set(&[1, 5, 700]);
        let b = set(&[5, 700]);
        assert_eq!(bounded(Join::new(&[&a, &b], &[&b]), 5), []);
        assert_eq!(bounded(Join::new(&[&a], &[&a]), 5), []);
    }

    #[test]
    fn join_exclusion_applies_below_set_upper_bits() {
        // 100 and 101 share a layer-1 bit; excluding 100 must leave 101.
        let a = set(&[100, 101]);
        let b = set(&[100]);
        assert_eq!(bounded(Join::new(&[&a], &[&b]), 5), [101]);
    }

    #[test]
    fn try_new_with_no_required_sets_is_an_error() {
        let s = set(&[1]);
        assert!(Join::try_new(&[], &[]).is_err());
        assert!(Join::try_new(&[], &[&s]).is_err());
        assert!(Join::try_new(&[&s], &[]).is_ok());
    }

    #[test]
    fn empty_join_error_display() {
        let text = EmptyJoinError::new().to_string();
        assert!(text.contains("no required sets"), "{text}");
        assert!(text.contains("pass the set of live indices"), "{text}");
    }

    #[test]
    fn join_debug_is_short() {
        let a = set(&[1, 2, 3]);
        let (required, excluded) = ([&a, &a], [&a]);
        let join = Join::new(&required, &excluded);
        assert_eq!(
            debug_bounded(&join, 100),
            "Join { required: 2, excluded: 1, .. }"
        );
    }
}
