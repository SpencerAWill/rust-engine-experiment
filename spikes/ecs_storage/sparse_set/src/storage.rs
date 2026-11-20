//! The sparse-set storage: a packed array of values and entities, and a sparse index.

use core::any::Any;

use crate::{Entity, InvariantViolation};

/// The marker for "no position" in a [`SparseIndex`].
const EMPTY: u32 = u32::MAX;

/// Parallel arrays of entities and values. The two vectors have the same length after every
/// operation, and only the paired operations below can change them.
#[derive(Debug)]
struct Packed<T> {
    entities: Vec<Entity>,
    data: Vec<T>,
}

impl<T> Packed<T> {
    fn new() -> Self {
        Self {
            entities: Vec::new(),
            data: Vec::new(),
        }
    }

    fn push(&mut self, entity: Entity, value: T) {
        self.entities.push(entity);
        self.data.push(value);
    }

    fn swap_remove(&mut self, i: usize) -> (Entity, T) {
        (self.entities.swap_remove(i), self.data.swap_remove(i))
    }

    fn swap(&mut self, i: usize, j: usize) {
        self.entities.swap(i, j);
        self.data.swap(i, j);
    }

    fn len(&self) -> usize {
        self.entities.len()
    }

    fn entities(&self) -> &[Entity] {
        &self.entities
    }

    fn data(&self) -> &[T] {
        &self.data
    }

    fn data_mut(&mut self) -> &mut [T] {
        &mut self.data
    }

    fn split_mut(&mut self) -> (&[Entity], &mut [T]) {
        (&self.entities, &mut self.data)
    }
}

/// Maps an entity's index to a position in the packed arrays.
#[derive(Debug)]
pub struct SparseIndex(Vec<u32>);

impl SparseIndex {
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }

    /// The position stored for `index`, or `None` if there is none (an index past the end is
    /// absent).
    #[inline]
    pub(crate) fn get(&self, index: u32) -> Option<u32> {
        match self.0.get(index as usize) {
            Some(&p) if p != EMPTY => Some(p),
            _ => None,
        }
    }

    /// Stores `position` for `index`, growing the vector as needed.
    pub(crate) fn set(&mut self, index: u32, position: u32) {
        let i = index as usize;
        if i >= self.0.len() {
            self.0.resize(i + 1, EMPTY);
        }
        self.0[i] = position;
    }

    /// Forgets the position of `index`.
    pub(crate) fn clear(&mut self, index: u32) {
        if let Some(slot) = self.0.get_mut(index as usize) {
            *slot = EMPTY;
        }
    }
}

fn position(i: usize) -> u32 {
    u32::try_from(i).expect("a sparse set holds fewer than u32::MAX entries")
}

/// The values of one component type, with a sparse index from entity to position.
#[derive(Debug)]
pub struct SparseSet<T> {
    sparse: SparseIndex,
    packed: Packed<T>,
}

impl<T> SparseSet<T> {
    pub(crate) fn new() -> Self {
        Self {
            sparse: SparseIndex::new(),
            packed: Packed::new(),
        }
    }

    /// The position of `entity`, if it has a value here. Compares the full ID, so a stale ID
    /// whose index was reused is absent.
    pub(crate) fn index_of(&self, entity: Entity) -> Option<usize> {
        let d = self.sparse.get(entity.index())? as usize;
        (self.packed.entities().get(d) == Some(&entity)).then_some(d)
    }

    /// Stores `value` for `entity` and returns the value it replaced. The caller guarantees
    /// that `entity` is alive.
    pub(crate) fn insert(&mut self, entity: Entity, value: T) -> Option<T> {
        if let Some(d) = self.index_of(entity) {
            return Some(core::mem::replace(&mut self.packed.data_mut()[d], value));
        }
        let at = position(self.packed.len());
        self.packed.push(entity, value);
        self.sparse.set(entity.index(), at);
        None
    }

    /// Removes `entity`'s value by swapping the last entry into its place.
    pub(crate) fn remove(&mut self, entity: Entity) -> Option<T> {
        let d = self.index_of(entity)?;
        let (_, value) = self.packed.swap_remove(d);
        self.sparse.clear(entity.index());
        if let Some(moved) = self.packed.entities().get(d) {
            self.sparse.set(moved.index(), position(d));
        }
        Some(value)
    }

    /// Swaps two positions and fixes both sparse slots. Group maintenance only.
    pub(crate) fn swap(&mut self, i: usize, j: usize) {
        self.packed.swap(i, j);
        let (ei, ej) = (self.packed.entities()[i], self.packed.entities()[j]);
        self.sparse.set(ei.index(), position(i));
        self.sparse.set(ej.index(), position(j));
    }

    pub(crate) fn get(&self, entity: Entity) -> Option<&T> {
        self.index_of(entity).map(|d| &self.packed.data()[d])
    }

