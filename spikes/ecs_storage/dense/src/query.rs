//! Typed tuple queries over [`Join`] of the columns' presence sets.
//!
//! A query names its parameters (`&T`, `&mut T`, `Option<&T>`, `Option<&mut T>`, [`Entity`],
//! tuples of those) and its filters (`.with::<T>()`, `.without::<T>()`). Running it joins the
//! presence sets of the types it requires, minus those of the types it excludes, and fetches
//! each parameter at the indices the join yields, in ascending order (spec section 4.5).
//!
//! The tuple-query API follows the design of `bevy_ecs` and `hecs` (typed tuples of `&T`,
//! `&mut T` and `Option<&T>`, with `with` and `without` filters), and the joins follow the
//! bit-set joins of the `specs` crate. No code was taken from any of them; the implementation
//! is original (spec section 11).

use core::any::{TypeId, type_name};
use core::fmt;
use core::iter::FusedIterator;
use core::marker::PhantomData;
use std::collections::BTreeMap;

use engine::collections::{HierarchicalBitSet, Join};

use crate::Entity;
use crate::column::{DenseColumn, ErasedColumn, ValuesMut};
use crate::world::{ComponentId, World};

/// Implementation details of the sealed traits; unnameable outside the crate.
pub(crate) mod seal {
    use super::{
        BTreeMap, ComponentId, DenseColumn, Entity, ErasedColumn, HierarchicalBitSet, TypeId,
        World, fmt, type_name,
    };

    /// One component type a query mentions, for the repeated-type check.
    #[derive(Debug, Clone, Copy)]
    pub struct TypeEntry {
        /// The type.
        pub id: TypeId,
        /// Its name, for panic messages.
        pub name: &'static str,
    }

    impl TypeEntry {
        /// The entry of `T`.
        pub fn of<T: 'static>() -> Self {
            Self {
                id: TypeId::of::<T>(),
                name: type_name::<T>(),
            }
        }
    }

    /// What a mutable query's run collects while it takes its columns: every column is
    /// borrowed once, by one pass over the column list, so no two parameters alias and no
    /// `unsafe` is needed.
    pub struct MutCx<'w> {
        ids: &'w BTreeMap<TypeId, ComponentId>,
        columns: Vec<Option<&'w mut Box<dyn ErasedColumn>>>,
        entities: &'w [Entity],
        alive: &'w HierarchicalBitSet,
        /// The presence sets of the required types.
        pub required: Vec<&'w HierarchicalBitSet>,
        /// The presence sets of the excluded types that have a column.
        pub excluded: Vec<&'w HierarchicalBitSet>,
        /// A required type has no column, so the run matches nothing.
        pub empty: bool,
    }

    impl fmt::Debug for MutCx<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("MutCx")
                .field("required", &self.required.len())
                .field("excluded", &self.excluded.len())
                .field("empty", &self.empty)
                .finish_non_exhaustive()
        }
    }

    impl<'w> MutCx<'w> {
        /// Starts a run over `world`: collects its columns as `Some(&mut _)`.
        pub fn new(world: &'w mut World) -> Self {
            let (ids, columns, entities, alive) = world.split_for_query();
            Self {
                ids,
                columns: columns.iter_mut().map(Some).collect(),
                entities,
                alive,
                required: Vec::new(),
                excluded: Vec::new(),
                empty: false,
            }
        }

        /// Takes the column of `ty`, if it has one and nobody took it yet.
        fn take(&mut self, ty: TypeId) -> Option<&'w mut Box<dyn ErasedColumn>> {
            let id = self.ids.get(&ty)?;
            self.columns[id.index()].take()
        }

        /// Takes the column of `T` for reading, if it has one and nobody took it yet.
        pub fn take_shared<T: 'static>(&mut self) -> Option<&'w DenseColumn<T>> {
            self.take(TypeId::of::<T>())
                .map(|b| &**b)
                .and_then(|b| b.as_any().downcast_ref::<DenseColumn<T>>())
        }

        /// Takes the column of `T` for writing, if it has one and nobody took it yet.
        ///
        /// Hands out only the presence set and the value accessor, never the column itself,
        /// so a parameter cannot clear a slot or change a length during a run.
        pub fn take_split<T: 'static>(
            &mut self,
        ) -> Option<(&'w HierarchicalBitSet, crate::column::ValuesMut<'w, T>)> {
            self.take(TypeId::of::<T>())
                .and_then(|b| b.as_any_mut().downcast_mut::<DenseColumn<T>>())
                .map(DenseColumn::split_mut)
        }

        /// The entity at each index.
        pub fn entities(&self) -> &'w [Entity] {
            self.entities
        }

        /// The set of live indices, which drives a run with no required type.
        pub fn alive(&self) -> &'w HierarchicalBitSet {
            self.alive
        }
    }

    /// The fetch machinery of a query parameter.
    pub trait Param {
        /// What the query yields for one matching entity.
        type Item<'w>;
        /// Per-run state: borrowed columns.
        type Fetch<'w>;
        /// Appends the component types this parameter mentions, in declaration order.
        fn types(out: &mut Vec<TypeEntry>);
        /// Pushes the presence sets of the required types onto `required`. Returns `false` if a
        /// required type has no column (the query matches nothing).
        fn resolve<'w>(world: &'w World, required: &mut Vec<&'w HierarchicalBitSet>) -> bool;
        /// Borrows the columns of a read-only run.
        fn bind(world: &World) -> Self::Fetch<'_>;
        /// Takes the columns of a mutable run and records the required presence sets in `cx`.
        fn bind_mut<'w>(cx: &mut MutCx<'w>) -> Self::Fetch<'w>;
        /// The item at `index`, which the join yielded.
        fn get<'a>(fetch: &'a mut Self::Fetch<'_>, index: usize) -> Self::Item<'a>;
    }

    /// `get` for read-only parameters, which can hand out items that outlive the fetch borrow.
    pub trait ReadOnly: Param {
        /// As `Param::get`, with the lifetime of the world borrow.
        fn get_shared<'w>(fetch: &Self::Fetch<'w>, index: usize) -> Self::Item<'w>;
    }

    /// The machinery of a query filter on a mutable run (a read-only query resolves its
    /// filters as they are added).
    pub trait Filter {
        /// Appends the component types this filter mentions.
        fn types(out: &mut Vec<TypeEntry>);
        /// Takes the filter's columns and records their presence sets in `cx`.
        fn resolve_mut(cx: &mut MutCx<'_>);
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
        write!(f, "With<{}>", type_name::<T>())
    }
}

