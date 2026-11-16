//! `HierarchicalBitSet`: a set of indices in six layers of `u64` words.

use alloc::vec::Vec;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::iter::FusedIterator;

use super::BitSet;
use super::descent::{LAYERS, WORD_SHIFT, WordBits, WordCursor, WordSource};
use super::exceeds_max_bits;
use super::join::JoinSource;
use super::words::{self, MAX_BITS, WORD_BITS, bits_for_words, words_for_bits};
use crate::collections::error::{BitSetInvariantError, InvariantRule, TryReserveError};

/// The vectors: layers 0 to 4.
const VECTOR_LAYERS: usize = LAYERS - 1;

/// The shift that turns a bit index into the index of its word in layer `k`.
#[inline]
const fn shift(k: usize) -> usize {
    WORD_SHIFT as usize * (k + 1)
}

/// A set of small non-negative integers in six layers of `u64` words, so that iteration can skip
/// empty ranges.
///
/// Layer 0 holds the members in the layout of [`BitSet`](super::BitSet): bit `i` is bit `i % 64`
/// of word `i / 64`. Each bit of an upper layer records whether one word of the layer below is
/// non-zero (see the [module documentation](super)). Valid indices are
/// `0..`[`MAX_BITS`](Self::MAX_BITS) on every target.
///
/// Equality and [`Hash`] depend only on the members, never on the capacity or on how the set got
/// there.
///
/// # Examples
///
/// ```
/// use engine::collections::HierarchicalBitSet;
///
/// let mut positions = HierarchicalBitSet::new();
/// positions.insert(3);
/// positions.insert(1_000_000);
/// assert!(positions.contains(1_000_000));
/// assert!(!positions.contains(4));
/// assert_eq!(positions.iter().collect::<Vec<_>>(), [3, 1_000_000]);
/// assert_eq!(positions.len(), 2);
/// assert_eq!((positions.first(), positions.last()), (Some(3), Some(1_000_000)));
/// ```
#[derive(Clone, Default)]
pub struct HierarchicalBitSet {
    layers: [Vec<u64>; VECTOR_LAYERS],
    top: u64,
}

impl HierarchicalBitSet {
    /// The number of valid indices: the largest index is `MAX_BITS - 1`.
    ///
    /// It is `u32::MAX` on every target, as for [`BitSet`](super::BitSet).
    pub const MAX_BITS: usize = MAX_BITS;