    pub(crate) fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        let d = self.index_of(entity)?;
        Some(&mut self.packed.data_mut()[d])
    }

    /// The fields of the storage, borrowed separately.
    pub(crate) fn split(&self) -> (&SparseIndex, &[Entity], &[T]) {
        (&self.sparse, self.packed.entities(), self.packed.data())
    }

    /// The fields of the storage, borrowed separately, with the values mutable.
    pub(crate) fn split_mut(&mut self) -> (&SparseIndex, &[Entity], &mut [T]) {
        let (entities, data) = self.packed.split_mut();
        (&self.sparse, entities, data)
    }

    pub(crate) fn len(&self) -> usize {
        self.packed.len()
    }

    /// Checks S1 to S3.
    fn check_storage(&self, is_alive: &dyn Fn(Entity) -> bool) -> Result<(), InvariantViolation> {
        let entities = self.packed.entities();
        let name = core::any::type_name::<T>();
        for (d, e) in entities.iter().enumerate() {
            if self.sparse.get(e.index()) != Some(position(d)) {
                return Err(InvariantViolation::new(format!(
                    "S1: in the storage of {name}, the sparse slot of {e:?} at position {d} is {:?}",
                    self.sparse.get(e.index())
                )));
            }
        }
        for (i, slot) in self.sparse.0.iter().enumerate() {
            if *slot == EMPTY {
                continue;
            }
            let s = *slot as usize;
            if s >= entities.len() || entities[s].index() as usize != i {
                return Err(InvariantViolation::new(format!(
                    "S2: in the storage of {name}, the sparse slot at index {i} holds {s}, which is not a position of an entity with that index"
                )));
            }
        }
        for e in entities {
            if !is_alive(*e) {
                return Err(InvariantViolation::new(format!(
                    "S3: the storage of {name} holds {e:?}, which is not alive"
                )));
            }
        }
        Ok(())
    }
}

/// What the `World` can do with a storage without knowing its value type.
pub trait ErasedStorage: Any {
    fn index_of(&self, entity: Entity) -> Option<usize>;
    fn swap(&mut self, i: usize, j: usize);
    /// Removes the entity's value, dropping it. Returns whether there was one.
    fn remove_entity(&mut self, entity: Entity) -> bool;
    fn len(&self) -> usize;
    fn entities(&self) -> &[Entity];
    fn type_name(&self) -> &'static str;
    fn check(&self, is_alive: &dyn Fn(Entity) -> bool) -> Result<(), InvariantViolation>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: 'static> ErasedStorage for SparseSet<T> {
    fn index_of(&self, entity: Entity) -> Option<usize> {
        SparseSet::index_of(self, entity)
    }

    fn swap(&mut self, i: usize, j: usize) {
        SparseSet::swap(self, i, j);
    }

    fn remove_entity(&mut self, entity: Entity) -> bool {
        self.remove(entity).is_some()
    }

    fn len(&self) -> usize {
        SparseSet::len(self)
    }

    fn entities(&self) -> &[Entity] {
        self.packed.entities()
    }