impl<T> fmt::Debug for Without<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Without<{}>", type_name::<T>())
    }
}

/// The panic of a required fetch whose slot is empty: the presence bit lied (invariant D1).
#[cold]
#[track_caller]
fn missing_value(name: &str, index: usize) -> ! {
    panic!(
        "dense query: `{name}` has no value at index {index} although its presence bit is set \
         (invariant D1)"
    )
}

impl<T: 'static> seal::Param for &T {
    type Item<'w> = &'w T;
    type Fetch<'w> = Option<&'w DenseColumn<T>>;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>());
    }

    fn resolve<'w>(world: &'w World, required: &mut Vec<&'w HierarchicalBitSet>) -> bool {
        world.column::<T>().is_some_and(|c| {
            required.push(c.present());
            true
        })
    }

    fn bind(world: &World) -> Self::Fetch<'_> {
        world.column::<T>()
    }

    fn bind_mut<'w>(cx: &mut seal::MutCx<'w>) -> Self::Fetch<'w> {
        let column = cx.take_shared::<T>();
        match column {
            Some(c) => cx.required.push(c.present()),
            None => cx.empty = true,
        }
        column
    }

    #[inline]
    fn get<'a>(fetch: &'a mut Option<&DenseColumn<T>>, index: usize) -> &'a T {
        (*fetch)
            .and_then(|c| c.get(index))
            .unwrap_or_else(|| missing_value(type_name::<T>(), index))
    }
}

impl<T: 'static> QueryParam for &T {}

impl<T: 'static> seal::ReadOnly for &T {
    #[inline]
    fn get_shared<'w>(fetch: &Self::Fetch<'w>, index: usize) -> Self::Item<'w> {
        (*fetch)
            .and_then(|c| c.get(index))
            .unwrap_or_else(|| missing_value(type_name::<T>(), index))
    }
}

impl<T: 'static> ReadOnly for &T {}

impl<T: 'static> seal::Param for &mut T {
    type Item<'w> = &'w mut T;
    type Fetch<'w> = Option<ValuesMut<'w, T>>;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>());
    }

    fn resolve<'w>(world: &'w World, required: &mut Vec<&'w HierarchicalBitSet>) -> bool {
        world.column::<T>().is_some_and(|c| {
            required.push(c.present());
            true
        })
    }

    fn bind(_world: &World) -> Self::Fetch<'_> {
        unreachable!("a read-only query has no `&mut` parameter")
    }

    fn bind_mut<'w>(cx: &mut seal::MutCx<'w>) -> Self::Fetch<'w> {
        let split = cx.take_split::<T>();
        if let Some((set, values)) = split {
            cx.required.push(set);
            Some(values)
        } else {
            cx.empty = true;
            None
        }
    }

    #[inline]
    fn get<'a>(fetch: &'a mut Option<ValuesMut<'_, T>>, index: usize) -> &'a mut T {
        fetch
            .as_mut()
            .and_then(|v| v.get(index))
            .unwrap_or_else(|| missing_value(type_name::<T>(), index))
    }
}

impl<T: 'static> QueryParam for &mut T {}

impl<T: 'static> seal::Param for Option<&T> {
    type Item<'w> = Option<&'w T>;
    type Fetch<'w> = Option<&'w DenseColumn<T>>;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>());
    }

    fn resolve<'w>(_world: &'w World, _required: &mut Vec<&'w HierarchicalBitSet>) -> bool {
        true
    }

    fn bind(world: &World) -> Self::Fetch<'_> {
        world.column::<T>()
    }

    fn bind_mut<'w>(cx: &mut seal::MutCx<'w>) -> Self::Fetch<'w> {
        cx.take_shared::<T>()
    }

    #[inline]
    fn get<'a>(fetch: &'a mut Option<&DenseColumn<T>>, index: usize) -> Option<&'a T> {
        (*fetch).and_then(|c| c.get(index))
    }
}

impl<T: 'static> QueryParam for Option<&T> {}

impl<T: 'static> seal::ReadOnly for Option<&T> {
    #[inline]
    fn get_shared<'w>(fetch: &Self::Fetch<'w>, index: usize) -> Self::Item<'w> {
        (*fetch).and_then(|c| c.get(index))
    }
}

