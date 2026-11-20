//! Tables: the entity list and columns of one archetype, kept the same length by construction.

use core::any::{Any, TypeId};

use crate::archetype::ComponentId;
use crate::bundle::Bundle;
use crate::{Entity, InvariantViolation};

/// The values of one component type in one archetype, one per row.
#[derive(Debug)]
pub(crate) struct Column<T>(Vec<T>);

impl<T> Column<T> {
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }
}

/// What code that does not know a column's value type can do with it. The method names avoid
/// `Any::type_id`, which a `Box<dyn ErasedColumn>` would otherwise resolve to the box's own type.
pub(crate) trait ErasedColumn: Any {
    fn len(&self) -> usize;
    fn value_type_id(&self) -> TypeId;
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

    fn value_type_id(&self) -> TypeId {
        TypeId::of::<T>()
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

    #[cfg(test)]
    pub(crate) fn column(&self, i: usize) -> &dyn ErasedColumn {
        &*self.columns[i]
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
    /// Panics if `columns` does not cover every column exactly once: the row would break T1.
    /// Internal only.
    pub(crate) fn push_bundle<B: Bundle>(&mut self, entity: Entity, bundle: B, columns: &[usize]) {
        bundle.write(&mut RowWriter(&mut self.columns), columns);
        self.entities.push(entity);
        assert!(
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

    /// Checks T1 (every column as long as the entity list) and T2 (column `i` holds
    /// `types[i]`).
    pub(crate) fn check(&self, types: &[TypeId]) -> Result<(), InvariantViolation> {
        for (i, column) in self.columns.iter().enumerate() {
            if column.len() != self.entities.len() {
                return Err(InvariantViolation::new(format!(
                    "T1: the column of {} holds {} values, but the table has {} entities",
                    column.value_type_name(),
                    column.len(),
                    self.entities.len()
                )));
            }
            if types.get(i) != Some(&column.value_type_id()) {
                return Err(InvariantViolation::new(format!(
                    "T2: column {i} holds {}, which is not the type of the archetype's component {i}",
                    column.value_type_name()
                )));
            }
        }
        if types.len() != self.columns.len() {
            return Err(InvariantViolation::new(format!(
                "T2: the archetype has {} components but its table has {} columns",
                types.len(),
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

    fn types_ac() -> [TypeId; 2] {
        [TypeId::of::<u32>(), TypeId::of::<u64>()]
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
}
