//! The typed, generational [`GenId`].

use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::num::NonZeroU32;

use crate::genid::error::InvalidGenIdBits;

/// A generational identifier without a type: the index of a slot and the generation the slot had
/// when the ID was issued.
///
/// This is the untyped core of [`GenId`]. Use it where the value type is not known at compile time:
/// a registry of runtime-defined components, a serialiser, or the C ABI. Converting to and from a
/// typed ID is explicit in both directions ([`GenId::untyped`] and [`GenId::from_untyped`]), and
/// nothing converts one typed ID into another.
///
/// The fields are private. Only [`to_bits`](Self::to_bits) is a contract; the struct layout is not.
/// There is no `Default`: no ID is a sensible default, and 0 is not a valid encoding.
///
/// # Examples
///
/// ```
/// use engine::genid::UntypedGenId;
///
/// let id = UntypedGenId::from_bits((3 << 32) | 2).unwrap();
/// assert_eq!((id.index(), id.generation().get()), (3, 2));
/// assert_eq!(format!("{id:?}"), "UntypedGenId(3v2)");
/// ```
///
/// There is no default ID:
///
/// ```compile_fail
/// use engine::genid::UntypedGenId;
///
/// let _ = UntypedGenId::default();
/// ```
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct UntypedGenId {
    index: u32,
    generation: NonZeroU32,
}

impl UntypedGenId {
    #[inline]
    pub(crate) const fn new(index: u32, generation: NonZeroU32) -> Self {
        Self { index, generation }
    }

    /// Slot index, for diagnostics and dense side-tables. Not unique over time:
    /// a freed slot is reused with a higher generation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine::genid::UntypedGenId;
    /// let id = UntypedGenId::from_bits((7 << 32) | 1).unwrap();
    /// assert_eq!(id.index(), 7);
    /// ```
    #[inline]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// Generation of the slot when this ID was issued.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine::genid::UntypedGenId;
    /// let id = UntypedGenId::from_bits((7 << 32) | 5).unwrap();
    /// assert_eq!(id.generation().get(), 5);
    /// ```
    #[inline]
    pub const fn generation(self) -> NonZeroU32 {
        self.generation
    }

    /// Stable encoding: `(index as u64) << 32 | generation as u64`. Never 0.
    ///
    /// The index is in the high half, so the numeric order of the bits equals
    /// the [`Ord`] order of IDs. **This layout is frozen**: it feeds PRNG
    /// stream ids, so it is part of deterministic output (ADR 0003). Changing
    /// it is a breaking change that requires an ADR.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine::genid::UntypedGenId;
    /// let id = UntypedGenId::from_bits((3 << 32) | 2).unwrap();
    /// assert_eq!(id.to_bits(), 0x0000_0003_0000_0002);
    /// ```
    #[inline]
    pub const fn to_bits(self) -> u64 {
        ((self.index as u64) << 32) | self.generation.get() as u64
    }

    /// Inverse of [`to_bits`](Self::to_bits). Returns `None` if and only if the
    /// generation (low) half is 0.
    ///
    /// The result is only meaningful for the map or allocator that issued the original ID.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine::genid::UntypedGenId;
    /// assert!(UntypedGenId::from_bits(0).is_none());
    /// assert!(UntypedGenId::from_bits(1).is_some());
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

impl From<UntypedGenId> for u64 {
    /// Same as [`UntypedGenId::to_bits`].
    #[inline]
    fn from(id: UntypedGenId) -> u64 {
        id.to_bits()
    }
}

impl TryFrom<u64> for UntypedGenId {
    type Error = InvalidGenIdBits;

    /// Same as [`UntypedGenId::from_bits`], with the rejected bits in the error.
    #[inline]
    fn try_from(bits: u64) -> Result<Self, InvalidGenIdBits> {
        Self::from_bits(bits).ok_or(InvalidGenIdBits::new(bits))
    }
}

impl PartialOrd for UntypedGenId {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for UntypedGenId {
    /// Orders by `(index, generation)`, identical to the numeric order of [`to_bits`](Self::to_bits).
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_bits().cmp(&other.to_bits())
    }
}

impl Hash for UntypedGenId {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.to_bits().hash(state);
    }
}

impl fmt::Debug for UntypedGenId {
    /// Prints `UntypedGenId(3v2)`: slot 3, generation 2.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UntypedGenId({}v{})", self.index, self.generation)
    }
}

