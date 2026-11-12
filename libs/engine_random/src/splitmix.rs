//! `SplitMix64`, used only to expand seeds and derive streams.
//!
//! From Steele, Lea & Flood, "Fast Splittable Pseudorandom Number Generators" (2014),
//! as published in the constants below.

/// The `SplitMix64` output function (a bijection on `u64`).
#[inline]
pub(crate) const fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// `SplitMix64` generator: a counter advanced by the golden-ratio constant, then mixed.
pub(crate) struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    #[inline]
    pub(crate) const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    #[inline]
    pub(crate) const fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix64(self.state)
    }
}
