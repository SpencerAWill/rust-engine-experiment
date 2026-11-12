//! The typed, generational [`Handle`].

use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::num::NonZeroU32;

use crate::error::InvalidHandleBits;

/// A typed, generational identifier: 8 bytes, and `Option<Handle<T>>` is also 8 bytes.
///
/// A handle names a slot (`index`) and the slot's `generation` when the handle
/// was issued. When the slot is freed its generation is bumped, so every
/// outstanding handle becomes stale instead of silently pointing at a new value.
///
/// A handle refers to a `T` but never owns one, so it is `Copy`, `Send`, and
/// `Sync` whatever `T` is, and it implements every comparison trait without
/// requiring anything of `T`.
///
/// There is deliberately no `cast::<U>()`: it would silently defeat the type
/// safety the phantom type exists to provide. Type-erased uses (FFI,
/// serialization) go through [`to_bits`](Self::to_bits) and
/// [`from_bits`](Self::from_bits), where the type change is explicit at both ends.
///
/// # Examples
///
/// ```
/// use engine_handle::Handle;
///
/// struct Particle;
///
/// let h = Handle::<Particle>::from_bits((3 << 32) | 2).unwrap();
/// assert_eq!(h.index(), 3);
/// assert_eq!(h.generation().get(), 2);
/// assert_eq!(format!("{h:?}"), "Handle<Particle>(3v2)");
/// ```
///
/// Handles for different types do not mix:
///
/// ```compile_fail
/// use engine_handle::Handle;
///
/// struct A;
/// struct B;
///
/// fn takes_b(_: Handle<B>) {}
/// takes_b(Handle::<A>::from_bits(1).unwrap());
/// ```
pub struct Handle<T> {
    index: u32,
    generation: NonZeroU32,
    // `fn() -> T` keeps the handle covariant in `T`, `Send + Sync` for any `T`,
    // and free of drop-check implications: the handle never owns a `T`.
    _marker: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    #[inline]
    pub(crate) const fn new(index: u32, generation: NonZeroU32) -> Self {
        Self {
            index,
            generation,
            _marker: PhantomData,
        }
    }

    /// Slot index, for diagnostics and dense side-tables. Not unique over time:
    /// a freed slot is reused with a higher generation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::Handle;
    /// let h = Handle::<u8>::from_bits((7 << 32) | 1).unwrap();
    /// assert_eq!(h.index(), 7);
    /// ```
    #[inline]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// Generation of the slot when this handle was issued.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::Handle;
    /// let h = Handle::<u8>::from_bits((7 << 32) | 5).unwrap();
    /// assert_eq!(h.generation().get(), 5);
    /// ```
    #[inline]
    pub const fn generation(self) -> NonZeroU32 {
        self.generation
    }

    /// Stable encoding: `(index as u64) << 32 | generation as u64`. Never 0.
    ///
    /// The index is in the high half, so the numeric order of the bits equals
    /// the [`Ord`] order of handles. **This layout is frozen**: it feeds PRNG
    /// stream ids, so it is part of deterministic output (ADR 0003). Changing
    /// it is a breaking change that requires an ADR.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::Handle;
    /// let h = Handle::<u8>::from_bits((3 << 32) | 2).unwrap();
    /// assert_eq!(h.to_bits(), 0x0000_0003_0000_0002);
    /// ```
    #[inline]
    pub const fn to_bits(self) -> u64 {
        ((self.index as u64) << 32) | self.generation.get() as u64
    }

    /// Inverse of [`to_bits`](Self::to_bits). Returns `None` if and only if the
    /// generation (low) half is 0.
    ///
    /// The result is only meaningful for the pool that issued the original handle.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_handle::Handle;
    /// assert!(Handle::<u8>::from_bits(0).is_none());
    /// assert!(Handle::<u8>::from_bits(1).is_some());
    /// ```
    #[inline]
    pub const fn from_bits(bits: u64) -> Option<Self> {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "intentional split into halves"
        )]
        let (index, generation) = ((bits >> 32) as u32, bits as u32);
        match NonZeroU32::new(generation) {
            Some(generation) => Some(Self::new(index, generation)),
            None => None,
        }
    }
}

