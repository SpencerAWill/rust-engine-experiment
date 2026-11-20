//! Typed tuple queries.
//!
//! A query names its parameters (`&T`, `&mut T`, `Option<&T>`, `Option<&mut T>`, `Entity`, tuples
//! of those) and its filters (`.with::<T>()`, `.without::<T>()`). Running it visits the archetypes
//! in id order, keeps those that have every required type and none of the excluded ones, binds
//! each parameter once per archetype to a whole column slice, and then fetches by row index, so
//! a row costs an index into slices and no probe (spec section 4.6).
//!
//! The tuple-query API follows the design of `bevy_ecs` and `hecs` (typed tuples of `&T`,
//! `&mut T` and `Option<&T>`, with `with` and `without` filters, matched against archetypes).
//! No code was taken from either; the implementation is original and adapted from this
//! repository's own sparse-set backend (spec section 11).

use core::fmt;
use core::iter::FusedIterator;
use core::marker::PhantomData;

use crate::Entity;
use crate::archetype::Archetype;
use crate::world::World;

/// Implementation details of the sealed traits; unnameable outside the crate.
pub(crate) mod seal {
    use core::any::TypeId;

    use crate::Entity;
    use crate::archetype::{Archetype, ComponentId};
    use crate::table::{ErasedColumn, column_slice, column_slice_mut};

    /// How a query uses a component type.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Role {
        /// `&T`, `&mut T` or `With<T>`: the archetype must have it.
        Required,
        /// `Option<&T>` or `Option<&mut T>`.
        Optional,
        /// `Without<T>`: the archetype must not have it.
        Excluded,
    }

    /// One component type a query mentions.
    #[derive(Debug, Clone, Copy)]
    pub struct TypeEntry {
        /// The type.
        pub id: TypeId,
        /// Its name, for panic messages.
        pub name: &'static str,
        /// How the query uses it.
        pub role: Role,
        /// The world's id for the type, if it was ever registered (set when a run starts).
        pub component: Option<ComponentId>,
    }

    impl TypeEntry {
        /// An entry for `T`.
        pub fn of<T: 'static>(role: Role) -> Self {
            Self {
                id: TypeId::of::<T>(),
                name: core::any::type_name::<T>(),
                role,
                component: None,
            }
        }

        /// Whether `archetype` passes this entry: it has a required type and lacks an excluded
        /// one.
        pub fn passes(&self, archetype: &Archetype) -> bool {
            let has = self
                .component
                .is_some_and(|c| archetype.column_of(c).is_some());
            match self.role {
                Role::Required => has,
                Role::Optional => true,
                Role::Excluded => !has,
            }
        }
    }

    /// A column handed to one parameter of a running query.
    pub enum Slot<'w> {
        /// From a read-only query.
        Shared(&'w dyn ErasedColumn),
        /// From a mutable query.
        Mut(&'w mut dyn ErasedColumn),
    }

    impl core::fmt::Debug for Slot<'_> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                Slot::Shared(c) => write!(f, "Shared({})", c.value_type_name()),
                Slot::Mut(c) => write!(f, "Mut({})", c.value_type_name()),
            }
        }
    }

    impl<'w> Slot<'w> {
        /// The column's values, shared.
        pub fn into_ref<T: 'static>(self) -> &'w [T] {
            let column: &'w dyn ErasedColumn = match self {
                Slot::Shared(c) => c,
                Slot::Mut(c) => c,
            };
            column_slice(column)
        }

        /// The column's values, exclusive.
        pub fn into_mut<T: 'static>(self) -> &'w mut [T] {
            match self {
                Slot::Mut(c) => column_slice_mut(c),
                Slot::Shared(_) => unreachable!("a read-only query has no `&mut` parameter"),
            }
        }
    }

    /// The columns of the archetype a run is on, one slot per parameter type in declaration
    /// order, handed out by `next` as the fetches are bound. The vector is allocated once per
    /// run and refilled for each matched archetype.
    #[derive(Debug)]
    pub struct Slots<'w> {
        columns: Vec<Option<Slot<'w>>>,
        cursor: usize,
        entities: &'w [Entity],
        len: usize,
    }

    impl<'w> Slots<'w> {
        /// Room for `params` slots.
        pub fn new(params: usize) -> Self {
            Self {
                columns: Vec::with_capacity(params),
                cursor: 0,
                entities: &[],
                len: 0,
            }
        }

        /// Points the slots at `archetype`'s columns, shared. `entries[..params]` are the
        /// parameter types.
        pub fn fill_shared(&mut self, archetype: &'w Archetype, entries: &[TypeEntry]) {
            self.columns.clear();
            let table = &archetype.table;
            self.columns.extend(entries.iter().map(|e| {
                e.component
                    .and_then(|c| archetype.column_of(c))
                    .map(|k| Slot::Shared(table.column(k)))
            }));
            self.entities = table.entities();
            self.len = table.len();
            self.cursor = 0;
        }

        /// Points the slots at `archetype`'s columns, exclusive: one pass over the column list
        /// hands each `&mut` to its slot, so no two slots alias and no `unsafe` is needed.
        pub fn fill_exclusive(&mut self, archetype: &'w mut Archetype, entries: &[TypeEntry]) {
            let Archetype {
                components, table, ..
            } = archetype;
            self.len = table.len();
            let (entities, columns) = table.columns_mut();
            self.entities = entities;
            self.columns.clear();
            self.columns.resize_with(entries.len(), || None);
            for (k, column) in columns.iter_mut().enumerate() {
                let id = Some(components[k]);
                if let Some(j) = entries.iter().position(|e| e.component == id) {
                    self.columns[j] = Some(Slot::Mut(&mut **column));
                }
            }
            self.cursor = 0;
        }

        /// The next slot.
        pub fn next(&mut self) -> Option<Slot<'w>> {
            let n = self.cursor;
            self.cursor += 1;
            self.columns[n].take()
        }

        /// The archetype's entities.
        pub fn entities(&self) -> &'w [Entity] {
            self.entities
        }

        /// The archetype's row count.
        pub fn len(&self) -> usize {
            self.len
        }
    }

    /// The fetch machinery of a query parameter.
    pub trait Param {
        /// What the query yields for one matching entity.
        type Item<'w>;
        /// Per-archetype state: the bound column slices.
        type Fetch<'w>;
        /// Appends the component types this parameter mentions, in declaration order.
        fn types(out: &mut Vec<TypeEntry>);
        /// Takes this parameter's slots, in the same order as `types`.
        fn bind<'w>(slots: &mut Slots<'w>) -> Self::Fetch<'w>;
        /// The item for row `row`, which is less than the archetype's row count.
        fn get<'a>(fetch: &'a mut Self::Fetch<'_>, row: usize) -> Self::Item<'a>;
    }

    /// `get` for read-only parameters, which can hand out items that outlive the fetch borrow.
    pub trait ReadOnly: Param {
        /// As `Param::get`, with the lifetime of the world borrow.
        fn get_shared<'w>(fetch: &Self::Fetch<'w>, row: usize) -> Self::Item<'w>;
    }

    /// The type list of a query filter.
    pub trait Filter {
        /// Appends the component types this filter mentions.
        fn types(out: &mut Vec<TypeEntry>);
    }
}

