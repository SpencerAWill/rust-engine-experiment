//! The private hasher behind every collection.
//!
//! Hash values never reach any output: iteration order is insertion order. The
//! algorithm and seed are therefore not part of the contract.

use foldhash::fast::FixedState;

/// Arbitrary ("seed collection"); private and not part of the contract.
const SEED: u64 = 0x5EED_C011_EC71_0A5D;

#[cfg(not(test))]
pub(crate) type Hasher = FixedState;

/// The hasher the collections are built with, which tests can swap for a degenerate one.
#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) enum Hasher {
    /// The real one.
    Fold(FixedState),
    /// Every key hashes to 0: every observable behaviour must be unchanged.
    Constant,
}

#[cfg(test)]
#[derive(Clone, Debug, Default)]
pub(crate) struct ConstantHasher;

#[cfg(test)]
impl core::hash::Hasher for ConstantHasher {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, _bytes: &[u8]) {}
}

#[cfg(test)]
pub(crate) enum TestHasherState {
    Fold(<FixedState as core::hash::BuildHasher>::Hasher),
    Constant(ConstantHasher),
}

#[cfg(test)]
impl core::hash::Hasher for TestHasherState {
    fn finish(&self) -> u64 {
        match self {
            Self::Fold(h) => h.finish(),
            Self::Constant(h) => h.finish(),
        }
    }
    fn write(&mut self, bytes: &[u8]) {
        match self {
            Self::Fold(h) => h.write(bytes),
            Self::Constant(h) => h.write(bytes),
        }
    }
}

#[cfg(test)]
impl core::hash::BuildHasher for Hasher {
    type Hasher = TestHasherState;
    fn build_hasher(&self) -> TestHasherState {
        match self {
            Self::Fold(s) => TestHasherState::Fold(s.build_hasher()),
            Self::Constant => TestHasherState::Constant(ConstantHasher),
        }
    }
}

#[cfg(not(test))]
pub(crate) const fn default_hasher() -> Hasher {
    FixedState::with_seed(SEED)
}

#[cfg(test)]
pub(crate) const fn default_hasher() -> Hasher {
    Hasher::Fold(FixedState::with_seed(SEED))
}

#[cfg(test)]
pub(crate) const fn constant_hasher() -> Hasher {
    Hasher::Constant
}

#[cfg(test)]
mod tests {
    use core::hash::BuildHasher;

    use super::*;

    #[test]
    fn default_hasher_is_fixed() {
        assert_eq!(
            default_hasher().hash_one(42_u64),
            default_hasher().hash_one(42_u64)
        );
        assert_eq!(constant_hasher().hash_one("a"), 0);
        assert_eq!(constant_hasher().hash_one(7_u64), 0);
    }
}
