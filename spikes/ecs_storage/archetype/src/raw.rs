//! Runtime-registered components: a layout and the byte column that stores values of it (M3.6).
//!
//! A [`ByteColumn`] keeps `len` values of one [`RawLayout`] back to back in a `Vec` of 16-byte,
//! 16-aligned blocks, so the start of the column is aligned for every layout and
//! [`RawColumn::cast`] can view it as a slice of any `Pod` type of that layout. The column only
//! implements the same moves as `Column<T>`; the views offer no way to push, remove or resize.

use core::any::Any;
use core::fmt;
use core::slice::{ChunksExact, ChunksExactMut};

use bytemuck::{Pod, Zeroable};

use crate::table::{ColumnDesc, ErasedColumn};

/// The size and alignment of a runtime-registered component's values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawLayout {
    size: usize,
    align: usize,
}

/// Why a [`RawLayout`] was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutError {
    /// The size was 0.
    ZeroSize,
    /// The alignment was not a power of two.
    AlignNotPowerOfTwo {
        /// The rejected alignment.
        align: usize,
    },
    /// The alignment was above 16, the alignment of the column's storage.
    AlignTooLarge {
        /// The rejected alignment.
        align: usize,
    },
    /// The size was not a multiple of the alignment.
    SizeNotMultipleOfAlign {
        /// The rejected size.
        size: usize,
        /// The alignment it must be a multiple of.
        align: usize,
    },
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::ZeroSize => write!(f, "a component layout needs a size of at least 1 byte"),
            Self::AlignNotPowerOfTwo { align } => {
                write!(
                    f,
                    "alignment {align} is not a power of two; use 1, 2, 4, 8 or 16"
                )
            }
            Self::AlignTooLarge { align } => {
                write!(
                    f,
                    "alignment {align} is above 16, the most a byte column supports"
                )
            }
            Self::SizeNotMultipleOfAlign { size, align } => write!(
                f,
                "size {size} is not a multiple of alignment {align}; round the size up"
            ),
        }
    }
}

impl core::error::Error for LayoutError {}

impl RawLayout {
    /// The largest supported alignment: the alignment of the column's storage blocks.
    const MAX_ALIGN: usize = 16;

    /// Checks `size` and `align` and returns the layout.
    ///
    /// # Errors
    ///
    /// Returns a [`LayoutError`] if `size` is 0, `align` is not a power of two or is above 16,
    /// or `size` is not a multiple of `align`.
    pub const fn new(size: usize, align: usize) -> Result<Self, LayoutError> {
        if size == 0 {
            return Err(LayoutError::ZeroSize);
        }
        if !align.is_power_of_two() {
            return Err(LayoutError::AlignNotPowerOfTwo { align });
        }
        if align > Self::MAX_ALIGN {
            return Err(LayoutError::AlignTooLarge { align });
        }
        if !size.is_multiple_of(align) {
            return Err(LayoutError::SizeNotMultipleOfAlign { size, align });
        }
        Ok(Self { size, align })
    }

    /// The size of one value in bytes.
    pub const fn size(self) -> usize {
        self.size
    }

    /// The alignment of one value in bytes.
    pub const fn align(self) -> usize {
        self.align
    }
}

/// Why a column could not be viewed as a slice of some type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastError {
    /// The type's size differs from the layout's.
    SizeMismatch {
        /// The layout's size.
        expected: usize,
        /// The type's size.
        got: usize,
    },
    /// The type's alignment is larger than the layout's.
    AlignTooLarge {
        /// The layout's alignment.
        column: usize,
        /// The type's alignment.
        requested: usize,
    },
}

impl fmt::Display for CastError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::SizeMismatch { expected, got } => write!(
                f,
                "the column holds {expected}-byte values but the type is {got} bytes; \
                 cast to a type of the column's size"
            ),
            Self::AlignTooLarge { column, requested } => write!(
                f,
                "the column's values are aligned to {column} bytes but the type needs {requested}; \
                 register the component with an alignment of at least {requested}"
            ),
        }
    }
}

impl core::error::Error for CastError {}

/// The storage unit of a byte column: 16 bytes, 16-aligned.
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Block([u8; 16]);

const BLOCK: usize = 16;

/// `len` values of one layout back to back. Bytes past `len * size` are zero.
#[derive(Debug)]
pub(crate) struct ByteColumn {
    layout: RawLayout,
    len: usize,
    blocks: Vec<Block>,
}