    /// An empty set. It does not allocate.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            layers: [const { Vec::new() }; VECTOR_LAYERS],
            top: 0,
        }
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

    /// The number of members. This takes O(words of layer 0) time.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        words::count(&self.layers[0])
    }

    /// Whether the set has no members. This takes O(1) time.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.top == 0
    }

    /// The number of bits that fit without allocating in layer 0: 64 per word allocated, up to
    /// `MAX_BITS`.
    #[must_use]
    #[inline]
    pub fn capacity(&self) -> usize {
        bits_for_words(self.layers[0].capacity())
    }

    /// Makes [`capacity`](Self::capacity) at least `bits`, in every layer, so no insert below
    /// `bits` allocates.
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
        words_for_bits(bits).ok_or_else(TryReserveError::capacity_overflow)?;
        for (k, layer) in self.layers.iter_mut().enumerate() {
            // Layer `k` holds one bit per word of the layer below, 64 words per word.
            let words = bits.div_ceil(1 << shift(k));
            layer
                .try_reserve_exact(words.saturating_sub(layer.len()))
                .map_err(TryReserveError::from_alloc)?;
        }
        Ok(())
    }

    /// Drops trailing zero words from every layer, then shrinks the allocations to fit.
    pub fn shrink_to_fit(&mut self) {
        for layer in &mut self.layers {
            let keep = words::trimmed(layer).len();
            layer.truncate(keep);
            layer.shrink_to_fit();
        }
    }

    /// Removes every member and keeps the allocations.
    #[inline]
    pub fn clear(&mut self) {
        for layer in &mut self.layers {
            layer.clear();
        }
        self.top = 0;
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
        // The common case: the word exists already. Then so do the upper words it needs.
        let Some(w) = self.layers[0].get_mut(word) else {
            return self.insert_growing(index);
        };
        let before = *w;
        *w = before | mask;
        if before == 0 {
            self.set_parents(word);
        }
        Ok(before & mask == 0)
    }

    /// The slow path of `try_insert`: grows to hold `index`, whose word is then a new zero word.
    #[cold]
    #[inline(never)]
    fn insert_growing(&mut self, index: usize) -> Result<bool, TryReserveError> {
        self.grow_to_hold(index)?;
        let (word, mask) = words::locate(index);
        self.layers[0][word] = mask;
        self.set_parents(word);
        Ok(true)
    }

    /// Grows every layer to hold `index`, filled with zeros. A failed reservation leaves the set
    /// unchanged, because every layer is reserved before any is resized.
    #[cold]
    #[inline(never)]
    fn grow_to_hold(&mut self, index: usize) -> Result<(), TryReserveError> {
        for (k, layer) in self.layers.iter_mut().enumerate() {
            let needed = (index >> shift(k)) + 1;
            layer
                .try_reserve(needed.saturating_sub(layer.len()))
                .map_err(TryReserveError::from_alloc)?;
        }
        for (k, layer) in self.layers.iter_mut().enumerate() {
            let needed = (index >> shift(k)) + 1;
            if needed > layer.len() {
                layer.resize(needed, 0);
            }
        }
        Ok(())
    }

    /// The word at `index` of layer `k` (1 to 5), for writing.
    #[inline]
    fn parent_word_mut(&mut self, k: usize, index: usize) -> &mut u64 {
        if k == VECTOR_LAYERS {
            &mut self.top
        } else {
            &mut self.layers[k][index]
        }
    }

    /// Sets the parent bits of layer-0 word `word`, upwards, stopping at the first parent word
    /// that was already non-zero.
    #[inline(never)]
    fn set_parents(&mut self, word: usize) {
        let mut child = word;
        for k in 1..LAYERS {
            let parent = self.parent_word_mut(k, child >> WORD_SHIFT);
            let before = *parent;
            *parent |= 1 << (child % WORD_BITS);
            if before != 0 {
                return;
            }
            child >>= WORD_SHIFT;
        }
    }

    /// Clears the parent bits of layer-0 word `word`, upwards, stopping at the first parent word
    /// that stays non-zero.
    #[inline]
    fn clear_parents(&mut self, word: usize) {
        let mut child = word;
        for k in 1..LAYERS {
            let parent = self.parent_word_mut(k, child >> WORD_SHIFT);
            *parent &= !(1 << (child % WORD_BITS));
            if *parent != 0 {
                return;
            }
            child >>= WORD_SHIFT;
        }
    }

    /// Removes `index`, returning `true` if it was a member. It never allocates.
    #[inline]
    pub fn remove(&mut self, index: usize) -> bool {
        let (word, mask) = words::locate(index);
        let Some(w) = self.layers[0].get_mut(word) else {
            return false;
        };
        let removed = *w & mask != 0;
        *w &= !mask;
        if removed && *w == 0 {
            self.clear_parents(word);
        }
        removed
    }

    /// Adds every member of `other`.
    ///
    /// Each layer grows to the trimmed length of `other`'s layer, so `other`'s spare capacity
    /// does not carry over.
    pub fn union_with(&mut self, other: &Self) {
        for (mine, theirs) in self.layers.iter_mut().zip(&other.layers) {
            let theirs = words::trimmed(theirs);
            if mine.len() < theirs.len() {
                mine.resize(theirs.len(), 0);
            }
            for (m, t) in mine.iter_mut().zip(theirs) {
                *m |= t;
            }
        }
        self.top |= other.top;
    }

    /// Keeps only the members that are also in `other`. It never allocates.
    pub fn intersect_with(&mut self, other: &Self) {
        // The cursor holds no borrow between calls: it keeps copies of the upper words it has
        // yet to visit, and clearing parents only touches bits of paths it has already passed.
        let mut cursor = WordCursor::new(self.top);
        while let Some((index, word)) = cursor.next_word(&*self) {
            let kept = word & other.word(0, index);
            if kept != word {
                self.layers[0][index] = kept;
                if kept == 0 {
                    self.clear_parents(index);
                }
            }
        }
    }

    /// Removes the members that are also in `other`. It never allocates.
    pub fn difference_with(&mut self, other: &Self) {
        // Only words both sets have non-zero can change, so the cursor walks their AND. As in
        // `intersect_with`, the source is rebuilt for each call so that no borrow of `self`
        // outlives it.
        let mut cursor = WordCursor::new(self.top & other.top);
        loop {
            let required = [&*self, other];
            let source = JoinSource {
                required: &required,
                excluded: &[],
            };
            let Some((index, common)) = cursor.next_word(&source) else {
                return;
            };
            let word = &mut self.layers[0][index];
            *word &= !common;
            if *word == 0 {
                self.clear_parents(index);
            }
        }
    }

    /// Whether every member is also in `other`.
    ///
    /// # Examples
    ///
    /// ```
    /// use engine::collections::HierarchicalBitSet;
    ///
    /// let archetype = HierarchicalBitSet::from([0, 2, 5, 9]);
    /// let query = HierarchicalBitSet::from([2, 5]);
    /// assert!(query.is_subset(&archetype));
    /// assert!(archetype.is_superset(&query));
    /// ```
    #[must_use]
    pub fn is_subset(&self, other: &Self) -> bool {
        let mut cursor = WordCursor::new(self.top);
        while let Some((index, word)) = cursor.next_word(self) {
            if word & !other.word(0, index) != 0 {
                return false;
            }
        }
        true
    }

    /// Whether every member of `other` is also in this set.
    #[must_use]
    pub fn is_superset(&self, other: &Self) -> bool {
        other.is_subset(self)
    }

    /// Whether the two sets have no member in common.
    ///
    /// # Examples
    ///
    /// ```
    /// use engine::collections::HierarchicalBitSet;
    ///
    /// let a = HierarchicalBitSet::from([1, 2]);
    /// assert!(a.is_disjoint(&HierarchicalBitSet::from([3, 1_000_000])));
    /// assert!(!a.is_disjoint(&HierarchicalBitSet::from([2])));
    /// ```
    #[must_use]
    pub fn is_disjoint(&self, other: &Self) -> bool {
        let required = [self, other];
        let source = JoinSource {
            required: &required,
            excluded: &[],
        };
        WordCursor::new(self.top & other.top)
            .next_word(&source)
            .is_none()
    }

    /// Whether `index` is a member. An index past the end is not, and nothing is allocated.
    #[must_use]
    #[inline]
    pub fn contains(&self, index: usize) -> bool {
        let (word, mask) = words::locate(index);
        self.layers[0].get(word).is_some_and(|w| w & mask != 0)
    }

    /// An iterator over the members, in ascending order. It skips empty ranges: 64, 4,096,
    /// 262,144 indices and so on at a time.
    ///
    /// # Examples
    ///
    /// ```
    /// use engine::collections::HierarchicalBitSet;
    ///
    /// let set = HierarchicalBitSet::from([1_000_000, 3]);
    /// assert_eq!(set.iter().collect::<Vec<_>>(), [3, 1_000_000]);
    /// ```
    #[must_use]
    #[inline]
    pub fn iter(&self) -> HierarchicalIter<'_> {
        HierarchicalIter {
            set: self,
            cursor: WordCursor::new(self.top),
            current: WordBits::default(),
        }
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
    /// use engine::collections::HierarchicalBitSet;
    ///
    /// let mut set = HierarchicalBitSet::new();
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

    /// Adds every index, or panics naming `operation`, the public item the caller used.
    #[track_caller]
    fn extend_named(&mut self, operation: &str, indices: impl IntoIterator<Item = usize>) {
        for index in indices {
            if self.try_insert(index).is_err() {
                limit_exceeded(operation, format_args!("bit index {index}"), "try_extend");
            }
        }
    }

    /// Rebuilds layers 1 to 5 from layer 0.
    fn rebuild_upper_layers(&mut self) {
        for k in 1..LAYERS {
            let parent = summarize(&self.layers[k - 1]);
            if k == VECTOR_LAYERS {
                self.top = parent.first().copied().unwrap_or(0);
            } else {
                self.layers[k] = parent;
            }
        }
    }

    /// The smallest member. This takes O(1) time: it descends from the top layer.
    #[must_use]
    #[inline]
    pub fn first(&self) -> Option<usize> {
        self.descend(|word| word.trailing_zeros() as usize)
    }

    /// The largest member. This takes O(1) time: it descends from the top layer.
    #[must_use]
    #[inline]
    pub fn last(&self) -> Option<usize> {
        self.descend(|word| WORD_BITS - 1 - word.leading_zeros() as usize)
    }

    /// Walks from `top` to layer 0, taking the bit `pick` chooses in each non-zero word.
    #[inline]
    fn descend(&self, pick: impl Fn(u64) -> usize) -> Option<usize> {
        if self.top == 0 {
            return None;
        }
        let mut index = pick(self.top);
        for layer in self.layers.iter().rev() {
            index = (index << WORD_SHIFT) | pick(layer[index]);
        }
        Some(index)
    }

    /// Layer 0, in the layout of [`BitSet::as_words`](super::BitSet::as_words).
    ///
    /// It may end in zero words, after removals; use
    /// [`as_trimmed_words`](Self::as_trimmed_words) for a canonical form.
    #[must_use]
    #[inline]
    pub fn as_words(&self) -> &[u64] {
        &self.layers[0]
    }

    /// [`as_words`](Self::as_words) without its trailing zero words. Equal sets return equal
    /// slices.
    #[must_use]
    #[inline]
    pub fn as_trimmed_words(&self) -> &[u64] {
        words::trimmed(&self.layers[0])
    }

    /// Checks the layer rule: bit `b` of word `j` of each upper layer is set if and only if word
    /// `j * 64 + b` of the layer below is non-zero, `top` uses bits 0 to 3 only, and no bit at or
    /// above `MAX_BITS` is set.
    ///
    /// A set built through the public API always passes; tests and fuzz targets call this after
    /// each operation.
    ///
    /// # Errors
    ///
    /// Returns [`BitSetInvariantError`] naming the first layer and word that break the rule.
    ///
    /// # Examples
    ///
    /// ```
    /// use engine::collections::HierarchicalBitSet;
    ///
    /// let mut set = HierarchicalBitSet::new();
    /// set.insert(70);
    /// assert!(set.check_invariants().is_ok());
    /// ```
    pub fn check_invariants(&self) -> Result<(), BitSetInvariantError> {
        if let Some(highest) = words::highest_bit_u64(&self.layers[0])
            && exceeds_max_bits(highest)
        {
            let last_word = words::trimmed(&self.layers[0]).len() - 1;
            return Err(BitSetInvariantError::new(
                0,
                last_word,
                InvariantRule::BitAtOrAboveMax,
            ));
        }
        if self.top >> 4 != 0 {
            return Err(BitSetInvariantError::new(
                LAYERS - 1,
                0,
                InvariantRule::TopBitAboveThree,
            ));
        }
        for k in 1..LAYERS {
            let child = &self.layers[k - 1];
            let parent: &[u64] = if k == VECTOR_LAYERS {
                core::slice::from_ref(&self.top)
            } else {
                &self.layers[k]
            };
            for j in 0..parent.len().max(child.len().div_ceil(WORD_BITS)) {
                let expected = child
                    .get(j * WORD_BITS..)
                    .unwrap_or_default()
                    .iter()
                    .take(WORD_BITS)
                    .enumerate()
                    .fold(0_u64, |bits, (b, w)| bits | u64::from(*w != 0) << b);
                if parent.get(j).copied().unwrap_or(0) != expected {
                    return Err(BitSetInvariantError::new(k, j, InvariantRule::ParentBit));
                }
            }
        }
        Ok(())
    }
}

