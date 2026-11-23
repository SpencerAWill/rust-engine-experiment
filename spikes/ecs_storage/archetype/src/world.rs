//! The `World`: entities and their components, stored in one table per set of component types.

use core::any::TypeId;
use core::fmt;
use std::collections::BTreeMap;

use engine::genid::GenIdAllocator;

use crate::archetype::{Archetype, ArchetypeId, ComponentId};
use crate::bundle::{self, Bundle, BundleInfo};
use crate::query::{Query, QueryMut, QueryParam, ReadOnly};
use crate::raw::{ByteColumn, RawLayout};
use crate::table::{Column, ColumnDesc, ErasedColumn, Table};
use crate::{Entity, EntityMarker};

mod raw;

pub use raw::RawError;

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

/// Where an entity's row is: its archetype and its row in that archetype's table.
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

/// Which kind of component an id names, and what that kind needs to make and check a column.
#[derive(Debug)]
pub(crate) enum ComponentKind {
    /// A Rust type, found by its `TypeId`.
    Rust {
        type_id: TypeId,
        /// Creates an empty column of this type, for a new archetype that has it.
        new_column: fn() -> Box<dyn ErasedColumn>,
    },
    /// A runtime-registered component of this layout, stored in a byte column.
    Raw(RawLayout),
}

/// What the world knows about one component.
#[derive(Debug)]
pub(crate) struct ComponentInfo {
    pub(crate) kind: ComponentKind,
    pub(crate) name: Box<str>,
}

impl ComponentInfo {
    /// An empty column for a new archetype that has this component.
    fn new_column(&self) -> Box<dyn ErasedColumn> {
        match &self.kind {
            ComponentKind::Rust { new_column, .. } => new_column(),
            ComponentKind::Raw(layout) => Box::new(ByteColumn::new(*layout)),
        }
    }

    /// The `TypeId` of a Rust type's component, `None` for a raw one.
    fn type_id(&self) -> Option<TypeId> {
        match &self.kind {
            ComponentKind::Rust { type_id, .. } => Some(*type_id),
            ComponentKind::Raw(_) => None,
        }
    }

    /// What this component's column must report (T2).
    fn desc(&self) -> ColumnDesc {
        match &self.kind {
            ComponentKind::Rust { type_id, .. } => ColumnDesc::Typed(*type_id),
            ComponentKind::Raw(layout) => ColumnDesc::Raw(*layout),
        }
    }
}

/// Entities and their components. Each distinct set of component types has its own table, and
/// inserting or removing a component moves the entity's row between tables.
///
/// # Examples
///
/// ```
/// use spike_ecs_archetype::World;
///
/// struct Name(&'static str);
/// struct Age(u32);
///
/// let mut world = World::new();
/// let entity = world.spawn();
/// world.insert(entity, Name("ada"));
/// world.insert(entity, Age(36));
/// assert_eq!(world.get::<Name>(entity).map(|n| n.0), Some("ada"));
/// assert_eq!(world.remove::<Age>(entity).map(|a| a.0), Some(36));
/// assert!(world.despawn(entity));
/// assert!(world.get::<Name>(entity).is_none());
/// ```
pub struct World {
    entities: GenIdAllocator<EntityMarker>,
    live: usize,
    ids: BTreeMap<TypeId, ComponentId>,
    components: Vec<ComponentInfo>,
    archetypes: Vec<Archetype>,
    by_components: BTreeMap<Box<[ComponentId]>, ArchetypeId>,
    /// One entry per bundle type spawned so far, keyed by the tuple's `TypeId`.
    bundles: BTreeMap<TypeId, BundleInfo>,
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
            by_components,
            bundles: BTreeMap::new(),
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
    /// (2³¹−2 entity slots in use or retired).
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