impl ByteColumn {
    pub(crate) fn new(layout: RawLayout) -> Self {
        Self {
            layout,
            len: 0,
            blocks: Vec::new(),
        }
    }

    fn all_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.blocks)
    }

    fn all_bytes_mut(&mut self) -> &mut [u8] {
        bytemuck::cast_slice_mut(&mut self.blocks)
    }

    fn bytes(&self) -> &[u8] {
        &self.all_bytes()[..self.len * self.layout.size]
    }

    fn bytes_mut(&mut self) -> &mut [u8] {
        let n = self.len * self.layout.size;
        &mut self.all_bytes_mut()[..n]
    }

    /// Appends one value.
    pub(crate) fn push(&mut self, bytes: &[u8]) {
        debug_assert_eq!(
            bytes.len(),
            self.layout.size,
            "ByteColumn::push got a value of the wrong size; this is a bug in the prototype"
        );
        let start = self.len * self.layout.size;
        let need = (start + self.layout.size).div_ceil(BLOCK);
        if need > self.blocks.len() {
            self.blocks.reserve(need - self.blocks.len());
            self.blocks.resize(need, Block::zeroed());
        }
        let end = start + self.layout.size;
        self.all_bytes_mut()[start..end].copy_from_slice(bytes);
        self.len += 1;
    }

    /// Replaces the value at `row`.
    pub(crate) fn overwrite(&mut self, row: usize, bytes: &[u8]) {
        debug_assert_eq!(
            bytes.len(),
            self.layout.size,
            "ByteColumn::overwrite got a value of the wrong size; this is a bug in the prototype"
        );
        let size = self.layout.size;
        self.bytes_mut()[row * size..(row + 1) * size].copy_from_slice(bytes);
    }

    /// Whether every byte past the last value is zero, and no block is unused.
    pub(crate) fn tail_is_zero(&self) -> bool {
        let used = self.len * self.layout.size;
        self.blocks.len() == used.div_ceil(BLOCK)
            && self.all_bytes()[used..].iter().all(|b| *b == 0)
    }

    /// Writes a nonzero byte past the last value, or appends a nonzero block when the values
    /// fill their blocks exactly. For tests of `check_invariants`.
    #[cfg(test)]
    pub(crate) fn dirty_the_tail(&mut self) {
        let used = self.len * self.layout.size;
        if used < self.blocks.len() * BLOCK {
            self.all_bytes_mut()[used] = 0xFF;
        } else {
            self.blocks.push(Block([0xFF; 16]));
        }
    }

    /// The column behind `column`.
    ///
    /// # Panics
    ///
    /// Panics if `column` is a typed column: a raw component id names a byte column (T2).
    pub(crate) fn from_erased(column: &dyn ErasedColumn) -> &Self {
        column
            .as_any()
            .downcast_ref()
            .expect("T2: a raw component id names a byte column")
    }

    /// The column behind `column`, exclusive. Panics as [`ByteColumn::from_erased`].
    pub(crate) fn from_erased_mut(column: &mut dyn ErasedColumn) -> &mut Self {
        column
            .as_any_mut()
            .downcast_mut()
            .expect("T2: a raw component id names a byte column")
    }

    /// Drops the last value after zeroing it, and releases blocks past the new end.
    fn pop_last(&mut self) {
        let size = self.layout.size;
        let last = self.len - 1;
        self.all_bytes_mut()[last * size..(last + 1) * size].fill(0);
        self.len = last;
        self.blocks.truncate((last * size).div_ceil(BLOCK));
    }
}

impl ErasedColumn for ByteColumn {
    fn len(&self) -> usize {
        self.len
    }

    fn describe(&self) -> ColumnDesc {
        ColumnDesc::Raw(self.layout)
    }