impl<T: 'static> ReadOnly for Option<&T> {}

impl<T: 'static> seal::Param for Option<&mut T> {
    type Item<'w> = Option<&'w mut T>;
    type Fetch<'w> = Option<ValuesMut<'w, T>>;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>());
    }

    fn resolve<'w>(_world: &'w World, _required: &mut Vec<&'w HierarchicalBitSet>) -> bool {
        true
    }

    fn bind(_world: &World) -> Self::Fetch<'_> {
        unreachable!("a read-only query has no `&mut` parameter")
    }

    fn bind_mut<'w>(cx: &mut seal::MutCx<'w>) -> Self::Fetch<'w> {
        cx.take_split::<T>().map(|(_, values)| values)
    }

    #[inline]
    fn get<'a>(fetch: &'a mut Option<ValuesMut<'_, T>>, index: usize) -> Option<&'a mut T> {
        fetch.as_mut().and_then(|v| v.get(index))
    }
}

impl<T: 'static> QueryParam for Option<&mut T> {}

impl seal::Param for Entity {
    type Item<'w> = Entity;
    type Fetch<'w> = &'w [Entity];

    fn types(_: &mut Vec<seal::TypeEntry>) {}

    fn resolve<'w>(_world: &'w World, _required: &mut Vec<&'w HierarchicalBitSet>) -> bool {
        true
    }

    fn bind(world: &World) -> Self::Fetch<'_> {
        world.entities()
    }

    fn bind_mut<'w>(cx: &mut seal::MutCx<'w>) -> Self::Fetch<'w> {
        cx.entities()
    }

    #[inline]
    fn get(fetch: &mut &[Entity], index: usize) -> Entity {
        fetch[index]
    }
}

impl QueryParam for Entity {}

impl seal::ReadOnly for Entity {
    #[inline]
    fn get_shared<'w>(fetch: &Self::Fetch<'w>, index: usize) -> Self::Item<'w> {
        fetch[index]
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

            fn resolve<'w>(
                world: &'w World,
                required: &mut Vec<&'w HierarchicalBitSet>,
            ) -> bool {
                let mut all = true;
                $(all &= $p::resolve(world, required);)+
                all
            }

            fn bind(world: &World) -> Self::Fetch<'_> {
                ($($p::bind(world),)+)
            }

            fn bind_mut<'w>(cx: &mut seal::MutCx<'w>) -> Self::Fetch<'w> {
                ($($p::bind_mut(cx),)+)
            }

            #[inline]
            fn get<'a>(fetch: &'a mut ($($p::Fetch<'_>,)+), index: usize) -> Self::Item<'a> {
                let ($($p,)+) = fetch;
                ($($p::get($p, index),)+)
            }
        }

        impl<$($p: QueryParam),+> QueryParam for ($($p,)+) {}

        #[allow(non_snake_case)]
        impl<$($p: ReadOnly),+> seal::ReadOnly for ($($p,)+) {
            #[inline]
            fn get_shared<'w>(fetch: &Self::Fetch<'w>, index: usize) -> Self::Item<'w> {
                let ($($p,)+) = fetch;
                ($($p::get_shared($p, index),)+)
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

    fn resolve_mut(_cx: &mut seal::MutCx<'_>) {}
}

impl Filter for () {}

impl<T: 'static> seal::Filter for With<T> {
    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>());
    }

    fn resolve_mut(cx: &mut seal::MutCx<'_>) {
        match cx.take_shared::<T>() {
            Some(c) => cx.required.push(c.present()),
            None => cx.empty = true,
        }
    }
}

impl<T: 'static> Filter for With<T> {}

impl<T: 'static> seal::Filter for Without<T> {
    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>());
    }

    fn resolve_mut(cx: &mut seal::MutCx<'_>) {
        if let Some(c) = cx.take_shared::<T>() {
            cx.excluded.push(c.present());
        }
    }
}

impl<T: 'static> Filter for Without<T> {}

impl<F: Filter, G: Filter> seal::Filter for (F, G) {
    fn types(out: &mut Vec<seal::TypeEntry>) {
        F::types(out);
        G::types(out);
    }

    fn resolve_mut(cx: &mut seal::MutCx<'_>) {
        F::resolve_mut(cx);
        G::resolve_mut(cx);
    }
}

impl<F: Filter, G: Filter> Filter for (F, G) {}

/// Panics if a type appears twice among the parameters and filters. `op` names the running
/// method in the message.
#[track_caller]
fn check_types<Q: QueryParam, F: Filter>(op: &str) {
    let mut entries = Vec::new();
    Q::types(&mut entries);
    F::types(&mut entries);
    for (i, e) in entries.iter().enumerate() {
        assert!(
            !entries[..i].iter().any(|other| other.id == e.id),
            "{op}: `{}` appears twice in the query; name each type once, as a parameter or a filter",
            e.name
        );
    }
}

/// The sets a run joins: the required ones, or the set of live indices if none is required.
/// `Query` and `QueryMut` both choose here, so they cannot disagree.
fn join_lists<'a>(
    required: &'a [&'a HierarchicalBitSet],
    alive: &'a [&'a HierarchicalBitSet; 1],
) -> &'a [&'a HierarchicalBitSet] {
    if required.is_empty() { alive } else { required }
}

