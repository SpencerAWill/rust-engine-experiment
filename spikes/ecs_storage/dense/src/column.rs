//! Columns: a type's values and presence set, owned together.
//!
//! A [`DenseColumn`] keeps one slot per entity index and a presence bit set over the same
//! indices. Its fields are private to this module and every operation changes the slots and
//! the bits together or neither, so invariant D1 (a bit is set exactly when its slot is
//! `Some`) cannot be broken from outside.

use core::any::{Any, TypeId, type_name};

use engine::collections::HierarchicalBitSet;

/// The values of one component type, by entity index, and the set of indices that hold one.
#[derive(Debug)]
pub(crate) struct DenseColumn<T> {
    present: HierarchicalBitSet,
    slots: Vec<Option<T>>,
}

impl<T> DenseColumn<T> {
    /// An empty column.
    pub(crate) fn new() -> Self {
        Self {
            present: HierarchicalBitSet::new(),
            slots: Vec::new(),
        }
    }

    /// Stores `value` at `index`, growing the slots with `None` as needed, and returns the
    /// value it replaced.
    pub(crate) fn insert(&mut self, index: usize, value: T) -> Option<T> {
        if index >= self.slots.len() {
            self.slots.resize_with(index + 1, || None);
        }
        self.present.insert(index);
        self.slots[index].replace(value)
    }

    /// Takes the value at `index`, if any. An index past the end is absent and nothing grows.
    pub(crate) fn remove(&mut self, index: usize) -> Option<T> {
        let value = self.slots.get_mut(index)?.take();
        if value.is_some() {
            self.present.remove(index);
        }
        value
    }

    /// The value at `index`, if any.
    pub(crate) fn get(&self, index: usize) -> Option<&T> {
        self.slots.get(index)?.as_ref()
    }

    /// The value at `index`, mutably, if any.
    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.slots.get_mut(index)?.as_mut()
    }

    /// The indices that hold a value.
    pub(crate) fn present(&self) -> &HierarchicalBitSet {
        &self.present
    }

    /// The presence set and write access to the values, for a mutable query.
    #[cfg(test)]
    pub(crate) fn split_mut(&mut self) -> (&HierarchicalBitSet, ValuesMut<'_, T>) {
        (&self.present, ValuesMut(&mut self.slots))
    }
}

/// Write access to a column's values that cannot clear a slot or change the length (lesson
/// ecs-5): it offers only `get`.
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct ValuesMut<'a, T>(&'a mut [Option<T>]);

#[cfg(test)]
impl<T> ValuesMut<'_, T> {
    /// The value at `index`, mutably, if any.
    pub(crate) fn get(&mut self, index: usize) -> Option<&mut T> {
        self.0.get_mut(index)?.as_mut()
    }
}

/// What code that does not know a column's value type can do with it. The method names
/// avoid `type_id`, which `Any` already has.
pub(crate) trait ErasedColumn {
    /// The `TypeId` of the values.
    fn value_type_id(&self) -> TypeId;
    /// The name of the value type.
    fn value_type_name(&self) -> &'static str;
    /// Drops the value at `index`, if any, and clears its bit.
    fn remove_index(&mut self, index: usize);
    /// The indices that hold a value.
    fn present(&self) -> &HierarchicalBitSet;
    /// The number of values.
    fn present_count(&self) -> usize;
    /// The column as `Any`, to downcast to `DenseColumn<T>`.
    fn as_any(&self) -> &dyn Any;
    /// The column as `Any`, mutably.
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// Checks invariant D1, returning a message naming it if it does not hold.
    fn check(&self) -> Result<(), String>;
}