/// One parameter of a query: `&T`, `&mut T`, `Option<&T>`, `Option<&mut T>`, [`Entity`], or a
/// tuple of 1 to 4 parameters (tuples nest). Sealed.
pub trait QueryParam: seal::Param {}

/// A [`QueryParam`] without `&mut`, which [`World::query`] accepts. Sealed.
pub trait ReadOnly: QueryParam + seal::ReadOnly {}

/// A filter of a query: `()`, [`With`], [`Without`], or a pair of filters. Sealed.
pub trait Filter: seal::Filter {}

/// The entity must have a `T`. See [`Query::with`].
pub struct With<T>(PhantomData<fn() -> T>);

/// The entity must not have a `T`. See [`Query::without`].
pub struct Without<T>(PhantomData<fn() -> T>);

impl<T> fmt::Debug for With<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "With<{}>", core::any::type_name::<T>())
    }
}

impl<T> fmt::Debug for Without<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Without<{}>", core::any::type_name::<T>())
    }
}

const REQUIRED: &str = "a matched archetype has a column for every required type";

impl<T: 'static> seal::Param for &T {
    type Item<'w> = &'w T;
    type Fetch<'w> = &'w [T];

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>(seal::Role::Required));
    }

    fn bind<'w>(slots: &mut seal::Slots<'w>) -> Self::Fetch<'w> {
        let len = slots.len();
        &slots.next().expect(REQUIRED).into_ref::<T>()[..len]
    }

    #[inline]
    fn get<'a>(fetch: &'a mut &[T], row: usize) -> Self::Item<'a> {
        &fetch[row]
    }
}

impl<T: 'static> QueryParam for &T {}

impl<T: 'static> seal::ReadOnly for &T {
    #[inline]
    fn get_shared<'w>(fetch: &Self::Fetch<'w>, row: usize) -> Self::Item<'w> {
        let column: &'w [T] = fetch;
        &column[row]
    }
}

impl<T: 'static> ReadOnly for &T {}

impl<T: 'static> seal::Param for &mut T {
    type Item<'w> = &'w mut T;
    type Fetch<'w> = &'w mut [T];

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>(seal::Role::Required));
    }

    fn bind<'w>(slots: &mut seal::Slots<'w>) -> Self::Fetch<'w> {
        let len = slots.len();
        &mut slots.next().expect(REQUIRED).into_mut::<T>()[..len]
    }

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>, row: usize) -> Self::Item<'a> {
        &mut fetch[row]
    }
}

