//! Error types.

use core::error::Error;
use core::fmt;

/// Returned by [`GenIdMap::try_insert`](crate::GenIdMap::try_insert) when
/// the map is full. Carries the rejected value back, so nothing is lost.
///
/// # Examples
///
/// ```
/// use engine_genid::CapacityError;
///
/// fn describe(e: CapacityError<String>) -> (String, String) {
///     (e.to_string(), e.into_inner())
/// }
/// # let _ = describe;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CapacityError<T> {
    value: T,
    limit: u32,
}

impl<T> CapacityError<T> {
    pub(crate) const fn new(value: T, limit: u32) -> Self {
        Self { value, limit }
    }

    /// The most slots the map that rejected the value can hold.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_genid::GenIdMap;
    /// let mut map = GenIdMap::new();
    /// if let Err(e) = map.try_insert(1) {
    ///     assert!(e.limit() > 0);
    /// }
    /// ```
    pub const fn limit(&self) -> u32 {
        self.limit
    }

    /// Returns the value that could not be inserted.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_genid::GenIdMap;
    /// // A map only fills up after 2³¹−2 slots, so this just shows the API shape:
    /// let mut map = GenIdMap::new();
    /// match map.try_insert(7) {
    ///     Ok(id) => assert_eq!(map[id], 7),
    ///     Err(e) => assert_eq!(e.into_inner(), 7),
    /// }
    /// ```
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> fmt::Display for CapacityError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GenIdMap is full: all {} slot indices are in use or retired; remove values or use another map",
            self.limit
        )
    }
}

impl<T: fmt::Debug> Error for CapacityError<T> {}

/// Returned by `GenId::try_from(u64)` when the generation (low) half of the bits is zero, which
/// no ID ever has.
///
/// # Examples
///
/// ```
/// use engine_genid::GenId;
///
/// let err = GenId::<u8>::try_from(5_u64 << 32).unwrap_err();
/// assert_eq!(err.bits(), 5_u64 << 32);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct InvalidGenIdBits {
    bits: u64,
}

impl InvalidGenIdBits {
    pub(crate) const fn new(bits: u64) -> Self {
        Self { bits }
    }

    /// The rejected bits.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_genid::GenId;
    /// assert_eq!(GenId::<u8>::try_from(0_u64).unwrap_err().bits(), 0);
    /// ```
    pub const fn bits(&self) -> u64 {
        self.bits
    }
}

impl fmt::Display for InvalidGenIdBits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:#018x} is not a generational ID: its generation (low 32 bits) is zero",
            self.bits
        )
    }
}

impl Error for InvalidGenIdBits {}

/// An internal invariant of a map or allocator does not hold, which would be a bug in this
/// crate. Returned by the `check_invariants` methods, which exist in tests and with the
/// `test-invariants` feature.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "test-invariants")]
/// # {
/// use engine_genid::GenIdMap;
///
/// let mut map = GenIdMap::new();
/// let _ = map.insert(1);
/// assert!(map.check_invariants().is_ok());
/// # }
/// ```
#[cfg(any(test, feature = "test-invariants"))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InvariantViolation {
    message: alloc::string::String,
}

#[cfg(any(test, feature = "test-invariants"))]
impl InvariantViolation {
    pub(crate) fn new(message: alloc::string::String) -> Self {
        Self { message }
    }
}

#[cfg(any(test, feature = "test-invariants"))]
impl fmt::Display for InvariantViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invariant violated: {}", self.message)
    }
}

#[cfg(any(test, feature = "test-invariants"))]
impl Error for InvariantViolation {}
