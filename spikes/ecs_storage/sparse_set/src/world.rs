//! The `World`: entities and their components, stored in one sparse set per type.

use core::any::TypeId;
use core::fmt;
use std::collections::BTreeMap;

use engine::genid::GenIdAllocator;

use crate::storage::{ErasedStorage, SparseSet};
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

/// The dense index of a component type: the position of its storage in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ComponentId(u32);

/// Entities and their components. Each component type has its own sparse set.
///
/// # Examples
///
/// ```
/// use spike_ecs_sparse_set::World;
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
    entities: GenIdAllocator<EntityMarker>,
    live: usize,
    ids: BTreeMap<TypeId, ComponentId>,
    storages: Vec<Box<dyn ErasedStorage>>,
}

impl World {
    /// An empty world with no groups.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entities: GenIdAllocator::new(),
            live: 0,
            ids: BTreeMap::new(),
            storages: Vec::new(),
        }
    }

    /// Creates an entity with no components.
    pub fn spawn(&mut self) -> Entity {
        self.live += 1;
        self.entities.allocate()
    }

    /// Removes the entity and all its components. Returns `false` if it was not alive.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.entities.is_alive(entity) {
            return false;
        }
        for storage in &mut self.storages {
            storage.remove_entity(entity);
        }
        self.entities.free(entity);
        self.live -= 1;
        true
    }

    /// Whether `entity` was spawned and not despawned.
    #[must_use]
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entities.is_alive(entity)
    }

    /// The number of live entities.
    #[must_use]
    pub fn len(&self) -> usize {
        self.live
    }

    /// Whether there are no live entities.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Stores `value` as the `T` component of `entity` and returns the value it replaced.
    ///
    /// # Panics
    ///
    /// Panics if `entity` has been despawned.
    #[track_caller]
    pub fn insert<T: 'static>(&mut self, entity: Entity, value: T) -> Option<T> {
        assert!(
            self.entities.is_alive(entity),
            "World::insert called on a despawned entity {entity:?}; spawn an entity first"
        );
        let id = self.id_or_create::<T>();
        self.storage_at_mut::<T>(id).insert(entity, value)
    }

    /// Removes and returns the `T` component of `entity`, or `None` if the entity is despawned
    /// or has none.
    pub fn remove<T: 'static>(&mut self, entity: Entity) -> Option<T> {
        self.storage_mut::<T>()?.remove(entity)
    }

    /// The `T` component of `entity`, or `None` if the entity is despawned or has none.
    #[must_use]
    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        self.storage::<T>()?.get(entity)
    }

    /// The `T` component of `entity`, mutably, or `None` if the entity is despawned or has none.
    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        self.storage_mut::<T>()?.get_mut(entity)
    }

    /// Checks the storage invariants (S1 to S3) and returns the first violation.
    ///
    /// # Errors
    ///
    /// Returns an [`InvariantViolation`] describing the first invariant that does not hold.
    pub fn check_invariants(&self) -> Result<(), InvariantViolation> {
        let is_alive = |e: Entity| self.entities.is_alive(e);
        for storage in &self.storages {
            storage.check(&is_alive)?;
        }
        Ok(())
    }

    fn id_or_create<T: 'static>(&mut self) -> ComponentId {
        if let Some(id) = self.ids.get(&TypeId::of::<T>()) {
            return *id;
        }
        let id = ComponentId(
            u32::try_from(self.storages.len()).expect("fewer than u32::MAX component types"),
        );
        self.storages.push(Box::new(SparseSet::<T>::new()));
        self.ids.insert(TypeId::of::<T>(), id);
        id
    }

    fn storage_at_mut<T: 'static>(&mut self, id: ComponentId) -> &mut SparseSet<T> {
        self.storages[id.0 as usize]
            .as_any_mut()
            .downcast_mut()
            .expect("a component id names the storage of its own type")
    }

    pub(crate) fn storage<T: 'static>(&self) -> Option<&SparseSet<T>> {
        let id = self.ids.get(&TypeId::of::<T>())?;
        self.storages[id.0 as usize].as_any().downcast_ref()
    }

    pub(crate) fn storage_mut<T: 'static>(&mut self) -> Option<&mut SparseSet<T>> {
        let id = *self.ids.get(&TypeId::of::<T>())?;
        Some(self.storage_at_mut(id))
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for World {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Storages<'a>(&'a [Box<dyn ErasedStorage>]);
        impl fmt::Debug for Storages<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_list()
                    .entries(
                        self.0
                            .iter()
                            .map(|s| format!("{}: {}", s.type_name(), s.len())),
                    )
                    .finish()
            }
        }
        f.debug_struct("World")
            .field("live", &self.live)
            .field("storages", &Storages(&self.storages))
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct A(u32);
    #[derive(Debug, PartialEq)]
    struct B(u32);

    #[test]
    fn components_are_independent_per_type() {
        let mut w = World::new();
        let e = w.spawn();
        assert_eq!(w.insert(e, A(1)), None);
        assert_eq!(w.get::<B>(e), None);
        assert_eq!(w.insert(e, B(2)), None);
        assert_eq!(w.insert(e, A(3)), Some(A(1)));
        assert_eq!(w.get::<A>(e), Some(&A(3)));
        assert_eq!(w.get::<B>(e), Some(&B(2)));
        w.get_mut::<B>(e).unwrap().0 = 9;
        assert_eq!(w.remove::<A>(e), Some(A(3)));
        assert_eq!(w.remove::<A>(e), None);
        assert_eq!(w.get::<B>(e), Some(&B(9)));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn despawn_removes_every_component() {
        let mut w = World::new();
        let a = w.spawn();
        let b = w.spawn();
        w.insert(a, A(1));
        w.insert(a, B(1));
        w.insert(b, A(2));
        assert!(w.despawn(a));
        assert!(!w.is_alive(a));
        assert_eq!(w.get::<A>(a), None);
        assert_eq!(w.get::<B>(a), None);
        assert_eq!(w.get::<A>(b), Some(&A(2)));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn stale_id_does_not_reach_the_reused_slot() {
        let mut w = World::new();
        let a = w.spawn();
        w.insert(a, A(1));
        assert!(w.despawn(a));
        let b = w.spawn();
        assert_eq!(a.index(), b.index());
        assert_ne!(a, b);
        w.insert(b, A(2));
        assert_eq!(w.get::<A>(a), None);
        assert_eq!(w.get_mut::<A>(a), None);
        assert_eq!(w.remove::<A>(a), None);
        assert!(!w.despawn(a));
        assert_eq!(w.get::<A>(b), Some(&A(2)));
        assert!(w.is_alive(b));
        assert_eq!(w.len(), 1);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn len_counts_live_entities() {
        let mut w = World::new();
        assert!(w.is_empty());
        let a = w.spawn();
        let _b = w.spawn();
        let _c = w.spawn();
        assert_eq!(w.len(), 3);
        assert!(w.despawn(a));
        assert_eq!(w.len(), 2);
        assert!(!w.despawn(a));
        assert_eq!(w.len(), 2);
        assert!(!w.is_empty());
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
    fn invariant_violation_displays_its_message() {
        let v = InvariantViolation::new("S1 broke".to_string());
        assert_eq!(v.to_string(), "invariant violated: S1 broke");
        assert_eq!(v.clone(), v);
        let _: &dyn std::error::Error = &v;
    }

    #[test]
    fn world_debug_shows_the_live_count_and_storages() {
        let mut w = World::default();
        let e = w.spawn();
        w.insert(e, A(1));
        let s = format!("{w:?}");
        assert!(s.contains("live: 1"), "{s}");
        assert!(s.contains("tests::A"), "{s}");
    }

    #[test]
    fn world_reports_a_corrupt_storage() {
        let mut w = World::new();
        let e = w.spawn();
        w.insert(e, A(1));
        // Free the ID behind the world's back: S3 must notice.
        w.entities.free(e);
        assert!(w.check_invariants().unwrap_err().to_string().contains("S3"));
    }
}