impl<T> From<Handle<T>> for u64 {
    /// Same as [`Handle::to_bits`].
    #[inline]
    fn from(handle: Handle<T>) -> u64 {
        handle.to_bits()
    }
}

impl<T> TryFrom<u64> for Handle<T> {
    type Error = InvalidHandleBits;

    /// Same as [`Handle::from_bits`], with the rejected bits in the error.
    #[inline]
    fn try_from(bits: u64) -> Result<Self, InvalidHandleBits> {
        Self::from_bits(bits).ok_or(InvalidHandleBits::new(bits))
    }
}

impl<T> Clone for Handle<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Handle<T> {}

impl<T> PartialEq for Handle<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl<T> Eq for Handle<T> {}

impl<T> PartialOrd for Handle<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Handle<T> {
    /// Orders by `(index, generation)`, identical to the numeric order of [`to_bits`](Handle::to_bits).
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_bits().cmp(&other.to_bits())
    }
}

impl<T> Hash for Handle<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.to_bits().hash(state);
    }
}

/// Writes `T`'s type name with every `path::` prefix removed
/// (`alloc::vec::Vec<a::B>` becomes `Vec<B>`), without allocating.
pub(crate) fn write_short_type_name<T: ?Sized>(f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let name = core::any::type_name::<T>();
    let bytes = name.as_bytes();
    let (mut segment, mut i) = (0, 0);
    while i < bytes.len() {
        match bytes[i] {
            b':' if bytes.get(i + 1) == Some(&b':') => {
                segment = i + 2;
                i += 2;
                continue;
            }
            b'<' | b'>' | b',' | b' ' | b'[' | b']' | b'(' | b')' | b'&' | b';' | b'*' => {
                f.write_str(&name[segment..=i])?;
                segment = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    f.write_str(&name[segment..])
}

impl<T> fmt::Debug for Handle<T> {
    /// Prints `Handle<Particle>(3v2)`: slot 3, generation 2, short type name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Handle<")?;
        write_short_type_name::<T>(f)?;
        write!(f, ">({}v{})", self.index, self.generation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::NonZeroU32;

    #[test]
    fn bits_layout_is_index_high_generation_low() {
        let h = Handle::<u8>::new(3, NonZeroU32::new(2).unwrap());
        assert_eq!(h.to_bits(), (3u64 << 32) | 2);
        assert_eq!(Handle::<u8>::from_bits(h.to_bits()), Some(h));
        assert_eq!(Handle::<u8>::from_bits(0), None);
        assert_eq!(Handle::<u8>::from_bits(7u64 << 32), None); // generation half zero
    }

    #[test]
    fn u64_conversions_match_to_bits_and_from_bits() {
        let h = Handle::<u8>::new(3, NonZeroU32::new(2).unwrap());
        assert_eq!(u64::from(h), h.to_bits());
        assert_eq!(Handle::<u8>::try_from(u64::from(h)), Ok(h));
        let err = Handle::<u8>::try_from(7_u64 << 32).unwrap_err();
        assert_eq!(err.bits(), 7_u64 << 32);
        let message = alloc::format!("{err}");
        assert!(message.contains("generation"), "message was {message}");
    }

    #[test]
    fn ord_matches_bits_order() {
        let mk = |i, g| Handle::<u8>::new(i, NonZeroU32::new(g).unwrap());
        let mut hs = [mk(2, 3), mk(1, 9), mk(2, 1)];
        let mut by_bits = hs;
        hs.sort();
        by_bits.sort_by_key(|h| h.to_bits());
        assert_eq!(hs, by_bits);
        assert_eq!(hs, [mk(1, 9), mk(2, 1), mk(2, 3)]);
    }

    #[test]
    fn debug_uses_short_type_name() {
        struct Particle;
        let h = Handle::<Particle>::new(3, NonZeroU32::new(2).unwrap());
        assert_eq!(alloc::format!("{h:?}"), "Handle<Particle>(3v2)");
    }

    #[test]
    fn debug_strips_paths_inside_generics() {
        struct Particle;
        let h = Handle::<alloc::vec::Vec<(Particle, &'static str)>>::new(0, NonZeroU32::MIN);
        assert_eq!(
            alloc::format!("{h:?}"),
            "Handle<Vec<(Particle, &str)>>(0v1)"
        );
    }
}