/// A set a query joins, for tests: the live set, or one component's presence set.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Joined {
    /// The set of live indices.
    Alive,
    /// The presence set of a component.
    Component(ComponentId),
}

/// The required and excluded sets a query joins, for tests.
#[cfg(test)]
pub(crate) type JoinedSets = (Vec<Joined>, Vec<Joined>);

/// The world's own sets by address, to name the sets of a join.
#[cfg(test)]
fn label_table(world: &World) -> Vec<(*const HierarchicalBitSet, Joined)> {
    let mut table = vec![(core::ptr::from_ref(world.alive_set()), Joined::Alive)];
    table.extend(
        world
            .present_sets()
            .map(|(id, set)| (core::ptr::from_ref(set), Joined::Component(id))),
    );
    table
}

/// Names each of `sets` by finding it in `table`.
#[cfg(test)]
fn labelled(
    table: &[(*const HierarchicalBitSet, Joined)],
    sets: &[&HierarchicalBitSet],
) -> Vec<Joined> {
    sets.iter()
        .map(|set| {
            let ptr = core::ptr::from_ref(*set);
            table
                .iter()
                .find(|(p, _)| *p == ptr)
                .map(|(_, joined)| *joined)
                .expect("a joined set belongs to the world")
        })
        .collect()
}

/// A read-only query, borrowed from [`World::query`]. Add filters with [`with`](Self::with)
/// and [`without`](Self::without), then run it with [`for_each`](Self::for_each),
/// [`iter`](Self::iter) or a `for` loop over `&query`.
///
/// The query resolves the presence sets it joins when it is built and as each filter is added
/// (a required type with no column makes it match nothing), so [`iter`](Self::iter) can
/// borrow them for the iterator's `Join`.
///
/// # Examples
///
/// ```
/// use spike_ecs_dense::{Entity, World};
///
/// struct Position(f32);
/// struct Frozen;
///
/// let mut world = World::new();
/// let moving = world.spawn();
/// world.insert(moving, Position(1.0));
/// let frozen = world.spawn();
/// world.insert(frozen, Position(2.0));
/// world.insert(frozen, Frozen);
///
/// let ids: Vec<Entity> = world
///     .query::<(Entity, &Position)>()
///     .without::<Frozen>()
///     .iter()
///     .map(|(e, _)| e)
///     .collect();
/// assert_eq!(ids, [moving]);
/// ```
#[must_use = "a query does nothing until it is run"]
pub struct Query<'w, Q, F = ()> {
    world: &'w World,
    required: Vec<&'w HierarchicalBitSet>,
    excluded: Vec<&'w HierarchicalBitSet>,
    /// The fallback when nothing is required, so the fallback allocates nothing.
    alive: [&'w HierarchicalBitSet; 1],
    /// A required type has no column: the run yields nothing and builds no `Join`.
    empty: bool,
    marker: PhantomData<fn() -> (Q, F)>,
}

impl<'w, Q: ReadOnly> Query<'w, Q> {
    pub(crate) fn new(world: &'w World) -> Self {
        let mut required = Vec::new();
        let resolved = Q::resolve(world, &mut required);
        Self {
            world,
            required,
            excluded: Vec::new(),
            alive: [world.alive_set()],
            empty: !resolved,
            marker: PhantomData,
        }
    }
}

impl<'w, Q: ReadOnly, F: Filter> Query<'w, Q, F> {
    /// Keeps only entities that have a `T`. If no entity has ever had one, the query matches
    /// nothing.
    pub fn with<T: 'static>(mut self) -> Query<'w, Q, (F, With<T>)> {
        match self.world.column::<T>() {
            Some(column) => self.required.push(column.present()),
            None => self.empty = true,
        }
        self.retyped()
    }

    /// Keeps only entities that do not have a `T`. A type no entity has ever had is ignored.
    pub fn without<T: 'static>(mut self) -> Query<'w, Q, (F, Without<T>)> {
        if let Some(column) = self.world.column::<T>() {
            self.excluded.push(column.present());
        }
        self.retyped()
    }

    fn retyped<G>(self) -> Query<'w, Q, G> {
        Query {
            world: self.world,
            required: self.required,
            excluded: self.excluded,
            alive: self.alive,
            empty: self.empty,
            marker: PhantomData,
        }
    }

    /// Calls `f` for each matching entity, in ascending index order.
    ///
    /// # Panics
    ///
    /// Panics with a `Query::for_each:` message if a type appears twice among the parameters
    /// and filters.
    #[track_caller]
    pub fn for_each(self, mut f: impl FnMut(Q::Item<'w>)) {
        for item in self.start("Query::for_each") {
            f(item);
        }
    }

    /// An iterator over the matching entities, in ascending index order. It borrows the query.
    ///
    /// # Panics
    ///
    /// Panics with a `Query::iter:` message if a type appears twice among the parameters and
    /// filters.
    #[track_caller]
    pub fn iter(&self) -> QueryIter<'_, 'w, Q, F> {
        self.start("Query::iter")
    }

    #[track_caller]
    fn start(&self, op: &str) -> QueryIter<'_, 'w, Q, F> {
        check_types::<Q, F>(op);
        let (required, excluded) = self.join_lists();
        QueryIter {
            join: (!self.empty).then(|| Join::new(required, excluded)),
            sets: if self.empty { &[] } else { required },
            fetch: Q::bind(self.world),
            marker: PhantomData,
        }
    }

    /// The sets a run joins.
    fn join_lists(&self) -> (&[&HierarchicalBitSet], &[&HierarchicalBitSet]) {
        (join_lists(&self.required, &self.alive), &self.excluded)
    }

    /// The sets a run would join, for tests: the required sets (the live set if none is
    /// required) and the excluded ones. A query that matches nothing reports the sets
    /// resolved so far.
    #[cfg(test)]
    pub(crate) fn joined(&self) -> JoinedSets {
        let table = label_table(self.world);
        let (required, excluded) = self.join_lists();
        let required = if self.empty {
            &self.required[..]
        } else {
            required
        };
        (labelled(&table, required), labelled(&table, excluded))
    }
}

