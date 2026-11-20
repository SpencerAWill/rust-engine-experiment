//! The `World`: entities and their components, stored in one dense column per type.

use core::any::TypeId;
use core::fmt;
use std::collections::BTreeMap;

use engine::collections::HierarchicalBitSet;
use engine::genid::GenIdAllocator;

use crate::column::{DenseColumn, ErasedColumn};
use crate::{Entity, EntityMarker};

/// An internal invariant of a [`World`] does not hold, which would be a bug in the prototype.
/// Returned by [`World::check_invariants`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvariantViolation {
    message: String,
}

impl InvariantViolation {
    pub(crate) fn new(message: String) -> Self {
        Self { message }
    }
}

impl fmt::Display for InvariantViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invariant violated: {}", self.message)
    }
}

impl std::error::Error for InvariantViolation {}

/// The dense index of a component type: the position of its column in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ComponentId(u32);

impl ComponentId {
    /// The position of the component's column in the world.
    pub(crate) fn index(self) -> usize {
        self.0 as usize
    }
}

/// Entities and their components. Each component type has its own dense column: one slot per
/// entity index, and a hierarchical bit set of the indices that hold a value.
///
/// # Examples
///
/// ```
/// use spike_ecs_dense::World;
///
/// struct Name(&'static str);
///
/// let mut world = World::new();
/// let entity = world.spawn();
/// world.insert(entity, Name("ada"));
/// assert_eq!(world.get::<Name>(entity).map(|n| n.0), Some("ada"));
/// assert!(world.despawn(entity));
/// assert!(world.get::<Name>(entity).is_none());
/// ```
pub struct World {
    allocator: GenIdAllocator<EntityMarker>,
    len: usize,
    /// The ID of the entity at each index; read only where `alive` has the index (W2).
    entities: Vec<Entity>,
    /// The indices of the live entities.
    alive: HierarchicalBitSet,
    ids: BTreeMap<TypeId, ComponentId>,
    columns: Vec<Box<dyn ErasedColumn>>,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for World {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let columns: Vec<(&str, usize)> = self
            .columns
            .iter()
            .map(|c| (c.value_type_name(), c.present_count()))
            .collect();
        f.debug_struct("World")
            .field("live", &self.len)
            .field("columns", &columns)
            .finish_non_exhaustive()
    }
}

impl World {
    /// An empty world.
    #[must_use]
    pub fn new() -> Self {
        Self {
            allocator: GenIdAllocator::new(),
            len: 0,
            entities: Vec::new(),
            alive: HierarchicalBitSet::new(),
            ids: BTreeMap::new(),
            columns: Vec::new(),
        }
    }

    /// Creates an entity with no components.
    ///
    /// # Panics
    ///
    /// Panics with a `World::spawn:` message if the world holds the most entities it can
    /// (2³¹−2 entity slots in use or retired). The world is unchanged.
    #[track_caller]
    pub fn spawn(&mut self) -> Entity {
        let Some(entity) = self.allocator.try_allocate() else {
            panic!(
                "World::spawn: the world holds the most entities it can (every entity slot is \
                 in use or retired); despawn entities to free slots"
            )
        };
        let index = entity.index() as usize;
        if index >= self.entities.len() {
            self.entities.resize(index + 1, entity);
        }
        self.entities[index] = entity;
        self.alive.insert(index);
        self.len += 1;
        entity
    }

