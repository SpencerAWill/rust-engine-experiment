//! The `World`: entities and their components, table types in one table per set of table types
//! and sparse types in one sparse set per type. Adapted from the archetype prototype's (M3.4)
//! and the sparse-set prototype's (M3.3) worlds.

use core::any::TypeId;
use core::fmt;
use std::collections::BTreeMap;

use engine::genid::GenIdAllocator;

use crate::archetype::{Archetype, ArchetypeId, ComponentId};
use crate::bundle::{self, Bundle, BundleInfo};
use crate::component::{Component, StorageKind, seal};
use crate::query::{Query, QueryMut, QueryParam, ReadOnly};
use crate::sparse::{ErasedSparseSet, SparseSet};
use crate::table::{Column, ErasedColumn, Table};
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

/// Where an entity's row is: its archetype and its row in that archetype's table. Sparse values
/// are found through their sets, not through the location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Location {
    pub(crate) archetype: ArchetypeId,
    pub(crate) row: u32,
}

impl Location {
    /// The location of an entity that has no row (despawned, or never spawned).
    pub(crate) const NONE: Self = Self {
        archetype: ArchetypeId::NONE,
        row: u32::MAX,
    };
}

/// What the world knows about one component type.
#[derive(Debug)]
pub(crate) struct ComponentInfo {
    pub(crate) type_id: TypeId,
    pub(crate) name: &'static str,
    pub(crate) kind: StorageKind,
    /// A sparse type's set: its index in `sparse_sets`.
    pub(crate) sparse: Option<usize>,
    /// A table type's way to create an empty column, for a new archetype that has it.
    pub(crate) new_column: Option<fn() -> Box<dyn ErasedColumn>>,
}

/// Entities and their components. Each component type picks its storage: table types live in
/// one table per distinct set of table types, and inserting or removing one moves the entity's
/// row between tables; sparse types live in a set of their own and never move a row.
///
/// # Examples
///
/// ```
/// use spike_ecs_hybrid::{Component, SparseStorage, TableStorage, World};
///
/// struct Name(&'static str);
/// struct Stunned;
/// impl Component for Name { type Storage = TableStorage; }
/// impl Component for Stunned { type Storage = SparseStorage; }
///
/// let mut world = World::new();
/// let entity = world.spawn();
/// world.insert(entity, Name("ada"));
/// world.insert(entity, Stunned);
/// assert_eq!(world.archetype_count(), 2); // the empty table and (Name)
/// assert!(world.remove::<Stunned>(entity).is_some());
/// assert!(world.despawn(entity));
/// assert!(world.get::<Name>(entity).is_none());
/// ```
pub struct World {
    entities: GenIdAllocator<EntityMarker>,
    live: usize,
    ids: BTreeMap<TypeId, ComponentId>,
    components: Vec<ComponentInfo>,
    archetypes: Vec<Archetype>,
    /// One cached [`BundleInfo`] per bundle type spawned so far.
    bundles: BTreeMap<TypeId, BundleInfo>,
    by_components: BTreeMap<Box<[ComponentId]>, ArchetypeId>,
    /// One set per registered sparse type, indexed by `ComponentInfo::sparse`.
    sparse_sets: Vec<Box<dyn ErasedSparseSet>>,
    /// Indexed by `entity.index()`.
    locations: Vec<Location>,
    /// How many times `by_components` was read, so a test can tell a warm transition from a
    /// cold one.
    #[cfg(test)]
    lookups: usize,
}

impl World {
    /// An empty world with one archetype, the one with no components.
    #[must_use]
    pub fn new() -> Self {
        let empty: Box<[ComponentId]> = Box::new([]);
        let mut by_components = BTreeMap::new();
        by_components.insert(empty.clone(), ArchetypeId::EMPTY);
        Self {
            entities: GenIdAllocator::new(),
            live: 0,
            ids: BTreeMap::new(),
            components: Vec::new(),
            archetypes: vec![Archetype::new(empty, Table::new(Box::new([])))],
            bundles: BTreeMap::new(),
            by_components,
            sparse_sets: Vec::new(),
            locations: Vec::new(),
            #[cfg(test)]
            lookups: 0,
        }
    }

    /// Creates an entity with no components.
    ///
    /// # Panics
    ///
    /// Panics with a `World::spawn:` message if the world holds the most entities it can
    /// (2³¹−2 entity slots in use or retired). The world is unchanged in that case.
    #[track_caller]
    pub fn spawn(&mut self) -> Entity {
        let Some(entity) = self.entities.try_allocate() else {
            panic!(
                "World::spawn: the world holds the most entities it can (every entity slot is \
                 in use or retired); despawn entities to free slots"
            )
        };
        let table = &mut self.archetypes[ArchetypeId::EMPTY.index()].table;
        let row = table.len();
        table.push_empty(entity);
        self.set_location(entity, ArchetypeId::EMPTY, row);
        self.live += 1;
        entity
    }

    /// Creates an entity with the components of `bundle`, of either storage kind. The table
    /// elements go straight into the table of the bundle's own archetype (the one with exactly
    /// its table types) as one row, and each sparse element into its set, so the entity never
    /// passes through an intermediate archetype. A bundle of sparse types only spawns into the
    /// empty archetype.
    ///
    /// # Examples
    ///
    /// ```
    /// use spike_ecs_hybrid::{Component, SparseStorage, TableStorage, World};
    ///
    /// struct Name(&'static str);
    /// struct Hp(u32);
    /// struct Stunned;
    /// impl Component for Name { type Storage = TableStorage; }
    /// impl Component for Hp { type Storage = TableStorage; }
    /// impl Component for Stunned { type Storage = SparseStorage; }
    ///
    /// let mut world = World::new();
    /// let entity = world.spawn_with((Stunned, Name("ada"), Hp(3)));
    /// assert_eq!(world.archetype_count(), 2); // the empty table and (Name, Hp)
    /// assert_eq!(world.get::<Hp>(entity).map(|hp| hp.0), Some(3));
    /// assert!(world.get::<Stunned>(entity).is_some());
    /// ```
    ///
    /// # Panics
    ///
    /// Panics with a `World::spawn_with:` message if a type appears twice in the bundle, or if
    /// the world holds the most entities it can (2³¹−2 entity slots in use or retired). In
    /// either case `len`, every existing entity and the set of registered types, archetypes,
    /// sparse sets and cached bundle infos are unchanged on the first call of its kind.
    #[track_caller]
    pub fn spawn_with<B: Bundle>(&mut self, bundle: B) -> Entity {
        let key = TypeId::of::<B>();
        // Warm: one lookup, then allocate and push.
        let (entity, archetype, row) = if let Some(info) = self.bundles.get(&key) {
            let entity = Self::allocate(&mut self.entities);
            let row = Self::place(
                &mut self.archetypes,
                &mut self.sparse_sets,
                info,
                entity,
                bundle,
            );
            (entity, info.archetype, row)
        } else {
            // A repeated type is checked before the allocation, so it uses no entity id; the
            // check registers nothing, so the allocation still comes before any registration.
            Self::check_bundle_types::<B>();
            let entity = Self::allocate(&mut self.entities);
            let info = self.build_bundle_info::<B>();
            let info = self.bundles.entry(key).or_insert(info);
            let row = Self::place(
                &mut self.archetypes,
                &mut self.sparse_sets,
                info,
                entity,
                bundle,
            );
            (entity, info.archetype, row)
        };
        self.set_location(entity, archetype, row);
        self.live += 1;
        entity
    }