impl<'q, 'w, Q: ReadOnly, F: Filter> IntoIterator for &'q Query<'w, Q, F> {
    type Item = Q::Item<'w>;
    type IntoIter = QueryIter<'q, 'w, Q, F>;

    /// # Panics
    ///
    /// Panics with a `Query::into_iter:` message if a type appears twice among the parameters
    /// and filters.
    #[track_caller]
    fn into_iter(self) -> QueryIter<'q, 'w, Q, F> {
        self.start("Query::into_iter")
    }
}

impl<Q, F> fmt::Debug for Query<'_, Q, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Query")
            .field("params", &type_name::<Q>())
            .field("filters", &type_name::<F>())
            .field("required", &self.required.len())
            .field("excluded", &self.excluded.len())
            .field("empty", &self.empty)
            .finish_non_exhaustive()
    }
}

/// The iterator of a [`Query`]: yields one item per matching entity, in ascending index
/// order. It borrows the query for `'q`; its items borrow the world for `'w`.
#[must_use = "iterators do nothing unless consumed"]
pub struct QueryIter<'q, 'w, Q: ReadOnly, F> {
    join: Option<Join<'q>>,
    /// The sets of `join`, for `size_hint`.
    sets: &'q [&'q HierarchicalBitSet],
    fetch: Q::Fetch<'w>,
    marker: PhantomData<fn() -> F>,
}

impl<'w, Q: ReadOnly, F> Iterator for QueryIter<'_, 'w, Q, F> {
    type Item = Q::Item<'w>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.join.as_mut()?.next()?;
        Some(Q::get_shared(&self.fetch, index))
    }

    /// `(0, Some(n))`, `n` the length of the smallest required set. The length is counted
    /// here, so a run that never asks pays nothing.
    fn size_hint(&self) -> (usize, Option<usize>) {
        let bound = self.sets.iter().map(|set| set.len()).min().unwrap_or(0);
        (0, Some(bound))
    }
}

impl<Q: ReadOnly, F> FusedIterator for QueryIter<'_, '_, Q, F> {}

impl<Q: ReadOnly, F> fmt::Debug for QueryIter<'_, '_, Q, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueryIter")
            .field("params", &type_name::<Q>())
            .field("running", &self.join.is_some())
            .finish_non_exhaustive()
    }
}

/// A query that may write, borrowed from [`World::query_mut`]. It is run with
/// [`for_each`](Self::for_each); a safe iterator cannot prove the `&mut` items it yields
/// disjoint, so there is no `iter`.
///
/// The query resolves its columns when it runs.
///
/// # Examples
///
/// ```
/// use spike_ecs_dense::World;
///
/// struct Position(f32);
/// struct Velocity(f32);
///
/// let mut world = World::new();
/// let e = world.spawn();
/// world.insert(e, Position(0.0));
/// world.insert(e, Velocity(2.0));
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

/// Takes the columns of `Q` and `F` from `world` in one pass and returns `Q`'s fetch and what
/// the run collected. `QueryMut::for_each` and `joined` both call it.
fn resolve_mut<Q: QueryParam, F: Filter>(world: &mut World) -> (Q::Fetch<'_>, seal::MutCx<'_>) {
    let mut cx = seal::MutCx::new(world);
    let fetch = Q::bind_mut(&mut cx);
    F::resolve_mut(&mut cx);
    (fetch, cx)
}

impl<'w, Q: QueryParam, F: Filter> QueryMut<'w, Q, F> {
    /// Keeps only entities that have a `T`. If no entity has ever had one, the query matches
    /// nothing.
    pub fn with<T: 'static>(self) -> QueryMut<'w, Q, (F, With<T>)> {
        QueryMut {
            world: self.world,
            marker: PhantomData,
        }
    }

    /// Keeps only entities that do not have a `T`. A type no entity has ever had is ignored.
    pub fn without<T: 'static>(self) -> QueryMut<'w, Q, (F, Without<T>)> {
        QueryMut {
            world: self.world,
            marker: PhantomData,
        }
    }