/// A typed generational ID: 8 bytes, and `Option<GenId<T>>` is also 8 bytes.
///
/// An ID names a slot (`index`) and the slot's `generation` when the ID
/// was issued. When the slot is freed its generation is bumped, so every
/// outstanding ID becomes stale instead of silently pointing at a new value.
///
/// An ID refers to a `T` but never owns one, so it is `Copy`, `Send`, and
/// `Sync` whatever `T` is, and it implements every comparison trait without
/// requiring anything of `T`.
///
/// There is deliberately no `cast::<U>()`: it would silently defeat the type
/// safety the phantom type exists to provide. Type-erased uses (FFI,
/// serialization) go through [`untyped`](Self::untyped), [`to_bits`](Self::to_bits) and
/// [`from_bits`](Self::from_bits), where the type change is explicit at both ends.
///
/// # Examples
///
/// ```
/// use engine::genid::GenId;
///
/// struct Particle;
///
/// let h = GenId::<Particle>::from_bits((3 << 32) | 2).unwrap();
/// assert_eq!(h.index(), 3);
/// assert_eq!(h.generation().get(), 2);
/// assert_eq!(format!("{h:?}"), "GenId<Particle>(3v2)");
/// ```
///
/// Going to the untyped core and back names the type again:
///
/// ```
/// use engine::genid::{GenId, UntypedGenId};
///
/// struct Texture;
///
/// let typed = GenId::<Texture>::from_bits((3 << 32) | 2).unwrap();
/// let erased: UntypedGenId = typed.untyped();
/// assert_eq!((erased.index(), erased.generation().get()), (3, 2));
/// assert_eq!(format!("{erased:?}"), "UntypedGenId(3v2)");
///
/// let again = GenId::<Texture>::from_untyped(erased);
/// assert_eq!(again, typed);
/// assert_eq!(again.to_bits(), erased.to_bits()); // one frozen layout
/// ```
///
/// IDs for different types do not mix:
///
/// ```compile_fail
/// use engine::genid::GenId;
///
/// struct A;
/// struct B;
///
/// fn takes_b(_: GenId<B>) {}
/// takes_b(GenId::<A>::from_bits(1).unwrap());
/// ```
///
/// There is no `cast`:
///
/// ```compile_fail
/// use engine::genid::GenId;
///
/// let _ = GenId::<u8>::from_bits(1).unwrap().cast::<u16>();
/// ```
///
/// An untyped ID does not turn into a typed one on its own:
///
/// ```compile_fail
/// use engine::genid::{GenId, UntypedGenId};
///
/// let _: GenId<u8> = UntypedGenId::from_bits(1).unwrap();
/// ```
#[repr(transparent)]
pub struct GenId<T> {
    untyped: UntypedGenId,
    // `fn() -> T` keeps the ID covariant in `T`, `Send + Sync` for any `T`,
    // and free of drop-check implications: the ID never owns a `T`.
    _marker: PhantomData<fn() -> T>,
}

impl<T> GenId<T> {
    #[inline]
    pub(crate) const fn new(index: u32, generation: NonZeroU32) -> Self {
        Self::from_untyped(UntypedGenId::new(index, generation))
    }

    /// The same index and generation, without the type.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine::genid::GenId;
    /// let id = GenId::<u8>::from_bits((3 << 32) | 2).unwrap();
    /// assert_eq!(id.untyped().to_bits(), id.to_bits());
    /// ```
    #[inline]
    pub const fn untyped(self) -> UntypedGenId {
        self.untyped
    }

    /// The same index and generation, typed as `T`.
    ///
    /// The result is only meaningful for an ID that a [`GenIdMap<T>`](crate::genid::GenIdMap) or a
    /// [`GenIdAllocator<T>`](crate::genid::GenIdAllocator) issued: the caller states the type, and
    /// nothing checks it (the same caveat as [`from_bits`](Self::from_bits)).
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine::genid::{GenId, UntypedGenId};
    /// let erased = UntypedGenId::from_bits((3 << 32) | 2).unwrap();
    /// let typed = GenId::<u8>::from_untyped(erased);
    /// assert_eq!(typed.untyped(), erased);
    /// ```
    #[inline]
    pub const fn from_untyped(id: UntypedGenId) -> Self {
        Self {
            untyped: id,
            _marker: PhantomData,
        }
    }