impl<T: 'static> QueryParam for &mut T {}

impl<T: 'static> seal::Param for Option<&T> {
    type Item<'w> = Option<&'w T>;
    type Fetch<'w> = Option<&'w [T]>;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>(seal::Role::Optional));
    }

    fn bind<'w>(slots: &mut seal::Slots<'w>) -> Self::Fetch<'w> {
        let len = slots.len();
        slots.next().map(|s| &s.into_ref::<T>()[..len])
    }

    #[inline]
    fn get<'a>(fetch: &'a mut Option<&[T]>, row: usize) -> Self::Item<'a> {
        fetch.as_ref().map(|c| &c[row])
    }
}

impl<T: 'static> QueryParam for Option<&T> {}

impl<T: 'static> seal::ReadOnly for Option<&T> {
    #[inline]
    fn get_shared<'w>(fetch: &Self::Fetch<'w>, row: usize) -> Self::Item<'w> {
        let column: Option<&'w [T]> = *fetch;
        column.map(|c| &c[row])
    }
}

impl<T: 'static> ReadOnly for Option<&T> {}

impl<T: 'static> seal::Param for Option<&mut T> {
    type Item<'w> = Option<&'w mut T>;
    type Fetch<'w> = Option<&'w mut [T]>;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>(seal::Role::Optional));
    }

    fn bind<'w>(slots: &mut seal::Slots<'w>) -> Self::Fetch<'w> {
        let len = slots.len();
        slots.next().map(|s| &mut s.into_mut::<T>()[..len])
    }

    #[inline]
    fn get<'a>(fetch: &'a mut Option<&mut [T]>, row: usize) -> Self::Item<'a> {
        fetch.as_mut().map(|c| &mut c[row])
    }
}

impl<T: 'static> QueryParam for Option<&mut T> {}

impl seal::Param for Entity {
    type Item<'w> = Entity;
    type Fetch<'w> = &'w [Entity];

    fn types(_: &mut Vec<seal::TypeEntry>) {}

    fn bind<'w>(slots: &mut seal::Slots<'w>) -> Self::Fetch<'w> {
        &slots.entities()[..slots.len()]
    }

    #[inline]
    fn get<'a>(fetch: &'a mut &[Entity], row: usize) -> Self::Item<'a> {
        fetch[row]
    }
}

impl QueryParam for Entity {}

impl seal::ReadOnly for Entity {
    #[inline]
    fn get_shared<'w>(fetch: &Self::Fetch<'w>, row: usize) -> Self::Item<'w> {
        fetch[row]
    }
}

impl ReadOnly for Entity {}

macro_rules! impl_tuple_param {
    ($($p:ident),+) => {
        #[allow(non_snake_case)]
        impl<$($p: QueryParam),+> seal::Param for ($($p,)+) {
            type Item<'w> = ($($p::Item<'w>,)+);
            type Fetch<'w> = ($($p::Fetch<'w>,)+);

            fn types(out: &mut Vec<seal::TypeEntry>) {
                $($p::types(out);)+
            }

            fn bind<'w>(slots: &mut seal::Slots<'w>) -> Self::Fetch<'w> {
                ($($p::bind(slots),)+)
            }

            #[inline]
            fn get<'a>(fetch: &'a mut ($($p::Fetch<'_>,)+), row: usize) -> Self::Item<'a> {
                let ($($p,)+) = fetch;
                ($($p::get($p, row),)+)
            }
        }

        impl<$($p: QueryParam),+> QueryParam for ($($p,)+) {}

        #[allow(non_snake_case)]
        impl<$($p: ReadOnly),+> seal::ReadOnly for ($($p,)+) {
            #[inline]
            fn get_shared<'w>(fetch: &Self::Fetch<'w>, row: usize) -> Self::Item<'w> {
                let ($($p,)+) = fetch;
                ($($p::get_shared($p, row),)+)
            }
        }

        impl<$($p: ReadOnly),+> ReadOnly for ($($p,)+) {}
    };
}

impl_tuple_param!(P0);
impl_tuple_param!(P0, P1);
impl_tuple_param!(P0, P1, P2);
impl_tuple_param!(P0, P1, P2, P3);

impl seal::Filter for () {
    fn types(_: &mut Vec<seal::TypeEntry>) {}
}

impl Filter for () {}

impl<T: 'static> seal::Filter for With<T> {
    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>(seal::Role::Required));
    }
}

impl<T: 'static> Filter for With<T> {}

impl<T: 'static> seal::Filter for Without<T> {
    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>(seal::Role::Excluded));
    }
}

impl<T: 'static> Filter for Without<T> {}

impl<F: Filter, G: Filter> seal::Filter for (F, G) {
    fn types(out: &mut Vec<seal::TypeEntry>) {
        F::types(out);
        G::types(out);
    }
}

