//! Tables: the entity list and columns of one archetype, kept the same length by construction.

use core::any::{Any, TypeId};

use crate::archetype::ComponentId;
use crate::bundle::Bundle;
use crate::raw::{ByteColumn, RawColumn, RawColumnMut, RawLayout};
use crate::{Entity, InvariantViolation};

/// The values of one component type in one archetype, one per row.
#[derive(Debug)]
pub(crate) struct Column<T>(Vec<T>);

impl<T> Column<T> {
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }
}

/// What a column holds, as `Table::check` compares it with what the archetype expects (T2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnDesc {
    /// A `Column<T>` of the Rust type with this `TypeId`.
    Typed(TypeId),
    /// A byte column of this layout.
    Raw(RawLayout),
}

/// What code that does not know a column's value type can do with it. The method names avoid
/// `Any::type_id`, which a `Box<dyn ErasedColumn>` would otherwise resolve to the box's own type.
pub trait ErasedColumn: Any {
    fn len(&self) -> usize;
    fn describe(&self) -> ColumnDesc;
    fn value_type_name(&self) -> &'static str;
    /// Removes the value at `row` by swapping the last value into its place, and drops it.
    fn swap_remove_drop(&mut self, row: usize);
    /// Removes the value at `row` the same way and pushes it onto `dst`, which must hold the
    /// same value type.
    fn swap_remove_into(&mut self, row: usize, dst: &mut dyn ErasedColumn);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: 'static> ErasedColumn for Column<T> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn describe(&self) -> ColumnDesc {
        ColumnDesc::Typed(TypeId::of::<T>())
    }

    fn value_type_name(&self) -> &'static str {
        core::any::type_name::<T>()
    }

    fn swap_remove_drop(&mut self, row: usize) {
        self.0.swap_remove(row);
    }

    fn swap_remove_into(&mut self, row: usize, dst: &mut dyn ErasedColumn) {
        let value = self.0.swap_remove(row);
        typed_mut::<T>(dst).0.push(value);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn typed<T: 'static>(column: &dyn ErasedColumn) -> &Column<T> {
    column
        .as_any()
        .downcast_ref()
        .expect("T2: a column holds the type its component id names")
}

fn typed_mut<T: 'static>(column: &mut dyn ErasedColumn) -> &mut Column<T> {
    column
        .as_any_mut()
        .downcast_mut()
        .expect("T2: a column holds the type its component id names")
}

/// The values of a column whose value type is `T`.
pub(crate) fn column_slice<T: 'static>(column: &dyn ErasedColumn) -> &[T] {
    &typed::<T>(column).0
}

/// The values of a column whose value type is `T`, exclusive.
pub(crate) fn column_slice_mut<T: 'static>(column: &mut dyn ErasedColumn) -> &mut [T] {
    &mut typed_mut::<T>(column).0
}

/// Pushes the elements of one bundle onto a table's columns. Only [`Table::push_bundle`] makes
/// one, and it checks that the row is complete.
#[derive(Debug)]
pub struct RowWriter<'t>(&'t mut [Box<dyn ErasedColumn>]);

impl RowWriter<'_> {
    /// Pushes `value` onto column `col`.
    pub(crate) fn push<T: 'static>(&mut self, col: usize, value: T) {
        typed_mut::<T>(&mut *self.0[col]).0.push(value);
    }
}

/// One column, borrowed exclusively, that can be read and written in place but not resized:
/// the only methods return a slice of a type the caller names, so no code outside [`Table`] can
/// push or remove a value and break T1.
#[derive(Debug)]
pub struct ColumnMut<'t>(&'t mut dyn ErasedColumn);

impl<'t> ColumnMut<'t> {
    /// The name of the column's value type.
    pub fn value_type_name(&self) -> &'static str {
        self.0.value_type_name()
    }

    /// The column's values, shared.
    pub fn slice<T: 'static>(self) -> &'t [T] {
        column_slice(self.0)
    }

    /// The column's values, exclusive: a slice, so its length cannot change.
    pub fn slice_mut<T: 'static>(self) -> &'t mut [T] {
        column_slice_mut(self.0)
    }
}

