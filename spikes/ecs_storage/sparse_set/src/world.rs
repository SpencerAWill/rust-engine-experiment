//! The `World`: entities and their components, stored in one sparse set per type.

use core::any::TypeId;
use core::fmt;
use std::collections::BTreeMap;

use engine::genid::GenIdAllocator;

use crate::query::{GroupSpec, Query, QueryMut, QueryParam, ReadOnly};
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

/// The dense index of a component type: the position of its storage in the world. Public only
/// because a sealed trait mentions it; `world` is a private module, so it cannot be named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentId(u32);

impl ComponentId {
    /// The position of the component's storage in the world.
    pub(crate) fn index(self) -> usize {
        self.0 as usize
    }
}

/// An owning group: member storages whose first `len` entries are the same entities in the
/// same order (G1) and are exactly the entities that have every member (G2).
#[derive(Debug)]
pub(crate) struct Group {
    members: Vec<ComponentId>,
    len: usize,
}

/// A group declared on a [`WorldBuilder`], before the world exists.
struct Declared {
    types: Vec<(TypeId, &'static str)>,
    register: fn(&mut World) -> Vec<ComponentId>,
}

/// Declares owning groups, then builds a [`World`].
///
/// # Examples
///
/// ```
/// use spike_ecs_sparse_set::World;
///
/// struct Position(f32);
/// struct Velocity(f32);
///
/// let mut world = World::builder().group::<(Position, Velocity)>().build();
/// let e = world.spawn();
/// world.insert(e, Position(0.0));
/// world.insert(e, Velocity(1.0));
/// assert!(world.check_invariants().is_ok());
/// ```
#[must_use]
#[derive(Default)]
pub struct WorldBuilder {
    groups: Vec<Declared>,
}

impl WorldBuilder {
    /// A builder with no groups.
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares an owning group over the types of `G`.
    ///
    /// # Panics
    ///
    /// Panics if a type of `G` already belongs to a declared group, or appears twice in `G`.
    #[track_caller]
    pub fn group<G: GroupSpec>(mut self) -> Self {
        let types = G::types();
        for (i, (id, name)) in types.iter().enumerate() {
            assert!(
                !types[..i].iter().any(|(other, _)| other == id),
                "WorldBuilder::group: `{name}` appears twice in the group; list each type once"
            );
            assert!(
                !self
                    .groups
                    .iter()
                    .any(|g| g.types.iter().any(|(other, _)| other == id)),
                "WorldBuilder::group: `{name}` already belongs to a group; a type can be in at most one group"
            );
        }
        self.groups.push(Declared {
            types,
            register: G::register,
        });
        self
    }

    /// Creates the world, with a storage for each member of each group.
    pub fn build(self) -> World {
        let mut world = World::empty();
        for declared in self.groups {
            let members = (declared.register)(&mut world);
            let index = world.groups.len();
            for m in &members {
                world.owner[m.0 as usize] = Some(index);
            }
            world.groups.push(Group { members, len: 0 });
        }
        world
    }
}

impl fmt::Debug for WorldBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let groups: Vec<Vec<&str>> = self
            .groups
            .iter()
            .map(|g| g.types.iter().map(|(_, n)| *n).collect())
            .collect();
        f.debug_struct("WorldBuilder")
            .field("groups", &groups)
            .finish()
    }
}

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
    groups: Vec<Group>,
    /// The group that owns each component type, indexed by `ComponentId`.
    owner: Vec<Option<usize>>,
}

impl World {
    fn empty() -> Self {
        Self {
            entities: GenIdAllocator::new(),
            live: 0,
            ids: BTreeMap::new(),
            storages: Vec::new(),
            groups: Vec::new(),
            owner: Vec::new(),
        }
    }

    /// An empty world with no groups.
    #[must_use]
    pub fn new() -> Self {
        WorldBuilder::new().build()
    }

    /// A builder that declares owning groups before the world exists.
    pub fn builder() -> WorldBuilder {
        WorldBuilder::new()
    }

    /// Creates an entity with no components.
    pub fn spawn(&mut self) -> Entity {
        let entity = self.entities.allocate();
        self.live += 1;
        entity
    }