    /// Slot index, for diagnostics and dense side-tables. Not unique over time:
    /// a freed slot is reused with a higher generation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine::genid::GenId;
    /// let h = GenId::<u8>::from_bits((7 << 32) | 1).unwrap();
    /// assert_eq!(h.index(), 7);
    /// ```
    #[inline]
    pub const fn index(self) -> u32 {
        self.untyped.index()
    }

    /// Generation of the slot when this ID was issued.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine::genid::GenId;
    /// let h = GenId::<u8>::from_bits((7 << 32) | 5).unwrap();
    /// assert_eq!(h.generation().get(), 5);
    /// ```
    #[inline]
    pub const fn generation(self) -> NonZeroU32 {
        self.untyped.generation()
    }

    /// Stable encoding, equal to [`self.untyped().to_bits()`](UntypedGenId::to_bits):
    /// `(index as u64) << 32 | generation as u64`. Never 0. **This layout is frozen**; see
    /// [`UntypedGenId::to_bits`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine::genid::GenId;
    /// let h = GenId::<u8>::from_bits((3 << 32) | 2).unwrap();
    /// assert_eq!(h.to_bits(), 0x0000_0003_0000_0002);
    /// ```
    #[inline]
    pub const fn to_bits(self) -> u64 {
        self.untyped.to_bits()
    }

    /// Inverse of [`to_bits`](Self::to_bits). Returns `None` if and only if the
    /// generation (low) half is 0.
    ///
    /// The result is only meaningful for the map that issued the original ID.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine::genid::GenId;
    /// assert!(GenId::<u8>::from_bits(0).is_none());
    /// assert!(GenId::<u8>::from_bits(1).is_some());
    /// ```
    #[inline]
    pub const fn from_bits(bits: u64) -> Option<Self> {
        match UntypedGenId::from_bits(bits) {
            Some(id) => Some(Self::from_untyped(id)),
            None => None,
        }
    }
}

impl<T> From<GenId<T>> for u64 {
    /// Same as [`GenId::to_bits`].
    #[inline]
    fn from(id: GenId<T>) -> u64 {
        id.to_bits()
    }
}

impl<T> TryFrom<u64> for GenId<T> {
    type Error = InvalidGenIdBits;

    /// Same as [`GenId::from_bits`], with the rejected bits in the error.
    #[inline]
    fn try_from(bits: u64) -> Result<Self, InvalidGenIdBits> {
        Self::from_bits(bits).ok_or(InvalidGenIdBits::new(bits))
    }
}

impl<T> Clone for GenId<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for GenId<T> {}

impl<T> PartialEq for GenId<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.untyped == other.untyped
    }
}

impl<T> Eq for GenId<T> {}

impl<T> PartialOrd for GenId<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for GenId<T> {
    /// Orders by `(index, generation)`, identical to the numeric order of [`to_bits`](GenId::to_bits).
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.untyped.cmp(&other.untyped)
    }
}

impl<T> Hash for GenId<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.untyped.hash(state);
    }
}

// The size and niche guarantees (spec 4.6), checked at compile time for a zero-sized and a large `T`.
const _: () = {
    assert!(size_of::<UntypedGenId>() == 8);
    assert!(size_of::<Option<UntypedGenId>>() == 8);
    assert!(size_of::<GenId<()>>() == 8);
    assert!(size_of::<Option<GenId<()>>>() == 8);
    assert!(size_of::<GenId<[u8; 4096]>>() == 8);
    assert!(size_of::<Option<GenId<[u8; 4096]>>>() == 8);
    assert!(align_of::<GenId<()>>() == align_of::<UntypedGenId>());
    assert!(align_of::<GenId<[u8; 4096]>>() == align_of::<UntypedGenId>());
};

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

impl<T> fmt::Debug for GenId<T> {
    /// Prints `GenId<Particle>(3v2)`: slot 3, generation 2, short type name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GenId<")?;
        write_short_type_name::<T>(f)?;
        write!(f, ">({}v{})", self.index(), self.generation())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::NonZeroU32;

    #[test]
    fn bits_layout_is_index_high_generation_low() {
        let h = GenId::<u8>::new(3, NonZeroU32::new(2).unwrap());
        assert_eq!(h.to_bits(), (3u64 << 32) | 2);
        assert_eq!(GenId::<u8>::from_bits(h.to_bits()), Some(h));
        assert_eq!(GenId::<u8>::from_bits(0), None);
        assert_eq!(GenId::<u8>::from_bits(7u64 << 32), None); // generation half zero
    }

