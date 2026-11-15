//! Iterators over the members of a bitset, in ascending order.

use core::iter::FusedIterator;

use super::words::WORD_BITS;

/// The state both iterators share: the bits of the word at each end that are not yet yielded.
///
/// `front` and `back` are word indices. When they are equal the two ends share one word, and
/// `front_bits` and `back_bits` are kept equal, so a bit taken from one end disappears from the
/// other.
#[derive(Clone, Debug)]
struct Cursor {
    front: usize,
    back: usize,
    front_bits: u64,
    back_bits: u64,
}

impl Cursor {
    fn new(words: &[u64]) -> Self {
        let front_bits = words.first().copied().unwrap_or(0);
        let back_bits = words.last().copied().unwrap_or(0);
        Self {
            front: 0,
            back: words.len().saturating_sub(1),
            front_bits,
            back_bits,
        }
    }

    fn next(&mut self, words: &[u64]) -> Option<usize> {
        loop {
            if self.front_bits != 0 {
                let bit = self.front_bits.trailing_zeros();
                self.front_bits &= self.front_bits - 1; // clear the lowest set bit
                if self.front == self.back {
                    self.back_bits = self.front_bits;
                }
                return Some(self.front * WORD_BITS + bit as usize);
            }
            if self.front >= self.back {
                return None;
            }
            self.front += 1;
            self.front_bits = if self.front == self.back {
                self.back_bits
            } else {
                words[self.front]
            };
        }
    }

    fn next_back(&mut self, words: &[u64]) -> Option<usize> {
        loop {
            if self.back_bits != 0 {
                let bit = WORD_BITS - 1 - self.back_bits.leading_zeros() as usize;
                self.back_bits &= !(1_u64 << bit);
                if self.front == self.back {
                    self.front_bits = self.back_bits;
                }
                return Some(self.back * WORD_BITS + bit);
            }
            if self.front >= self.back {
                return None;
            }
            self.back -= 1;
            self.back_bits = if self.front == self.back {
                self.front_bits
            } else {
                words[self.back]
            };
        }
    }
}

/// An iterator over the members of a [`BitSet`](super::BitSet) or [`BitArray`](super::BitArray),
/// in ascending order.
///
/// Created by their `iter` methods. It is double-ended: [`next_back`](DoubleEndedIterator::next_back)
/// yields the largest remaining member.
///
/// # Examples
///
/// ```
/// use engine::collections::BitArray;
///
/// let mut bits = BitArray::<2>::new();
/// bits.insert(3);
/// bits.insert(70);
/// bits.insert(100);
///
/// let mut iter = bits.iter();
/// assert_eq!(iter.next(), Some(3));
/// assert_eq!(iter.next_back(), Some(100));
/// assert_eq!(iter.collect::<Vec<_>>(), [70]);
/// ```
#[derive(Clone, Debug)]
pub struct Iter<'a> {
    words: &'a [u64],
    cursor: Cursor,
}

impl<'a> Iter<'a> {
    pub(crate) fn new(words: &'a [u64]) -> Self {
        Self {
            words,
            cursor: Cursor::new(words),
        }
    }
}

impl Iterator for Iter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        self.cursor.next(self.words)
    }
}

impl DoubleEndedIterator for Iter<'_> {
    fn next_back(&mut self) -> Option<usize> {
        self.cursor.next_back(self.words)
    }
}

impl FusedIterator for Iter<'_> {}
