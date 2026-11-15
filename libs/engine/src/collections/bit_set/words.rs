//! Word routines shared by `BitSet` and `BitArray`, so the two cannot drift apart.
//!
//! Bit `i` is bit `i % 64` of word `i / 64` (see the module documentation of `bit_set`).

/// Bits in a word.
pub(super) const WORD_BITS: usize = 64;

/// The word index and the single-bit mask of bit `i`.
pub(super) const fn locate(i: usize) -> (usize, u64) {
    (i / WORD_BITS, 1 << (i % WORD_BITS))
}

/// The number of set bits.
pub(super) fn count(words: &[u64]) -> usize {
    words.iter().map(|w| w.count_ones() as usize).sum()
}

/// The lowest set bit.
pub(super) fn first(words: &[u64]) -> Option<usize> {
    let (index, word) = words.iter().enumerate().find(|(_, w)| **w != 0)?;
    Some(index * WORD_BITS + word.trailing_zeros() as usize)
}

/// The highest set bit.
pub(super) fn last(words: &[u64]) -> Option<usize> {
    let (index, word) = words.iter().enumerate().rev().find(|(_, w)| **w != 0)?;
    Some(index * WORD_BITS + (WORD_BITS - 1 - word.leading_zeros() as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locate_splits_at_word_boundaries() {
        assert_eq!(locate(0), (0, 1));
        assert_eq!(locate(63), (0, 1 << 63));
        assert_eq!(locate(64), (1, 1));
        assert_eq!(locate(130), (2, 4));
    }

    #[test]
    fn count_first_last() {
        assert_eq!(count(&[]), 0);
        assert_eq!(first(&[]), None);
        assert_eq!(last(&[]), None);
        assert_eq!(first(&[0, 0]), None);
        assert_eq!(last(&[0, 0]), None);
        let words = [0, 0b1010_0000, 0, 1 << 63, 0];
        assert_eq!(count(&words), 3);
        assert_eq!(first(&words), Some(64 + 5));
        assert_eq!(last(&words), Some(3 * 64 + 63));
    }
}
