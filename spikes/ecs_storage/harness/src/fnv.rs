//! A small builder over the 64-bit FNV-1a hash.

use engine::prelude::{Vec2, Vec3};

const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

/// A 64-bit FNV-1a hash under construction, with the constants of the M3.0 spec §4.3.
///
/// Every per-entity hash of the workloads is a chain of these calls ending in [`finish`].
/// Multi-byte values are hashed as their little-endian bytes.
///
/// [`finish`]: Fnv1a::finish
#[derive(Clone, Copy, Debug)]
pub struct Fnv1a(u64);

impl Fnv1a {
    /// Starts a hash at the FNV offset basis.
    #[must_use]
    pub const fn new() -> Self {
        Self(OFFSET)
    }

    #[must_use]
    fn bytes(self, bytes: &[u8]) -> Self {
        let mut hash = self.0;
        for &b in bytes {
            hash = (hash ^ u64::from(b)).wrapping_mul(PRIME);
        }
        Self(hash)
    }

    /// Hashes one byte.
    #[must_use]
    pub fn u8(self, v: u8) -> Self {
        self.bytes(&[v])
    }

    /// Hashes the 4 little-endian bytes of `v`.
    #[must_use]
    pub fn u32(self, v: u32) -> Self {
        self.bytes(&v.to_le_bytes())
    }

    /// Hashes the 8 little-endian bytes of `v`.
    #[must_use]
    pub fn u64(self, v: u64) -> Self {
        self.bytes(&v.to_le_bytes())
    }

    /// Hashes the 4 little-endian bytes of `v.to_bits()`.
    #[must_use]
    pub fn f32(self, v: f32) -> Self {
        self.u32(v.to_bits())
    }

    /// Hashes `v.x` then `v.y`.
    #[must_use]
    pub fn vec2(self, v: Vec2) -> Self {
        self.f32(v.x).f32(v.y)
    }

    /// Hashes `v.x`, `v.y`, then `v.z`.
    #[must_use]
    pub fn vec3(self, v: Vec3) -> Self {
        self.f32(v.x).f32(v.y).f32(v.z)
    }

    /// The hash of everything fed in so far.
    #[must_use]
    pub const fn finish(self) -> u64 {
        self.0
    }
}

impl Default for Fnv1a {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_matches_the_published_vectors() {
        assert_eq!(Fnv1a::new().finish(), 0xcbf2_9ce4_8422_2325);
        assert_eq!(Fnv1a::new().u8(b'a').finish(), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn f32_hashes_its_bits() {
        assert_eq!(
            Fnv1a::new().f32(1.5).finish(),
            Fnv1a::new().u32(1.5_f32.to_bits()).finish()
        );
        let v = Vec3::new(1.0, -2.0, 3.5);
        assert_eq!(
            Fnv1a::new().vec3(v).finish(),
            Fnv1a::new().f32(v.x).f32(v.y).f32(v.z).finish()
        );
        let w = Vec2::new(1.0, -2.0);
        assert_eq!(
            Fnv1a::new().vec2(w).finish(),
            Fnv1a::new().f32(w.x).f32(w.y).finish()
        );
    }
}
