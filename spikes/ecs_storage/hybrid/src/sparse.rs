//! Sparse sets: the values of one component type, with a sparse index from entity to position.
//! Adapted from the sparse-set prototype's storage (M3.3), this repository's own code, without
//! groups.

use core::any::{Any, TypeId};

use crate::Entity;

/// The marker for "no position" in the sparse index.
const EMPTY: u32 = u32::MAX;

fn position(i: usize) -> u32 {
    u32::try_from(i).expect("a sparse set holds fewer than u32::MAX entries")
}

/// The values of one component type. `entities` and `data` have the same length after every
/// operation and change only together; `sparse[i]` holds the position of the entity with index
/// `i`, or [`EMPTY`].
#[derive(Debug)]
pub(crate) struct SparseSet<T> {
    sparse: Vec<u32>,
    entities: Vec<Entity>,
    data: Vec<T>,
}

impl<T> SparseSet<T> {
    pub(crate) fn new() -> Self {
        Self {
            sparse: Vec::new(),
            entities: Vec::new(),
            data: Vec::new(),
        }
    }

    /// The position of `entity`, if it has a value here. Compares the full ID, so a stale ID
    /// whose index was reused is absent.
    pub(crate) fn index_of(&self, entity: Entity) -> Option<usize> {
        let d = match self.sparse.get(entity.index() as usize) {
            Some(&p) if p != EMPTY => p as usize,
            _ => return None,
        };
        (self.entities.get(d) == Some(&entity)).then_some(d)
    }

    /// Stores `value` for `entity` and returns the value it replaced. The caller guarantees
    /// that `entity` is alive.
    pub(crate) fn insert(&mut self, entity: Entity, value: T) -> Option<T> {
        if let Some(d) = self.index_of(entity) {
            return Some(core::mem::replace(&mut self.data[d], value));
        }
        let at = position(self.entities.len());
        self.entities.push(entity);
        self.data.push(value);
        let i = entity.index() as usize;
        if i >= self.sparse.len() {
            self.sparse.resize(i + 1, EMPTY);
        }
        self.sparse[i] = at;
        None
    }

    /// Removes `entity`'s value by swapping the last entry into its place, and fixes the moved
    /// entry's sparse slot.
    pub(crate) fn remove(&mut self, entity: Entity) -> Option<T> {
        let d = self.index_of(entity)?;
        self.entities.swap_remove(d);
        let value = self.data.swap_remove(d);
        self.sparse[entity.index() as usize] = EMPTY;
        if let Some(moved) = self.entities.get(d) {
            self.sparse[moved.index() as usize] = position(d);
        }
        Some(value)
    }

    pub(crate) fn get(&self, entity: Entity) -> Option<&T> {
        self.index_of(entity).map(|d| &self.data[d])
    }

    pub(crate) fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        let d = self.index_of(entity)?;
        Some(&mut self.data[d])
    }

    pub(crate) fn len(&self) -> usize {
        self.entities.len()
    }

    /// A handle for mutable queries: it can read and write values in place but cannot insert,
    /// remove or change a length, so it cannot break S1 or S2.
    #[cfg(test)]
    pub(crate) fn split_mut(&mut self) -> SparseMut<'_, T> {
        SparseMut {
            sparse: &self.sparse,
            entities: &self.entities,
            data: &mut self.data,
        }
    }

    /// Checks S1 to S3.
    fn check_storage(&self, is_alive: &dyn Fn(Entity) -> bool) -> Result<(), String> {
        let name = core::any::type_name::<T>();
        if self.entities.len() != self.data.len() {
            return Err(format!(
                "S1: the set of {name} has {} entities but {} values",
                self.entities.len(),
                self.data.len()
            ));
        }
        for (d, e) in self.entities.iter().enumerate() {
            let slot = self.sparse.get(e.index() as usize).copied();
            if slot != Some(position(d)) {
                return Err(format!(
                    "S1: in the set of {name}, the sparse slot of {e:?} at position {d} is {slot:?}"
                ));
            }
        }
        for (i, slot) in self.sparse.iter().enumerate() {
            if *slot == EMPTY {
                continue;
            }
            let s = *slot as usize;
            if s >= self.entities.len() || self.entities[s].index() as usize != i {
                return Err(format!(
                    "S2: in the set of {name}, the sparse slot at index {i} holds {s}, which is \
                     not a position of an entity with that index"
                ));
            }
        }
        for e in &self.entities {
            if !is_alive(*e) {
                return Err(format!(
                    "S3: the set of {name} holds {e:?}, which is not alive"
                ));
            }
        }
        Ok(())
    }
}

/// A sparse set borrowed for a mutable query: values by entity, never a length change.
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct SparseMut<'a, T> {
    sparse: &'a [u32],
    entities: &'a [Entity],
    data: &'a mut [T],
}

#[cfg(test)]
impl<T> SparseMut<'_, T> {
    /// The value of `entity`, if it has one.
    pub(crate) fn get(&mut self, entity: Entity) -> Option<&mut T> {
        let d = match self.sparse.get(entity.index() as usize) {
            Some(&p) if p != EMPTY => p as usize,
            _ => return None,
        };
        if self.entities.get(d) == Some(&entity) {
            Some(&mut self.data[d])
        } else {
            None
        }
    }
}