    /// Removes the entity and all its components. Returns `false` if it was not alive.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.entities.is_alive(entity) {
            return false;
        }
        for g in 0..self.groups.len() {
            self.leave_group(g, entity);
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
        let old = self.storage_at_mut::<T>(id).insert(entity, value);
        if let (None, Some(g)) = (&old, self.owner[id.0 as usize]) {
            self.join_group(g, entity);
        }
        old
    }

    /// Removes and returns the `T` component of `entity`, or `None` if the entity is despawned
    /// or has none.
    pub fn remove<T: 'static>(&mut self, entity: Entity) -> Option<T> {
        let id = *self.ids.get(&TypeId::of::<T>())?;
        if let Some(g) = self.owner[id.0 as usize] {
            self.leave_group(g, entity);
        }
        self.storage_at_mut::<T>(id).remove(entity)
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

    /// Checks the storage invariants (S1 to S3) and the group invariants (G1, G2), and returns
    /// the first violation.
    ///
    /// # Errors
    ///
    /// Returns an [`InvariantViolation`] describing the first invariant that does not hold.
    pub fn check_invariants(&self) -> Result<(), InvariantViolation> {
        let is_alive = |e: Entity| self.entities.is_alive(e);
        for storage in &self.storages {
            storage.check(&is_alive)?;
        }
        for group in &self.groups {
            self.check_group(group)?;
        }
        Ok(())
    }

    fn check_group(&self, group: &Group) -> Result<(), InvariantViolation> {
        let first = &self.storages[group.members[0].0 as usize];
        let names = || {
            group
                .members
                .iter()
                .map(|m| self.storages[m.0 as usize].type_name())
                .collect::<Vec<_>>()
        };
        for m in &group.members {
            let storage = &self.storages[m.0 as usize];
            if storage.len() < group.len {
                return Err(InvariantViolation::new(format!(
                    "G1: the group {:?} has length {}, but the storage of {} holds only {}",
                    names(),
                    group.len,
                    storage.type_name(),
                    storage.len()
                )));
            }
            if storage.entities()[..group.len] != first.entities()[..group.len] {
                return Err(InvariantViolation::new(format!(
                    "G1: the first {} entries of the storage of {} differ from those of {} in the group {:?}",
                    group.len,
                    storage.type_name(),
                    first.type_name(),
                    names()
                )));
            }
        }
        for (position, entity) in first.entities().iter().enumerate() {
            let has_all = group
                .members
                .iter()
                .all(|m| self.storages[m.0 as usize].index_of(*entity).is_some());
            if has_all != (position < group.len) {
                return Err(InvariantViolation::new(format!(
                    "G2: in the group {:?} of length {}, {entity:?} at position {position} has every member: {has_all}",
                    names(),
                    group.len
                )));
            }
        }
        Ok(())
    }

    /// A read-only query over the entities that have the required types of `Q`.
    pub fn query<Q: ReadOnly>(&self) -> Query<'_, Q> {
        Query::new(self)
    }

    /// A query that may write through `&mut T` parameters.
    pub fn query_mut<Q: QueryParam>(&mut self) -> QueryMut<'_, Q> {
        QueryMut::new(self)
    }

    pub(crate) fn component_id(&self, ty: TypeId) -> Option<ComponentId> {
        self.ids.get(&ty).copied()
    }

    pub(crate) fn storages(&self) -> &[Box<dyn ErasedStorage>] {
        &self.storages
    }

    pub(crate) fn storages_mut(&mut self) -> &mut [Box<dyn ErasedStorage>] {
        &mut self.storages
    }

    /// The length of group `group`, for tests.
    #[cfg(test)]
    pub(crate) fn group_len(&self, group: usize) -> usize {
        self.groups[group].len
    }

    /// After a new insert into an owned storage: moves `entity` to the end of the group's
    /// prefix if it now has every member.
    fn join_group(&mut self, g: usize, entity: Entity) {
        let len = self.groups[g].len;
        for m in &self.groups[g].members {
            if self.storages[m.0 as usize].index_of(entity).is_none() {
                return;
            }
        }
        for k in 0..self.groups[g].members.len() {
            let m = self.groups[g].members[k].0 as usize;
            let at = self.storages[m]
                .index_of(entity)
                .expect("checked above that every member has the entity");
            self.storages[m].swap(at, len);
        }
        self.groups[g].len = len + 1;
    }

    /// Before a remove from an owned storage: moves `entity` out of the group's prefix, to
    /// position `len - 1`, if it is in it.
    fn leave_group(&mut self, g: usize, entity: Entity) {
        let len = self.groups[g].len;
        let first = self.groups[g].members[0].0 as usize;
        match self.storages[first].index_of(entity) {
            Some(p) if p < len => {}
            _ => return,
        }
        let last = len - 1;
        for k in 0..self.groups[g].members.len() {
            let m = self.groups[g].members[k].0 as usize;
            let at = self.storages[m]
                .index_of(entity)
                .expect("an entity in the prefix has every member");
            self.storages[m].swap(at, last);
        }
        self.groups[g].len = last;
    }

    pub(crate) fn id_or_create<T: 'static>(&mut self) -> ComponentId {
        if let Some(id) = self.ids.get(&TypeId::of::<T>()) {
            return *id;
        }
        let id = ComponentId(
            u32::try_from(self.storages.len()).expect("fewer than u32::MAX component types"),
        );
        self.storages.push(Box::new(SparseSet::<T>::new()));
        self.owner.push(None);
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

    #[derive(Debug, PartialEq)]
    struct C(u32);

    fn grouped() -> World {
        World::builder().group::<(A, B)>().build()
    }

    #[test]
    fn joining_and_leaving_a_group_moves_the_entity() {
        let mut w = grouped();
        let e1 = w.spawn();
        let e2 = w.spawn();
        w.insert(e1, A(1));
        w.insert(e2, A(2));
        assert_eq!(w.group_len(0), 0);
        w.insert(e2, B(2));
        assert_eq!(w.group_len(0), 1);
        w.insert(e1, B(1));
        assert_eq!(w.group_len(0), 2);
        assert_eq!(w.check_invariants(), Ok(()));
        assert_eq!(w.remove::<A>(e2), Some(A(2)));
        assert_eq!(w.group_len(0), 1);
        assert_eq!(w.get::<B>(e2), Some(&B(2)));
        assert_eq!(w.check_invariants(), Ok(()));
        w.insert(e2, A(5));
        assert_eq!(w.group_len(0), 2);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn replacing_an_owned_value_keeps_the_group_length() {
        let mut w = grouped();
        let e = w.spawn();
        w.insert(e, A(1));
        w.insert(e, B(1));
        assert_eq!(w.group_len(0), 1);
        assert_eq!(w.insert(e, A(2)), Some(A(1)));
        assert_eq!(w.group_len(0), 1);
        assert_eq!(w.get::<A>(e), Some(&A(2)));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn despawning_a_grouped_entity_shrinks_the_group() {
        let mut w = grouped();
        let es: Vec<_> = (0..3).map(|_| w.spawn()).collect();
        for e in &es {
            w.insert(*e, A(0));
            w.insert(*e, B(0));
        }
        assert_eq!(w.group_len(0), 3);
        assert!(w.despawn(es[0]));
        assert_eq!(w.group_len(0), 2);
        assert_eq!(w.check_invariants(), Ok(()));
        // An entity with only one member is not in the group.
        w.remove::<B>(es[1]);
        assert!(w.despawn(es[1]));
        assert_eq!(w.group_len(0), 1);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    #[should_panic(
        expected = "WorldBuilder::group: `spike_ecs_sparse_set::world::tests::B` already belongs to a group"
    )]
    fn type_in_two_groups_panics() {
        let _ = World::builder().group::<(A, B)>().group::<(B, C)>();
    }

    #[test]
    #[should_panic(
        expected = "WorldBuilder::group: `spike_ecs_sparse_set::world::tests::A` appears twice in the group"
    )]
    fn repeated_type_in_a_group_panics() {
        let _ = World::builder().group::<(A, B, A)>();
    }

    #[test]
    fn g1_violation_is_reported() {
        let mut w = grouped();
        for _ in 0..2 {
            let e = w.spawn();
            w.insert(e, A(0));
            w.insert(e, B(0));
        }
        assert_eq!(w.check_invariants(), Ok(()));
        // Reorder one member storage behind the group's back.
        w.storages[0].swap(0, 1);
        assert!(w.check_invariants().unwrap_err().to_string().contains("G1"));
    }

    #[test]
    fn g2_violation_is_reported() {
        let mut w = grouped();
        let e = w.spawn();
        w.insert(e, A(0));
        w.insert(e, B(0));
        w.groups[0].len = 0;
        assert!(w.check_invariants().unwrap_err().to_string().contains("G2"));
    }

    #[test]
    fn a_failed_spawn_leaves_len_unchanged() {
        let mut w = World::new();
        w.entities = GenIdAllocator::with_fixed_capacity(1);
        w.spawn();
        let failed = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| w.spawn()));
        assert!(failed.is_err());
        assert_eq!(w.len(), 1);
    }
}