impl<T: 'static> ErasedColumn for DenseColumn<T> {
    fn value_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn value_type_name(&self) -> &'static str {
        type_name::<T>()
    }

    fn remove_index(&mut self, index: usize) {
        self.remove(index);
    }

    fn present(&self) -> &HierarchicalBitSet {
        DenseColumn::present(self)
    }

    fn present_count(&self) -> usize {
        self.present.len()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn check(&self) -> Result<(), String> {
        let name = type_name::<T>();
        for i in &self.present {
            match self.slots.get(i) {
                Some(Some(_)) => {}
                Some(None) => {
                    return Err(format!(
                        "D1: in the column of {name}, bit {i} is set but its slot is empty"
                    ));
                }
                None => {
                    return Err(format!(
                        "D1: in the column of {name}, bit {i} is set at or past the {} slots",
                        self.slots.len()
                    ));
                }
            }
        }
        for (i, slot) in self.slots.iter().enumerate() {
            if slot.is_some() && !self.present.contains(i) {
                return Err(format!(
                    "D1: in the column of {name}, slot {i} holds a value but its bit is clear"
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;

    #[derive(Debug, PartialEq)]
    struct A(u32);
    struct Hidden;

    struct Counted(Rc<Cell<u32>>);

    impl Drop for Counted {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn insert_grows_with_none_and_sets_the_bit() {
        let mut col = DenseColumn::<A>::new();
        assert!(col.insert(3, A(1)).is_none());
        assert_eq!(col.slots.len(), 4);
        assert!(col.slots[..3].iter().all(Option::is_none));
        assert!(col.present().contains(3));
        assert_eq!(col.present().len(), 1);
        assert_eq!(col.insert(3, A(2)), Some(A(1)));
        assert!(col.present().contains(3));
        assert_eq!(col.get(3), Some(&A(2)));
        assert_eq!(ErasedColumn::check(&col), Ok(()));
    }

    #[test]
    fn remove_takes_the_value_and_clears_the_bit() {
        let mut col = DenseColumn::<A>::new();
        col.insert(3, A(1));
        assert_eq!(col.remove(3), Some(A(1)));
        assert!(!col.present().contains(3));
        assert_eq!(col.slots.len(), 4);
        assert_eq!(col.remove(3), None);
        assert_eq!(col.get(3), None);
        assert_eq!(ErasedColumn::check(&col), Ok(()));
    }

    #[test]
    fn absent_past_the_end_does_not_grow() {
        let mut col = DenseColumn::<A>::new();
        assert!(col.get(100).is_none());
        assert!(col.get_mut(100).is_none());
        assert!(col.remove(100).is_none());
        assert_eq!(col.slots.len(), 0);
        col.insert(2, A(1));
        assert!(col.remove(50).is_none());
        assert!(col.get(50).is_none());
        assert_eq!(col.slots.len(), 3);
        assert!(!col.present().contains(50));
    }

    #[test]
    fn values_mut_writes_in_place() {
        let mut col = DenseColumn::<A>::new();
        col.insert(0, A(1));
        col.insert(2, A(3));
        {
            let (present, mut values) = col.split_mut();
            assert!(present.contains(2));
            values.get(0).unwrap().0 = 10;
            assert!(values.get(1).is_none());
            assert!(values.get(99).is_none());
        }
        assert_eq!(col.get(0), Some(&A(10)));
        assert_eq!(col.get(2), Some(&A(3)));
        assert_eq!(ErasedColumn::check(&col), Ok(()));
    }

    #[test]
    fn remove_index_drops_the_value() {
        let drops = Rc::new(Cell::new(0));
        let mut col = DenseColumn::<Counted>::new();
        col.insert(1, Counted(Rc::clone(&drops)));
        col.insert(2, Counted(Rc::clone(&drops)));
        let erased: &mut dyn ErasedColumn = &mut col;
        erased.remove_index(1);
        assert_eq!(drops.get(), 1);
        assert_eq!(erased.present_count(), 1);
        erased.remove_index(1);
        erased.remove_index(77);
        assert_eq!(drops.get(), 1);
        assert_eq!(erased.check(), Ok(()));
    }

    #[test]
    fn zero_sized_slots_are_one_byte() {
        assert_eq!(size_of::<Option<Hidden>>(), 1);
        let mut col = DenseColumn::<Hidden>::new();
        col.insert(5, Hidden);
        assert!(col.get(5).is_some());
        assert!(col.get(4).is_none());
    }

    #[test]
    fn d1_violation_is_reported() {
        let mut bit_over_none = DenseColumn::<A>::new();
        bit_over_none.insert(2, A(1));
        bit_over_none.present.insert(1);
        let message = ErasedColumn::check(&bit_over_none).unwrap_err();
        assert!(message.contains("D1"), "got: {message}");

        let mut bit_past_the_end = DenseColumn::<A>::new();
        bit_past_the_end.insert(2, A(1));
        bit_past_the_end.present.insert(9);
        let message = ErasedColumn::check(&bit_past_the_end).unwrap_err();
        assert!(message.contains("D1"), "got: {message}");

        let mut some_without_bit = DenseColumn::<A>::new();
        some_without_bit.insert(2, A(1));
        some_without_bit.present.remove(2);
        let message = ErasedColumn::check(&some_without_bit).unwrap_err();
        assert!(message.contains("D1"), "got: {message}");
    }

    #[test]
    fn erased_column_describes_its_type() {
        let mut col = DenseColumn::<A>::new();
        col.insert(0, A(1));
        let erased: &mut dyn ErasedColumn = &mut col;
        assert_eq!(erased.value_type_id(), core::any::TypeId::of::<A>());
        assert!(erased.value_type_name().ends_with('A'));
        assert!(erased.present().contains(0));
        assert!(erased.as_any().is::<DenseColumn<A>>());
        assert!(erased.as_any_mut().is::<DenseColumn<A>>());
    }
}