    /// Allocates an entity id. Takes the field so the caller can hold a `bundles` borrow.
    ///
    /// # Panics
    ///
    /// Panics with a `World::spawn_with:` message if the allocator is full.
    #[track_caller]
    fn allocate(entities: &mut GenIdAllocator<EntityMarker>) -> Entity {
        let Some(entity) = entities.try_allocate() else {
            panic!(
                "World::spawn_with: the world holds the most entities it can (every entity slot \
                 is in use or retired); despawn entities to free slots"
            )
        };
        entity
    }

    /// Pushes `bundle` as `entity`'s row of the table `info` names, and its sparse elements
    /// into their sets; returns the row.
    fn place<B: Bundle>(
        archetypes: &mut [Archetype],
        sparse_sets: &mut [Box<dyn ErasedSparseSet>],
        info: &BundleInfo,
        entity: Entity,
        bundle: B,
    ) -> usize {
        let table = &mut archetypes[info.archetype.index()].table;
        let row = table.len();
        table.push_bundle(entity, bundle, &info.columns, sparse_sets, &info.sparse);
        row
    }

    /// Panics if a type appears twice in `B`.
    #[track_caller]
    fn check_bundle_types<B: Bundle>() {
        let types = B::types();
        for (k, (type_id, name)) in types.iter().enumerate() {
            assert!(
                types[..k].iter().all(|(other, _)| other != type_id),
                "World::spawn_with: `{name}` appears twice in the bundle; a bundle holds each \
                 component type once"
            );
        }
    }

    /// Registers the bundle's types and finds or creates the archetype of its table types.
    fn build_bundle_info<B: Bundle>(&mut self) -> BundleInfo {
        let types = B::types();
        let registered = B::register(self);
        let table_ids: Vec<ComponentId> = registered
            .iter()
            .filter(|(_, kind)| *kind == StorageKind::Table)
            .map(|(id, _)| *id)
            .collect();
        let list = bundle::sorted(&table_ids);
        let archetype = self.find_or_create(list.clone());
        let columns = table_ids
            .iter()
            .map(|id| {
                list.binary_search(id)
                    .expect("the bundle's table ids are in its list")
            })
            .collect();
        let sparse = registered
            .iter()
            .filter(|(_, kind)| *kind == StorageKind::Sparse)
            .map(|(id, _)| {
                self.components[id.index()]
                    .sparse
                    .expect("S4: a sparse type names its set")
            })
            .collect();
        BundleInfo {
            archetype,
            columns,
            sparse,
            types: types.into_iter().map(|(type_id, _)| type_id).collect(),
        }
    }