    /// Calls `f` for each matching entity, in ascending index order.
    ///
    /// # Panics
    ///
    /// Panics with a `QueryMut::for_each:` message if a type appears twice among the
    /// parameters and filters.
    #[track_caller]
    pub fn for_each(self, mut f: impl FnMut(Q::Item<'_>)) {
        check_types::<Q, F>("QueryMut::for_each");
        let (mut fetch, cx) = resolve_mut::<Q, F>(self.world);
        if cx.empty {
            return;
        }
        let alive = [cx.alive()];
        let required = join_lists(&cx.required, &alive);
        for index in Join::new(required, &cx.excluded) {
            f(Q::get(&mut fetch, index));
        }
    }

    /// The sets a run would join, for tests (as [`Query::joined`]).
    #[cfg(test)]
    pub(crate) fn joined(&mut self) -> JoinedSets {
        // The table is taken before the run borrows the columns mutably.
        let table = label_table(self.world);
        let (_fetch, cx) = resolve_mut::<Q, F>(&mut *self.world);
        let alive = [cx.alive()];
        let required = if cx.empty {
            &cx.required[..]
        } else {
            join_lists(&cx.required, &alive)
        };
        (labelled(&table, required), labelled(&table, &cx.excluded))
    }
}

impl<Q, F> fmt::Debug for QueryMut<'_, Q, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueryMut")
            .field("params", &type_name::<Q>())
            .field("filters", &type_name::<F>())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use core::any::TypeId;

    use super::Joined;
    use crate::{Entity, World};

    #[derive(Debug, PartialEq)]
    struct A(u32);
    #[derive(Debug, PartialEq)]
    struct B(u32);
    #[derive(Debug, PartialEq)]
    struct C(u32);
    #[derive(Debug, PartialEq)]
    struct D(u32);
    struct Hidden;

    fn spawn_with(w: &mut World, a: Option<u32>, b: Option<u32>) -> Entity {
        let e = w.spawn();
        if let Some(a) = a {
            w.insert(e, A(a));
        }
        if let Some(b) = b {
            w.insert(e, B(b));
        }
        e
    }

    #[test]
    fn required_types_select_entities() {
        let mut w = World::new();
        let e1 = spawn_with(&mut w, Some(1), Some(1));
        let _e2 = spawn_with(&mut w, Some(2), None);
        let _e3 = spawn_with(&mut w, None, Some(3));
        let e4 = spawn_with(&mut w, Some(4), Some(4));
        let got: Vec<Entity> = w
            .query::<(Entity, &A, &B)>()
            .iter()
            .map(|(e, _, _)| e)
            .collect();
        assert_eq!(got, [e1, e4]);
    }

    #[test]
    fn option_yields_some_exactly_when_present() {
        let mut w = World::new();
        spawn_with(&mut w, Some(1), Some(10));
        spawn_with(&mut w, Some(2), None);
        let got: Vec<(u32, Option<u32>)> = w
            .query::<(&A, Option<&B>)>()
            .iter()
            .map(|(a, b)| (a.0, b.map(|b| b.0)))
            .collect();
        assert_eq!(got, [(1, Some(10)), (2, None)]);
    }

    #[test]
    fn option_past_a_short_column_is_none() {
        let mut w = World::new();
        let first = spawn_with(&mut w, None, Some(7));
        for i in 0..5 {
            spawn_with(&mut w, Some(i), None);
        }
        // B's column has one slot; the entities with an A are past its end.
        let got: Vec<(Entity, Option<u32>)> = w
            .query::<(Entity, &A, Option<&B>)>()
            .iter()
            .map(|(e, _, b)| (e, b.map(|b| b.0)))
            .collect();
        assert_eq!(got.len(), 5);
        assert!(got.iter().all(|(_, b)| b.is_none()));
        assert!(got.iter().all(|(e, _)| *e != first));
        let mut seen = Vec::new();
        w.query_mut::<(&A, Option<&mut B>)>()
            .for_each(|(_, b)| seen.push(b.is_some()));
        assert_eq!(seen, [false; 5]);
        assert_eq!(w.check_invariants(), Ok(()));
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
        let e1 = spawn_with(&mut w, None, Some(1));
        spawn_with(&mut w, Some(2), None);
        let got: Vec<Entity> = w.query::<Entity>().with::<B>().iter().collect();
        assert_eq!(got, [e1]);
    }

    #[test]
    fn with_replaces_the_alive_fallback() {
        let mut w = World::new();
        spawn_with(&mut w, Some(1), Some(1));
        let id_a = w.component_id(TypeId::of::<A>()).unwrap();
        let id_b = w.component_id(TypeId::of::<B>()).unwrap();
        assert_eq!(w.query::<Entity>().joined(), (vec![Joined::Alive], vec![]));
        assert_eq!(
            w.query::<Entity>().with::<B>().joined(),
            (vec![Joined::Component(id_b)], vec![])
        );
        assert_eq!(
            w.query::<Entity>().without::<A>().joined(),
            (vec![Joined::Alive], vec![Joined::Component(id_a)])
        );
        assert_eq!(
            w.query::<(&A, &B)>().without::<C>().joined(),
            (
                vec![Joined::Component(id_a), Joined::Component(id_b)],
                vec![]
            )
        );
        assert_eq!(
            w.query_mut::<(&mut A, Option<&B>)>().joined(),
            (vec![Joined::Component(id_a)], vec![])
        );
        assert_eq!(
            w.query_mut::<Entity>().with::<B>().without::<A>().joined(),
            (vec![Joined::Component(id_b)], vec![Joined::Component(id_a)])
        );
        assert_eq!(
            w.query_mut::<Option<&A>>().joined(),
            (vec![Joined::Alive], vec![])
        );
    }