    fn type_name(&self) -> &'static str {
        core::any::type_name::<T>()
    }

    fn check(&self, is_alive: &dyn Fn(Entity) -> bool) -> Result<(), InvariantViolation> {
        self.check_storage(is_alive)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
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

    fn val(i: usize) -> u32 {
        u32::try_from(i).unwrap()
    }

    fn assert_paired<T>(set: &SparseSet<T>) {
        assert_eq!(set.packed.entities().len(), set.packed.data().len());
        assert_eq!(set.len(), set.packed.entities().len());
    }

    #[test]
    fn packed_operations_keep_both_arrays_paired() {
        let es = entities(4);
        let mut p = Packed::<u32>::new();
        for (i, e) in es.iter().enumerate() {
            p.push(*e, val(i) * 10);
            assert_eq!(p.entities().len(), p.data().len());
        }
        p.swap(0, 3);
        assert_eq!(p.entities(), [es[3], es[1], es[2], es[0]]);
        assert_eq!(p.data(), [30, 10, 20, 0]);
        let (e, v) = p.swap_remove(0);
        assert_eq!((e, v), (es[3], 30));
        assert_eq!(p.entities(), [es[0], es[1], es[2]]);
        assert_eq!(p.data(), [0, 10, 20]);
        {
            let (ents, data) = p.split_mut();
            data[1] += 1;
            assert_eq!(ents.len(), data.len());
        }
        p.data_mut()[2] = 99;
        assert_eq!(p.data(), [0, 11, 99]);
        assert_eq!(p.len(), 3);
    }

    #[test]
    fn insert_then_remove_round_trips() {
        let es = entities(5);
        let mut set = SparseSet::<u32>::new();
        for (i, e) in es.iter().enumerate() {
            assert_eq!(set.insert(*e, val(i)), None);
        }
        assert_eq!(set.len(), 5);
        for (i, e) in es.iter().enumerate() {
            assert_eq!(set.get(*e), Some(&val(i)));
        }
        assert_eq!(set.remove(es[1]), Some(1));
        assert_eq!(set.get(es[1]), None);
        assert_eq!(set.remove(es[1]), None);
        for i in [0, 2, 3, 4] {
            assert_eq!(set.get(es[i]), Some(&val(i)));
        }
        assert_paired(&set);
        assert_eq!(set.check(&|_| true), Ok(()));
        *set.get_mut(es[2]).unwrap() = 7;
        assert_eq!(set.get(es[2]), Some(&7));
    }

    #[test]
    fn insert_replaces_and_returns_the_old_value() {
        let es = entities(2);
        let mut set = SparseSet::<u32>::new();
        set.insert(es[0], 1);
        set.insert(es[1], 2);
        assert_eq!(set.insert(es[0], 10), Some(1));
        assert_eq!(set.len(), 2);
        assert_eq!(set.index_of(es[0]), Some(0));
        assert_eq!(set.get(es[0]), Some(&10));
    }

    #[test]
    fn removing_the_last_and_the_only_entry_keeps_the_index_consistent() {
        let es = entities(3);
        let mut set = SparseSet::<u32>::new();
        for e in &es {
            set.insert(*e, 0);
        }
        assert_eq!(set.remove(es[2]), Some(0));
        assert_eq!(set.check(&|_| true), Ok(()));
        assert_eq!(set.remove(es[0]), Some(0));
        assert_eq!(set.remove(es[1]), Some(0));
        assert_eq!(set.len(), 0);
        assert_eq!(set.check(&|_| true), Ok(()));
        assert_eq!(set.index_of(es[1]), None);
        set.insert(es[1], 5);
        assert_eq!(set.check(&|_| true), Ok(()));
    }

    #[test]
    fn index_past_the_sparse_end_is_absent() {
        let sparse = SparseIndex::new();
        assert_eq!(sparse.get(0), None);
        assert_eq!(sparse.get(1_000), None);
        let mut sparse = SparseIndex::new();
        sparse.set(3, 7);
        assert_eq!(sparse.get(3), Some(7));
        assert_eq!(sparse.get(2), None);
        assert_eq!(sparse.get(4), None);
        sparse.clear(3);
        assert_eq!(sparse.get(3), None);
        sparse.clear(100);
    }

    fn violation<T: 'static>(set: &SparseSet<T>, alive: bool) -> String {
        set.check(&|_| alive).unwrap_err().to_string()
    }

    #[test]
    fn s1_violation_is_reported() {
        let es = entities(2);
        let mut set = SparseSet::<u32>::new();
        set.insert(es[0], 0);
        set.insert(es[1], 1);
        set.sparse.set(es[0].index(), 1);
        set.sparse.set(es[1].index(), 0);
        assert!(violation(&set, true).contains("S1"));
    }

    #[test]
    fn s2_violation_is_reported() {
        let es = entities(2);
        let mut set = SparseSet::<u32>::new();
        set.insert(es[0], 0);
        set.insert(es[1], 1);
        set.sparse.set(9, 0);
        assert!(violation(&set, true).contains("S2"));
        let mut set = SparseSet::<u32>::new();
        set.insert(es[0], 0);
        set.sparse.set(9, 5);
        assert!(violation(&set, true).contains("S2"));
    }

    #[test]
    fn s3_violation_is_reported() {
        let es = entities(1);
        let mut set = SparseSet::<u32>::new();
        set.insert(es[0], 0);
        assert!(violation(&set, false).contains("S3"));
        assert_eq!(set.check(&|_| true), Ok(()));
    }

    #[test]
    fn zero_sized_values_do_not_allocate() {
        let es = entities(10);
        let mut set = SparseSet::<()>::new();
        for e in &es {
            set.insert(*e, ());
        }
        assert_eq!(set.len(), 10);
        assert_eq!(set.packed.data.capacity(), usize::MAX);
    }

    #[test]
    fn erased_storage_works_through_the_trait() {
        let es = entities(3);
        let mut set = SparseSet::<u8>::new();
        for e in &es {
            set.insert(*e, 1);
        }
        let erased: &mut dyn ErasedStorage = &mut set;
        assert_eq!(erased.len(), 3);
        assert_eq!(erased.index_of(es[1]), Some(1));
        assert_eq!(erased.entities(), es.as_slice());
        erased.swap(0, 2);
        assert_eq!(erased.index_of(es[0]), Some(2));
        assert!(erased.remove_entity(es[0]));
        assert!(!erased.remove_entity(es[0]));
        assert!(erased.type_name().contains("u8"));
        assert!(erased.as_any().is::<SparseSet<u8>>());
        assert!(erased.as_any_mut().is::<SparseSet<u8>>());
        assert_eq!(erased.check(&|_| true), Ok(()));
        let (sparse, ents, data) = set.split();
        assert_eq!(
            (sparse.get(es[1].index()), ents.len(), data.len()),
            (Some(1), 2, 2)
        );
        let (_, _, data) = set.split_mut();
        data[0] = 5;
    }
}