    /// Removes the entity and all its components, of both kinds. Returns `false` if it was not
    /// alive.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.entities.is_alive(entity) {
            return false;
        }
        let Location { archetype, row } = self.location_of(entity);
        let swapped = self.archetypes[archetype.index()]
            .table
            .swap_remove(row as usize);
        self.fix_swapped(swapped, row as usize);
        for set in &mut self.sparse_sets {
            set.remove_entity(entity);
        }
        self.locations[entity.index() as usize] = Location::NONE;
        self.entities.free(entity);
        self.live -= 1;
        true
    }

    /// A read-only query over the entities that have every type in `Q`, of either storage
    /// kind. Chain [`with`](Query::with) and [`without`](Query::without) for filters, then run
    /// it with [`for_each`](Query::for_each) or [`iter`](Query::iter).
    ///
    /// # Examples
    ///
    /// ```
    /// use spike_ecs_hybrid::{Component, SparseStorage, TableStorage, World};
    ///
    /// struct Hp(u32);
    /// struct Poisoned(u32);
    /// impl Component for Hp { type Storage = TableStorage; }
    /// impl Component for Poisoned { type Storage = SparseStorage; }
    ///
    /// let mut world = World::new();
    /// world.spawn_with((Hp(10), Poisoned(2)));
    /// world.spawn_with((Hp(7),));
    ///
    /// let mut total = 0;
    /// world.query::<(&Hp, &Poisoned)>().for_each(|(hp, _)| total += hp.0);
    /// assert_eq!(total, 10);
    /// ```
    pub fn query<Q: ReadOnly>(&self) -> Query<'_, Q> {
        Query::new(self)
    }

    /// A query that may write: `&mut T` and `Option<&mut T>` parameters are allowed. Run it
    /// with [`for_each`](QueryMut::for_each).
    pub fn query_mut<Q: QueryParam>(&mut self) -> QueryMut<'_, Q> {
        QueryMut::new(self)
    }

    /// The id of the type `type_id`, if it was ever registered.
    pub(crate) fn component_id_of(&self, type_id: TypeId) -> Option<ComponentId> {
        self.ids.get(&type_id).copied()
    }

    /// What the world knows about the type `id`.
    pub(crate) fn component_info(&self, id: ComponentId) -> &ComponentInfo {
        &self.components[id.index()]
    }

    /// The archetypes and the sparse sets, borrowed separately, for a read-only query run.
    pub(crate) fn parts(&self) -> (&[Archetype], &[Box<dyn ErasedSparseSet>]) {
        (&self.archetypes, &self.sparse_sets)
    }

    /// The archetypes and the sparse sets, borrowed separately and exclusively, for a mutable
    /// query run.
    pub(crate) fn parts_mut(&mut self) -> (&mut [Archetype], &mut [Box<dyn ErasedSparseSet>]) {
        (&mut self.archetypes, &mut self.sparse_sets)
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

    /// The number of archetypes (tables) created so far, the one with no components included.
    /// Archetypes are never removed, and inserting a sparse type never creates one.
    #[must_use]
    pub fn archetype_count(&self) -> usize {
        self.archetypes.len()
    }

    /// Stores `value` as the `T` component of `entity` and returns the value it replaced. A
    /// table type that the entity lacked moves it to the archetype with `T` added; a sparse
    /// type goes into its set and moves nothing.
    ///
    /// # Panics
    ///
    /// Panics if `entity` has been despawned.
    #[track_caller]
    pub fn insert<T: Component>(&mut self, entity: Entity, value: T) -> Option<T> {
        assert!(
            self.entities.is_alive(entity),
            "World::insert called on a despawned entity {entity:?}; spawn an entity first"
        );
        <T::Storage as seal::Storage>::insert(self, entity, value)
    }

    /// Removes and returns the `T` component of `entity`, or `None` if the entity is despawned
    /// or has none. A table type moves the entity to the archetype with `T` removed.
    pub fn remove<T: Component>(&mut self, entity: Entity) -> Option<T> {
        <T::Storage as seal::Storage>::remove(self, entity)
    }

    /// The `T` component of `entity`, or `None` if the entity is despawned or has none.
    #[must_use]
    pub fn get<T: Component>(&self, entity: Entity) -> Option<&T> {
        <T::Storage as seal::Storage>::get(self, entity)
    }

    /// The `T` component of `entity`, mutably, or `None` if the entity is despawned or has none.
    pub fn get_mut<T: Component>(&mut self, entity: Entity) -> Option<&mut T> {
        <T::Storage as seal::Storage>::get_mut(self, entity)
    }

    // The table paths: M3.4's.

    pub(crate) fn table_insert<T: Component>(&mut self, entity: Entity, value: T) -> Option<T> {
        let id = self.register::<T>();
        let Location { archetype, row } = self.location_of(entity);
        let (from, row) = (archetype.index(), row as usize);
        if let Some(col) = self.archetypes[from].column_of(id) {
            return Some(self.archetypes[from].table.replace(row, col, value));
        }
        let to = self.archetype_after_adding(archetype, id).index();
        let (src, dst) = self.two_archetypes(from, to);
        let swapped = Table::move_row_adding(
            &mut src.table,
            &src.components,
            row,
            &mut dst.table,
            &dst.components,
            value,
        );
        let new_row = dst.table.len() - 1;
        self.finish_move(entity, ArchetypeId::new(to), new_row, swapped, row);
        None
    }

    pub(crate) fn table_remove<T: Component>(&mut self, entity: Entity) -> Option<T> {
        if !self.entities.is_alive(entity) {
            return None;
        }
        let id = self.component_id::<T>()?;
        let Location { archetype, row } = self.location_of(entity);
        let (from, row) = (archetype.index(), row as usize);
        self.archetypes[from].column_of(id)?;
        let to = self.archetype_after_removing(archetype, id).index();
        let (src, dst) = self.two_archetypes(from, to);
        let (value, swapped) = Table::move_row_removing::<T>(
            &mut src.table,
            &src.components,
            row,
            &mut dst.table,
            &dst.components,
        );
        let new_row = dst.table.len() - 1;
        self.finish_move(entity, ArchetypeId::new(to), new_row, swapped, row);
        Some(value)
    }

    pub(crate) fn table_get<T: Component>(&self, entity: Entity) -> Option<&T> {
        if !self.entities.is_alive(entity) {
            return None;
        }
        let id = self.component_id::<T>()?;
        let Location { archetype, row } = self.location_of(entity);
        let archetype = &self.archetypes[archetype.index()];
        let col = archetype.column_of(id)?;
        Some(archetype.table.get(row as usize, col))
    }

    pub(crate) fn table_get_mut<T: Component>(&mut self, entity: Entity) -> Option<&mut T> {
        if !self.entities.is_alive(entity) {
            return None;
        }
        let id = self.component_id::<T>()?;
        let Location { archetype, row } = self.location_of(entity);
        let archetype = &mut self.archetypes[archetype.index()];
        let col = archetype.column_of(id)?;
        Some(archetype.table.get_mut(row as usize, col))
    }

    // The sparse paths: M3.3's, one `TypeId` lookup and one downcast per call. A set removes a
    // despawned entity's values, and compares full IDs, so no liveness check is needed here.

    pub(crate) fn sparse_insert<T: Component>(&mut self, entity: Entity, value: T) -> Option<T> {
        let id = self.register::<T>();
        self.sparse_set_mut::<T>(id).insert(entity, value)
    }

    pub(crate) fn sparse_remove<T: Component>(&mut self, entity: Entity) -> Option<T> {
        let id = self.component_id::<T>()?;
        self.sparse_set_mut::<T>(id).remove(entity)
    }

    pub(crate) fn sparse_get<T: Component>(&self, entity: Entity) -> Option<&T> {
        let id = self.component_id::<T>()?;
        self.sparse_set::<T>(id).get(entity)
    }

    pub(crate) fn sparse_get_mut<T: Component>(&mut self, entity: Entity) -> Option<&mut T> {
        let id = self.component_id::<T>()?;
        self.sparse_set_mut::<T>(id).get_mut(entity)
    }

    /// The set of sparse type `T`, whose id is `id`.
    fn sparse_set<T: Component>(&self, id: ComponentId) -> &SparseSet<T> {
        let index = self.components[id.index()]
            .sparse
            .expect("S4: a sparse type names its set");
        self.sparse_sets[index]
            .as_any()
            .downcast_ref()
            .expect("S4: a set holds the type its component id names")
    }

    fn sparse_set_mut<T: Component>(&mut self, id: ComponentId) -> &mut SparseSet<T> {
        let index = self.components[id.index()]
            .sparse
            .expect("S4: a sparse type names its set");
        self.sparse_sets[index]
            .as_any_mut()
            .downcast_mut()
            .expect("S4: a set holds the type its component id names")
    }

    /// Checks the archetype identity (A3) and kinds (A5), the table invariants (T1, T2), the
    /// location and edge invariants (A1, A2, A4) and the sparse-set invariants (S1 to S4), and
    /// returns the first violation.
    ///
    /// # Errors
    ///
    /// Returns an [`InvariantViolation`] describing the first invariant that does not hold.
    pub fn check_invariants(&self) -> Result<(), InvariantViolation> {
        self.check_identity()?;
        self.check_kinds()?;
        for archetype in &self.archetypes {
            let types: Vec<TypeId> = archetype
                .components
                .iter()
                .map(|c| self.components[c.index()].type_id)
                .collect();
            archetype.table.check(&types)?;
        }
        self.check_locations()?;
        self.check_edges()?;
        self.check_sparse_sets()?;
        self.check_bundles()
    }

    /// A5: archetypes and edges name table types only.
    fn check_kinds(&self) -> Result<(), InvariantViolation> {
        let is_table = |c: ComponentId| {
            self.components
                .get(c.index())
                .is_some_and(|info| info.kind == StorageKind::Table)
        };
        for (a, archetype) in self.archetypes.iter().enumerate() {
            let listed = archetype.components.iter().copied();
            let edged = archetype
                .add_edges()
                .map(|(c, _)| c)
                .chain(archetype.remove_edges().map(|(c, _)| c));
            for c in listed.chain(edged) {
                if !is_table(c) {
                    return Err(InvariantViolation::new(format!(
                        "A5: archetype {a} names component {}, which is not a table type",
                        c.index()
                    )));
                }
            }
        }
        Ok(())
    }

    /// S4, then each set's S1 to S3.
    fn check_sparse_sets(&self) -> Result<(), InvariantViolation> {
        let mut claimed = vec![false; self.sparse_sets.len()];
        for (c, info) in self.components.iter().enumerate() {
            match (info.kind, info.sparse, info.new_column) {
                (StorageKind::Table, None, Some(_)) => {}
                (StorageKind::Sparse, Some(k), None) => {
                    let ok = self
                        .sparse_sets
                        .get(k)
                        .is_some_and(|set| set.value_type_id() == info.type_id)
                        && !core::mem::replace(&mut claimed[k], true);
                    if !ok {
                        return Err(InvariantViolation::new(format!(
                            "S4: component {c} ({}) names set {k}, which is missing, holds \
                             another type or belongs to another component",
                            info.name
                        )));
                    }
                }
                _ => {
                    return Err(InvariantViolation::new(format!(
                        "S4: component {c} ({}) is {:?} but its set and column fields do not \
                         match that kind",
                        info.name, info.kind
                    )));
                }
            }
        }
        if let Some(k) = claimed.iter().position(|c| !c) {
            return Err(InvariantViolation::new(format!(
                "S4: set {k} belongs to no registered sparse type"
            )));
        }
        let is_alive = |e: Entity| self.entities.is_alive(e);
        for set in &self.sparse_sets {
            set.check(&is_alive).map_err(InvariantViolation::new)?;
        }
        Ok(())
    }

    /// B1: each cached bundle's archetype has exactly its table element types, `columns[k]` is
    /// the column of the `k`-th table element and `sparse[k]` the set of the `k`-th sparse one.
    fn check_bundles(&self) -> Result<(), InvariantViolation> {
        for info in self.bundles.values() {
            let fail = |why: &str| {
                Err(InvariantViolation::new(format!(
                    "B1: a cached bundle of {} types: {why}",
                    info.types.len()
                )))
            };
            let (mut table, mut sparse) = (Vec::new(), Vec::new());
            for type_id in &*info.types {
                let Some(id) = self.ids.get(type_id) else {
                    return fail("an element type is not registered");
                };
                match self.components[id.index()].kind {
                    StorageKind::Table => table.push(*id),
                    StorageKind::Sparse => sparse.push(*id),
                }
            }
            let Some(archetype) = self.archetypes.get(info.archetype.index()) else {
                return fail("its archetype does not exist");
            };
            if *archetype.components != *bundle::sorted(&table) {
                return fail("its archetype is not exactly the table element types");
            }
            let columns_ok = info.columns.len() == table.len()
                && table
                    .iter()
                    .zip(&*info.columns)
                    .all(|(id, col)| archetype.components.get(*col) == Some(id));
            if !columns_ok {
                return fail("a column index is not the column of its element");
            }
            let sparse_ok = info.sparse.len() == sparse.len()
                && sparse
                    .iter()
                    .zip(&*info.sparse)
                    .all(|(id, set)| self.components[id.index()].sparse == Some(*set));
            if !sparse_ok {
                return fail("a set index is not the set of its element");
            }
        }
        Ok(())
    }

    /// A3.
    fn check_identity(&self) -> Result<(), InvariantViolation> {
        if !self.archetypes[0].components.is_empty() {
            return Err(InvariantViolation::new(
                "A3: archetype 0 has components; it must be the empty archetype".to_string(),
            ));
        }
        for (a, archetype) in self.archetypes.iter().enumerate() {
            if !archetype.components.windows(2).all(|w| w[0] < w[1]) {
                return Err(InvariantViolation::new(format!(
                    "A3: the components of archetype {a} are not strictly ascending: {:?}",
                    archetype.components
                )));
            }
            if self.by_components.get(&archetype.components) != Some(&ArchetypeId::new(a)) {
                return Err(InvariantViolation::new(format!(
                    "A3: by_components does not map the components of archetype {a} to it: {:?}",
                    self.by_components.get(&archetype.components)
                )));
            }
        }
        if self.by_components.len() != self.archetypes.len() {
            return Err(InvariantViolation::new(format!(
                "A3: by_components has {} entries for {} archetypes",
                self.by_components.len(),
                self.archetypes.len()
            )));
        }
        Ok(())
    }

    /// A1 and A2.
    fn check_locations(&self) -> Result<(), InvariantViolation> {
        let mut rows = 0;
        for (a, archetype) in self.archetypes.iter().enumerate() {
            rows += archetype.table.len();
            for (r, entity) in archetype.table.entities().iter().enumerate() {
                let expected = Location {
                    archetype: ArchetypeId::new(a),
                    row: u32::try_from(r).expect("fewer than u32::MAX rows"),
                };
                let actual = self.locations.get(entity.index() as usize).copied();
                if actual != Some(expected) {
                    return Err(InvariantViolation::new(format!(
                        "A1: {entity:?} is at row {r} of archetype {a}, but its location is {actual:?}"
                    )));
                }
                if !self.entities.is_alive(*entity) {
                    return Err(InvariantViolation::new(format!(
                        "A2: archetype {a} holds {entity:?}, which is not alive"
                    )));
                }
            }
        }
        if rows != self.live {
            return Err(InvariantViolation::new(format!(
                "A2: the archetypes hold {rows} rows, but {} entities are alive",
                self.live
            )));
        }
        Ok(())
    }

    /// A4.
    fn check_edges(&self) -> Result<(), InvariantViolation> {
        for (a, archetype) in self.archetypes.iter().enumerate() {
            for (c, to) in archetype.add_edges() {
                let target = self.archetypes.get(to.index()).map(|t| &*t.components);
                let ok = archetype.column_of(c).is_none()
                    && target.is_some_and(|t| *t == *sorted_with(&archetype.components, c));
                if !ok {
                    return Err(InvariantViolation::new(format!(
                        "A4: the add edge of component {} from archetype {a} leads to {}, \
                         which is not its components plus that one",
                        c.index(),
                        to.index()
                    )));
                }
            }
            for (c, to) in archetype.remove_edges() {
                let target = self.archetypes.get(to.index()).map(|t| &*t.components);
                let ok = archetype.column_of(c).is_some()
                    && target.is_some_and(|t| *t == *sorted_without(&archetype.components, c));
                if !ok {
                    return Err(InvariantViolation::new(format!(
                        "A4: the remove edge of component {} from archetype {a} leads to {}, \
                         which is not its components minus that one",
                        c.index(),
                        to.index()
                    )));
                }
            }
        }
        Ok(())
    }

    /// Two different archetypes, mutably. A move changes the set of components, so its source
    /// and destination always differ.
    fn two_archetypes(&mut self, a: usize, b: usize) -> (&mut Archetype, &mut Archetype) {
        let [a, b] = self
            .archetypes
            .get_disjoint_mut([a, b])
            .expect("a move changes the set of components, so it changes the archetype");
        (a, b)
    }

    /// The id of `T`, if it was ever registered.
    pub(crate) fn component_id<T: Component>(&self) -> Option<ComponentId> {
        self.ids.get(&TypeId::of::<T>()).copied()
    }

    /// The id of `T`, registering it on first use: a table type gets a column constructor, a
    /// sparse type gets a set of its own.
    pub(crate) fn register<T: Component>(&mut self) -> ComponentId {
        if let Some(id) = self.component_id::<T>() {
            return id;
        }
        let id = ComponentId::new(
            u32::try_from(self.components.len()).expect("fewer than u32::MAX component types"),
        );
        let kind = <T::Storage as seal::Storage>::KIND;
        let (sparse, new_column) = match kind {
            StorageKind::Table => {
                let new_column: fn() -> Box<dyn ErasedColumn> = || Box::new(Column::<T>::new());
                (None, Some(new_column))
            }
            StorageKind::Sparse => {
                self.sparse_sets.push(Box::new(SparseSet::<T>::new()));
                (Some(self.sparse_sets.len() - 1), None)
            }
        };
        self.components.push(ComponentInfo {
            type_id: TypeId::of::<T>(),
            name: core::any::type_name::<T>(),
            kind,
            sparse,
            new_column,
        });
        self.ids.insert(TypeId::of::<T>(), id);
        id
    }

    fn set_location(&mut self, entity: Entity, archetype: ArchetypeId, row: usize) {
        let index = entity.index() as usize;
        if self.locations.len() <= index {
            self.locations.resize(index + 1, Location::NONE);
        }
        self.locations[index] = Location {
            archetype,
            row: u32::try_from(row).expect("fewer than u32::MAX rows"),
        };
    }

    /// After a row move: records where `entity` is now, and the new row of the entity swapped
    /// into its old row `old_row`.
    fn finish_move(
        &mut self,
        entity: Entity,
        to: ArchetypeId,
        new_row: usize,
        swapped: Option<Entity>,
        old_row: usize,
    ) {
        self.set_location(entity, to, new_row);
        self.fix_swapped(swapped, old_row);
    }

    /// Records that the entity swapped into a removed row, if any, now sits at `row`.
    fn fix_swapped(&mut self, swapped: Option<Entity>, row: usize) {
        if let Some(moved) = swapped {
            self.locations[moved.index() as usize].row =
                u32::try_from(row).expect("fewer than u32::MAX rows");
        }
    }

    /// Where a live `entity` is.
    fn location_of(&self, entity: Entity) -> Location {
        self.locations[entity.index() as usize]
    }

    /// The archetype reached from `from` by adding `component`, through the cached edge or, on
    /// a miss, by finding or creating it and recording the edge both ways.
    fn archetype_after_adding(&mut self, from: ArchetypeId, component: ComponentId) -> ArchetypeId {
        if let Some(to) = self.archetypes[from.index()].add_edge(component) {
            return to;
        }
        let list = sorted_with(&self.archetypes[from.index()].components, component);
        let to = self.find_or_create(list);
        self.archetypes[from.index()].set_add_edge(component, to);
        self.archetypes[to.index()].set_remove_edge(component, from);
        to
    }

    /// The archetype reached from `from` by removing `component`; see
    /// [`archetype_after_adding`](Self::archetype_after_adding).
    fn archetype_after_removing(
        &mut self,
        from: ArchetypeId,
        component: ComponentId,
    ) -> ArchetypeId {
        if let Some(to) = self.archetypes[from.index()].remove_edge(component) {
            return to;
        }
        let list = sorted_without(&self.archetypes[from.index()].components, component);
        let to = self.find_or_create(list);
        self.archetypes[from.index()].set_remove_edge(component, to);
        self.archetypes[to.index()].set_add_edge(component, from);
        to
    }

    /// The only reader of `by_components` outside `check_invariants`.
    fn lookup(&mut self, components: &[ComponentId]) -> Option<ArchetypeId> {
        #[cfg(test)]
        {
            self.lookups += 1;
        }
        self.by_components.get(components).copied()
    }

    /// The archetype with exactly the (ascending) `components`, created if it does not exist.
    fn find_or_create(&mut self, components: Box<[ComponentId]>) -> ArchetypeId {
        if let Some(id) = self.lookup(&components) {
            return id;
        }
        let columns: Box<[Box<dyn ErasedColumn>]> = components
            .iter()
            .map(|c| {
                (self.components[c.index()]
                    .new_column
                    .expect("A5: an archetype holds table types only"))()
            })
            .collect();
        let id = ArchetypeId::new(self.archetypes.len());
        self.archetypes
            .push(Archetype::new(components.clone(), Table::new(columns)));
        self.by_components.insert(components, id);
        id
    }
}

