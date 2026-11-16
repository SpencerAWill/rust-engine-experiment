//! The descent cursor: walks the layers of a hierarchical bitset from the top, visiting only
//! the layer-0 words whose parent bits are all set.
//!
//! The cursor reads its words through a [`WordSource`], so one cursor serves a set's own
//! iteration and any set built from several sets (a join, an intersection). A source's layer-0
//! word may be zero under set parent bits; the cursor skips such words.

use super::words::WORD_BITS;

/// The number of layers: layers 0 to 4 are vectors and layer 5 is the single word `top`.
pub(super) const LAYERS: usize = 6;

/// log2 of the word width: the bits of an index one layer consumes.
pub(super) const WORD_SHIFT: u32 = WORD_BITS.ilog2();

/// Where a cursor reads words from.
pub(super) trait WordSource {
    /// Word `index` of `layer` (0 to 5); a word past the end of a layer is `0`. Layer 5 has the
    /// single word `0`.
    fn word(&self, layer: usize, index: usize) -> u64;
}

/// A position in a descent: per layer, the bits of the current word not yet visited, and that
/// word's index. Layer 0's words are returned, not kept, so `bits[0]` stays zero.
#[derive(Clone, Debug)]
pub(super) struct WordCursor {
    bits: [u64; LAYERS],
    base: [usize; LAYERS],
}

impl WordCursor {
    /// A cursor over a source whose top word (layer 5, word 0) is `top_word`.
    #[inline]
    pub(super) fn new(top_word: u64) -> Self {
        let mut bits = [0; LAYERS];
        bits[LAYERS - 1] = top_word;
        Self {
            bits,
            base: [0; LAYERS],
        }
    }

    /// The next layer-0 word under a set path of upper bits, in ascending order, as
    /// `(word index, word)`. Words that are zero are skipped.
    ///
    /// A word of layer `k - 1` is read only for a set bit of layer `k`.
    #[inline]
    pub(super) fn next_word(&mut self, source: &impl WordSource) -> Option<(usize, u64)> {
        loop {
            // The lowest layer with an unvisited bit holds the next path to follow.
            let layer = (1..LAYERS).find(|&k| self.bits[k] != 0)?;
            let bit = self.bits[layer].trailing_zeros() as usize;
            self.bits[layer] &= self.bits[layer] - 1; // clear the lowest set bit
            let child = (self.base[layer] << WORD_SHIFT) | bit;
            let below = layer - 1;
            let word = source.word(below, child);
            if below == 0 {
                if word != 0 {
                    return Some((child, word));
                }
            } else {
                self.bits[below] = word;
                self.base[below] = child;
            }
        }
    }
}

/// The members of the layer-0 word being yielded: the index of its first bit and the bits not
/// yet yielded. Both iterators over a descent keep one.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct WordBits {
    base: usize,
    bits: u64,
}

impl WordBits {
    /// The next member in ascending order, refilling from `cursor` when the word is spent.
    #[inline]
    pub(super) fn next_member(
        &mut self,
        cursor: &mut WordCursor,
        source: &impl WordSource,
    ) -> Option<usize> {
        while self.bits == 0 {
            let (word, bits) = cursor.next_word(source)?;
            self.base = word * WORD_BITS;
            self.bits = bits;
        }
        let bit = self.bits.trailing_zeros() as usize;
        self.bits &= self.bits - 1; // clear the lowest set bit
        Some(self.base + bit)
    }
}