#[cold]
#[track_caller]
fn limit_exceeded(operation: &str, subject: fmt::Arguments<'_>, twin: &str) -> ! {
    panic!(
        "HierarchicalBitSet::{operation}: {subject} is not allowed: indices and capacities are \
         limited to HierarchicalBitSet::MAX_BITS ({max}) and the allocation must succeed; use \
         {twin} to handle it",
        max = HierarchicalBitSet::MAX_BITS
    );
}

/// The parent word of each run of 64 words of `child`: bit `b` of word `j` is set when word
/// `j * 64 + b` of `child` is non-zero.
fn summarize(child: &[u64]) -> Vec<u64> {
    child
        .chunks(WORD_BITS)
        .map(|run| {
            run.iter()
                .enumerate()
                .fold(0_u64, |bits, (b, w)| bits | u64::from(*w != 0) << b)
        })
        .collect()
}

impl WordSource for HierarchicalBitSet {
    #[inline]
    fn word(&self, layer: usize, index: usize) -> u64 {
        if layer == VECTOR_LAYERS {
            self.top
        } else {
            self.layers[layer].get(index).copied().unwrap_or(0)
        }
    }
}

impl From<&BitSet> for HierarchicalBitSet {
    /// Copies the members; the upper layers are built from layer 0.
    fn from(set: &BitSet) -> Self {
        let mut out = Self::new();
        out.layers[0].extend_from_slice(set.as_trimmed_words());
        out.rebuild_upper_layers();
        out
    }
}