/// `list` with `component` inserted in order.
fn sorted_with(list: &[ComponentId], component: ComponentId) -> Box<[ComponentId]> {
    let at = list.partition_point(|c| *c < component);
    let mut out = Vec::with_capacity(list.len() + 1);
    out.extend_from_slice(&list[..at]);
    out.push(component);
    out.extend_from_slice(&list[at..]);
    out.into_boxed_slice()
}

/// `list` without `component`.
fn sorted_without(list: &[ComponentId], component: ComponentId) -> Box<[ComponentId]> {
    list.iter().copied().filter(|c| *c != component).collect()
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for World {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let archetypes: Vec<(Vec<&str>, usize)> = self
            .archetypes
            .iter()
            .map(|a| {
                let names = a
                    .components
                    .iter()
                    .map(|c| self.components[c.index()].name)
                    .collect();
                (names, a.table.len())
            })
            .collect();
        let sparse_sets: Vec<(&str, usize)> = self
            .sparse_sets
            .iter()
            .map(|s| (s.value_type_name(), s.len()))
            .collect();
        f.debug_struct("World")
            .field("live", &self.live)
            .field("archetypes", &archetypes)
            .field("sparse_sets", &sparse_sets)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::BundleInfo;
    use crate::component::components;
    use std::cell::Cell;
    use std::rc::Rc;

    #[derive(Debug, PartialEq)]
    struct A(u32);
    #[derive(Debug, PartialEq)]
    struct B(u32);
    #[derive(Debug, PartialEq)]
    struct C(u32);
    #[derive(Debug, PartialEq)]
    struct S(u32);
    #[derive(Debug, PartialEq)]
    struct R(u32);
    components!(TableStorage: A, B, C);
    components!(SparseStorage: S, R);

    fn location(w: &World, e: Entity) -> Location {
        w.locations[e.index() as usize]
    }

    fn message(payload: &(dyn core::any::Any + Send)) -> &str {
        payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or_default()
    }

    fn violation(w: &World) -> String {
        w.check_invariants().unwrap_err().to_string()
    }

    #[test]
    fn components_are_independent_per_type() {
        let mut w = World::new();
        let e = w.spawn();
        assert_eq!(w.insert(e, A(1)), None);
        assert_eq!(w.insert(e, S(2)), None);
        assert_eq!(w.insert(e, C(3)), None);
        assert_eq!(w.remove::<A>(e), Some(A(1)));
        assert_eq!(w.get::<A>(e), None);
        assert_eq!(w.get::<S>(e), Some(&S(2)));
        assert_eq!(w.get::<C>(e), Some(&C(3)));
        w.get_mut::<S>(e).unwrap().0 += 10;
        w.get_mut::<C>(e).unwrap().0 += 10;
        assert_eq!(w.get::<S>(e), Some(&S(12)));
        assert_eq!(w.get::<C>(e), Some(&C(13)));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn table_insert_then_remove_returns_to_the_first_archetype() {
        let mut w = World::new();
        let e = w.spawn();
        w.insert(e, A(1));
        let first = location(&w, e).archetype;
        w.insert(e, B(2));
        assert_eq!(w.archetype_count(), 3);
        assert_eq!(w.remove::<B>(e), Some(B(2)));
        assert_eq!(location(&w, e).archetype, first);
        assert_eq!(w.archetype_count(), 3);
        assert_eq!(w.get::<A>(e), Some(&A(1)));
        w.remove::<A>(e);
        assert_eq!(location(&w, e).archetype, ArchetypeId::EMPTY);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn a_second_transition_hits_the_edge() {
        let mut w = World::new();
        let cycle = |w: &mut World, e: Entity| {
            w.insert(e, A(1));
            w.insert(e, B(2));
            w.remove::<B>(e);
            w.remove::<A>(e);
        };
        let first = w.spawn();
        cycle(&mut w, first);
        let (lookups, count) = (w.lookups, w.archetype_count());
        assert!(
            lookups > 0,
            "the cold transitions must have looked archetypes up"
        );
        let second = w.spawn();
        cycle(&mut w, second);
        assert_eq!(w.lookups, lookups);
        assert_eq!(w.archetype_count(), count);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    #[allow(clippy::many_single_char_names)] // test locals: w, e, f
    fn sparse_toggle_keeps_the_location() {
        let mut w = World::new();
        let (e, f) = (w.spawn(), w.spawn());
        w.insert(e, A(1));
        w.insert(f, A(2));
        let (loc, count) = (location(&w, e), w.archetype_count());
        let lookups = w.lookups;
        assert_eq!(w.insert(e, S(5)), None);
        assert_eq!(location(&w, e), loc);
        assert_eq!(w.get::<S>(e), Some(&S(5)));
        assert_eq!(w.get::<S>(f), None);
        assert_eq!(w.remove::<S>(e), Some(S(5)));
        assert_eq!(w.remove::<S>(e), None);
        assert_eq!(location(&w, e), loc);
        assert_eq!(w.archetype_count(), count);
        assert_eq!(
            w.lookups, lookups,
            "a sparse toggle never reads by_components"
        );
        let sid = w.component_id::<S>().unwrap();
        assert!(w.archetypes.iter().all(|a| a.column_of(sid).is_none()));
        assert_eq!(w.check_invariants(), Ok(()));
        // A sparse insert on an entity with no table types stays in the empty archetype.
        let third = w.spawn();
        w.insert(third, S(1));
        assert_eq!(location(&w, third).archetype, ArchetypeId::EMPTY);
        assert_eq!(w.archetype_count(), count);
    }

    #[test]
    fn replacing_a_value_does_not_move_the_entity() {
        let mut w = World::new();
        let e = w.spawn();
        assert_eq!(w.insert(e, A(1)), None);
        assert_eq!(w.insert(e, S(1)), None);
        let (loc, count) = (location(&w, e), w.archetype_count());
        assert_eq!(w.insert(e, A(2)), Some(A(1)));
        assert_eq!(w.insert(e, S(2)), Some(S(1)));
        assert_eq!(location(&w, e), loc);
        assert_eq!(w.archetype_count(), count);
        assert_eq!(w.get::<A>(e), Some(&A(2)));
        assert_eq!(w.get::<S>(e), Some(&S(2)));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn removing_an_absent_type_creates_no_archetype() {
        let mut w = World::new();
        let e = w.spawn();
        w.insert(e, A(1));
        let count = w.archetype_count();
        assert_eq!(w.remove::<B>(e), None);
        assert_eq!(w.remove::<R>(e), None);
        assert_eq!(w.archetype_count(), count);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    /// Counts its drops into a shared cell.
    macro_rules! droppy {
        ($($n:ident),+) => {$(
            struct $n(Rc<Cell<u32>>);
            impl Drop for $n {
                fn drop(&mut self) {
                    self.0.set(self.0.get() + 1);
                }
            }
        )+};
    }
    droppy!(SD, RD, TD);
    components!(SparseStorage: SD, RD);
    components!(TableStorage: TD);

    #[test]
    fn despawn_clears_every_sparse_set() {
        let (ds, dr, dt) = (
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
        );
        let mut w = World::new();
        let es: Vec<Entity> = (0..3)
            .map(|_| {
                let e = w.spawn();
                w.insert(e, TD(Rc::clone(&dt)));
                w.insert(e, SD(Rc::clone(&ds)));
                w.insert(e, RD(Rc::clone(&dr)));
                e
            })
            .collect();
        assert!(w.despawn(es[0]));
        assert_eq!(
            (ds.get(), dr.get(), dt.get()),
            (1, 1, 1),
            "each value drops once"
        );
        assert_eq!(location(&w, es[2]).row, 0, "the last row filled the hole");
        assert!(w.get::<SD>(es[0]).is_none() && w.get::<RD>(es[0]).is_none());
        assert!(w.get::<SD>(es[2]).is_some());
        assert_eq!(w.check_invariants(), Ok(()));
        drop(w);
        assert_eq!((ds.get(), dr.get(), dt.get()), (3, 3, 3));
    }

    #[test]
    fn stale_id_does_not_reach_the_reused_slot() {
        let mut w = World::new();
        let a = w.spawn();
        w.insert(a, A(1));
        w.insert(a, S(1));
        assert!(w.despawn(a));
        let b = w.spawn();
        assert_eq!(b.index(), a.index(), "the slot is reused");
        assert_ne!(a, b);
        assert_eq!(w.get::<S>(b), None, "b has none of a's sparse values");
        assert_eq!(w.get::<A>(b), None);
        w.insert(b, A(7));
        w.insert(b, S(7));
        for _ in 0..2 {
            assert_eq!(w.get::<A>(a), None);
            assert_eq!(w.get::<S>(a), None);
            assert_eq!(w.get_mut::<S>(a), None);
            assert_eq!(w.remove::<A>(a), None);
            assert_eq!(w.remove::<S>(a), None);
            assert!(!w.despawn(a));
        }
        assert!(w.is_alive(b));
        assert_eq!(w.get::<A>(b), Some(&A(7)));
        assert_eq!(w.get::<S>(b), Some(&S(7)));
        let failed = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| w.insert(a, S(9))));
        assert!(
            message(&*failed.unwrap_err())
                .starts_with("World::insert called on a despawned entity")
        );
        let failed = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| w.insert(a, A(9))));
        assert!(
            message(&*failed.unwrap_err())
                .starts_with("World::insert called on a despawned entity")
        );
        assert_eq!(w.get::<S>(b), Some(&S(7)));
        assert_eq!(w.get::<A>(b), Some(&A(7)));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn len_counts_live_entities() {
        let mut w = World::new();
        assert!(w.is_empty());
        let es: Vec<Entity> = (0..3).map(|_| w.spawn()).collect();
        assert_eq!(w.len(), 3);
        assert!(w.despawn(es[1]));
        assert_eq!(w.len(), 2);
        assert!(!w.despawn(es[1]));
        assert_eq!(w.len(), 2);
        assert!(!w.is_empty());
    }

    #[test]
    #[should_panic(expected = "World::insert called on a despawned entity")]
    fn insert_on_a_despawned_entity_panics_for_a_table_type() {
        let mut w = World::new();
        let e = w.spawn();
        w.despawn(e);
        w.insert(e, A(1));
    }

    #[test]
    #[should_panic(expected = "World::insert called on a despawned entity")]
    fn insert_on_a_despawned_entity_panics_for_a_sparse_type() {
        let mut w = World::new();
        let e = w.spawn();
        w.despawn(e);
        w.insert(e, S(1));
    }

    #[test]
    fn a_failed_spawn_leaves_the_world_unchanged() {
        let mut w = World::new();
        w.entities = GenIdAllocator::with_fixed_capacity(0);
        let failed = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| w.spawn()));
        let payload = failed.unwrap_err();
        assert!(
            message(&*payload).starts_with("World::spawn:"),
            "got: {}",
            message(&*payload)
        );
        assert_eq!(w.len(), 0);
        assert_eq!(w.archetype_count(), 1);
        assert_eq!(w.archetypes[0].table.len(), 0);
        assert!(w.components.is_empty() && w.sparse_sets.is_empty());
        assert_eq!(w.check_invariants(), Ok(()));
    }

    fn two_entities() -> (World, Entity, Entity) {
        let mut w = World::new();
        let (e, f) = (w.spawn(), w.spawn());
        w.insert(e, A(1));
        w.insert(f, A(2));
        w.insert(e, S(3));
        (w, e, f)
    }

    #[test]
    fn a1_violation_is_reported() {
        let (mut w, e, _) = two_entities();
        w.locations[e.index() as usize].row = 1;
        assert!(violation(&w).contains("A1"), "got: {}", violation(&w));
    }

    #[test]
    fn a2_violation_is_reported() {
        let (mut w, _, _) = two_entities();
        w.live += 1;
        assert!(violation(&w).contains("A2"), "got: {}", violation(&w));
    }

    #[test]
    fn a3_violation_is_reported() {
        let (mut w, _, _) = two_entities();
        w.by_components.clear();
        assert!(violation(&w).contains("A3"), "got: {}", violation(&w));
        let (mut w, _, _) = two_entities();
        w.archetypes[0].components = Box::new([ComponentId::new(0)]);
        assert!(violation(&w).contains("A3"), "got: {}", violation(&w));
    }

    #[test]
    fn a4_violation_is_reported() {
        let (mut w, _, _) = two_entities();
        let a = ComponentId::new(0);
        w.archetypes[0].set_add_edge(a, ArchetypeId::EMPTY);
        assert!(violation(&w).contains("A4"), "got: {}", violation(&w));
        let (mut w, _, _) = two_entities();
        w.archetypes[0].set_remove_edge(a, ArchetypeId::EMPTY);
        assert!(violation(&w).contains("A4"), "got: {}", violation(&w));
    }

    #[test]
    fn a5_violation_is_reported() {
        let (w, _, _) = two_entities();
        assert_eq!(w.check_kinds(), Ok(()));
        // A sparse id in a non-empty archetype's list, in ascending position, with the
        // `by_components` key updated to match, so A3 passes.
        let (mut w, _, _) = two_entities();
        let sid = w.component_id::<S>().unwrap();
        let old = w.archetypes[1].components.clone();
        let mut list = old.to_vec();
        list.push(sid);
        list.sort_unstable();
        assert!(list.windows(2).all(|p| p[0] < p[1]));
        let list: Box<[ComponentId]> = list.into();
        w.by_components.remove(&old);
        w.by_components.insert(list.clone(), ArchetypeId::new(1));
        w.archetypes[1].components = list;
        let err = violation(&w);
        assert!(err.contains("A5"), "got: {err}");
        // A sparse id as a recorded add edge.
        let (mut w, _, _) = two_entities();
        let sid = w.component_id::<S>().unwrap();
        w.archetypes[0].set_add_edge(sid, ArchetypeId::EMPTY);
        let err = violation(&w);
        assert!(err.contains("A5"), "got: {err}");
    }

    #[test]
    fn s4_violation_is_reported() {
        let (mut w, _, _) = two_entities();
        let s = w.component_id::<S>().unwrap();
        w.components[s.index()].sparse = Some(7);
        assert!(violation(&w).contains("S4"), "got: {}", violation(&w));
        // A set that belongs to no type.
        let (mut w, _, _) = two_entities();
        w.sparse_sets.push(Box::new(SparseSet::<R>::new()));
        assert!(violation(&w).contains("S4"), "got: {}", violation(&w));
        // A set of another type.
        let (mut w, _, _) = two_entities();
        w.sparse_sets[0] = Box::new(SparseSet::<R>::new());
        assert!(violation(&w).contains("S4"), "got: {}", violation(&w));
    }

    #[test]
    fn set_violations_surface_through_check_invariants() {
        let (mut w, e, _) = two_entities();
        assert!(w.despawn(e));
        assert_eq!(w.check_invariants(), Ok(()));
        // S3: a value left in a set for an entity that is no longer alive.
        let s = w.component_id::<S>().unwrap();
        w.sparse_set_mut::<S>(s).insert(e, S(1));
        assert!(violation(&w).contains("S3"), "got: {}", violation(&w));
    }

    #[test]
    fn invariant_violation_displays_its_message() {
        let v = InvariantViolation::new("T1: broken".to_string());
        assert_eq!(v.to_string(), "invariant violated: T1: broken");
        assert_eq!(v.clone(), v);
        let boxed: Box<dyn std::error::Error> = Box::new(v);
        assert!(boxed.to_string().contains("T1"));
    }

    #[test]
    fn world_debug_shows_tables_and_sets() {
        let (w, _, _) = two_entities();
        let text = format!("{w:?}");
        assert!(text.contains("live: 2"), "got: {text}");
        assert!(text.contains("tests::A"), "got: {text}");
        assert!(text.contains("tests::S"), "got: {text}");
        assert!(World::default().is_empty());
    }

    #[test]
    fn spawn_with_creates_one_archetype_of_the_table_types() {
        let mut w = World::new();
        let e = w.spawn_with((A(1), S(2), C(3)));
        assert_eq!(w.archetype_count(), 2);
        let (a, c) = (
            w.component_id::<A>().unwrap(),
            w.component_id::<C>().unwrap(),
        );
        assert_eq!(&*w.archetypes[1].components, &[a, c]);
        assert_eq!(location(&w, e).archetype, ArchetypeId::new(1));
        assert_eq!(w.sparse_sets.len(), 1);
        assert_eq!(w.sparse_sets[0].len(), 1);
        assert_eq!(w.get::<S>(e), Some(&S(2)));
        assert_eq!(w.len(), 1);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn mixed_bundle_values_land_in_their_own_storage() {
        let mut w = World::new();
        // Register in one order, spawn in another, so tuple order is not column order.
        w.register::<A>();
        w.register::<C>();
        w.register::<S>();
        let e = w.spawn_with((S(1), C(2), A(3)));
        let f = w.spawn_with((C(20), S(10), R(30), A(40)));
        assert_eq!(w.get::<A>(e), Some(&A(3)));
        assert_eq!(w.get::<C>(e), Some(&C(2)));
        assert_eq!(w.get::<S>(e), Some(&S(1)));
        assert_eq!(w.get::<R>(e), None);
        assert_eq!(w.get::<A>(f), Some(&A(40)));
        assert_eq!(w.get::<C>(f), Some(&C(20)));
        assert_eq!(w.get::<S>(f), Some(&S(10)));
        assert_eq!(w.get::<R>(f), Some(&R(30)));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn sparse_only_bundle_spawns_into_the_empty_archetype() {
        let mut w = World::new();
        let e = w.spawn_with((S(1), R(2)));
        assert_eq!(w.archetype_count(), 1);
        assert_eq!(location(&w, e).archetype, ArchetypeId::EMPTY);
        assert_eq!(w.get::<S>(e), Some(&S(1)));
        assert_eq!(w.get::<R>(e), Some(&R(2)));
        assert_eq!(w.len(), 1);
        assert_eq!(w.check_invariants(), Ok(()));
        assert!(w.despawn(e));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn a_warm_spawn_with_creates_nothing() {
        let mut w = World::new();
        w.spawn_with((A(1), S(2)));
        let lookups = w.lookups;
        assert!(lookups > 0);
        let (types, sets, bundles) = (w.components.len(), w.sparse_sets.len(), w.bundles.len());
        w.spawn_with((A(3), S(4)));
        assert_eq!(w.lookups, lookups);
        assert_eq!(w.archetype_count(), 2);
        assert_eq!(
            (w.components.len(), w.sparse_sets.len(), w.bundles.len()),
            (types, sets, bundles)
        );
    }

    #[test]
    #[should_panic(expected = "World::spawn_with: `")]
    fn repeated_table_type_in_a_bundle_panics() {
        let mut w = World::new();
        w.spawn_with((A(1), B(2), A(3)));
    }

    #[test]
    #[should_panic(expected = "appears twice in the bundle")]
    fn repeated_sparse_type_in_a_bundle_panics() {
        let mut w = World::new();
        w.spawn_with((S(1), A(2), S(3)));
    }

    #[test]
    fn a_repeated_type_leaves_the_world_unchanged() {
        let mut w = World::new();
        let failed =
            std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| w.spawn_with((S(1), S(2)))));
        assert!(message(&*failed.unwrap_err()).contains("S` appears twice in the bundle"));
        assert_eq!(w.len(), 0);
        assert!(w.components.is_empty() && w.sparse_sets.is_empty() && w.bundles.is_empty());
        assert_eq!(w.archetype_count(), 1);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn a_failed_spawn_with_leaves_the_world_unchanged() {
        let mut w = World::new();
        w.entities = GenIdAllocator::with_fixed_capacity(0);
        let failed = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
            w.spawn_with((A(1), S(2), C(3)))
        }));
        let payload = failed.unwrap_err();
        assert!(
            message(&*payload).starts_with("World::spawn_with:"),
            "got: {}",
            message(&*payload)
        );
        assert_eq!(w.len(), 0);
        assert_eq!(w.archetype_count(), 1);
        assert_eq!(w.archetypes[0].table.len(), 0);
        assert!(w.ids.is_empty() && w.components.is_empty());
        assert!(w.sparse_sets.is_empty());
        assert!(w.bundles.is_empty());
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn b1_violation_is_reported() {
        let mut w = World::new();
        w.spawn_with((A(1), S(2), C(3)));
        assert_eq!(w.check_invariants(), Ok(()));
        let key = *w.bundles.keys().next().unwrap();
        let corrupt = |w: &mut World, f: &dyn Fn(&mut BundleInfo)| {
            f(w.bundles.get_mut(&key).unwrap());
            violation(w)
        };
        let mut x = World::new();
        x.spawn_with((A(1), S(2), C(3)));
        assert!(corrupt(&mut x, &|i| i.archetype = ArchetypeId::EMPTY).contains("B1"));
        let mut x = World::new();
        x.spawn_with((A(1), S(2), C(3)));
        assert!(corrupt(&mut x, &|i| i.columns.swap(0, 1)).contains("B1"));
        let mut x = World::new();
        x.spawn_with((A(1), S(2), C(3)));
        assert!(corrupt(&mut x, &|i| i.sparse[0] = 5).contains("B1"));
        let mut x = World::new();
        x.spawn_with((A(1), S(2), C(3)));
        assert!(corrupt(&mut x, &|i| i.types = Box::new([])).contains("B1"));
    }
}