    fn value_type_name(&self) -> &'static str {
        "<raw>"
    }

    fn swap_remove_drop(&mut self, row: usize) {
        assert!(
            row < self.len,
            "ByteColumn::swap_remove_drop: row {row} is out of range for {} values",
            self.len
        );
        let size = self.layout.size;
        let last = self.len - 1;
        if row != last {
            self.all_bytes_mut()
                .copy_within(last * size..(last + 1) * size, row * size);
        }
        self.pop_last();
    }

    fn swap_remove_into(&mut self, row: usize, dst: &mut dyn ErasedColumn) {
        let dst = Self::from_erased_mut(dst);
        assert_eq!(
            dst.layout, self.layout,
            "T2: a byte column moves rows only into a byte column of the same layout"
        );
        assert!(
            row < self.len,
            "ByteColumn::swap_remove_into: row {row} is out of range for {} values",
            self.len
        );
        let size = self.layout.size;
        dst.push(&self.bytes()[row * size..(row + 1) * size]);
        self.swap_remove_drop(row);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn check_cast<T>(layout: RawLayout) -> Result<(), CastError> {
    if size_of::<T>() != layout.size {
        return Err(CastError::SizeMismatch {
            expected: layout.size,
            got: size_of::<T>(),
        });
    }
    if align_of::<T>() > layout.align {
        return Err(CastError::AlignTooLarge {
            column: layout.align,
            requested: align_of::<T>(),
        });
    }
    Ok(())
}

/// The error for a `bytemuck` failure after [`check_cast`] passed. It cannot happen, since the
/// storage is 16-aligned and the length is a whole number of `T`; reporting it as a size
/// mismatch keeps the cast panic-free.
fn unreachable_cast<T>(layout: RawLayout) -> CastError {
    CastError::SizeMismatch {
        expected: layout.size,
        got: size_of::<T>(),
    }
}

/// A shared view of one byte column: its values as bytes, or cast to a `Pod` type.
///
/// It cannot push, remove or resize.
#[derive(Debug, Clone, Copy)]
pub struct RawColumn<'t>(&'t ByteColumn);

impl<'t> RawColumn<'t> {
    pub(crate) fn new(column: &'t ByteColumn) -> Self {
        Self(column)
    }

    /// The number of values.
    pub fn len(&self) -> usize {
        self.0.len
    }

    /// Whether the column holds no values.
    pub fn is_empty(&self) -> bool {
        self.0.len == 0
    }

    /// The layout of the column's values.
    pub fn layout(&self) -> RawLayout {
        self.0.layout
    }

    /// The bytes of value `row`, or `None` past the end.
    pub fn get(&self, row: usize) -> Option<&'t [u8]> {
        self.rows().nth(row)
    }

    /// The bytes of each value in row order.
    pub fn rows(&self) -> ChunksExact<'t, u8> {
        self.0.bytes().chunks_exact(self.0.layout.size)
    }

    /// The column as a slice of `T`, one element per row.
    ///
    /// # Errors
    ///
    /// Returns a [`CastError`] if `size_of::<T>()` differs from the layout's size or
    /// `align_of::<T>()` exceeds its alignment.
    pub fn cast<T: Pod>(&self) -> Result<&'t [T], CastError> {
        check_cast::<T>(self.0.layout)?;
        bytemuck::try_cast_slice(self.0.bytes()).map_err(|_| unreachable_cast::<T>(self.0.layout))
    }
}

/// An exclusive view of one byte column: [`RawColumn`]'s reads plus writes in place.
///
/// It cannot push, remove or resize.
#[derive(Debug)]
pub struct RawColumnMut<'t>(&'t mut ByteColumn);

impl<'t> RawColumnMut<'t> {
    pub(crate) fn new(column: &'t mut ByteColumn) -> Self {
        Self(column)
    }