impl<F: Filter, G: Filter> Filter for (F, G) {}

/// What a run needs to match archetypes: every type the query mentions with its component id,
/// parameters first, and how many of them are parameters (the rest are filters). One allocation.
struct Plan {
    entries: Vec<seal::TypeEntry>,
    params: usize,
}

impl Plan {
    /// Collects and checks the query's types and looks their ids up. `op` names the running
    /// method in panic messages. `Query` and `QueryMut` both start here, so they cannot
    /// disagree.
    #[track_caller]
    fn new<Q: QueryParam, F: Filter>(world: &World, op: &str) -> Self {
        let mut entries = Vec::with_capacity(8);
        Q::types(&mut entries);
        let params = entries.len();
        F::types(&mut entries);
        for (i, e) in entries.iter().enumerate() {
            assert!(
                !entries[..i].iter().any(|other| other.id == e.id),
                "{op}: `{}` appears twice in the query; name each type once, as a parameter or a filter",
                e.name
            );
        }
        for e in &mut entries {
            e.component = world.component_id_of(e.id);
        }
        Self { entries, params }
    }

    /// Whether the archetype has every required type and none of the excluded ones. Both query
    /// types and `matched_archetypes` use this one function.
    fn matches(&self, archetype: &Archetype) -> bool {
        self.entries.iter().all(|e| e.passes(archetype))
    }

    /// The parameter entries, which the slots cover.
    fn param_entries(&self) -> &[seal::TypeEntry] {
        &self.entries[..self.params]
    }
}

/// A read-only query, borrowed from [`World::query`]. Add filters with [`with`](Self::with)
/// and [`without`](Self::without), then run it with [`for_each`](Self::for_each),
/// [`iter`](Self::iter) or a `for` loop.
///
/// # Examples
///
/// ```
/// use spike_ecs_archetype::{Entity, World};
///
/// struct Position(f32);
/// struct Frozen;
///
/// let mut world = World::new();
/// let moving = world.spawn_with((Position(1.0),));
/// let frozen = world.spawn_with((Position(2.0), Frozen));
///
/// let ids: Vec<Entity> = world
///     .query::<(Entity, &Position)>()
///     .without::<Frozen>()
///     .iter()
///     .map(|(e, _)| e)
///     .collect();
/// assert_eq!(ids, [moving]);
/// assert!(world.is_alive(frozen));
/// ```
#[must_use = "a query does nothing until it is run"]
pub struct Query<'w, Q, F = ()> {
    world: &'w World,
    marker: PhantomData<fn() -> (Q, F)>,
}

impl<'w, Q: ReadOnly> Query<'w, Q> {
    pub(crate) fn new(world: &'w World) -> Self {
        Self {
            world,
            marker: PhantomData,
        }
    }
}

impl<'w, Q: ReadOnly, F: Filter> Query<'w, Q, F> {
    /// Keeps only entities that have a `T`.
    pub fn with<T: 'static>(self) -> Query<'w, Q, (F, With<T>)> {
        Query {
            world: self.world,
            marker: PhantomData,
        }
    }

    /// Keeps only entities that do not have a `T`.
    pub fn without<T: 'static>(self) -> Query<'w, Q, (F, Without<T>)> {
        Query {
            world: self.world,
            marker: PhantomData,
        }
    }

    /// Calls `f` for each matching entity.
    ///
    /// # Panics
    ///
    /// Panics if a type appears twice among the parameters and filters.
    #[track_caller]
    pub fn for_each(self, mut f: impl FnMut(Q::Item<'w>)) {
        let world: &'w World = self.world;
        let plan = Plan::new::<Q, F>(world, "Query::for_each");
        let mut slots = seal::Slots::new(plan.params);
        for archetype in world.archetypes() {
            if archetype.table.len() == 0 || !plan.matches(archetype) {
                continue;
            }
            slots.fill_shared(archetype, plan.param_entries());
            let len = slots.len();
            let fetch = Q::bind(&mut slots);
            for row in 0..len {
                f(Q::get_shared(&fetch, row));
            }
        }
    }

    /// An iterator over the matching entities.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`for_each`](Self::for_each).
    #[track_caller]
    pub fn iter(&self) -> QueryIter<'w, Q, F> {
        self.start("Query::iter")
    }

    #[track_caller]
    fn start(&self, op: &str) -> QueryIter<'w, Q, F> {
        let world: &'w World = self.world;
        let plan = Plan::new::<Q, F>(world, op);
        let archetypes = world.archetypes();
        let remaining = archetypes
            .iter()
            .filter(|a| plan.matches(a))
            .map(|a| a.table.len())
            .sum();
        let slots = seal::Slots::new(plan.params);
        QueryIter {
            archetypes,
            plan,
            slots,
            next_archetype: 0,
            fetch: None,
            row: 0,
            len: 0,
            remaining,
            marker: PhantomData,
        }
    }

    /// The ids of the archetypes a run would match, in id order, rows or not, for tests.
    #[cfg(test)]
    pub(crate) fn matched_archetypes(&self) -> Vec<crate::archetype::ArchetypeId> {
        let plan = Plan::new::<Q, F>(self.world, "Query::matched_archetypes");
        matched(self.world.archetypes(), &plan)
    }
}

