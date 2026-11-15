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

/// `words` without its trailing zero words.
pub(super) fn trimmed(words: &[u64]) -> &[u64] {
    let end = words
        .iter()
        .rposition(|&w| w != 0)
        .map_or(0, |last| last + 1);
    &words[..end]
}

/// The highest set bit, as a `u64` because on a 32-bit target it may not fit in a `usize`.
pub(super) fn highest_bit_u64(words: &[u64]) -> Option<u64> {
    let (index, word) = words.iter().enumerate().rev().find(|(_, w)| **w != 0)?;
    Some(index as u64 * WORD_BITS as u64 + u64::from(63 - word.leading_zeros()))
}

/// `dst |= src`, word by word. A `src` longer than `dst` is cut off, so callers grow `dst` first.
pub(super) fn union_into(dst: &mut [u64], src: &[u64]) {
    for (d, s) in dst.iter_mut().zip(src) {
        *d |= s;
    }
}

/// `dst &= src`. Words of `dst` past the end of `src` are zeroed.
pub(super) fn intersect_into(dst: &mut [u64], src: &[u64]) {
    let shared = src.len().min(dst.len());
    for (d, s) in dst.iter_mut().zip(src) {
        *d &= s;
    }
    dst[shared..].fill(0);
}

/// `dst &= !src`. Words of `dst` past the end of `src` are unchanged.
pub(super) fn difference_into(dst: &mut [u64], src: &[u64]) {
    for (d, s) in dst.iter_mut().zip(src) {
        *d &= !s;
    }
}

/// `dst ^= src`. A `src` longer than `dst` is cut off, so callers grow `dst` first.
pub(super) fn xor_into(dst: &mut [u64], src: &[u64]) {
    for (d, s) in dst.iter_mut().zip(src) {
        *d ^= s;
    }
}

/// Whether every bit of `a` is in `b`. Words past the end of either slice are zero.
pub(super) fn is_subset(a: &[u64], b: &[u64]) -> bool {
    a.iter()
        .enumerate()
        .all(|(i, &w)| w & !b.get(i).copied().unwrap_or(0) == 0)
}

/// Whether `a` and `b` share no bit. Words past the end of either slice are zero.
pub(super) fn is_disjoint(a: &[u64], b: &[u64]) -> bool {
    a.iter().zip(b).all(|(x, y)| x & y == 0)
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
    fn highest_bit_u64_and_the_limit() {
        assert_eq!(highest_bit_u64(&[]), None);
        assert_eq!(highest_bit_u64(&[0, 0]), None);
        assert_eq!(highest_bit_u64(&[1, 0, 0, 1 << 5, 0]), Some(3 * 64 + 5));
        assert_eq!(highest_bit_u64(&[1 << 63]), Some(63));
    }

    #[test]
    fn trimmed_drops_trailing_zero_words() {
        assert_eq!(trimmed(&[]), &[] as &[u64]);
        assert_eq!(trimmed(&[0, 0]), &[] as &[u64]);
        assert_eq!(trimmed(&[0, 3, 0, 0]), &[0, 3]);
        assert_eq!(trimmed(&[5]), &[5]);
    }

    #[test]
    fn in_place_routines_treat_missing_words_as_zero() {
        let mut dst = [0b1100, 0b1, 0b1];
        union_into(&mut dst, &[0b0011]);
        assert_eq!(dst, [0b1111, 0b1, 0b1]);
        xor_into(&mut dst, &[0b0101, 0b1]);
        assert_eq!(dst, [0b1010, 0, 0b1]);
        difference_into(&mut dst, &[0b1000]);
        assert_eq!(dst, [0b0010, 0, 0b1]);
        intersect_into(&mut dst, &[0b0110]);
        assert_eq!(dst, [0b0010, 0, 0]);
    }

    #[test]
    fn relations_treat_missing_words_as_zero() {
        assert!(is_subset(&[], &[]));
        assert!(is_subset(&[1, 0, 0], &[1]));
        assert!(!is_subset(&[1, 0, 1], &[1]));
        assert!(is_subset(&[1], &[3, 4]));
        assert!(is_disjoint(&[1, 1], &[2]));
        assert!(!is_disjoint(&[1, 1], &[3]));
        assert!(is_disjoint(&[], &[1]));
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