/// An archetype's entities and component columns. Column `i` holds the archetype's `i`-th
/// component in ascending [`ComponentId`] order. The fields are private and every operation
/// changes the entity list and all columns together (T1).
#[derive(Debug)]
pub(crate) struct Table {
    entities: Vec<Entity>,
    columns: Box<[Box<dyn ErasedColumn>]>,
}

impl core::fmt::Debug for dyn ErasedColumn {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Column<{}>({})", self.value_type_name(), self.len())
    }
}

impl Table {
    pub(crate) fn new(columns: Box<[Box<dyn ErasedColumn>]>) -> Self {
        Self {
            entities: Vec::new(),
            columns,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.entities.len()
    }

    pub(crate) fn entities(&self) -> &[Entity] {
        &self.entities
    }

    pub(crate) fn column(&self, i: usize) -> &dyn ErasedColumn {
        &*self.columns[i]
    }

    /// The entity list and the columns, the columns exclusive. The entity list is not part of
    /// the columns, so a query can read it while it writes a column; a caller cannot change the
    /// length of either through these borrows (T1).
    pub(crate) fn columns_mut(&mut self) -> (&[Entity], impl Iterator<Item = ColumnMut<'_>>) {
        (
            &self.entities,
            self.columns.iter_mut().map(|c| ColumnMut(&mut **c)),
        )
    }

    /// The entity list and the byte columns whose index `wanted` maps to `Some(tag)`, each with
    /// its tag. Only those columns are downcast, once each, so `wanted` must accept byte columns
    /// only. The views cannot push, remove or resize (T1).
    pub(crate) fn raw_columns(
        &self,
        wanted: impl Fn(usize) -> Option<usize>,
    ) -> (&[Entity], impl Iterator<Item = (usize, RawColumn<'_>)>) {
        (
            &self.entities,
            self.columns
                .iter()
                .enumerate()
                .filter_map(move |(i, c)| Some((wanted(i)?, c)))
                .map(|(k, c)| (k, RawColumn::new(ByteColumn::from_erased(&**c)))),
        )
    }

    /// As [`Table::raw_columns`], the views exclusive; several can be held at once.
    pub(crate) fn raw_columns_mut(
        &mut self,
        wanted: impl Fn(usize) -> Option<usize>,
    ) -> (&[Entity], impl Iterator<Item = (usize, RawColumnMut<'_>)>) {
        (
            &self.entities,
            self.columns
                .iter_mut()
                .enumerate()
                .filter_map(move |(i, c)| Some((wanted(i)?, c)))
                .map(|(k, c)| (k, RawColumnMut::new(ByteColumn::from_erased_mut(&mut **c)))),
        )
    }

    /// Adds a row to a table with no columns.
    ///
    /// # Panics
    ///
    /// Panics if the table has columns: a row without values would break T1. Internal only.
    pub(crate) fn push_empty(&mut self, entity: Entity) {
        assert!(
            self.columns.is_empty(),
            "Table::push_empty called on a table with {} columns (T1); this is a bug in the prototype",
            self.columns.len()
        );
        self.entities.push(entity);
    }

    /// Adds a row from `bundle`, whose `k`-th element goes to column `columns[k]`, and the
    /// entity. The whole row is written by this one operation (T1).
    ///
    /// # Panics
    ///
    /// In debug builds, panics if `columns` does not cover every column exactly once: the row
    /// would break T1. Internal only.
    pub(crate) fn push_bundle<B: Bundle>(&mut self, entity: Entity, bundle: B, columns: &[usize]) {
        bundle.write(&mut RowWriter(&mut self.columns), columns);
        self.entities.push(entity);
        debug_assert!(
            self.columns.iter().all(|c| c.len() == self.entities.len()),
            "Table::push_bundle left a column at a different length than the entity list (T1); \
             this is a bug in the prototype"
        );
    }

    /// Drops the row and returns the entity moved into the hole, if any.
    pub(crate) fn swap_remove(&mut self, row: usize) -> Option<Entity> {
        for column in &mut self.columns {
            column.swap_remove_drop(row);
        }
        self.entities.swap_remove(row);
        self.entities.get(row).copied()
    }

    /// Moves `src`'s row to `dst`, which has `src`'s components plus `T`, and stores `value` as
    /// the new `T`. The component lists are ascending, and `dst`'s is `src`'s plus one id.
    /// Returns the entity swapped into `row` of `src`, if any.
    pub(crate) fn move_row_adding<T: 'static>(
        src: &mut Table,
        src_ids: &[ComponentId],
        row: usize,
        dst: &mut Table,
        dst_ids: &[ComponentId],
        value: T,
    ) -> Option<Entity> {
        debug_assert_eq!(dst_ids.len(), src_ids.len() + 1);
        let mut added = dst_ids.len() - 1;
        let mut j = 0;
        for (i, id) in src_ids.iter().enumerate() {
            while dst_ids[j] != *id {
                added = j;
                j += 1;
            }
            src.columns[i].swap_remove_into(row, &mut *dst.columns[j]);
            j += 1;
        }
        typed_mut::<T>(&mut *dst.columns[added]).0.push(value);
        dst.entities.push(src.entities.swap_remove(row));
        src.entities.get(row).copied()
    }

    /// Moves `src`'s row to `dst`, which has `src`'s components minus `T`, and returns the
    /// removed `T` with the entity swapped into `row` of `src`, if any.
    pub(crate) fn move_row_removing<T: 'static>(
        src: &mut Table,
        src_ids: &[ComponentId],
        row: usize,
        dst: &mut Table,
        dst_ids: &[ComponentId],
    ) -> (T, Option<Entity>) {
        debug_assert_eq!(src_ids.len(), dst_ids.len() + 1);
        let mut removed = None;
        let mut j = 0;
        for (i, id) in src_ids.iter().enumerate() {
            if dst_ids.get(j) == Some(id) {
                src.columns[i].swap_remove_into(row, &mut *dst.columns[j]);
                j += 1;
            } else {
                removed = Some(typed_mut::<T>(&mut *src.columns[i]).0.swap_remove(row));
            }
        }
        dst.entities.push(src.entities.swap_remove(row));
        let value = removed.expect("the removed component is one of the source's components");
        (value, src.entities.get(row).copied())
    }

    /// Adds a row whose `k`-th value, in ascending component order, is `values[k].1`, and the
    /// entity. Every column is a byte column and the table has exactly `values.len()` of them.
    /// The whole row is written by this one operation (T1).
    ///
    /// # Panics
    ///
    /// Panics if `values` does not cover every column exactly once, or a column is typed.
    /// Internal only.
    pub(crate) fn push_raw_row(&mut self, entity: Entity, values: &[(ComponentId, &[u8])]) {
        assert_eq!(
            values.len(),
            self.columns.len(),
            "Table::push_raw_row got {} values for {} columns (T1); this is a bug in the prototype",
            values.len(),
            self.columns.len()
        );
        for (column, (_, bytes)) in self.columns.iter_mut().zip(values) {
            ByteColumn::from_erased_mut(&mut **column).push(bytes);
        }
        self.entities.push(entity);
    }

    /// [`Table::move_row_adding`] for a byte column: moves `src`'s row to `dst`, which has
    /// `src`'s components plus one raw component, and stores `bytes` as its value.
    pub(crate) fn move_row_adding_raw(
        src: &mut Table,
        src_ids: &[ComponentId],
        row: usize,
        dst: &mut Table,
        dst_ids: &[ComponentId],
        bytes: &[u8],
    ) -> Option<Entity> {
        debug_assert_eq!(dst_ids.len(), src_ids.len() + 1);
        let mut added = dst_ids.len() - 1;
        let mut j = 0;
        for (i, id) in src_ids.iter().enumerate() {
            while dst_ids[j] != *id {
                added = j;
                j += 1;
            }
            src.columns[i].swap_remove_into(row, &mut *dst.columns[j]);
            j += 1;
        }
        ByteColumn::from_erased_mut(&mut *dst.columns[added]).push(bytes);
        dst.entities.push(src.entities.swap_remove(row));
        src.entities.get(row).copied()
    }

    /// [`Table::move_row_removing`] for a byte column: moves `src`'s row to `dst`, which has
    /// `src`'s components minus one raw component, and drops that value.
    pub(crate) fn move_row_removing_raw(
        src: &mut Table,
        src_ids: &[ComponentId],
        row: usize,
        dst: &mut Table,
        dst_ids: &[ComponentId],
    ) -> Option<Entity> {
        debug_assert_eq!(src_ids.len(), dst_ids.len() + 1);
        let mut j = 0;
        for (i, id) in src_ids.iter().enumerate() {
            if dst_ids.get(j) == Some(id) {
                src.columns[i].swap_remove_into(row, &mut *dst.columns[j]);
                j += 1;
            } else {
                src.columns[i].swap_remove_drop(row);
            }
        }
        dst.entities.push(src.entities.swap_remove(row));
        src.entities.get(row).copied()
    }

    /// Stores `bytes` in place of the value at (`row`, `col`) of a byte column.
    pub(crate) fn overwrite_raw(&mut self, row: usize, col: usize, bytes: &[u8]) {
        ByteColumn::from_erased_mut(&mut *self.columns[col]).overwrite(row, bytes);
    }

    /// The bytes of the value at (`row`, `col`) of a byte column.
    pub(crate) fn raw_value(&self, row: usize, col: usize) -> Option<&[u8]> {
        RawColumn::new(ByteColumn::from_erased(&*self.columns[col])).get(row)
    }

    pub(crate) fn get<T: 'static>(&self, row: usize, col: usize) -> &T {
        &typed::<T>(&*self.columns[col]).0[row]
    }