/// The ids of the archetypes `plan` matches, in id order.
#[cfg(test)]
fn matched(archetypes: &[Archetype], plan: &Plan) -> Vec<crate::archetype::ArchetypeId> {
    archetypes
        .iter()
        .enumerate()
        .filter(|(_, a)| plan.matches(a))
        .map(|(i, _)| crate::archetype::ArchetypeId::new(i))
        .collect()
}

impl<'w, Q: ReadOnly, F: Filter> IntoIterator for Query<'w, Q, F> {
    type Item = Q::Item<'w>;
    type IntoIter = QueryIter<'w, Q, F>;

    /// # Panics
    ///
    /// Panics under the same conditions as [`Query::for_each`].
    #[track_caller]
    fn into_iter(self) -> QueryIter<'w, Q, F> {
        self.start("Query::into_iter")
    }
}

impl<'w, Q: ReadOnly, F: Filter> IntoIterator for &Query<'w, Q, F> {
    type Item = Q::Item<'w>;
    type IntoIter = QueryIter<'w, Q, F>;

    /// # Panics
    ///
    /// Panics under the same conditions as [`Query::for_each`].
    #[track_caller]
    fn into_iter(self) -> QueryIter<'w, Q, F> {
        self.start("Query::into_iter")
    }
}

impl<Q: ReadOnly, F: Filter> fmt::Debug for Query<'_, Q, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Query")
            .field("params", &core::any::type_name::<Q>())
            .field("filters", &core::any::type_name::<F>())
            .finish_non_exhaustive()
    }
}

/// The iterator of a [`Query`]: yields one item per matching entity, archetype by archetype.
#[must_use = "iterators do nothing unless consumed"]
pub struct QueryIter<'w, Q: ReadOnly, F: Filter> {
    archetypes: &'w [Archetype],
    plan: Plan,
    slots: seal::Slots<'w>,
    next_archetype: usize,
    /// The bound columns of the current archetype, if one is open.
    fetch: Option<Q::Fetch<'w>>,
    row: usize,
    len: usize,
    /// Rows of the matched archetypes not yet yielded (the current one included).
    remaining: usize,
    marker: PhantomData<fn() -> F>,
}

impl<Q: ReadOnly, F: Filter> QueryIter<'_, Q, F> {
    /// Opens the next matched archetype that has rows. Returns whether there was one.
    fn open_next(&mut self) -> bool {
        while let Some(archetype) = self.archetypes.get(self.next_archetype) {
            self.next_archetype += 1;
            if archetype.table.len() > 0 && self.plan.matches(archetype) {
                self.slots.fill_shared(archetype, self.plan.param_entries());
                self.len = self.slots.len();
                self.row = 0;
                self.fetch = Some(Q::bind(&mut self.slots));
                return true;
            }
        }
        self.fetch = None;
        false
    }
}

impl<'w, Q: ReadOnly, F: Filter> Iterator for QueryIter<'w, Q, F> {
    type Item = Q::Item<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.row < self.len
                && let Some(fetch) = &self.fetch
            {
                let item = Q::get_shared(fetch, self.row);
                self.row += 1;
                self.remaining -= 1;
                return Some(item);
            }
            if !self.open_next() {
                return None;
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<Q: ReadOnly, F: Filter> FusedIterator for QueryIter<'_, Q, F> {}

impl<Q: ReadOnly, F: Filter> fmt::Debug for QueryIter<'_, Q, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueryIter")
            .field("remaining", &self.remaining)
            .finish_non_exhaustive()
    }
}

/// A query that may write, borrowed from [`World::query_mut`]. It is run with
/// [`for_each`](Self::for_each); there is no `iter` (an external mutable iterator is an API
/// question for a later milestone).
///
/// # Examples
///
/// ```
/// use spike_ecs_archetype::World;
///
/// struct Position(f32);
/// struct Velocity(f32);
///
/// let mut world = World::new();
/// let e = world.spawn_with((Position(0.0), Velocity(2.0)));
///
/// world
///     .query_mut::<(&mut Position, &Velocity)>()
///     .for_each(|(p, v)| p.0 += v.0);
/// assert_eq!(world.get::<Position>(e).map(|p| p.0), Some(2.0));
/// ```
#[must_use = "a query does nothing until it is run"]
pub struct QueryMut<'w, Q, F = ()> {
    world: &'w mut World,
    marker: PhantomData<fn() -> (Q, F)>,
}

