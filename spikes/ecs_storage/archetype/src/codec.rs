//! The copy access path: a value's bytes in a byte column, written and read field by field
//! (M3.6 spec §4.4).

use crate::raw::RawLayout;

/// A type a workload stores in a byte column by copying: it names the column's layout and turns
/// a value into bytes and back.
///
/// Fields are written as little-endian bytes in declaration order. `decode(encode(x))` returns
/// `x` bit for bit, `NaN` payloads included.
pub trait Codec: Sized {
    /// The layout of a column of this type.
    const LAYOUT: RawLayout;

    /// Writes the value into `out`, which is exactly `LAYOUT.size()` bytes.
    fn encode(&self, out: &mut [u8]);

    /// Reads a value from `bytes`, which is exactly `LAYOUT.size()` bytes.
    fn decode(bytes: &[u8]) -> Self;
}

/// A layout in a constant, for [`Codec::LAYOUT`]. The arguments are literals, so the panic never
/// fires at run time: it is a compile-time error in a `const`.
///
/// # Panics
///
/// Panics (at compile time in a `const`) if the layout is invalid.
#[must_use]
pub(crate) const fn layout(size: usize, align: usize) -> RawLayout {
    match RawLayout::new(size, align) {
        Ok(layout) => layout,
        Err(_) => panic!("a codec's layout constant is invalid"),
    }
}

/// Writes `values` as little-endian `f32`s into `out`.
pub(crate) fn write_f32s(values: &[f32], out: &mut [u8]) {
    let (chunks, _) = out.as_chunks_mut::<4>();
    for (value, chunk) in values.iter().zip(chunks) {
        *chunk = value.to_le_bytes();
    }
}

/// Reads `N` little-endian `f32`s from `bytes`.
pub(crate) fn read_f32s<const N: usize>(bytes: &[u8]) -> [f32; N] {
    let mut values = [0.0_f32; N];
    let (chunks, _) = bytes.as_chunks::<4>();
    for (value, chunk) in values.iter_mut().zip(chunks) {
        *value = f32::from_le_bytes(*chunk);
    }
    values
}