impl From<&HierarchicalBitSet> for BitSet {
    /// Copies the members; the upper layers are dropped.
    fn from(set: &HierarchicalBitSet) -> Self {
        // Cannot panic: a hierarchical set never has a bit at or above `MAX_BITS`.
        BitSet::from_words(set.as_trimmed_words())
    }
}

impl FromIterator<usize> for HierarchicalBitSet {
    /// # Panics
    ///
    /// Panics if an index is `>= MAX_BITS`. Use [`HierarchicalBitSet::try_extend`] on a new set
    /// to handle that.
    #[track_caller]
    fn from_iter<I: IntoIterator<Item = usize>>(iter: I) -> Self {
        let mut set = Self::new();
        set.extend_named("from_iter", iter);
        set
    }
}

impl Extend<usize> for HierarchicalBitSet {
    /// # Panics
    ///
    /// Panics if an index is `>= MAX_BITS`. Use [`HierarchicalBitSet::try_extend`] to handle
    /// that.
    #[track_caller]
    fn extend<I: IntoIterator<Item = usize>>(&mut self, iter: I) {
        self.extend_named("extend", iter);
    }
}

impl<'a> Extend<&'a usize> for HierarchicalBitSet {
    /// # Panics
    ///
    /// Panics if an index is `>= MAX_BITS`. Use [`HierarchicalBitSet::try_extend`] to handle
    /// that.
    #[track_caller]
    fn extend<I: IntoIterator<Item = &'a usize>>(&mut self, iter: I) {
        self.extend_named("extend", iter.into_iter().copied());
    }
}