    /// The same column as a shared view that lives as long as the borrow.
    pub(crate) fn into_shared(self) -> RawColumn<'t> {
        RawColumn::new(self.0)
    }

    /// The number of values.
    pub fn len(&self) -> usize {
        self.0.len
    }

    /// Whether the column holds no values.
    pub fn is_empty(&self) -> bool {
        self.0.len == 0
    }

    /// The layout of the column's values.
    pub fn layout(&self) -> RawLayout {
        self.0.layout
    }

    /// The bytes of value `row`, or `None` past the end.
    pub fn get(&self, row: usize) -> Option<&[u8]> {
        RawColumn(self.0).get(row)
    }

    /// The bytes of value `row`, exclusive, or `None` past the end.
    pub fn get_mut(&mut self, row: usize) -> Option<&mut [u8]> {
        let size = self.0.layout.size;
        self.0.bytes_mut().chunks_exact_mut(size).nth(row)
    }

    /// The bytes of each value in row order.
    pub fn rows(&self) -> ChunksExact<'_, u8> {
        RawColumn(self.0).rows()
    }

    /// The bytes of each value in row order, exclusive.
    pub fn rows_mut(self) -> ChunksExactMut<'t, u8> {
        let size = self.0.layout.size;
        self.0.bytes_mut().chunks_exact_mut(size)
    }

    /// The column as a slice of `T`, one element per row.
    ///
    /// # Errors
    ///
    /// As [`RawColumn::cast`].
    pub fn cast<T: Pod>(&self) -> Result<&[T], CastError> {
        RawColumn(self.0).cast()
    }

    /// The column as an exclusive slice of `T`, one element per row.
    ///
    /// # Errors
    ///
    /// As [`RawColumn::cast`].
    pub fn cast_mut<T: Pod>(self) -> Result<&'t mut [T], CastError> {
        check_cast::<T>(self.0.layout)?;
        let layout = self.0.layout;
        bytemuck::try_cast_slice_mut(self.0.bytes_mut()).map_err(|_| unreachable_cast::<T>(layout))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::{Column, ErasedColumn};
    use bytemuck::{Pod, Zeroable};

    fn layout(size: usize, align: usize) -> RawLayout {
        RawLayout::new(size, align).unwrap()
    }

    fn column_of(size: usize, align: usize, rows: &[Vec<u8>]) -> ByteColumn {
        let mut c = ByteColumn::new(layout(size, align));
        for r in rows {
            c.push(r);
        }
        c
    }

    #[test]
    fn zero_size_is_rejected() {
        assert_eq!(RawLayout::new(0, 1), Err(LayoutError::ZeroSize));
    }

    #[test]
    fn non_power_of_two_align_is_rejected() {
        assert_eq!(
            RawLayout::new(6, 3),
            Err(LayoutError::AlignNotPowerOfTwo { align: 3 })
        );
        assert_eq!(
            RawLayout::new(4, 0),
            Err(LayoutError::AlignNotPowerOfTwo { align: 0 })
        );
    }

    #[test]
    fn align_above_16_is_rejected() {
        assert_eq!(
            RawLayout::new(32, 32),
            Err(LayoutError::AlignTooLarge { align: 32 })
        );
    }

    #[test]
    fn size_not_a_multiple_of_align_is_rejected() {
        assert_eq!(
            RawLayout::new(6, 4),
            Err(LayoutError::SizeNotMultipleOfAlign { size: 6, align: 4 })
        );
    }

    #[test]
    fn align_16_is_accepted() {
        let l = RawLayout::new(16, 16).unwrap();
        assert_eq!((l.size(), l.align()), (16, 16));
    }

    #[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq)]
    #[repr(C, align(16))]
    struct A16([u8; 16]);

    #[test]
    fn cast_succeeds_at_every_alignment() {
        for (align, ok) in [(1usize, 0u8), (2, 1), (4, 2), (8, 3), (16, 4)] {
            let mut c = ByteColumn::new(layout(align, align));
            let mut row = vec![7u8; align];
            row[0] = 1;
            c.push(&row);
            c.push(&row);
            let v = RawColumn::new(&c);
            let n = match ok {
                0 => v.cast::<u8>().map(<[u8]>::len),
                1 => v.cast::<u16>().map(<[u16]>::len),
                2 => v.cast::<u32>().map(<[u32]>::len),
                3 => v.cast::<u64>().map(<[u64]>::len),
                _ => v.cast::<A16>().map(<[A16]>::len),
            };
            assert_eq!(n, Ok(2), "align {align}");
        }
    }

    #[test]
    fn cast_rejects_a_size_mismatch() {
        let c = column_of(4, 4, &[vec![0; 4]]);
        assert_eq!(
            RawColumn::new(&c).cast::<u16>(),
            Err(CastError::SizeMismatch {
                expected: 4,
                got: 2
            })
        );
    }

    #[test]
    fn cast_rejects_a_larger_alignment() {
        let mut c = column_of(4, 1, &[vec![0; 4]]);
        let err = CastError::AlignTooLarge {
            column: 1,
            requested: 4,
        };
        assert_eq!(RawColumn::new(&c).cast::<u32>(), Err(err));
        assert_eq!(RawColumnMut::new(&mut c).cast_mut::<u32>(), Err(err));
    }

    #[test]
    fn cast_lengths_are_the_row_count() {
        let mut c = ByteColumn::new(layout(12, 4));
        assert_eq!(RawColumn::new(&c).cast::<[f32; 3]>().unwrap().len(), 0);
        for i in 0..5u8 {
            c.push(&[i; 12]);
            let n = usize::from(i) + 1;
            assert_eq!(RawColumn::new(&c).cast::<[f32; 3]>().unwrap().len(), n);
            assert_eq!(
                RawColumnMut::new(&mut c)
                    .cast_mut::<[f32; 3]>()
                    .unwrap()
                    .len(),
                n
            );
            assert_eq!(RawColumn::new(&c).rows().len(), n);
        }
    }

    #[test]
    fn cast_mut_writes_reach_the_bytes() {
        let mut c = column_of(4, 4, &[vec![0; 4], vec![0; 4]]);
        RawColumnMut::new(&mut c).cast_mut::<u32>().unwrap()[1] = 0x0102_0304;
        assert_eq!(
            RawColumn::new(&c).get(1),
            Some(&0x0102_0304u32.to_ne_bytes()[..])
        );
        assert_eq!(RawColumn::new(&c).get(2), None);
    }

    #[test]
    fn swap_remove_drop_zeroes_the_tail() {
        let mut c = column_of(12, 4, &[vec![1; 12], vec![2; 12], vec![3; 12]]);
        c.swap_remove_drop(0);
        assert_eq!(c.len(), 2);
        let v = RawColumn::new(&c);
        assert_eq!(v.get(0), Some(&[3u8; 12][..]));
        assert_eq!(v.get(1), Some(&[2u8; 12][..]));
        assert!(c.tail_is_zero());
        c.swap_remove_drop(1);
        c.swap_remove_drop(0);
        assert_eq!(c.len(), 0);
        assert!(c.tail_is_zero());
    }

    #[test]
    fn swap_remove_into_moves_the_bytes_and_zeroes_the_tail() {
        let mut src = column_of(12, 4, &[vec![1; 12], vec![2; 12], vec![3; 12]]);
        let mut dst = column_of(12, 4, &[vec![9; 12]]);
        src.swap_remove_into(0, &mut dst);
        assert_eq!(src.len(), 2);
        assert_eq!(dst.len(), 2);
        assert_eq!(RawColumn::new(&dst).get(1), Some(&[1u8; 12][..]));
        assert_eq!(RawColumn::new(&src).get(0), Some(&[3u8; 12][..]));
        assert!(src.tail_is_zero() && dst.tail_is_zero());
        src.swap_remove_into(1, &mut dst);
        assert_eq!(RawColumn::new(&dst).get(2), Some(&[2u8; 12][..]));
        assert!(src.tail_is_zero() && dst.tail_is_zero());
    }

    #[test]
    #[should_panic(expected = "T2: a raw component id names a byte column")]
    fn swap_remove_into_a_typed_column_panics() {
        let mut src = column_of(4, 4, &[vec![1; 4]]);
        let mut dst: Box<dyn ErasedColumn> = Box::new(Column::<u32>::new());
        src.swap_remove_into(0, &mut *dst);
    }

    #[test]
    fn from_erased_finds_the_byte_column() {
        let mut boxed: Box<dyn ErasedColumn> = Box::new(column_of(4, 4, &[vec![1; 4]]));
        assert_eq!(ByteColumn::from_erased(&*boxed).len, 1);
        ByteColumn::from_erased_mut(&mut *boxed).push(&[2; 4]);
        assert_eq!(boxed.len(), 2);
    }

    #[test]
    #[should_panic(expected = "T2: a raw component id names a byte column")]
    fn from_erased_of_a_typed_column_panics() {
        let typed: Box<dyn ErasedColumn> = Box::new(Column::<u32>::new());
        let _ = ByteColumn::from_erased(&*typed);
    }

    #[test]
    fn overwrite_replaces_one_value() {
        let mut c = column_of(4, 4, &[vec![1; 4], vec![2; 4]]);
        c.overwrite(0, &[5; 4]);
        assert_eq!(RawColumn::new(&c).get(0), Some(&[5u8; 4][..]));
        assert_eq!(RawColumn::new(&c).get(1), Some(&[2u8; 4][..]));
    }

    #[test]
    fn push_grows_by_doubling() {
        let mut c = ByteColumn::new(layout(12, 4));
        let mut changes = 0;
        let mut cap = c.blocks.capacity();
        for i in 0..1_000u32 {
            c.push(&[u8::try_from(i % 251).unwrap(); 12]);
            if c.blocks.capacity() != cap {
                changes += 1;
                cap = c.blocks.capacity();
            }
        }
        let need = (1_000 * 12usize).div_ceil(16);
        assert!(
            c.blocks.capacity() < 2 * need,
            "cap {}",
            c.blocks.capacity()
        );
        assert!(changes <= 12, "{changes} reallocations");
        assert!(c.tail_is_zero());
    }

    #[test]
    fn describe_names_the_layout() {
        let c = ByteColumn::new(layout(8, 4));
        assert_eq!(c.describe(), crate::table::ColumnDesc::Raw(layout(8, 4)));
        assert_eq!(c.value_type_name(), "<raw>");
    }
}