    #[test]
    fn a_query_that_matches_nothing_reports_the_sets_resolved_so_far() {
        let mut w = World::new();
        spawn_with(&mut w, Some(1), None);
        let id_a = w.component_id(TypeId::of::<A>()).unwrap();
        let only_a = (vec![Joined::Component(id_a)], vec![]);
        assert_eq!(w.query::<(&A, &D)>().joined(), only_a);
        assert_eq!(w.query_mut::<(&mut A, &mut D)>().joined(), only_a);
        assert_eq!(w.query::<&A>().with::<D>().joined(), only_a);
        assert_eq!(w.query::<&D>().joined(), (vec![], vec![]));
        assert_eq!(w.query_mut::<&mut D>().joined(), (vec![], vec![]));
    }

    #[test]
    fn filters_on_unregistered_types() {
        let mut w = World::new();
        let e = spawn_with(&mut w, Some(1), None);
        // `with` of a type with no column matches nothing.
        assert_eq!(w.query::<&A>().with::<D>().iter().count(), 0);
        assert_eq!(w.query::<Entity>().with::<D>().iter().count(), 0);
        let mut n = 0;
        w.query_mut::<&mut A>().with::<D>().for_each(|_| n += 1);
        assert_eq!(n, 0);
        // `without` of one is ignored and is not in the joined list.
        let got: Vec<Entity> = w.query::<Entity>().without::<D>().iter().collect();
        assert_eq!(got, [e]);
        assert_eq!(
            w.query::<&A>().without::<D>().joined().1,
            Vec::<Joined>::new()
        );
        let mut n = 0;
        w.query_mut::<&mut A>().without::<D>().for_each(|_| n += 1);
        assert_eq!(n, 1);
    }

    #[test]
    fn entity_parameter_yields_the_entity() {
        let mut w = World::new();
        let e = spawn_with(&mut w, Some(5), None);
        let mut seen = Vec::new();
        w.query::<(Entity, &A)>()
            .for_each(|(id, a)| seen.push((id, a.0)));
        assert_eq!(seen, [(e, 5)]);
        let mut seen = Vec::new();
        w.query_mut::<(Entity, &mut A)>()
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
        let mut ids = Vec::new();
        w.query_mut::<(Entity, (&mut A, &B, &C, Option<&D>))>()
            .for_each(|(id, (a, b, c, d))| {
                a.0 += b.0 + c.0 + d.map_or(0, |d| d.0);
                ids.push(id);
            });
        assert_eq!(ids, [e, f]);
        assert_eq!(w.get::<A>(e), Some(&A(10)));
        assert_eq!(w.get::<A>(f), Some(&A(60)));
    }

    #[test]
    fn never_inserted_required_type_matches_nothing() {
        let mut w = World::new();
        spawn_with(&mut w, Some(1), None);
        assert_eq!(w.query::<(&A, &B)>().iter().count(), 0);
        assert_eq!(w.query::<&A>().with::<B>().iter().count(), 0);
        let mut n = 0;
        w.query::<(&A, &D)>().for_each(|_| n += 1);
        w.query_mut::<(&mut A, &D)>().for_each(|_| n += 1);
        assert_eq!(n, 0);
        // An optional or excluded type with no column does not block the query.
        assert_eq!(
            w.query::<(&A, Option<&D>)>().without::<C>().iter().count(),
            1
        );
    }