impl<'w, Q: QueryParam> QueryMut<'w, Q> {
    pub(crate) fn new(world: &'w mut World) -> Self {
        Self {
            world,
            marker: PhantomData,
        }
    }
}

impl<'w, Q: QueryParam, F: Filter> QueryMut<'w, Q, F> {
    /// Keeps only entities that have a `T`.
    pub fn with<T: 'static>(self) -> QueryMut<'w, Q, (F, With<T>)> {
        QueryMut {
            world: self.world,
            marker: PhantomData,
        }
    }

    /// Keeps only entities that do not have a `T`.
    pub fn without<T: 'static>(self) -> QueryMut<'w, Q, (F, Without<T>)> {
        QueryMut {
            world: self.world,
            marker: PhantomData,
        }
    }

    /// Calls `f` for each matching entity.
    ///
    /// # Panics
    ///
    /// Panics if a type appears twice among the parameters and filters.
    #[track_caller]
    pub fn for_each(self, mut f: impl FnMut(Q::Item<'_>)) {
        let plan = Plan::new::<Q, F>(self.world, "QueryMut::for_each");
        let mut slots = seal::Slots::new(plan.params);
        for archetype in self.world.archetypes_mut() {
            if archetype.table.len() == 0 || !plan.matches(archetype) {
                continue;
            }
            slots.fill_exclusive(archetype, plan.param_entries());
            let len = slots.len();
            let mut fetch = Q::bind(&mut slots);
            for row in 0..len {
                f(Q::get(&mut fetch, row));
            }
        }
    }

    /// The ids of the archetypes a run would match, in id order, rows or not, for tests.
    #[cfg(test)]
    pub(crate) fn matched_archetypes(&self) -> Vec<crate::archetype::ArchetypeId> {
        let plan = Plan::new::<Q, F>(self.world, "QueryMut::matched_archetypes");
        matched(self.world.archetypes(), &plan)
    }
}

impl<Q: QueryParam, F: Filter> fmt::Debug for QueryMut<'_, Q, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueryMut")
            .field("params", &core::any::type_name::<Q>())
            .field("filters", &core::any::type_name::<F>())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use crate::archetype::ArchetypeId;
    use crate::{Entity, World};

    #[derive(Debug, PartialEq)]
    struct A(u32);
    #[derive(Debug, PartialEq)]
    struct B(u32);
    #[derive(Debug, PartialEq)]
    struct C(u32);
    #[derive(Debug, PartialEq)]
    struct D(u32);

    fn sorted(mut v: Vec<Entity>) -> Vec<Entity> {
        v.sort_by_key(|e| e.to_bits());
        v
    }

    fn ids(v: &[usize]) -> Vec<ArchetypeId> {
        v.iter().map(|&i| ArchetypeId::new(i)).collect()
    }

    #[test]
    fn required_types_select_entities() {
        let mut w = World::new();
        let (e1, e2, e3, e4) = (w.spawn(), w.spawn(), w.spawn(), w.spawn());
        w.insert(e1, A(1));
        w.insert(e1, B(1));
        w.insert(e2, A(2));
        w.insert(e3, B(3));
        w.insert(e4, A(4));
        w.insert(e4, B(4));
        let got: Vec<Entity> = w
            .query::<(Entity, &A, &B)>()
            .iter()
            .map(|(e, _, _)| e)
            .collect();
        assert_eq!(sorted(got), sorted(vec![e1, e4]));
    }

    #[test]
    fn option_yields_some_exactly_when_present() {
        let mut w = World::new();
        let (e1, e2) = (w.spawn(), w.spawn());
        w.insert(e1, A(1));
        w.insert(e1, B(10));
        w.insert(e2, A(2));
        let mut got: Vec<(u32, Option<u32>)> = w
            .query::<(&A, Option<&B>)>()
            .iter()
            .map(|(a, b)| (a.0, b.map(|b| b.0)))
            .collect();
        got.sort_unstable();
        assert_eq!(got, [(1, Some(10)), (2, None)]);
    }

    #[test]
    fn with_and_without_filters_chain() {
        let mut w = World::new();
        let keep = w.spawn();
        w.insert(keep, A(0));
        w.insert(keep, B(0));
        w.insert(keep, D(0));
        let no_d = w.spawn();
        w.insert(no_d, A(0));
        w.insert(no_d, B(0));
        let has_c = w.spawn();
        w.insert(has_c, A(0));
        w.insert(has_c, B(0));
        w.insert(has_c, C(0));
        w.insert(has_c, D(0));
        let got: Vec<Entity> = w
            .query::<(Entity, &A)>()
            .with::<B>()
            .without::<C>()
            .with::<D>()
            .iter()
            .map(|(e, _)| e)
            .collect();
        assert_eq!(got, [keep]);
    }