    /// Creates an entity with the components of `bundle`, a flat tuple of 1 to 8 values, placed
    /// straight into the table for exactly those types.
    ///
    /// # Panics
    ///
    /// Panics with a `World::spawn_with:` message if a type appears twice in the bundle, or if
    /// the world holds the most entities it can (2³¹−2 entity slots in use or retired). `len`
    /// and every existing entity are unchanged in either case; the bundle's archetype may
    /// already have been created.
    #[track_caller]
    pub fn spawn_with<B: Bundle>(&mut self, bundle: B) -> Entity {
        let key = TypeId::of::<B>();
        // One lookup when the bundle type is known. A new type builds its info first: allocating
        // first would leak an entity id if the duplicate-type panic fired afterwards.
        let (entity, archetype, row) = if let Some(info) = self.bundles.get(&key) {
            Self::place(&mut self.entities, &mut self.archetypes, info, bundle)
        } else {
            let info = self.build_bundle_info::<B>();
            let info = self.bundles.entry(key).or_insert(info);
            Self::place(&mut self.entities, &mut self.archetypes, info, bundle)
        };
        self.set_location(entity, archetype, row);
        self.live += 1;
        entity
    }

    /// Allocates an entity and pushes `bundle` onto the table `info` names; returns the entity,
    /// the archetype and the row. Takes the fields it needs so `info` can borrow `bundles`.
    ///
    /// # Panics
    ///
    /// Panics with a `World::spawn_with:` message if the allocator is full.
    #[track_caller]
    fn place<B: Bundle>(
        entities: &mut GenIdAllocator<EntityMarker>,
        archetypes: &mut [Archetype],
        info: &BundleInfo,
        bundle: B,
    ) -> (Entity, ArchetypeId, usize) {
        let Some(entity) = entities.try_allocate() else {
            panic!(
                "World::spawn_with: the world holds the most entities it can (every entity slot \
                 is in use or retired); despawn entities to free slots"
            )
        };
        let table = &mut archetypes[info.archetype.index()].table;
        let row = table.len();
        table.push_bundle(entity, bundle, &info.columns);
        (entity, info.archetype, row)
    }

    /// Finds or creates the archetype of bundle type `B` and records its column order.
    ///
    /// # Panics
    ///
    /// Panics if a type appears twice in `B`.
    #[track_caller]
    fn build_bundle_info<B: Bundle>(&mut self) -> BundleInfo {
        let types = B::types();
        for (k, (type_id, name)) in types.iter().enumerate() {
            assert!(
                types[..k].iter().all(|(other, _)| other != type_id),
                "World::spawn_with: `{name}` appears twice in the bundle; a bundle holds each \
                 component type once"
            );
        }
        let ids = B::register(self);
        let list = bundle::sorted(&ids);
        let archetype = self.find_or_create(list.clone());
        let columns = ids
            .iter()
            .map(|id| {
                list.binary_search(id)
                    .expect("the bundle's ids are in its list")
            })
            .collect();
        BundleInfo {
            archetype,
            columns,
            types: types.into_iter().map(|(type_id, _)| type_id).collect(),
        }
    }

