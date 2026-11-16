//! Error types.

use core::error::Error;
use core::fmt;

/// Returned by `try_reserve` when the capacity overflows or the allocator fails.
///
/// # Examples
///
/// ```
/// use engine::collections::IndexMap;
///
/// let mut map = IndexMap::<u32, u32>::new();
/// let error = map.try_reserve(usize::MAX).unwrap_err();
/// assert!(error.to_string().contains("allocation failed"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TryReserveError(());

impl TryReserveError {
    pub(crate) fn from_inner(_inner: indexmap::TryReserveError) -> Self {
        Self(())
    }

    /// An index or capacity past `BitSet::MAX_BITS`.
    pub(crate) const fn capacity_overflow() -> Self {
        Self(())
    }

    pub(crate) fn from_alloc(_inner: alloc::collections::TryReserveError) -> Self {
        Self(())
    }
}

impl fmt::Display for TryReserveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            "memory allocation failed: the capacity overflowed or the allocator reported an error",
        )
    }
}

impl Error for TryReserveError {}

/// Returned by `BitArray::try_insert` and `try_remove` when the index is past the end.
///
/// # Examples
///
/// ```
/// use engine::collections::BitArray;
///
/// let mut bits = BitArray::<1>::new();
/// let error = bits.try_insert(70).unwrap_err();
/// assert_eq!(error.index(), 70);
/// assert_eq!(error.capacity(), 64);
/// assert_eq!(
///     error.to_string(),
///     "bit index 70 is out of range: this set holds bits 0 to 63"
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BitIndexError {
    index: usize,
    capacity: usize,
}

impl BitIndexError {
    pub(crate) const fn new(index: usize, capacity: usize) -> Self {
        Self { index, capacity }
    }

    /// The index that was out of range.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// The number of bits the set holds: valid indices are `0..capacity`.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }
}

impl fmt::Display for BitIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bit index {} is out of range: ", self.index)?;
        match self.capacity.checked_sub(1) {
            Some(last) => write!(f, "this set holds bits 0 to {last}"),
            None => f.write_str("this set holds no bits"),
        }
    }
}

impl Error for BitIndexError {}

/// Which rule of the layers a [`BitSetInvariantError`] reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InvariantRule {
    /// A parent bit disagrees with the word it summarizes.
    ParentBit,
    /// A bit at or above `HierarchicalBitSet::MAX_BITS` is set.
    BitAtOrAboveMax,
    /// A bit above bit 3 of the top word is set.
    TopBitAboveThree,
}

/// Returned by `HierarchicalBitSet::check_invariants` when the layers break their rule.
///
/// A set built through the public API never produces it; it reports a bug in the engine.
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
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BitSetInvariantError {
    layer: usize,
    word: usize,
    rule: InvariantRule,
}

impl BitSetInvariantError {
    pub(crate) const fn new(layer: usize, word: usize, rule: InvariantRule) -> Self {
        Self { layer, word, rule }
    }

    /// The layer (0 to 5) of the word that breaks the rule.
    #[must_use]
    pub const fn layer(&self) -> usize {
        self.layer
    }

    /// The index of the word in its layer.
    #[must_use]
    pub const fn word(&self) -> usize {
        self.word
    }
}

impl fmt::Display for BitSetInvariantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "hierarchical bitset layer {}, word {}: ",
            self.layer, self.word
        )?;
        f.write_str(match self.rule {
            InvariantRule::ParentBit => {
                "a parent bit disagrees with whether its child word is non-zero"
            }
            InvariantRule::BitAtOrAboveMax => "a bit at or above MAX_BITS is set",
            InvariantRule::TopBitAboveThree => "a bit above bit 3 of the top word is set",
        })
    }
}

impl Error for BitSetInvariantError {}

/// Returned by `Join::try_new` when no required set is given.
///
/// A join of no required sets would be every index not excluded, which has no end.
///
/// # Examples
///
/// ```
/// use engine::collections::Join;
///
/// let error = Join::try_new(&[], &[]).unwrap_err();
/// assert!(error.to_string().contains("no required sets"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct EmptyJoinError(());

impl EmptyJoinError {
    pub(crate) const fn new() -> Self {
        Self(())
    }
}

impl fmt::Display for EmptyJoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            "no required sets were given; a join needs at least one set to draw indices from \
             (to iterate everything not excluded, pass the set of live indices)",
        )
    }
}

impl Error for EmptyJoinError {}