impl<const N: usize> From<[usize; N]> for HierarchicalBitSet {
    /// # Panics
    ///
    /// Panics if an index is `>= MAX_BITS`. Use [`HierarchicalBitSet::try_extend`] on a new set
    /// to handle that.
    #[track_caller]
    fn from(indices: [usize; N]) -> Self {
        let mut set = Self::new();
        set.extend_named("from", indices);
        set
    }
}

impl<'a> IntoIterator for &'a HierarchicalBitSet {
    type Item = usize;
    type IntoIter = HierarchicalIter<'a>;

    fn into_iter(self) -> HierarchicalIter<'a> {
        self.iter()
    }
}

/// An iterator over the members of a [`HierarchicalBitSet`], in ascending order.
///
/// Created by [`HierarchicalBitSet::iter`]. It reads a word of a layer only under a set bit of
/// the layer above, so empty ranges cost nothing; it keeps one word per layer, with no
/// allocation.
///
/// # Examples
///
/// ```
/// use engine::collections::HierarchicalBitSet;
///
/// let mut positions = HierarchicalBitSet::new();
/// positions.insert(3);
/// positions.insert(1_000_000);
/// assert!(positions.contains(1_000_000));
/// assert_eq!(positions.iter().collect::<Vec<_>>(), [3, 1_000_000]);
/// ```
#[derive(Clone, Debug)]
pub struct HierarchicalIter<'a> {
    set: &'a HierarchicalBitSet,
    cursor: WordCursor,
    current: WordBits,
}

impl Iterator for HierarchicalIter<'_> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<usize> {
        self.current.next_member(&mut self.cursor, self.set)
    }
}

impl FusedIterator for HierarchicalIter<'_> {}

impl PartialEq for HierarchicalBitSet {
    fn eq(&self, other: &Self) -> bool {
        // The upper layers follow from layer 0, so they are not compared.
        self.as_trimmed_words() == other.as_trimmed_words()
    }
}

impl Eq for HierarchicalBitSet {}

impl Hash for HierarchicalBitSet {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_trimmed_words().hash(state);
    }
}

