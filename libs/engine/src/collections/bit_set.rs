//! Sets of small non-negative integers, stored one bit per index.
//!
//! [`BitSet`] will live on the heap and grow to fit any index it is given; [`BitArray`] is
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

pub use array::BitArray;
pub use iter::Iter;