    /// Removes the entity and all its components. Returns `false` if it was not alive.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.allocator.is_alive(entity) {
            return false;
        }
        let index = entity.index() as usize;
        for column in &mut self.columns {
            column.remove_index(index);
        }
        self.alive.remove(index);
        self.allocator.free(entity);
        self.len -= 1;
        true
    }

    /// Whether `entity` was spawned and not despawned.
    #[must_use]
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.allocator.is_alive(entity)
    }

    /// The number of live entities.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether there are no live entities.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Stores `value` as the `T` component of `entity` and returns the value it replaced.
    ///
    /// # Panics
    ///
    /// Panics if `entity` has been despawned.
    #[track_caller]
    pub fn insert<T: 'static>(&mut self, entity: Entity, value: T) -> Option<T> {
        assert!(
            self.allocator.is_alive(entity),
            "World::insert called on a despawned entity {entity:?}; spawn an entity first"
        );
        self.id_or_create::<T>();
        self.column_mut::<T>()
            .expect("the column of a registered type exists (invariant D2)")
            .insert(entity.index() as usize, value)
    }

    /// Removes and returns the `T` component of `entity`, or `None` if the entity is despawned
    /// or has none.
    pub fn remove<T: 'static>(&mut self, entity: Entity) -> Option<T> {
        if !self.allocator.is_alive(entity) {
            return None;
        }
        self.column_mut::<T>()?.remove(entity.index() as usize)
    }

    /// The `T` component of `entity`, or `None` if the entity is despawned or has none.
    #[must_use]
    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        if !self.allocator.is_alive(entity) {
            return None;
        }
        self.column::<T>()?.get(entity.index() as usize)
    }

    /// The `T` component of `entity`, mutably, or `None` if the entity is despawned or has
    /// none.
    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        if !self.allocator.is_alive(entity) {
            return None;
        }
        self.column_mut::<T>()?.get_mut(entity.index() as usize)
    }

    /// Checks the column invariants (D1, D2) and the world invariants (W1, W2), and returns
    /// the first violation.
    ///
    /// # Errors
    ///
    /// Returns an [`InvariantViolation`] describing the first invariant that does not hold.
    pub fn check_invariants(&self) -> Result<(), InvariantViolation> {
        self.check_types()?;
        for column in &self.columns {
            column.check().map_err(InvariantViolation::new)?;
        }
        self.check_live_set()?;
        for column in &self.columns {
            if let Some(i) = column.present().iter().find(|i| !self.alive.contains(*i)) {
                return Err(InvariantViolation::new(format!(
                    "W1: the column of {} has a value at index {i}, which is not alive",
                    column.value_type_name()
                )));
            }
        }
        Ok(())
    }

    /// D2: each column's type is the one the map assigns to its id, and the ids are exactly
    /// `0..columns.len()`.
    fn check_types(&self) -> Result<(), InvariantViolation> {
        if self.ids.len() != self.columns.len() {
            return Err(InvariantViolation::new(format!(
                "D2: the world maps {} types but holds {} columns",
                self.ids.len(),
                self.columns.len()
            )));
        }
        let mut seen = vec![false; self.columns.len()];
        for (type_id, id) in &self.ids {
            let Some(column) = self.columns.get(id.index()) else {
                return Err(InvariantViolation::new(format!(
                    "D2: component id {} is past the {} columns",
                    id.index(),
                    self.columns.len()
                )));
            };
            if column.value_type_id() != *type_id || seen[id.index()] {
                return Err(InvariantViolation::new(format!(
                    "D2: component id {} does not belong to the column of {}",
                    id.index(),
                    column.value_type_name()
                )));
            }
            seen[id.index()] = true;
        }
        Ok(())
    }

    /// W2: `alive` has `len` members, and each maps to a live entity with its own index.
    fn check_live_set(&self) -> Result<(), InvariantViolation> {
        let members = self.alive.len();
        if members != self.len {
            return Err(InvariantViolation::new(format!(
                "W2: {members} indices are alive but len() is {}",
                self.len
            )));
        }
        for i in &self.alive {
            let ok = self
                .entities
                .get(i)
                .is_some_and(|e| e.index() as usize == i && self.allocator.is_alive(*e));
            if !ok {
                return Err(InvariantViolation::new(format!(
                    "W2: index {i} is alive but its entity entry is missing, stale or has another index"
                )));
            }
        }
        Ok(())
    }

    /// The id of `T`'s column, creating the column on the first use.
    fn id_or_create<T: 'static>(&mut self) -> ComponentId {
        let type_id = TypeId::of::<T>();
        if let Some(id) = self.ids.get(&type_id) {
            return *id;
        }
        let id = ComponentId(
            u32::try_from(self.columns.len()).expect("more than u32::MAX component types"),
        );
        self.ids.insert(type_id, id);
        self.columns.push(Box::new(DenseColumn::<T>::new()));
        id
    }

    fn column<T: 'static>(&self) -> Option<&DenseColumn<T>> {
        let id = self.ids.get(&TypeId::of::<T>())?;
        self.columns[id.index()].as_any().downcast_ref()
    }

    fn column_mut<T: 'static>(&mut self) -> Option<&mut DenseColumn<T>> {
        let id = self.ids.get(&TypeId::of::<T>())?;
        self.columns[id.index()].as_any_mut().downcast_mut()
    }
}