impl fmt::Debug for HierarchicalBitSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collections::tests::{bounded, debug_bounded};
    use std::string::ToString;

    /// The non-zero words of `layer` as `(word index, word)` pairs.
    fn nonzero(layer: &[u64]) -> std::vec::Vec<(usize, u64)> {
        layer
            .iter()
            .enumerate()
            .filter(|(_, w)| **w != 0)
            .map(|(i, w)| (i, *w))
            .collect()
    }

    #[test]
    fn hierarchical_layer_layout() {
        let mut set = HierarchicalBitSet::new();
        for i in [0, 63, 64, 4_096, 262_144] {
            assert!(set.insert(i));
        }
        assert_eq!(
            nonzero(&set.layers[0]),
            [
                (0, 0x8000_0000_0000_0001),
                (1, 0x1),
                (64, 0x1),
                (4_096, 0x1)
            ]
        );
        assert_eq!(nonzero(&set.layers[1]), [(0, 0x3), (1, 0x1), (64, 0x1)]);
        assert_eq!(nonzero(&set.layers[2]), [(0, 0x3), (1, 0x1)]);
        assert_eq!(nonzero(&set.layers[3]), [(0, 0x3)]);
        assert_eq!(nonzero(&set.layers[4]), [(0, 0x1)]);
        assert_eq!(set.top, 0x1);
        assert_eq!(set.check_invariants(), Ok(()));
    }

    #[test]
    fn remove_clears_only_empty_parents() {
        let mut set = HierarchicalBitSet::new();
        for i in [0, 64, 4_096] {
            set.insert(i);
        }
        assert_eq!(set.layers[1][0], 0b11);
        assert_eq!(set.layers[2][0], 0b11);
        assert!(set.remove(64));
        assert_eq!(set.layers[1][0], 0b01); // word 1 emptied: layer 1's bit 1 clears
        assert_eq!(set.layers[2][0], 0b11); // word 0 of layer 1 still has bit 0
        assert_eq!(set.check_invariants(), Ok(()));
        assert!(set.remove(0));
        assert_eq!(set.layers[1][0], 0);
        assert_eq!(set.layers[2][0], 0b10); // layer 2's bit 0 clears, bit 1 stays
        assert_eq!(set.check_invariants(), Ok(()));
        assert!(!set.remove(0));
        assert!(!set.remove(usize::MAX));
        assert!(set.remove(4_096));
        assert!(set.is_empty());
        assert_eq!(set.top, 0);
        assert_eq!(set.check_invariants(), Ok(()));
    }

    #[test]
    fn try_insert_at_the_limit_is_an_error() {
        for index in [usize::MAX, HierarchicalBitSet::MAX_BITS] {
            let mut set = HierarchicalBitSet::new();
            assert!(set.try_insert(index).is_err());
            assert!(set.is_empty());
            assert_eq!(set.len(), 0);
            assert_eq!(set.as_words(), []);
            assert_eq!(set.check_invariants(), Ok(()));
        }
    }

    #[test]
    fn max_bits_is_u32_max() {
        assert_eq!(HierarchicalBitSet::MAX_BITS as u64, u64::from(u32::MAX));
    }

    #[test]
    fn check_invariants_reports_a_broken_set() {
        // A parent bit over a zero child word.
        let mut set = HierarchicalBitSet::new();
        set.layers[0] = std::vec![0];
        set.layers[1] = std::vec![1];
        let error = set.check_invariants().unwrap_err();
        assert_eq!((error.layer(), error.word()), (1, 0));

        // A zero parent over a non-zero child word.
        let mut set = HierarchicalBitSet::new();
        set.layers[0] = std::vec![0, 0, 1];
        set.layers[1] = std::vec![0];
        let error = set.check_invariants().unwrap_err();
        assert_eq!((error.layer(), error.word()), (1, 0));

        // A broken rule higher up: layer 2's word 0 should have bit 0 set.
        let mut set = HierarchicalBitSet::new();
        set.insert(70);
        set.layers[2][0] = 0;
        let error = set.check_invariants().unwrap_err();
        assert_eq!((error.layer(), error.word()), (2, 0));

        // A bit in `top` above bit 3.
        let mut set = HierarchicalBitSet::new();
        set.top = 1 << 4;
        let error = set.check_invariants().unwrap_err();
        assert_eq!((error.layer(), error.word()), (5, 0));
        assert!(error.to_string().contains("layer 5, word 0"));

        // A well-formed set passes.
        let set: HierarchicalBitSet = {
            let mut s = HierarchicalBitSet::new();
            s.insert(70);
            s
        };
        assert_eq!(set.check_invariants(), Ok(()));
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn check_invariants_reports_a_bit_at_max_bits() {
        // Zeros are allocated lazily, so only the last word is ever touched.
        let mut set = HierarchicalBitSet::new();
        set.layers[0] = std::vec![0; 1 << 26];
        *set.layers[0].last_mut().unwrap() = 1 << 63; // bit 2^32 - 1 = MAX_BITS
        let error = set.check_invariants().unwrap_err();
        assert_eq!((error.layer(), error.word()), (0, (1 << 26) - 1));
        assert!(error.to_string().contains("MAX_BITS"));
    }

    #[test]
    fn debug_format() {
        let mut set = HierarchicalBitSet::new();
        assert_eq!(debug_bounded(&set, 64), "{}");
        set.insert(1_000_000);
        set.insert(3);
        assert_eq!(debug_bounded(&set, 64), "{3, 1000000}");
    }

    #[test]
    fn shrink_to_fit_keeps_members() {
        let mut set = HierarchicalBitSet::with_capacity(1 << 20);
        set.insert(1);
        set.insert(200_000);
        set.insert(900_000);
        set.remove(900_000);
        set.shrink_to_fit();
        assert_eq!(set.as_words().len(), 200_000 / 64 + 1);
        assert_eq!(set.capacity(), (200_000 / 64 + 1) * 64);
        assert_eq!(set.len(), 2);
        assert!(set.contains(1) && set.contains(200_000));
        assert_eq!(set.check_invariants(), Ok(()));
        // Still usable after shrinking.
        assert!(set.insert(900_000));
        assert_eq!(set.check_invariants(), Ok(()));
    }

    #[test]
    fn capacity_and_clear() {
        let mut set = HierarchicalBitSet::with_capacity(4_096);
        assert!(set.capacity() >= 4_096);
        assert!(set.is_empty());
        assert_eq!(set.as_words(), []);
        set.insert(4_000);
        set.insert(1);
        let capacity = set.capacity();
        set.clear();
        assert!(set.is_empty());
        assert_eq!(set.capacity(), capacity);
        assert_eq!(set.check_invariants(), Ok(()));
        // One past the limit does not fit in a `usize` on a 32-bit target.
        if let Some(over) = HierarchicalBitSet::MAX_BITS.checked_add(1) {
            assert!(set.try_ensure_capacity(over).is_err());
        }
        set.ensure_capacity(100_000);
        assert!(set.capacity() >= 100_000);
        assert!(set.is_empty());
    }

    #[test]
    fn first_last_contains_and_len() {
        let mut set = HierarchicalBitSet::new();
        assert_eq!((set.first(), set.last()), (None, None));
        for i in [70, 5, 300_000, 64] {
            set.insert(i);
        }
        assert!(!set.insert(70));
        assert_eq!((set.first(), set.last()), (Some(5), Some(300_000)));
        assert_eq!(set.len(), 4);
        assert!(set.contains(64) && !set.contains(65) && !set.contains(usize::MAX));
        assert_eq!(set.as_trimmed_words().len(), 300_000 / 64 + 1);
        set.remove(300_000);
        assert_eq!(set.last(), Some(70));
        assert_eq!(set.as_words().len(), 300_000 / 64 + 1); // not shrunk
        assert_eq!(set.as_trimmed_words().len(), 2);
    }

    #[test]
    fn equality_and_hash_ignore_capacity() {
        use core::hash::{Hash, Hasher};
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
        fn hash(set: &HierarchicalBitSet) -> u64 {
            let mut hasher = Fnv(0xcbf2_9ce4_8422_2325);
            set.hash(&mut hasher);
            hasher.finish()
        }
        let mut grown = HierarchicalBitSet::with_capacity(8_192);
        grown.insert(1);
        grown.insert(8_000);
        grown.remove(8_000);
        let mut small = HierarchicalBitSet::new();
        small.insert(1);
        assert_eq!(grown, small);
        assert_eq!(hash(&grown), hash(&small));
        small.insert(2);
        assert_ne!(grown, small);
        assert_ne!(hash(&grown), hash(&small));
        assert_eq!(bounded(small.as_trimmed_words().iter(), 3), [&0b110]);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn top_of_range() {
        // One test, so only one set of up to 512 MiB exists at a time.
        let max = HierarchicalBitSet::MAX_BITS;
        // Layer 4's word index is `i >> 30`, so 0 sets top's bit 0 and the rest set bits 1 to 3.
        let members = [max - 1, 1 << 30, 1 << 31, 3 << 30, 0];
        let mut set = HierarchicalBitSet::new();
        for &i in &members {
            assert!(set.insert(i), "insert {i}");
            assert!(set.contains(i));
            assert_eq!(set.check_invariants(), Ok(()), "after insert {i}");
        }
        assert_eq!(set.top, 0b1111); // all of top's bits 0 to 3
        assert!(set.try_insert(max).is_err());
        assert!(!set.contains(max));
        assert_eq!(set.check_invariants(), Ok(()));
        assert_eq!((set.first(), set.last()), (Some(0), Some(max - 1)));
        assert_eq!(set.len(), 5);
        assert_eq!(
            debug_bounded(&set, 100),
            "{0, 1073741824, 2147483648, 3221225472, 4294967294}"
        );
        for &i in &members {
            assert!(set.remove(i), "remove {i}");
            assert!(!set.contains(i));
            assert_eq!(set.check_invariants(), Ok(()), "after remove {i}");
        }
        assert!(set.is_empty());
        assert_eq!(set.top, 0);
        assert_eq!((set.first(), set.last()), (None, None));
    }

    /// A source that counts the words it is asked for.
    struct Counting<'a> {
        set: &'a HierarchicalBitSet,
        reads: core::cell::Cell<usize>,
    }

    impl WordSource for Counting<'_> {
        fn word(&self, layer: usize, index: usize) -> u64 {
            self.reads.set(self.reads.get() + 1);
            self.set.word(layer, index)
        }
    }

    #[test]
    fn iteration_skips_empty_ranges() {
        let set: HierarchicalBitSet = [0, 1_000_000].into_iter().collect();
        let source = Counting {
            set: &set,
            reads: core::cell::Cell::new(0),
        };
        let mut cursor = WordCursor::new(set.top);
        let mut found = std::vec::Vec::new();
        while let Some((word, bits)) = cursor.next_word(&source) {
            found.push((word, bits));
            assert!(found.len() <= 3, "the cursor does not stop");
        }
        assert_eq!(found, [(0, 1), (15_625, 1 << (1_000_000 % 64))]);
        // A scan of layer 0 would read 15,626 words.
        assert!(
            source.reads.get() <= 16,
            "read {} words",
            source.reads.get()
        );
        assert_eq!(bounded(set.iter(), 4), [0, 1_000_000]);
    }

    #[test]
    fn cursor_skips_zero_words_under_set_parents() {
        // A source whose layer-0 words are all zero under set parent bits, as a join may make.
        struct Zeros;
        impl WordSource for Zeros {
            fn word(&self, layer: usize, index: usize) -> u64 {
                u64::from(layer > 0 && index == 0)
            }
        }
        let mut cursor = WordCursor::new(1);
        assert_eq!(cursor.next_word(&Zeros), None);
        assert_eq!(cursor.next_word(&Zeros), None);
    }

    #[test]
    fn try_extend_stops_at_the_failing_index() {
        let mut set = HierarchicalBitSet::new();
        assert!(set.try_extend([1, 2, usize::MAX, 4]).is_err());
        assert_eq!(bounded(set.iter(), 4), [1, 2]);
        assert_eq!(set.check_invariants(), Ok(()));
    }

    #[test]
    fn construction_and_extension() {
        let mut set = HierarchicalBitSet::from([9, 3]);
        set.extend([70, 3]);
        set.extend(&[5_000]);
        assert_eq!(bounded(set.iter(), 6), [3, 9, 70, 5_000]);
        let collected: HierarchicalBitSet = [5_000, 70].into_iter().collect();
        assert_eq!(bounded((&collected).into_iter(), 3), [70, 5_000]);
        assert_eq!(set.check_invariants(), Ok(()));
    }

    #[test]
    fn iterator_is_fused_and_clones() {
        let set = HierarchicalBitSet::from([1, 100_000]);
        let mut iter = set.iter();
        assert_eq!(iter.next(), Some(1));
        let mut copy = iter.clone();
        assert_eq!(iter.next(), Some(100_000));
        assert_eq!((iter.next(), iter.next()), (None, None));
        assert_eq!(copy.next(), Some(100_000));
        assert_eq!(copy.next(), None);
    }

    #[test]
    fn bit_set_round_trip_keeps_members() {
        let flat: BitSet = [0, 63, 64, 4_096, 262_144, 1_000_000].into_iter().collect();
        let set = HierarchicalBitSet::from(&flat);
        assert_eq!(set.check_invariants(), Ok(()));
        assert_eq!(set.as_trimmed_words(), flat.as_trimmed_words());
        assert_eq!(
            bounded(set.iter(), 7),
            [0, 63, 64, 4_096, 262_144, 1_000_000]
        );
        assert_eq!(BitSet::from(&set), flat);
        let empty = HierarchicalBitSet::from(&BitSet::new());
        assert!(empty.is_empty());
        assert_eq!(empty.check_invariants(), Ok(()));
        assert_eq!(BitSet::from(&empty), BitSet::new());
    }

    fn set_of(items: &[usize]) -> HierarchicalBitSet {
        items.iter().copied().collect()
    }

    #[test]
    fn intersect_with_clears_emptied_parents() {
        // 0 and 64 share a layer-1 word; 4_096 is under another layer-1 word.
        let mut set = set_of(&[0, 64, 4_096, 262_144]);
        set.intersect_with(&set_of(&[0, 262_144]));
        assert_eq!(bounded(set.iter(), 5), [0, 262_144]);
        assert_eq!(set.layers[1][0], 0b01);
        assert_eq!(set.layers[1][1], 0);
        assert_eq!(set.layers[1][64], 1);
        assert_eq!(set.layers[2][0], 0b01);
        assert_eq!(set.layers[2][1], 0b01);
        assert_eq!(set.check_invariants(), Ok(()));
        set.intersect_with(&HierarchicalBitSet::new());
        assert!(set.is_empty());
        assert_eq!(set.top, 0);
        assert_eq!(set.check_invariants(), Ok(()));
    }

    #[test]
    fn difference_with_clears_emptied_parents() {
        let mut set = set_of(&[0, 64, 4_096, 262_144]);
        set.difference_with(&set_of(&[64, 4_096, 1_000_000]));
        assert_eq!(bounded(set.iter(), 5), [0, 262_144]);
        assert_eq!(set.layers[1][0], 0b01);
        assert_eq!(set.layers[1][1], 0);
        assert_eq!(set.layers[1][64], 1);
        assert_eq!(set.check_invariants(), Ok(()));
        set.difference_with(&set.clone());
        assert!(set.is_empty());
        assert_eq!(set.check_invariants(), Ok(()));
    }

    #[test]
    fn union_with_grows_to_trimmed_length() {
        let mut big = HierarchicalBitSet::with_capacity(65_536);
        big.insert(3);
        let mut set = HierarchicalBitSet::new();
        set.union_with(&big);
        assert_eq!(set.as_words().len(), 1);
        assert_eq!(set.layers[1].len(), 1);
        assert_eq!(bounded(set.iter(), 3), [3]);
        set.union_with(&set_of(&[5_000, 70]));
        assert_eq!(bounded(set.iter(), 5), [3, 70, 5_000]);
        assert_eq!(set.check_invariants(), Ok(()));
    }

    #[test]
    fn subset_ignores_capacity() {
        let small = set_of(&[1, 2]);
        let mut big = HierarchicalBitSet::with_capacity(65_536);
        big.extend([1, 2, 3]);
        assert!(small.is_subset(&big));
        assert!(big.is_superset(&small));
        assert!(!big.is_subset(&small));
        assert!(HierarchicalBitSet::with_capacity(65_536).is_subset(&small));
        assert!(small.is_disjoint(&set_of(&[3, 100_000])));
        assert!(!small.is_disjoint(&big));
        assert!(small.is_disjoint(&HierarchicalBitSet::new()));
    }
}