    /// Removes the entity and all its components. Returns `false` if it was not alive.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.entities.is_alive(entity) {
            return false;
        }
        let Location { archetype, row } = self.location_of(entity);
        let swapped = self.archetypes[archetype.index()]
            .table
            .swap_remove(row as usize);
        self.fix_swapped(swapped, row as usize);
        self.locations[entity.index() as usize] = Location::NONE;
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

    /// The number of archetypes created so far, the one with no components included.
    /// Archetypes are never removed.
    #[must_use]
    pub fn archetype_count(&self) -> usize {
        self.archetypes.len()
    }

    /// Stores `value` as the `T` component of `entity` and returns the value it replaced. If the
    /// entity had no `T`, it moves to the archetype with `T` added.
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

    /// Removes and returns the `T` component of `entity`, moving the entity to the archetype
    /// with `T` removed, or returns `None` if the entity is despawned or has none.
    pub fn remove<T: 'static>(&mut self, entity: Entity) -> Option<T> {
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

    /// The `T` component of `entity`, or `None` if the entity is despawned or has none.
    #[must_use]
    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        if !self.entities.is_alive(entity) {
            return None;
        }
        let id = self.component_id::<T>()?;
        let Location { archetype, row } = self.location_of(entity);
        let archetype = &self.archetypes[archetype.index()];
        let col = archetype.column_of(id)?;
        Some(archetype.table.get(row as usize, col))
    }

    /// The `T` component of `entity`, mutably, or `None` if the entity is despawned or has none.
    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        if !self.entities.is_alive(entity) {
            return None;
        }
        let id = self.component_id::<T>()?;
        let Location { archetype, row } = self.location_of(entity);
        let archetype = &mut self.archetypes[archetype.index()];
        let col = archetype.column_of(id)?;
        Some(archetype.table.get_mut(row as usize, col))
    }

    /// A read-only query over `Q`; see [`Query`]. Nothing runs until the query does.
    pub fn query<Q: ReadOnly>(&self) -> Query<'_, Q> {
        Query::new(self)
    }

    /// A query over `Q` that may write; see [`QueryMut`]. Nothing runs until the query does.
    pub fn query_mut<Q: QueryParam>(&mut self) -> QueryMut<'_, Q> {
        QueryMut::new(self)
    }

    /// Checks the table invariants (T1, T2, T3), the archetype invariants (A1 to A4) and the bundle
    /// invariant (B1), and returns the first violation.
    ///
    /// # Errors
    ///
    /// Returns an [`InvariantViolation`] describing the first invariant that does not hold.
    pub fn check_invariants(&self) -> Result<(), InvariantViolation> {
        self.check_identity()?;
        for archetype in &self.archetypes {
            let descs: Vec<ColumnDesc> = archetype
                .components
                .iter()
                .map(|c| self.components[c.index()].desc())
                .collect();
            archetype.table.check(&descs)?;
        }
        self.check_locations()?;
        self.check_edges()?;
        self.check_bundles()
    }

    /// B1.
    fn check_bundles(&self) -> Result<(), InvariantViolation> {
        for info in self.bundles.values() {
            let Some(archetype) = self.archetypes.get(info.archetype.index()) else {
                return Err(InvariantViolation::new(format!(
                    "B1: a bundle names archetype {}, which does not exist",
                    info.archetype.index()
                )));
            };
            let type_of = |c: ComponentId| self.components[c.index()].type_id();
            let mut want: Vec<Option<TypeId>> = info.types.iter().map(|t| Some(*t)).collect();
            want.sort_unstable();
            let mut have: Vec<Option<TypeId>> =
                archetype.components.iter().map(|c| type_of(*c)).collect();
            have.sort_unstable();
            if want != have {
                return Err(InvariantViolation::new(format!(
                    "B1: a bundle of {} types is cached with archetype {}, whose types differ",
                    info.types.len(),
                    info.archetype.index()
                )));
            }
            for (k, (type_id, col)) in info.types.iter().zip(&info.columns).enumerate() {
                let ok = archetype
                    .components
                    .get(*col)
                    .is_some_and(|c| type_of(*c) == Some(*type_id));
                if !ok {
                    return Err(InvariantViolation::new(format!(
                        "B1: element {k} of a bundle is cached at column {col} of archetype {}, \
                         which is not its type's column",
                        info.archetype.index()
                    )));
                }
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
    pub(crate) fn component_id<T: 'static>(&self) -> Option<ComponentId> {
        self.ids.get(&TypeId::of::<T>()).copied()
    }

    /// The id of the component type `type_id`, if it was ever registered.
    pub(crate) fn component_id_of(&self, type_id: TypeId) -> Option<ComponentId> {
        self.ids.get(&type_id).copied()
    }

    /// Every archetype, indexed by `ArchetypeId`.
    pub(crate) fn archetypes(&self) -> &[Archetype] {
        &self.archetypes
    }

    /// Every archetype, exclusive.
    pub(crate) fn archetypes_mut(&mut self) -> &mut [Archetype] {
        &mut self.archetypes
    }

    /// The id the next registered component gets.
    fn next_component_id(&self) -> ComponentId {
        ComponentId::new(
            u32::try_from(self.components.len()).expect("fewer than u32::MAX component types"),
        )
    }

    /// The id of `T`, registering it on first use.
    pub(crate) fn register<T: 'static>(&mut self) -> ComponentId {
        if let Some(id) = self.component_id::<T>() {
            return id;
        }
        let id = self.next_component_id();
        self.components.push(ComponentInfo {
            kind: ComponentKind::Rust {
                type_id: TypeId::of::<T>(),
                new_column: || Box::new(Column::<T>::new()),
            },
            name: core::any::type_name::<T>().into(),
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
            .map(|c| self.components[c.index()].new_column())
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
                    .map(|c| &*self.components[c.index()].name)
                    .collect();
                (names, a.table.len())
            })
            .collect();
        f.debug_struct("World")
            .field("live", &self.live)
            .field("archetypes", &archetypes)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[derive(Debug, PartialEq)]
    struct A(u32);
    #[derive(Debug, PartialEq)]
    struct B(u32);
    #[derive(Debug, PartialEq)]
    struct C(u32);

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

    #[test]
    fn components_are_independent_per_type() {
        let mut w = World::new();
        let e = w.spawn();
        assert_eq!(w.insert(e, A(1)), None);
        assert_eq!(w.insert(e, B(2)), None);
        assert_eq!(w.insert(e, C(3)), None);
        assert_eq!(w.remove::<A>(e), Some(A(1)));
        assert_eq!(w.get::<A>(e), None);
        assert_eq!(w.get::<B>(e), Some(&B(2)));
        assert_eq!(w.get::<C>(e), Some(&C(3)));
        w.get_mut::<B>(e).unwrap().0 += 10;
        assert_eq!(w.get::<B>(e), Some(&B(12)));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn insert_moves_to_the_archetype_with_the_type_added() {
        let mut w = World::new();
        let e = w.spawn();
        let f = w.spawn();
        assert_eq!(w.archetype_count(), 1);
        assert_eq!(location(&w, e).archetype, ArchetypeId::EMPTY);
        w.insert(e, A(1));
        assert_eq!(w.archetype_count(), 2);
        let a = location(&w, e).archetype;
        assert_eq!(w.archetypes[a.index()].components.len(), 1);
        w.insert(e, B(2));
        assert_eq!(w.archetype_count(), 3);
        let ab = location(&w, e).archetype;
        assert_eq!(w.archetypes[ab.index()].components.len(), 2);
        assert_eq!(location(&w, f).archetype, ArchetypeId::EMPTY);
        assert_eq!(w.get::<A>(e), Some(&A(1)));
        assert_eq!(w.get::<B>(e), Some(&B(2)));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn insert_then_remove_returns_to_the_first_archetype() {
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
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn removing_the_last_component_goes_to_the_empty_archetype() {
        let mut w = World::new();
        let e = w.spawn();
        w.insert(e, A(1));
        assert_eq!(w.remove::<A>(e), Some(A(1)));
        assert_eq!(location(&w, e).archetype, ArchetypeId::EMPTY);
        assert_eq!(w.archetype_count(), 2);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn removing_an_absent_type_creates_no_archetype() {
        let mut w = World::new();
        let e = w.spawn();
        w.insert(e, A(1));
        let count = w.archetype_count();
        assert_eq!(w.remove::<B>(e), None);
        assert_eq!(w.remove::<C>(e), None);
        let other = w.spawn();
        w.insert(other, B(0));
        let count_after_b = w.archetype_count();
        assert_eq!(count_after_b, count + 1);
        assert_eq!(w.remove::<B>(e), None, "registered, but not on this entity");
        assert_eq!(w.archetype_count(), count_after_b);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn replacing_a_value_does_not_move_the_entity() {
        let mut w = World::new();
        let e = w.spawn();
        assert_eq!(w.insert(e, A(1)), None);
        let (loc, count) = (location(&w, e), w.archetype_count());
        assert_eq!(w.insert(e, A(2)), Some(A(1)));
        assert_eq!(location(&w, e), loc);
        assert_eq!(w.archetype_count(), count);
        assert_eq!(w.get::<A>(e), Some(&A(2)));
    }

    #[test]
    fn a_warm_transition_uses_the_edge() {
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
    fn despawn_drops_the_row_and_fixes_the_swapped_location() {
        struct Droppy(Rc<Cell<u32>>);
        impl Drop for Droppy {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let drops = Rc::new(Cell::new(0));
        let mut w = World::new();
        let es: Vec<Entity> = (0..3)
            .map(|_| {
                let e = w.spawn();
                w.insert(e, Droppy(Rc::clone(&drops)));
                e
            })
            .collect();
        assert!(w.despawn(es[0]));
        assert_eq!(drops.get(), 1);
        assert_eq!(location(&w, es[2]).row, 0, "the last row filled the hole");
        assert_eq!(location(&w, es[0]), Location::NONE);
        assert!(w.get::<Droppy>(es[2]).is_some());
        assert!(w.get::<Droppy>(es[0]).is_none());
        assert_eq!(w.check_invariants(), Ok(()));
        drop(w);
        assert_eq!(drops.get(), 3);
    }

    #[test]
    fn stale_id_does_not_reach_the_reused_slot() {
        let mut w = World::new();
        let a = w.spawn();
        w.insert(a, A(1));
        assert!(w.despawn(a));
        let b = w.spawn();
        assert_eq!(b.index(), a.index(), "the slot is reused");
        assert_ne!(a, b);
        w.insert(b, A(7));
        assert_eq!(w.get::<A>(a), None);
        assert_eq!(w.remove::<A>(a), None);
        assert!(!w.despawn(a));
        assert!(w.is_alive(b));
        assert_eq!(w.get::<A>(b), Some(&A(7)));
        let failed = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| w.insert(a, A(9))));
        assert!(
            message(&*failed.unwrap_err())
                .starts_with("World::insert called on a despawned entity")
        );
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
    fn insert_on_a_despawned_entity_panics() {
        let mut w = World::new();
        let e = w.spawn();
        w.despawn(e);
        w.insert(e, A(1));
    }

    #[test]
    fn a_failed_spawn_leaves_len_unchanged() {
        let mut w = World::new();
        w.entities = GenIdAllocator::with_fixed_capacity(1);
        w.spawn();
        let failed = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| w.spawn()));
        let payload = failed.unwrap_err();
        assert!(
            message(&*payload).starts_with("World::spawn:"),
            "got: {}",
            message(&*payload)
        );
        assert_eq!(w.len(), 1);
        assert_eq!(w.archetypes[0].table.len(), 1);
    }

    fn violation(w: &World) -> String {
        w.check_invariants().unwrap_err().to_string()
    }

    fn two_entities() -> (World, Entity, Entity) {
        let mut w = World::new();
        let (e, f) = (w.spawn(), w.spawn());
        w.insert(e, A(1));
        w.insert(f, A(2));
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
    fn invariant_violation_displays_its_message() {
        let v = InvariantViolation::new("T1: broken".to_string());
        assert_eq!(v.to_string(), "invariant violated: T1: broken");
        let boxed: Box<dyn std::error::Error> = Box::new(v);
        assert!(boxed.to_string().contains("T1"));
    }

    #[test]
    fn world_debug_shows_the_live_count_and_archetypes() {
        let (w, _, _) = two_entities();
        let text = format!("{w:?}");
        assert!(text.contains("live: 2"), "got: {text}");
        assert!(text.contains('A'), "got: {text}");
        assert!(World::default().is_empty());
    }

    #[derive(Debug, PartialEq)]
    struct Hidden;

    #[test]
    fn spawn_with_creates_exactly_one_archetype() {
        let mut w = World::new();
        assert_eq!(w.archetype_count(), 1);
        let e = w.spawn_with((A(1), B(2), C(3)));
        assert_eq!(w.archetype_count(), 2);
        assert_eq!(w.len(), 1);
        assert_eq!(location(&w, e).archetype, ArchetypeId::new(1));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn a_warm_spawn_with_does_not_look_up_by_components() {
        let mut w = World::new();
        w.spawn_with((A(1), B(2)));
        let lookups = w.lookups;
        assert!(lookups > 0);
        w.spawn_with((A(3), B(4)));
        assert_eq!(w.lookups, lookups);
        assert_eq!(w.archetype_count(), 2);
    }

    #[test]
    fn bundle_values_land_in_their_own_columns() {
        let mut w = World::new();
        // Register A, B, C in that order first, so ids ascend; then spawn in another order.
        let seed = w.spawn();
        w.insert(seed, A(0));
        w.insert(seed, B(0));
        w.insert(seed, C(0));
        let e = w.spawn_with((C(30), A(10), B(20)));
        assert_eq!(w.get::<A>(e), Some(&A(10)));
        assert_eq!(w.get::<B>(e), Some(&B(20)));
        assert_eq!(w.get::<C>(e), Some(&C(30)));
        let f = w.spawn_with((B(2), C(3), A(1)));
        assert_eq!(w.get::<A>(f), Some(&A(1)));
        assert_eq!(w.get::<B>(f), Some(&B(2)));
        assert_eq!(w.get::<C>(f), Some(&C(3)));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn zero_sized_bundle_element_is_a_component() {
        let mut w = World::new();
        let e = w.spawn_with((A(1), Hidden));
        assert_eq!(w.get::<Hidden>(e), Some(&Hidden));
        assert_eq!(w.get::<A>(e), Some(&A(1)));
        assert_eq!(w.remove::<Hidden>(e), Some(Hidden));
        assert_eq!(w.get::<Hidden>(e), None);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn bundles_of_one_and_eight_types_work() {
        macro_rules! t {
            ($($n:ident),*) => { $( struct $n(u8); )* };
        }
        t!(T1, T2, T3, T4, T5, T6, T7, T8);
        let mut w = World::new();
        let one = w.spawn_with((T1(1),));
        assert_eq!(w.get::<T1>(one).map(|t| t.0), Some(1));
        let eight = w.spawn_with((T8(8), T7(7), T6(6), T5(5), T4(4), T3(3), T2(2), T1(1)));
        let got = [
            w.get::<T1>(eight).map(|t| t.0),
            w.get::<T2>(eight).map(|t| t.0),
            w.get::<T3>(eight).map(|t| t.0),
            w.get::<T4>(eight).map(|t| t.0),
            w.get::<T5>(eight).map(|t| t.0),
            w.get::<T6>(eight).map(|t| t.0),
            w.get::<T7>(eight).map(|t| t.0),
            w.get::<T8>(eight).map(|t| t.0),
        ];
        assert_eq!(got, [1, 2, 3, 4, 5, 6, 7, 8].map(Some));
        assert_eq!(w.archetype_count(), 3);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    #[should_panic(expected = "appears twice in the bundle")]
    fn repeated_type_in_a_bundle_panics() {
        World::new().spawn_with((A(1), B(2), A(3)));
    }

    #[test]
    fn repeated_type_panic_names_the_method_and_type() {
        let mut w = World::new();
        let failed =
            std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| w.spawn_with((A(1), A(2)))));
        let payload = failed.unwrap_err();
        let text = message(&*payload);
        assert!(text.starts_with("World::spawn_with:"), "got: {text}");
        assert!(
            text.contains("::A` appears twice in the bundle"),
            "got: {text}"
        );
        assert_eq!(w.len(), 0);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn a_failed_spawn_with_leaves_len_unchanged() {
        let mut w = World::new();
        w.entities = GenIdAllocator::with_fixed_capacity(1);
        w.spawn_with((A(1), B(2)));
        let failed =
            std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| w.spawn_with((A(3), B(4)))));
        let payload = failed.unwrap_err();
        assert!(
            message(&*payload).starts_with("World::spawn_with:"),
            "got: {}",
            message(&*payload)
        );
        assert_eq!(w.len(), 1);
        assert_eq!(w.archetypes[1].table.len(), 1);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn a_failed_cold_spawn_with_leaves_len_unchanged() {
        let mut w = World::new();
        w.entities = GenIdAllocator::with_fixed_capacity(0);
        let failed =
            std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| w.spawn_with((A(1), B(2)))));
        let payload = failed.unwrap_err();
        assert!(
            message(&*payload).starts_with("World::spawn_with:"),
            "got: {}",
            message(&*payload)
        );
        assert_eq!(w.len(), 0);
        assert_eq!(w.archetype_count(), 2);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn b1_violation_is_reported() {
        let mut w = World::new();
        w.spawn_with((C(3), A(1), B(2)));
        assert_eq!(w.check_invariants(), Ok(()));
        // Swap two column indices.
        for info in w.bundles.values_mut() {
            info.columns.swap(0, 1);
        }
        assert!(violation(&w).contains("B1"), "got: {}", violation(&w));
        // Point the bundle at another archetype.
        let mut w = World::new();
        w.spawn_with((A(1), B(2)));
        w.spawn_with((A(1), C(2)));
        let other = w
            .bundles
            .values()
            .map(|i| i.archetype.index())
            .max()
            .unwrap();
        let first = w
            .bundles
            .values_mut()
            .find(|i| i.archetype.index() != other)
            .unwrap();
        first.archetype = ArchetypeId::new(other);
        assert!(violation(&w).contains("B1"), "got: {}", violation(&w));
    }
}