#[cfg(test)]
mod tests {
    use core::cell::Cell;
    use std::rc::Rc;

    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct A(u32);
    #[derive(Debug, PartialEq)]
    struct B(u32);

    struct Tracked<const K: u8>(Rc<Cell<u32>>);

    impl<const K: u8> Drop for Tracked<K> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    /// The indices with a `T` and the value at each index below `upto`, for comparing a
    /// column before and after an operation on another type.
    fn snapshot<T: Clone + 'static>(w: &World, upto: u32) -> (Vec<usize>, Vec<Option<T>>) {
        let bits = w
            .column::<T>()
            .map_or_else(Vec::new, |c| c.present().iter().collect());
        let values = (0..upto)
            .map(|i| {
                let slot = w.entities.get(i as usize).copied();
                slot.and_then(|e| w.get::<T>(e).cloned())
            })
            .collect();
        (bits, values)
    }

    fn panic_message(f: impl FnOnce()) -> String {
        let failed = std::panic::catch_unwind(core::panic::AssertUnwindSafe(f));
        let payload = failed.unwrap_err();
        payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or_default()
            .to_owned()
    }

    #[test]
    fn components_are_independent_per_type() {
        let mut w = World::new();
        let e = w.spawn();
        let other = w.spawn();
        w.insert(e, A(1));
        w.insert(other, A(5));
        let before = snapshot::<A>(&w, 2);
        assert_eq!(w.insert(e, B(2)), None);
        assert_eq!(snapshot::<A>(&w, 2), before);
        assert_eq!(w.insert(other, B(3)), None);
        assert_eq!(w.remove::<B>(e), Some(B(2)));
        assert_eq!(snapshot::<A>(&w, 2), before);
        assert_eq!(w.get::<B>(other), Some(&B(3)));
        w.get_mut::<B>(other).unwrap().0 = 9;
        assert_eq!(w.get::<B>(other), Some(&B(9)));
        assert_eq!(snapshot::<A>(&w, 2), before);
        assert_eq!(w.remove::<A>(e), Some(A(1)));
        assert_eq!(w.remove::<A>(e), None);
        assert_eq!(w.get::<A>(other), Some(&A(5)));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn replacing_a_value_returns_the_old_one() {
        let mut w = World::new();
        let e = w.spawn();
        assert_eq!(w.insert(e, A(1)), None);
        assert_eq!(w.insert(e, A(2)), Some(A(1)));
        assert_eq!(w.get::<A>(e), Some(&A(2)));
        assert_eq!(w.column::<A>().unwrap().present().len(), 1);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn despawn_clears_every_column() {
        let drops = Rc::new(Cell::new(0));
        let mut w = World::new();
        let e = w.spawn();
        let keep = w.spawn();
        w.insert(e, Tracked::<0>(Rc::clone(&drops)));
        w.insert(e, Tracked::<1>(Rc::clone(&drops)));
        w.insert(e, Tracked::<2>(Rc::clone(&drops)));
        w.insert(keep, Tracked::<0>(Rc::clone(&drops)));
        w.insert(keep, Tracked::<2>(Rc::clone(&drops)));
        assert!(w.despawn(e));
        assert_eq!(drops.get(), 3);
        assert_eq!(w.column::<Tracked<0>>().unwrap().present().len(), 1);
        assert_eq!(w.column::<Tracked<1>>().unwrap().present().len(), 0);
        assert_eq!(w.column::<Tracked<2>>().unwrap().present().len(), 1);
        assert!(w.get::<Tracked<0>>(keep).is_some());
        assert!(w.get::<Tracked<2>>(keep).is_some());
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn stale_id_does_not_reach_the_reused_slot() {
        let mut w = World::new();
        let a = w.spawn();
        w.insert(a, A(1));
        w.insert(a, B(2));
        assert!(w.despawn(a));
        let b = w.spawn();
        assert_eq!(a.index(), b.index());
        assert_ne!(a, b);
        assert_eq!(w.get::<A>(b), None);
        assert_eq!(w.get::<B>(b), None);
        w.insert(b, A(7));
        assert_eq!(w.get::<A>(a), None);
        assert_eq!(w.get_mut::<A>(a), None);
        assert_eq!(w.remove::<A>(a), None);
        assert!(!w.despawn(a));
        assert!(w.is_alive(b));
        assert!(!w.is_alive(a));
        assert_eq!(w.get::<A>(b), Some(&A(7)));
        assert_eq!(w.len(), 1);
        let message = panic_message(|| {
            w.insert(a, A(9));
        });
        assert!(
            message.starts_with("World::insert called on a despawned entity"),
            "got: {message}"
        );
        assert_eq!(w.get::<A>(b), Some(&A(7)));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn len_counts_live_entities() {
        let mut w = World::new();
        assert!(w.is_empty());
        let a = w.spawn();
        let b = w.spawn();
        assert_eq!(w.len(), 2);
        assert!(!w.is_empty());
        assert!(w.despawn(a));
        assert_eq!(w.len(), 1);
        assert!(!w.despawn(a));
        assert_eq!(w.len(), 1);
        assert!(w.despawn(b));
        assert!(w.is_empty());
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    #[should_panic(expected = "World::insert called on a despawned entity")]
    fn insert_on_a_despawned_entity_panics() {
        let mut w = World::new();
        let e = w.spawn();
        w.despawn(e);
        w.insert(e, A(1));
    }

    #[test]
    fn a_failed_spawn_leaves_len_unchanged() {
        let mut w = World::new();
        w.allocator = GenIdAllocator::with_fixed_capacity(1);
        let first = w.spawn();
        let message = panic_message(|| {
            w.spawn();
        });
        assert!(message.starts_with("World::spawn:"), "got: {message}");
        assert_eq!(w.len(), 1);
        assert_eq!(w.alive.len(), 1);
        assert!(w.is_alive(first));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    fn violation(w: &World) -> String {
        w.check_invariants().unwrap_err().to_string()
    }

    #[test]
    fn d2_violation_is_reported() {
        let mut w = World::new();
        let e = w.spawn();
        w.insert(e, A(1));
        w.insert(e, B(2));
        w.ids.insert(TypeId::of::<A>(), ComponentId(1));
        assert!(violation(&w).contains("D2"), "got: {}", violation(&w));

        let mut w = World::new();
        let e = w.spawn();
        w.insert(e, A(1));
        w.columns.push(Box::new(DenseColumn::<B>::new()));
        assert!(violation(&w).contains("D2"), "got: {}", violation(&w));
    }

    #[test]
    fn w1_violation_is_reported() {
        let mut w = World::new();
        let e = w.spawn();
        w.insert(e, A(1));
        // A consistent column entry (D1 holds) at an index that is not alive.
        w.column_mut::<A>().unwrap().insert(9, A(2));
        let message = violation(&w);
        assert!(message.contains("W1"), "got: {message}");
    }

    #[test]
    fn w2_violation_is_reported() {
        let mut w = World::new();
        let e = w.spawn();
        w.insert(e, A(1));
        w.len = 5;
        assert!(violation(&w).contains("W2"), "got: {}", violation(&w));
        w.len = 1;
        assert_eq!(w.check_invariants(), Ok(()));
        w.alive.insert(4);
        w.len = 2;
        assert!(violation(&w).contains("W2"), "got: {}", violation(&w));
        w.alive.remove(4);
        w.len = 1;
        assert_eq!(w.check_invariants(), Ok(()));
        // The table maps the index to some other entity.
        let other = w.allocator.allocate();
        w.entities[e.index() as usize] = other;
        assert!(violation(&w).contains("W2"), "got: {}", violation(&w));
    }

    #[test]
    fn invariant_violation_displays_its_message() {
        let v = InvariantViolation::new("D1: example".to_owned());
        assert_eq!(v.to_string(), "invariant violated: D1: example");
    }

    #[test]
    fn world_debug_shows_the_live_count_and_columns() {
        let mut w = World::new();
        let e = w.spawn();
        w.insert(e, A(1));
        let shown = format!("{w:?}");
        assert!(shown.contains("live: 1"), "got: {shown}");
        assert!(shown.contains("::A"), "got: {shown}");
        assert!(shown.contains(", 1)"), "got: {shown}");
        assert_eq!(
            format!("{:?}", World::default()),
            format!("{:?}", World::new())
        );
    }
}