    #[test]
    fn with_alone_drives_a_query() {
        let mut w = World::new();
        let (e1, e2) = (w.spawn(), w.spawn());
        w.insert(e1, B(1));
        w.insert(e2, A(2));
        let got: Vec<Entity> = w.query::<Entity>().with::<B>().into_iter().collect();
        assert_eq!(got, [e1]);
    }

    #[test]
    fn entity_parameter_yields_the_entity() {
        let mut w = World::new();
        let e = w.spawn();
        w.insert(e, A(5));
        let mut seen = Vec::new();
        w.query::<(Entity, &A)>()
            .for_each(|(id, a)| seen.push((id, a.0)));
        assert_eq!(seen, [(e, 5)]);
    }

    #[test]
    fn nested_tuple_works() {
        let mut w = World::new();
        let e = w.spawn();
        w.insert(e, A(1));
        w.insert(e, B(2));
        w.insert(e, C(3));
        w.insert(e, D(4));
        let f = w.spawn();
        w.insert(f, A(10));
        w.insert(f, B(20));
        w.insert(f, C(30));
        let mut sums = Vec::new();
        w.query_mut::<(Entity, (&mut A, &B, &C, Option<&D>))>()
            .for_each(|(id, (a, b, c, d))| {
                a.0 += b.0 + c.0 + d.map_or(0, |d| d.0);
                sums.push(id);
            });
        assert_eq!(sorted(sums), sorted(vec![e, f]));
        assert_eq!(w.get::<A>(e), Some(&A(10)));
        assert_eq!(w.get::<A>(f), Some(&A(60)));
    }

    #[test]
    fn never_inserted_required_type_matches_nothing() {
        let mut w = World::new();
        let e = w.spawn();
        w.insert(e, A(1));
        assert_eq!(w.query::<(&A, &B)>().iter().count(), 0);
        assert_eq!(w.query::<&A>().with::<B>().iter().count(), 0);
        assert_eq!(w.query::<(&A, &B)>().matched_archetypes(), ids(&[]));
        let mut n = 0;
        w.query_mut::<(&mut A, &B)>().for_each(|_| n += 1);
        assert_eq!(n, 0);
        assert_eq!(w.query_mut::<(&mut A, &B)>().matched_archetypes(), ids(&[]));
        // An optional or excluded type that was never inserted does not block the query.
        assert_eq!(
            w.query::<(&A, Option<&B>)>().without::<C>().iter().count(),
            1
        );
    }

    #[test]
    fn mutable_query_writes_are_visible() {
        let mut w = World::new();
        let es: Vec<Entity> = (0..4).map(|_| w.spawn()).collect();
        for (i, e) in es.iter().enumerate() {
            w.insert(*e, A(u32::try_from(i).unwrap()));
            if i % 2 == 0 {
                w.insert(*e, B(100));
            }
        }
        w.query_mut::<(&mut A, &B)>().for_each(|(a, b)| a.0 += b.0);
        let got: Vec<u32> = es.iter().map(|e| w.get::<A>(*e).unwrap().0).collect();
        assert_eq!(got, [100, 1, 102, 3]);
    }

    #[test]
    fn iter_for_each_and_into_iter_agree() {
        let mut w = World::new();
        for i in 0..10u32 {
            let e = w.spawn();
            w.insert(e, A(i));
            if i % 3 == 0 {
                w.insert(e, B(i));
            }
        }
        let by_iter: Vec<u32> = w.query::<(&A, &B)>().iter().map(|(a, _)| a.0).collect();
        let mut by_for_each = Vec::new();
        w.query::<(&A, &B)>()
            .for_each(|(a, _)| by_for_each.push(a.0));
        let by_into: Vec<u32> = w
            .query::<(&A, &B)>()
            .into_iter()
            .map(|(a, _)| a.0)
            .collect();
        let q = w.query::<(&A, &B)>();
        let mut by_ref = Vec::new();
        for (a, _) in &q {
            by_ref.push(a.0);
        }
        assert_eq!(by_iter.len(), 4);
        assert_eq!(by_iter, by_for_each);
        assert_eq!(by_iter, by_into);
        assert_eq!(by_iter, by_ref);
        // The query is still usable afterwards.
        assert_eq!(q.into_iter().count(), 4);
    }

    #[test]
    fn size_hint_is_exact_across_archetypes() {
        let mut w = World::new();
        for i in 0..7u32 {
            let e = w.spawn_with((A(i),));
            if i % 2 == 0 {
                w.insert(e, B(i));
            }
            if i % 3 == 0 {
                w.insert(e, C(i));
            }
        }
        let q = w.query::<&A>();
        let mut it = q.iter();
        assert_eq!(it.size_hint(), (7, Some(7)));
        for left in (0..7).rev() {
            assert!(it.next().is_some());
            assert_eq!(it.size_hint(), (left, Some(left)));
        }
        assert!(it.next().is_none());
        assert!(it.next().is_none());
        assert_eq!(it.size_hint(), (0, Some(0)));
        // A filter that drops whole archetypes keeps the hint exact.
        let q = w.query::<&A>().without::<B>();
        let it = q.iter();
        assert_eq!(it.size_hint(), (3, Some(3)));
        assert_eq!(it.count(), 3);
        // A query that matches nothing is an empty, fused iterator.
        let mut it = w.query::<&D>().iter();
        assert_eq!(it.size_hint(), (0, Some(0)));
        assert!(it.next().is_none());
        assert!(it.next().is_none());
    }