    #[test]
    fn mutable_query_writes_are_visible() {
        let mut w = World::new();
        let es: Vec<Entity> = (0..4)
            .map(|i| spawn_with(&mut w, Some(i), (i % 2 == 0).then_some(100)))
            .collect();
        w.query_mut::<(&mut A, &B)>().for_each(|(a, b)| a.0 += b.0);
        let got: Vec<u32> = es.iter().map(|e| w.get::<A>(*e).unwrap().0).collect();
        assert_eq!(got, [100, 1, 102, 3]);
        // Filters on a mutable query.
        w.query_mut::<&mut A>().without::<B>().for_each(|a| a.0 = 0);
        w.query_mut::<&mut A>().with::<B>().for_each(|a| a.0 += 1);
        let got: Vec<u32> = es.iter().map(|e| w.get::<A>(*e).unwrap().0).collect();
        assert_eq!(got, [101, 0, 103, 0]);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn iter_and_for_each_agree() {
        let mut w = World::new();
        for i in 0..10u32 {
            spawn_with(&mut w, Some(i), (i % 3 == 0).then_some(i));
        }
        let q = w.query::<(&A, &B)>();
        let by_iter: Vec<u32> = q.iter().map(|(a, _)| a.0).collect();
        let mut by_ref = Vec::new();
        for (a, _) in &q {
            by_ref.push(a.0);
        }
        let mut by_for_each = Vec::new();
        w.query::<(&A, &B)>()
            .for_each(|(a, _)| by_for_each.push(a.0));
        assert_eq!(by_iter, [0, 3, 6, 9]);
        assert_eq!(by_iter, by_for_each);
        assert_eq!(by_iter, by_ref);
        // The query is still usable after a run.
        assert_eq!(q.iter().count(), 4);
    }

    #[test]
    fn size_hint_bounds_the_count() {
        let mut w = World::new();
        for i in 0..4 {
            spawn_with(&mut w, Some(i), (i % 2 == 0).then_some(i));
        }
        let q = w.query::<&A>().with::<B>();
        let mut it = q.iter();
        // The smallest required set (B) has two members.
        assert_eq!(it.size_hint(), (0, Some(2)));
        assert!(it.next().is_some());
        assert!(it.next().is_some());
        assert!(it.next().is_none());
        assert!(it.next().is_none(), "the iterator is fused");
        // A query that cannot match is empty and fused too.
        let empty = World::new();
        let q = empty.query::<&A>();
        let mut it = q.iter();
        assert_eq!(it.size_hint(), (0, Some(0)));
        assert!(it.next().is_none());
        assert!(it.next().is_none());
        // The fallback bound is the live count.
        let q = w.query::<Entity>();
        assert_eq!(q.iter().size_hint(), (0, Some(4)));
    }

    #[test]
    #[should_panic(
        expected = "Query::for_each: `spike_ecs_dense::query::tests::A` appears twice in the query"
    )]
    fn repeated_type_panics_in_query_for_each() {
        let w = World::new();
        w.query::<(&A, &A)>().for_each(|_| {});
    }

    #[test]
    #[should_panic(
        expected = "Query::iter: `spike_ecs_dense::query::tests::A` appears twice in the query"
    )]
    fn repeated_type_panics_in_query_iter() {
        let w = World::new();
        let _ = w.query::<(&A, Option<&A>)>().iter();
    }

    #[test]
    #[should_panic(
        expected = "Query::into_iter: `spike_ecs_dense::query::tests::A` appears twice in the query"
    )]
    fn repeated_type_panics_in_a_for_loop() {
        let w = World::new();
        let q = w.query::<(&A, &A)>();
        for _ in &q {}
    }

    #[test]
    #[should_panic(
        expected = "QueryMut::for_each: `spike_ecs_dense::query::tests::A` appears twice in the query"
    )]
    fn repeated_type_panics_in_query_mut_for_each() {
        let mut w = World::new();
        w.query_mut::<(&mut A, &A)>().for_each(|_| {});
    }

    #[test]
    #[should_panic(
        expected = "Query::iter: `spike_ecs_dense::query::tests::A` appears twice in the query"
    )]
    fn a_type_in_a_parameter_and_a_filter_panics() {
        let w = World::new();
        let _ = w.query::<&A>().without::<A>().iter();
    }

    #[test]
    fn query_without_a_required_type_visits_every_live_entity() {
        let mut w = World::new();
        let bare = w.spawn();
        let with_a = spawn_with(&mut w, Some(1), None);
        let dead = w.spawn();
        w.despawn(dead);
        let also_bare = w.spawn();
        let all: Vec<Entity> = w.query::<Entity>().iter().collect();
        assert_eq!(all.len(), 3);
        assert!(all.contains(&bare) && all.contains(&with_a) && all.contains(&also_bare));
        let mut n = 0;
        w.query::<Entity>().for_each(|_| n += 1);
        assert_eq!(n, 3);

        let opt: Vec<Option<u32>> = w
            .query::<Option<&A>>()
            .iter()
            .map(|a| a.map(|a| a.0))
            .collect();
        assert_eq!(opt.len(), 3);
        assert_eq!(opt.iter().filter(|a| **a == Some(1)).count(), 1);

        let without: Vec<Entity> = w.query::<Entity>().without::<A>().iter().collect();
        assert_eq!(without.len(), 2);
        assert!(!without.contains(&with_a));
        let mut n = 0;
        w.query_mut::<Entity>().without::<A>().for_each(|_| n += 1);
        assert_eq!(n, 2);
    }

    #[test]
    fn zero_sized_component_filters() {
        let mut w = World::new();
        let shown = spawn_with(&mut w, Some(1), None);
        let hidden = spawn_with(&mut w, Some(2), None);
        w.insert(hidden, Hidden);
        let with: Vec<Entity> = w
            .query::<(Entity, &A)>()
            .with::<Hidden>()
            .iter()
            .map(|(e, _)| e)
            .collect();
        assert_eq!(with, [hidden]);
        let without: Vec<Entity> = w
            .query::<(Entity, &A)>()
            .without::<Hidden>()
            .iter()
            .map(|(e, _)| e)
            .collect();
        assert_eq!(without, [shown]);
        assert_eq!(w.query::<(Entity, &Hidden)>().iter().count(), 1);
    }

    #[test]
    fn queries_yield_ascending_indices() {
        let mut w = World::new();
        let es: Vec<Entity> = (0..8).map(|i| spawn_with(&mut w, Some(i), None)).collect();
        for e in [es[1], es[4], es[6]] {
            w.despawn(e);
        }
        for i in 0..3 {
            spawn_with(&mut w, Some(100 + i), Some(i));
        }
        let by_iter: Vec<u32> = w
            .query::<(Entity, &A)>()
            .iter()
            .map(|(e, _)| e.index())
            .collect();
        assert_eq!(by_iter.len(), 8);
        assert!(by_iter.windows(2).all(|p| p[0] < p[1]), "{by_iter:?}");
        let mut by_for_each = Vec::new();
        w.query::<Entity>()
            .for_each(|e| by_for_each.push(e.index()));
        assert!(
            by_for_each.windows(2).all(|p| p[0] < p[1]),
            "{by_for_each:?}"
        );
        let mut by_mut = Vec::new();
        w.query_mut::<(Entity, &mut A)>()
            .for_each(|(e, _)| by_mut.push(e.index()));
        assert_eq!(by_mut, by_iter);
    }
}