/// What the world can do with a set without knowing its value type. The method names avoid
/// `Any::type_id`, which a `Box<dyn ErasedSparseSet>` would otherwise resolve to the box's type.
pub trait ErasedSparseSet: Any {
    /// Removes the entity's value, dropping it, if there is one.
    fn remove_entity(&mut self, entity: Entity);
    fn len(&self) -> usize;
    fn value_type_id(&self) -> TypeId;
    fn value_type_name(&self) -> &'static str;
    /// Checks S1 to S3; the error names the invariant.
    fn check(&self, is_alive: &dyn Fn(Entity) -> bool) -> Result<(), String>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: 'static> ErasedSparseSet for SparseSet<T> {
    fn remove_entity(&mut self, entity: Entity) {
        self.remove(entity);
    }

    fn len(&self) -> usize {
        SparseSet::len(self)
    }

    fn value_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn value_type_name(&self) -> &'static str {
        core::any::type_name::<T>()
    }

    fn check(&self, is_alive: &dyn Fn(Entity) -> bool) -> Result<(), String> {
        self.check_storage(is_alive)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl core::fmt::Debug for dyn ErasedSparseSet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "SparseSet<{}>({})", self.value_type_name(), self.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EntityMarker;
    use engine::genid::GenIdAllocator;
    use std::cell::Cell;
    use std::rc::Rc;

    fn entities(n: usize) -> Vec<Entity> {
        let mut alloc = GenIdAllocator::<EntityMarker>::new();
        (0..n).map(|_| alloc.allocate()).collect()
    }

    fn alive(_: Entity) -> bool {
        true
    }

    #[test]
    fn insert_then_get() {
        let es = entities(3);
        let mut s = SparseSet::new();
        assert_eq!(s.insert(es[2], 20u32), None);
        assert_eq!(s.insert(es[0], 0), None);
        assert_eq!(s.get(es[2]), Some(&20));
        assert_eq!(s.get(es[1]), None);
        assert_eq!(s.insert(es[2], 21), Some(20));
        assert_eq!(s.len(), 2);
        *s.get_mut(es[0]).unwrap() += 5;
        assert_eq!(s.get(es[0]), Some(&5));
        assert_eq!(s.check_storage(&alive), Ok(()));
    }

    #[test]
    fn sparse_remove_fixes_the_moved_entry() {
        let es = entities(4);
        let mut s = SparseSet::new();
        for (i, e) in es.iter().enumerate() {
            s.insert(*e, u32::try_from(i).unwrap() * 10);
        }
        // A middle entry: the last entry moves into its place.
        assert_eq!(s.remove(es[1]), Some(10));
        assert_eq!(s.index_of(es[3]), Some(1));
        assert_eq!(s.get(es[3]), Some(&30));
        assert_eq!(s.get(es[1]), None);
        assert_eq!(s.check_storage(&alive), Ok(()));
        // The last entry: nothing moves.
        assert_eq!(s.remove(es[2]), Some(20));
        assert_eq!(s.check_storage(&alive), Ok(()));
        assert_eq!(s.remove(es[2]), None);
        assert_eq!(s.remove(es[3]), Some(30));
        assert_eq!(s.remove(es[0]), Some(0));
        assert_eq!(s.len(), 0);
        assert_eq!(s.check_storage(&alive), Ok(()));
    }

    #[test]
    fn stale_id_is_absent() {
        let mut alloc = GenIdAllocator::<EntityMarker>::new();
        let a = alloc.allocate();
        let mut s = SparseSet::new();
        s.insert(a, 1u32);
        s.remove(a);
        alloc.free(a);
        let b = alloc.allocate();
        assert_eq!(a.index(), b.index());
        s.insert(b, 2);
        assert_eq!(s.get(a), None);
        assert_eq!(s.remove(a), None);
        assert_eq!(s.get(b), Some(&2));
        assert_eq!(s.check_storage(&alive), Ok(()));
    }

    #[test]
    fn sparse_mut_writes_in_place() {
        let es = entities(2);
        let mut s = SparseSet::new();
        s.insert(es[0], 1u32);
        let mut m = s.split_mut();
        *m.get(es[0]).unwrap() += 9;
        assert!(m.get(es[1]).is_none());
        assert_eq!(s.get(es[0]), Some(&10));
    }

    #[test]
    fn remove_entity_drops_the_value() {
        struct Droppy(Rc<Cell<u32>>);
        impl Drop for Droppy {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let drops = Rc::new(Cell::new(0));
        let es = entities(2);
        let mut s = SparseSet::new();
        s.insert(es[0], Droppy(Rc::clone(&drops)));
        let erased: &mut dyn ErasedSparseSet = &mut s;
        erased.remove_entity(es[1]);
        assert_eq!(drops.get(), 0);
        erased.remove_entity(es[0]);
        assert_eq!(drops.get(), 1);
        assert_eq!(erased.len(), 0);
        assert_eq!(erased.value_type_id(), TypeId::of::<Droppy>());
    }

    fn filled() -> (SparseSet<u32>, Vec<Entity>) {
        let es = entities(3);
        let mut s = SparseSet::new();
        for e in &es {
            s.insert(*e, 1);
        }
        (s, es)
    }

    #[test]
    fn s1_violation_is_reported() {
        let (mut s, _) = filled();
        s.sparse[0] = 2;
        let err = s.check_storage(&alive).unwrap_err();
        assert!(err.contains("S1"), "got: {err}");
    }

    #[test]
    fn s2_violation_is_reported() {
        let (mut s, _) = filled();
        // A slot that holds a valid position of another index's entity.
        s.sparse.push(0);
        let err = s.check_storage(&alive).unwrap_err();
        assert!(err.contains("S2"), "got: {err}");
    }

    #[test]
    fn s3_violation_is_reported() {
        let (s, es) = filled();
        let err = s.check_storage(&|e| e != es[1]).unwrap_err();
        assert!(err.contains("S3"), "got: {err}");
    }
}