    #[test]
    #[should_panic(
        expected = "QueryMut::for_each: `spike_ecs_archetype::query::tests::A` appears twice in the query"
    )]
    fn repeated_type_panics() {
        let mut w = World::new();
        w.query_mut::<(&mut A, &A)>().for_each(|_| {});
    }

    #[test]
    #[should_panic(
        expected = "Query::iter: `spike_ecs_archetype::query::tests::A` appears twice in the query"
    )]
    fn a_type_in_a_parameter_and_a_filter_panics() {
        let w = World::new();
        let _ = w.query::<&A>().without::<A>().iter();
    }

    #[test]
    #[should_panic(
        expected = "Query::into_iter: `spike_ecs_archetype::query::tests::A` appears twice in the query"
    )]
    fn for_loop_over_a_query_reference_names_into_iter() {
        let w = World::new();
        let q = w.query::<(&A, &A)>();
        for _ in &q {}
    }

    #[test]
    fn query_without_a_required_type_visits_every_live_entity() {
        let mut w = World::new();
        let bare = w.spawn();
        let only_a = w.spawn_with((A(1),));
        let ab = w.spawn_with((A(2), B(2)));
        let only_b = w.spawn_with((B(3),));
        let all = sorted(vec![bare, only_a, ab, only_b]);
        // The empty archetype (id 0) is matched, and its entity is visited.
        let got: Vec<Entity> = w.query::<Entity>().iter().collect();
        assert_eq!(sorted(got), all);
        assert_eq!(w.query::<Entity>().matched_archetypes().len(), 4);
        let mut n = 0;
        w.query::<Entity>().for_each(|_| n += 1);
        assert_eq!(n, 4);
        let got: Vec<Option<u32>> = {
            let mut v: Vec<_> = w
                .query::<Option<&A>>()
                .iter()
                .map(|a| a.map(|a| a.0))
                .collect();
            v.sort_unstable();
            v
        };
        assert_eq!(got, [None, None, Some(1), Some(2)]);
        let got: Vec<Entity> = w.query::<Entity>().without::<A>().iter().collect();
        assert_eq!(sorted(got), sorted(vec![bare, only_b]));
        // `QueryMut` takes the same path.
        let mut seen = Vec::new();
        w.query_mut::<Entity>()
            .without::<A>()
            .for_each(|e| seen.push(e));
        assert_eq!(sorted(seen), sorted(vec![bare, only_b]));
    }

    #[test]
    fn matched_archetypes_includes_empty_matches() {
        let mut w = World::new();
        let e = w.spawn_with((A(1), B(1)));
        let f = w.spawn_with((A(2),));
        // Archetypes: 0 empty, 1 (A, B), 2 (A).
        assert_eq!(w.query::<&A>().matched_archetypes(), ids(&[1, 2]));
        w.remove::<B>(e);
        // `e` moved to (A); archetype 1 is now empty but still matches.
        assert_eq!(w.query::<&A>().matched_archetypes(), ids(&[1, 2]));
        assert_eq!(w.query::<(&A, &B)>().matched_archetypes(), ids(&[1]));
        assert_eq!(w.query::<(&A, &B)>().iter().count(), 0);
        assert_eq!(
            w.query_mut::<(&mut A, &B)>().matched_archetypes(),
            ids(&[1])
        );
        assert_eq!(w.query::<&A>().iter().count(), 2);
        let _ = f;
    }

    #[test]
    fn archetypes_are_visited_in_id_order() {
        let mut w = World::new();
        // Create archetypes out of component order: 1 (C), 2 (B), 3 (A).
        let c = w.spawn_with((C(0),));
        let b = w.spawn_with((B(0),));
        let a = w.spawn_with((A(0),));
        assert_eq!(
            w.query::<Entity>().with::<A>().matched_archetypes(),
            ids(&[3])
        );
        let got: Vec<Entity> = w.query::<Entity>().iter().collect();
        assert_eq!(got, [c, b, a]);
        let mut seen = Vec::new();
        w.query_mut::<Entity>().for_each(|e| seen.push(e));
        assert_eq!(seen, [c, b, a]);
        assert_eq!(w.query::<Entity>().matched_archetypes(), ids(&[0, 1, 2, 3]));
    }
}