    pub(crate) fn get_mut<T: 'static>(&mut self, row: usize, col: usize) -> &mut T {
        &mut typed_mut::<T>(&mut *self.columns[col]).0[row]
    }

    /// Stores `value` in place of the value at (`row`, `col`) and returns the old one.
    pub(crate) fn replace<T: 'static>(&mut self, row: usize, col: usize, value: T) -> T {
        core::mem::replace(self.get_mut(row, col), value)
    }

    /// Checks T1 (every column as long as the entity list), T2 (column `i` is what
    /// `expected[i]` says) and T3 (a byte column has no nonzero byte past its values).
    pub(crate) fn check(&self, expected: &[ColumnDesc]) -> Result<(), InvariantViolation> {
        for (i, column) in self.columns.iter().enumerate() {
            if column.len() != self.entities.len() {
                return Err(InvariantViolation::new(format!(
                    "T1: the column of {} holds {} values, but the table has {} entities",
                    column.value_type_name(),
                    column.len(),
                    self.entities.len()
                )));
            }
            let desc = column.describe();
            if expected.get(i) != Some(&desc) {
                return Err(InvariantViolation::new(format!(
                    "T2: column {i} holds {}, which is not the type or layout of the archetype's \
                     component {i}",
                    column.value_type_name()
                )));
            }
            if matches!(desc, ColumnDesc::Raw(_))
                && !ByteColumn::from_erased(&**column).tail_is_zero()
            {
                return Err(InvariantViolation::new(format!(
                    "T3: the byte column {i} has a nonzero byte past its {} values",
                    column.len()
                )));
            }
        }
        if expected.len() != self.columns.len() {
            return Err(InvariantViolation::new(format!(
                "T2: the archetype has {} components but its table has {} columns",
                expected.len(),
                self.columns.len()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EntityMarker;
    use engine::genid::GenIdAllocator;

    fn entities(n: usize) -> Vec<Entity> {
        let mut alloc = GenIdAllocator::<EntityMarker>::new();
        (0..n).map(|_| alloc.allocate()).collect()
    }

    fn col<T: 'static>() -> Box<dyn ErasedColumn> {
        Box::new(Column::<T>::new())
    }

    fn table(columns: Vec<Box<dyn ErasedColumn>>) -> Table {
        Table::new(columns.into_boxed_slice())
    }

    #[test]
    fn push_bundle_fills_every_column() {
        let es = entities(2);
        let mut t = table(vec![col::<u32>(), col::<u64>()]);
        // Tuple order (u64, u32) is the reverse of column order.
        t.push_bundle(es[0], (7u64, 3u32), &[1, 0]);
        t.push_bundle(es[1], (8u64, 4u32), &[1, 0]);
        assert_eq!(t.entities(), &es[..]);
        assert_eq!(*t.get::<u32>(1, 0), 4);
        assert_eq!(*t.get::<u64>(0, 1), 7);
        assert_eq!(t.column(0).len(), 2);
        assert_eq!(t.column(1).len(), 2);
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "T1")]
    fn push_bundle_with_a_missing_column_panics() {
        let es = entities(1);
        let mut t = table(vec![col::<u32>(), col::<u64>()]);
        t.push_bundle(es[0], (3u32,), &[0]);
    }

    const ID0: ComponentId = ComponentId::new(0);
    const ID1: ComponentId = ComponentId::new(1);
    const ID2: ComponentId = ComponentId::new(2);

    /// A table `[ID0: u32, ID2: u64]` holding `n` rows: row `i` is `(10 * i, 100 * i)`.
    fn filled(es: &[Entity]) -> Table {
        let mut empty = table(vec![]);
        let mut a = table(vec![col::<u32>()]);
        let mut ac = table(vec![col::<u32>(), col::<u64>()]);
        for (i, e) in es.iter().enumerate() {
            let i = u32::try_from(i).unwrap();
            empty.push_empty(*e);
            let moved = Table::move_row_adding::<u32>(&mut empty, &[], 0, &mut a, &[ID0], 10 * i);
            assert_eq!(moved, None);
            let moved = Table::move_row_adding::<u64>(
                &mut a,
                &[ID0],
                0,
                &mut ac,
                &[ID0, ID2],
                u64::from(100 * i),
            );
            assert_eq!(moved, None);
        }
        ac
    }

    fn types_ac() -> [ColumnDesc; 2] {
        [
            ColumnDesc::Typed(TypeId::of::<u32>()),
            ColumnDesc::Typed(TypeId::of::<u64>()),
        ]
    }

    #[test]
    fn swap_remove_reports_the_moved_entity() {
        let es = entities(3);
        let mut t = filled(&es);
        assert_eq!(t.swap_remove(0), Some(es[2]));
        assert_eq!(t.entities(), [es[2], es[1]]);
        assert_eq!(*t.get::<u32>(0, 0), 20);
        assert_eq!(*t.get::<u64>(0, 1), 200);
        assert_eq!(t.check(&types_ac()), Ok(()));
    }

    #[test]
    fn move_row_adding_pairs_columns_by_id() {
        let es = entities(2);
        let mut src = filled(&es);
        let mut dst = table(vec![col::<u32>(), col::<u8>(), col::<u64>()]);
        let swapped =
            Table::move_row_adding::<u8>(&mut src, &[ID0, ID2], 0, &mut dst, &[ID0, ID1, ID2], 7);
        assert_eq!(swapped, Some(es[1]));
        assert_eq!(dst.entities(), [es[0]]);
        assert_eq!(*dst.get::<u32>(0, 0), 0);
        assert_eq!(*dst.get::<u8>(0, 1), 7);
        assert_eq!(*dst.get::<u64>(0, 2), 0);
        assert_eq!(src.entities(), [es[1]]);
        assert_eq!(*src.get::<u32>(0, 0), 10);
        assert_eq!(*src.get::<u64>(0, 1), 100);
    }

    #[test]
    fn move_row_removing_returns_the_value() {
        let es = entities(2);
        let mut src = filled(&es);
        let mut dst = table(vec![col::<u64>()]);
        let (value, swapped) =
            Table::move_row_removing::<u32>(&mut src, &[ID0, ID2], 0, &mut dst, &[ID2]);
        assert_eq!(value, 0);
        assert_eq!(swapped, Some(es[1]));
        assert_eq!(dst.entities(), [es[0]]);
        assert_eq!(*dst.get::<u64>(0, 0), 0);
        let (value, swapped) =
            Table::move_row_removing::<u32>(&mut src, &[ID0, ID2], 0, &mut dst, &[ID2]);
        assert_eq!(value, 10);
        assert_eq!(swapped, None);
        assert_eq!(*dst.get::<u64>(1, 0), 100);
    }

    #[test]
    fn moving_the_last_row_reports_no_swap() {
        let es = entities(2);
        let mut src = filled(&es);
        let mut dst = table(vec![col::<u32>(), col::<u8>(), col::<u64>()]);
        let swapped =
            Table::move_row_adding::<u8>(&mut src, &[ID0, ID2], 1, &mut dst, &[ID0, ID1, ID2], 1);
        assert_eq!(swapped, None);
        assert_eq!(src.entities(), [es[0]]);
        assert_eq!(src.check(&types_ac()), Ok(()));
        let (_, swapped) = Table::move_row_removing::<u8>(
            &mut dst,
            &[ID0, ID1, ID2],
            0,
            &mut table(vec![col::<u32>(), col::<u64>()]),
            &[ID0, ID2],
        );
        assert_eq!(swapped, None);
        assert_eq!(dst.len(), 0);
        let mut one = filled(&es[..1]);
        assert_eq!(one.swap_remove(0), None);
        assert_eq!(one.len(), 0);
    }

    #[test]
    fn replace_and_get_mut_write_in_place() {
        let es = entities(1);
        let mut t = filled(&es);
        assert_eq!(t.replace::<u32>(0, 0, 5), 0);
        *t.get_mut::<u64>(0, 1) += 1;
        assert_eq!(*t.get::<u32>(0, 0), 5);
        assert_eq!(*t.get::<u64>(0, 1), 1);
    }

    #[test]
    fn zero_sized_columns_do_not_allocate() {
        struct Tag;
        let es = entities(1000);
        let mut empty = table(vec![]);
        let mut tagged = table(vec![col::<Tag>()]);
        for e in &es {
            empty.push_empty(*e);
            Table::move_row_adding::<Tag>(&mut empty, &[], 0, &mut tagged, &[ID0], Tag);
        }
        assert_eq!(tagged.len(), 1000);
        let column = tagged
            .column(0)
            .as_any()
            .downcast_ref::<Column<Tag>>()
            .unwrap();
        assert_eq!(column.0.len(), 1000);
        assert_eq!(
            column.0.capacity(),
            usize::MAX,
            "a zero-sized Vec never allocates"
        );
    }

    #[test]
    #[should_panic(expected = "push_empty")]
    fn push_empty_on_a_table_with_columns_panics() {
        let es = entities(1);
        table(vec![col::<u32>()]).push_empty(es[0]);
    }

    #[test]
    fn t1_violation_is_reported() {
        let es = entities(2);
        let mut t = filled(&es);
        t.entities.push(es[0]);
        let err = t.check(&types_ac()).unwrap_err();
        assert!(err.to_string().contains("T1"), "got: {err}");
    }

    #[test]
    fn t2_violation_is_reported() {
        let es = entities(1);
        let mut t = filled(&es);
        t.columns.swap(0, 1);
        let err = t.check(&types_ac()).unwrap_err();
        assert!(err.to_string().contains("T2"), "got: {err}");
    }

    fn lay(size: usize, align: usize) -> RawLayout {
        RawLayout::new(size, align).unwrap()
    }

    fn raw_col(size: usize, align: usize) -> Box<dyn ErasedColumn> {
        Box::new(ByteColumn::new(lay(size, align)))
    }

    /// A table of one 12-byte column holding `n` rows.
    fn raw_table(n: usize) -> (Table, Vec<Entity>) {
        let es = entities(n);
        let mut t = table(vec![raw_col(12, 4)]);
        for e in &es {
            t.push_raw_row(*e, &[(ID0, &[7u8; 12][..])]);
        }
        (t, es)
    }

    #[test]
    fn check_invariants_reports_a_wrong_layout() {
        let (t, _) = raw_table(1);
        assert_eq!(t.check(&[ColumnDesc::Raw(lay(12, 4))]), Ok(()));
        let err = t.check(&[ColumnDesc::Raw(lay(12, 2))]).unwrap_err();
        assert!(err.to_string().contains("T2"), "got: {err}");
        let err = t.check(&[ColumnDesc::Raw(lay(16, 4))]).unwrap_err();
        assert!(err.to_string().contains("T2"), "got: {err}");
        let err = t
            .check(&[ColumnDesc::Typed(TypeId::of::<u32>())])
            .unwrap_err();
        assert!(err.to_string().contains("T2"), "got: {err}");
    }

    #[test]
    fn check_invariants_reports_a_dirty_tail() {
        let (mut t, _) = raw_table(2);
        assert_eq!(t.check(&[ColumnDesc::Raw(lay(12, 4))]), Ok(()));
        ByteColumn::from_erased_mut(&mut *t.columns[0]).dirty_the_tail();
        let err = t.check(&[ColumnDesc::Raw(lay(12, 4))]).unwrap_err();
        assert!(err.to_string().contains("T3"), "got: {err}");
    }

    #[test]
    fn check_invariants_allows_spare_blocks_but_not_a_dirty_byte_in_them() {
        let (mut t, _) = raw_table(4);
        let descs = [ColumnDesc::Raw(lay(12, 4))];
        t.swap_remove(0);
        t.swap_remove(0);
        t.swap_remove(0);
        // One 12-byte value uses 1 of the 3 blocks the four values needed.
        assert_eq!(t.check(&descs), Ok(()));
        let col = ByteColumn::from_erased_mut(&mut *t.columns[0]);
        assert!(col.has_spare_blocks());
        let last = col.last_storage_byte();
        assert!(col.dirty_byte(last), "the byte is in a spare block");
        let err = t.check(&descs).unwrap_err();
        assert!(err.to_string().contains("T3"), "got: {err}");
    }

    #[test]
    fn check_invariants_reports_a_short_raw_column() {
        let (mut t, _) = raw_table(2);
        t.columns[0].swap_remove_drop(0);
        let err = t.check(&[ColumnDesc::Raw(lay(12, 4))]).unwrap_err();
        assert!(err.to_string().contains("T1"), "got: {err}");
    }

    #[test]
    fn raw_moves_pair_columns_by_id() {
        let es = entities(2);
        let mut src = table(vec![raw_col(4, 4), raw_col(12, 4)]);
        let ids = [ID0, ID2];
        for (i, e) in es.iter().enumerate() {
            let i = u8::try_from(i).unwrap();
            src.push_raw_row(*e, &[(ID0, &[i; 4][..]), (ID2, &[i + 10; 12][..])]);
        }
        let mut mid = table(vec![raw_col(4, 4), raw_col(2, 2), raw_col(12, 4)]);
        let swapped =
            Table::move_row_adding_raw(&mut src, &ids, 0, &mut mid, &[ID0, ID1, ID2], &[5, 6]);
        assert_eq!(swapped, Some(es[1]));
        assert_eq!(mid.raw_value(0, 0), Some(&[0u8; 4][..]));
        assert_eq!(mid.raw_value(0, 1), Some(&[5u8, 6][..]));
        assert_eq!(mid.raw_value(0, 2), Some(&[10u8; 12][..]));
        assert_eq!(src.raw_value(0, 0), Some(&[1u8; 4][..]));
        let mut back = table(vec![raw_col(4, 4), raw_col(12, 4)]);
        let swapped = Table::move_row_removing_raw(&mut mid, &[ID0, ID1, ID2], 0, &mut back, &ids);
        assert_eq!(swapped, None);
        assert_eq!(back.entities(), [es[0]]);
        assert_eq!(back.raw_value(0, 1), Some(&[10u8; 12][..]));
        assert_eq!(mid.len(), 0);
        let descs = [ColumnDesc::Raw(lay(4, 4)), ColumnDesc::Raw(lay(12, 4))];
        assert_eq!(back.check(&descs), Ok(()));
        assert_eq!(src.check(&descs), Ok(()));
    }
}