    #[test]
    fn u64_conversions_match_to_bits_and_from_bits() {
        let h = GenId::<u8>::new(3, NonZeroU32::new(2).unwrap());
        assert_eq!(u64::from(h), h.to_bits());
        assert_eq!(GenId::<u8>::try_from(u64::from(h)), Ok(h));
        let err = GenId::<u8>::try_from(7_u64 << 32).unwrap_err();
        assert_eq!(err.bits(), 7_u64 << 32);
        let message = alloc::format!("{err}");
        assert!(message.contains("generation"), "message was {message}");
        assert!(
            message.contains("is not a generational ID"),
            "message was {message}"
        );
    }

    #[test]
    fn ord_matches_bits_order() {
        let mk = |i, g| GenId::<u8>::new(i, NonZeroU32::new(g).unwrap());
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
        let h = GenId::<Particle>::new(3, NonZeroU32::new(2).unwrap());
        assert_eq!(alloc::format!("{h:?}"), "GenId<Particle>(3v2)");
    }

    #[test]
    fn debug_strips_paths_inside_generics() {
        struct Particle;
        let h = GenId::<alloc::vec::Vec<(Particle, &'static str)>>::new(0, NonZeroU32::MIN);
        assert_eq!(alloc::format!("{h:?}"), "GenId<Vec<(Particle, &str)>>(0v1)");
    }

    #[test]
    fn untyped_bits_layout_is_index_high_generation_low() {
        let u = UntypedGenId::new(3, NonZeroU32::new(2).unwrap());
        assert_eq!(u.to_bits(), 0x0000_0003_0000_0002);
        assert_eq!(
            GenId::<u8>::from_untyped(u).to_bits(),
            0x0000_0003_0000_0002
        );
        assert_eq!(UntypedGenId::from_bits(u.to_bits()), Some(u));
        assert_eq!(UntypedGenId::from_bits(0), None);
        assert_eq!(UntypedGenId::from_bits(7_u64 << 32), None); // generation half zero
    }

    #[test]
    fn untyped_extreme_encodings() {
        let max = UntypedGenId::from_bits(u64::MAX).unwrap();
        assert_eq!((max.index(), max.generation().get()), (u32::MAX, u32::MAX));
        let min = UntypedGenId::from_bits(1).unwrap();
        assert_eq!((min.index(), min.generation().get()), (0, 1));
        assert_eq!(UntypedGenId::from_bits(1_u64 << 32), None);
        // The same answers through the typed ID.
        let typed_max = GenId::<u8>::from_bits(u64::MAX).unwrap();
        assert_eq!(typed_max.untyped(), max);
        assert_eq!(typed_max.to_bits(), u64::MAX);
        let typed_min = GenId::<u8>::from_bits(1).unwrap();
        assert_eq!(typed_min.untyped(), min);
        assert_eq!(GenId::<u8>::from_bits(1_u64 << 32), None);
    }

    #[test]
    fn untyped_debug_has_no_type_name() {
        let u = UntypedGenId::new(3, NonZeroU32::new(2).unwrap());
        assert_eq!(alloc::format!("{u:?}"), "UntypedGenId(3v2)");
    }

    #[test]
    fn untyped_u64_conversions() {
        let u = UntypedGenId::new(3, NonZeroU32::new(2).unwrap());
        assert_eq!(u64::from(u), u.to_bits());
        assert_eq!(UntypedGenId::try_from(u.to_bits()), Ok(u));
        let err = UntypedGenId::try_from(5_u64 << 32).unwrap_err();
        assert_eq!(err.bits(), 5_u64 << 32);
    }

    #[test]
    fn untyped_ord_matches_bits_order() {
        let mk = |i, g| UntypedGenId::new(i, NonZeroU32::new(g).unwrap());
        let mut ids = [mk(2, 3), mk(1, 9), mk(2, 1)];
        let mut by_bits = ids;
        ids.sort();
        by_bits.sort_by_key(|id| id.to_bits());
        assert_eq!(ids, by_bits);
        assert_eq!(ids, [mk(1, 9), mk(2, 1), mk(2, 3)]);
    }
}
