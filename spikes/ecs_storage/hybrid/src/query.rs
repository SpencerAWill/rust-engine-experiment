//! Typed tuple queries over both storage kinds.
//!
//! A query names its parameters (`&T`, `&mut T`, `Option<&T>`, `Option<&mut T>`, `Entity`,
//! tuples of those) and its filters (`.with::<T>()`, `.without::<T>()`). Running it visits the
//! archetypes in id order and keeps those that have every required table type and none of the
//! excluded ones. Each parameter is built once per run (`init`: a sparse set is bound, a table
//! parameter gets an empty slice) and rebound to each matched archetype's columns (`set_table`),
//! so a table row costs an index into slices. A parameter or filter on a sparse type probes its
//! set per row; a query that names none has no probe in its row loop and reads no entity it
//! does not yield (spec section 4.7).
//!
//! The tuple-query API follows the design of `bevy_ecs` and `hecs` (typed tuples of `&T`,
//! `&mut T` and `Option<&T>`, with `with` and `without` filters, matched against archetypes).
//! No code was taken from either; the implementation is original and adapted from this
//! repository's own archetype and sparse-set backends (M3.4 and M3.3, spec section 11).

use core::fmt;
use core::iter::FusedIterator;
use core::marker::PhantomData;

use crate::Entity;
use crate::archetype::Archetype;
use crate::component::Component;
use crate::world::World;

/// Implementation details of the sealed traits; unnameable outside the crate.
pub(crate) mod seal {
    use core::any::TypeId;

    use crate::Entity;
    use crate::archetype::{Archetype, ComponentId};
    use crate::component::{Component, StorageKind, seal::Storage};
    use crate::sparse::{ErasedSparseSet, SparseMut, SparseSet, SparseSlotMut};
    use crate::table::{ColumnMut, ErasedColumn, column_slice};

    /// How a query uses a component type.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Role {
        /// `&T`, `&mut T` or `With<T>`: the entity must have it.
        Required,
        /// `Option<&T>` or `Option<&mut T>`.
        Optional,
        /// `Without<T>`: the entity must not have it.
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
        /// Where its values live.
        pub kind: StorageKind,
        /// The world's id for the type, if it was ever registered (set when a run starts).
        pub component: Option<ComponentId>,
        /// A sparse type's set, if registered (set when a run starts).
        pub set: Option<usize>,
    }

    impl TypeEntry {
        /// An entry for `T`.
        pub fn of<T: Component>(role: Role) -> Self {
            Self {
                id: TypeId::of::<T>(),
                name: core::any::type_name::<T>(),
                role,
                kind: <T::Storage as Storage>::KIND,
                component: None,
                set: None,
            }
        }

        /// Whether `archetype` passes this entry: it has a required table type and lacks an
        /// excluded one. Sparse types play no part in matching; their rows are probed.
        pub fn passes(&self, archetype: &Archetype) -> bool {
            if self.kind == StorageKind::Sparse {
                return true;
            }
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
    pub enum TableSlot<'w> {
        /// From a read-only query.
        Shared(&'w dyn ErasedColumn),
        /// From a mutable query.
        Mut(ColumnMut<'w>),
    }

    impl core::fmt::Debug for TableSlot<'_> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                TableSlot::Shared(c) => write!(f, "Shared({})", c.value_type_name()),
                TableSlot::Mut(_) => f.write_str("Mut(..)"),
            }
        }
    }

    impl<'w> TableSlot<'w> {
        /// The column's values, shared.
        pub fn into_ref<T: 'static>(self) -> &'w [T] {
            match self {
                TableSlot::Shared(c) => column_slice(c),
                TableSlot::Mut(c) => c.slice(),
            }
        }

        /// The column's values, exclusive.
        pub fn into_mut<T: 'static>(self) -> &'w mut [T] {
            match self {
                TableSlot::Mut(c) => c.slice_mut(),
                TableSlot::Shared(_) => unreachable!("a read-only query has no `&mut` parameter"),
            }
        }
    }

    /// A sparse set handed to one parameter or filter of a running query.
    #[derive(Debug)]
    pub enum SparseSlot<'w> {
        /// From a read-only query.
        Shared(&'w dyn ErasedSparseSet),
        /// From a mutable query: only ever the typed [`SparseMut`] or a shared reference comes
        /// out of it (lesson ecs-5).
        Mut(SparseSlotMut<'w>),
    }

    impl<'w> SparseSlot<'w> {
        /// The set, shared.
        pub fn into_ref<T: 'static>(self) -> &'w SparseSet<T> {
            match self {
                SparseSlot::Shared(s) => s
                    .as_any()
                    .downcast_ref::<SparseSet<T>>()
                    .expect("S4: a set holds the type its component id names"),
                SparseSlot::Mut(s) => s.into_shared(),
            }
        }

        /// The set as a [`SparseMut`].
        pub fn into_mut<T: 'static>(self) -> SparseMut<'w, T> {
            match self {
                SparseSlot::Mut(s) => s.into_typed(),
                SparseSlot::Shared(_) => unreachable!("a read-only query has no `&mut` parameter"),
            }
        }
    }

    /// What one run hands its parameters. The sparse slots are filled once per run, one slot
    /// per mentioned type in declaration order, and the table slots are refilled for each
    /// matched archetype; `take_*` hands them out in the same order, one per mentioned type.
    /// Both vectors are allocated once per run, and the sparse one only if the query names a
    /// registered sparse type.
    #[derive(Debug)]
    pub struct Binder<'w> {
        sparse: Vec<Option<SparseSlot<'w>>>,
        tables: Vec<Option<TableSlot<'w>>>,
        cursor: usize,
        entities: &'w [Entity],
        len: usize,
    }

    impl<'w> Binder<'w> {
        /// An empty binder: every `take_*` returns `None`.
        pub fn new() -> Self {
            Self {
                sparse: Vec::new(),
                tables: Vec::new(),
                cursor: 0,
                entities: &[],
                len: 0,
            }
        }

        fn any_sparse(entries: &[TypeEntry]) -> bool {
            entries.iter().any(|e| e.set.is_some())
        }

        /// Points the sparse slots at the world's sets, shared.
        pub fn fill_sparse_shared(
            &mut self,
            sets: &'w [Box<dyn ErasedSparseSet>],
            entries: &[TypeEntry],
        ) {
            if !Self::any_sparse(entries) {
                return;
            }
            self.sparse.extend(
                entries
                    .iter()
                    .map(|e| e.set.map(|k| SparseSlot::Shared(&*sets[k]))),
            );
            self.cursor = 0;
        }

        /// Points the sparse slots at the world's sets, exclusive: one pass over the set list
        /// hands each `&mut` to its slot, so no two slots alias and no `unsafe` is needed.
        pub fn fill_sparse_exclusive(
            &mut self,
            sets: &'w mut [Box<dyn ErasedSparseSet>],
            entries: &[TypeEntry],
        ) {
            if !Self::any_sparse(entries) {
                return;
            }
            self.sparse.resize_with(entries.len(), || None);
            for (k, set) in sets.iter_mut().enumerate() {
                if let Some(j) = entries.iter().position(|e| e.set == Some(k)) {
                    self.sparse[j] = Some(SparseSlot::Mut(SparseSlotMut::new(&mut **set)));
                }
            }
            self.cursor = 0;
        }

        /// Points the table slots at `archetype`'s columns, shared. `entries[..params]` are the
        /// parameter types.
        pub fn fill_tables_shared(
            &mut self,
            archetype: &'w Archetype,
            entries: &[TypeEntry],
            params: usize,
        ) {
            self.tables.clear();
            let table = &archetype.table;
            self.tables.extend(entries[..params].iter().map(|e| {
                if e.kind == StorageKind::Table {
                    e.component
                        .and_then(|c| archetype.column_of(c))
                        .map(|k| TableSlot::Shared(table.column(k)))
                } else {
                    None
                }
            }));
            self.entities = table.entities();
            self.len = table.len();
            self.cursor = 0;
        }

        /// Points the table slots at `archetype`'s columns, exclusive: one pass over the column
        /// list hands each `&mut` to its slot.
        pub fn fill_tables_exclusive(
            &mut self,
            archetype: &'w mut Archetype,
            entries: &[TypeEntry],
            params: usize,
        ) {
            let Archetype {
                components, table, ..
            } = archetype;
            self.len = table.len();
            let (entities, columns) = table.columns_mut();
            self.entities = entities;
            self.tables.clear();
            self.tables.resize_with(params, || None);
            for (k, column) in columns.enumerate() {
                let id = Some(components[k]);
                if let Some(j) = entries[..params]
                    .iter()
                    .position(|e| e.kind == StorageKind::Table && e.component == id)
                {
                    self.tables[j] = Some(TableSlot::Mut(column));
                }
            }
            self.cursor = 0;
        }

        /// Starts over at the first slot.
        pub fn rewind(&mut self) {
            self.cursor = 0;
        }

        /// The next sparse slot; `None` if the type is a table type or unregistered.
        pub fn take_sparse(&mut self) -> Option<SparseSlot<'w>> {
            let n = self.cursor;
            self.cursor += 1;
            self.sparse.get_mut(n).and_then(Option::take)
        }

        /// The next table slot; `None` if the type is a sparse type, or the archetype lacks it.
        pub fn take_table(&mut self) -> Option<TableSlot<'w>> {
            let n = self.cursor;
            self.cursor += 1;
            self.tables.get_mut(n).and_then(Option::take)
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
        /// Per-run state: sparse parts bound once, table parts rebound per archetype.
        type Fetch<'w>;
        /// Whether a row of this parameter is probed before the fetches (a required sparse
        /// type). A constant, so a table-only query has no probe in its loop.
        const PROBES: bool;
        /// Appends the component types this parameter mentions, in declaration order.
        fn types(out: &mut Vec<TypeEntry>);
        /// Builds the fetch from the run's sparse slots; table parts start empty.
        fn init<'w>(binder: &mut Binder<'w>) -> Self::Fetch<'w>;
        /// Rebinds the table parts to the archetype the binder is on.
        fn set_table<'w>(fetch: &mut Self::Fetch<'w>, binder: &mut Binder<'w>);
        /// Whether row `row` passes this parameter's probe. Only called if `PROBES`.
        fn passes(fetch: &mut Self::Fetch<'_>, row: usize, entities: &[Entity]) -> bool;
        /// The item for row `row`, which is less than the archetype's row count.
        fn get<'a, 'w: 'a>(
            fetch: &'a mut Self::Fetch<'w>,
            row: usize,
            entities: &'a [Entity],
        ) -> Self::Item<'a>;
    }

    /// `get` for read-only parameters, which can hand out items that outlive the fetch borrow.
    pub trait ReadOnly: Param {
        /// As `Param::get`, with the lifetime of the world borrow.
        fn get_shared<'w>(
            fetch: &Self::Fetch<'w>,
            row: usize,
            entities: &'w [Entity],
        ) -> Self::Item<'w>;
    }

    /// The type list and row probes of a query filter.
    pub trait Filter {
        /// Per-run state: the sparse sets a probe reads.
        type Fetch<'w>;
        /// Whether a row is probed: a sparse type is required or excluded.
        const PROBES: bool;
        /// Appends the component types this filter mentions.
        fn types(out: &mut Vec<TypeEntry>);
        /// Builds the fetch from the run's sparse slots.
        fn init<'w>(binder: &mut Binder<'w>) -> Self::Fetch<'w>;
        /// Whether row `row` passes. Only called if `PROBES`.
        fn passes(fetch: &mut Self::Fetch<'_>, row: usize, entities: &[Entity]) -> bool;
    }
}

use crate::component::seal::Storage as StorageImpl;
use seal::Binder;
use seal::Role;

/// One parameter of a query: `&T`, `&mut T`, `Option<&T>`, `Option<&mut T>` (`T: Component`),
/// [`Entity`], or a tuple of 1 to 4 parameters (tuples nest). Sealed.
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

impl<T: Component> seal::Param for &T {
    type Item<'w> = &'w T;
    type Fetch<'w> = <T::Storage as StorageImpl>::Ref<'w, T>;
    const PROBES: bool = <T::Storage as StorageImpl>::PROBES;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>(Role::Required));
    }

    fn init<'w>(binder: &mut Binder<'w>) -> Self::Fetch<'w> {
        <T::Storage as StorageImpl>::init_ref::<T>(binder.take_sparse())
    }

    #[inline]
    fn set_table<'w>(fetch: &mut Self::Fetch<'w>, binder: &mut Binder<'w>) {
        let (col, len) = (binder.take_table(), binder.len());
        <T::Storage as StorageImpl>::set_ref::<T>(fetch, col, len);
    }

    #[inline]
    fn passes(fetch: &mut Self::Fetch<'_>, row: usize, entities: &[Entity]) -> bool {
        <T::Storage as StorageImpl>::ref_has::<T>(fetch, row, entities)
    }

    #[inline]
    fn get<'a, 'w: 'a>(
        fetch: &'a mut Self::Fetch<'w>,
        row: usize,
        entities: &'a [Entity],
    ) -> Self::Item<'a> {
        <T::Storage as StorageImpl>::ref_row::<T>(fetch, row, entities)
    }
}

impl<T: Component> QueryParam for &T {}

impl<T: Component> seal::ReadOnly for &T {
    #[inline]
    fn get_shared<'w>(
        fetch: &Self::Fetch<'w>,
        row: usize,
        entities: &'w [Entity],
    ) -> Self::Item<'w> {
        <T::Storage as StorageImpl>::ref_row::<T>(fetch, row, entities)
    }
}

impl<T: Component> ReadOnly for &T {}

impl<T: Component> seal::Param for &mut T {
    type Item<'w> = &'w mut T;
    type Fetch<'w> = <T::Storage as StorageImpl>::Mut<'w, T>;
    const PROBES: bool = <T::Storage as StorageImpl>::PROBES;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>(Role::Required));
    }

    fn init<'w>(binder: &mut Binder<'w>) -> Self::Fetch<'w> {
        <T::Storage as StorageImpl>::init_mut::<T>(binder.take_sparse())
    }

    #[inline]
    fn set_table<'w>(fetch: &mut Self::Fetch<'w>, binder: &mut Binder<'w>) {
        let (col, len) = (binder.take_table(), binder.len());
        <T::Storage as StorageImpl>::set_mut::<T>(fetch, col, len);
    }

    #[inline]
    fn passes(fetch: &mut Self::Fetch<'_>, row: usize, entities: &[Entity]) -> bool {
        <T::Storage as StorageImpl>::mut_has::<T>(fetch, row, entities)
    }

    #[inline]
    fn get<'a, 'w: 'a>(
        fetch: &'a mut Self::Fetch<'w>,
        row: usize,
        entities: &'a [Entity],
    ) -> Self::Item<'a> {
        <T::Storage as StorageImpl>::mut_row::<T>(fetch, row, entities)
    }
}

impl<T: Component> QueryParam for &mut T {}

impl<T: Component> seal::Param for Option<&T> {
    type Item<'w> = Option<&'w T>;
    type Fetch<'w> = <T::Storage as StorageImpl>::Ref<'w, T>;
    const PROBES: bool = false;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>(Role::Optional));
    }

    fn init<'w>(binder: &mut Binder<'w>) -> Self::Fetch<'w> {
        <T::Storage as StorageImpl>::init_ref::<T>(binder.take_sparse())
    }

    #[inline]
    fn set_table<'w>(fetch: &mut Self::Fetch<'w>, binder: &mut Binder<'w>) {
        let (col, len) = (binder.take_table(), binder.len());
        <T::Storage as StorageImpl>::set_ref::<T>(fetch, col, len);
    }

    #[inline]
    fn passes(_: &mut Self::Fetch<'_>, _: usize, _: &[Entity]) -> bool {
        true
    }

    #[inline]
    fn get<'a, 'w: 'a>(
        fetch: &'a mut Self::Fetch<'w>,
        row: usize,
        entities: &'a [Entity],
    ) -> Self::Item<'a> {
        <T::Storage as StorageImpl>::ref_opt::<T>(fetch, row, entities)
    }
}

impl<T: Component> QueryParam for Option<&T> {}

impl<T: Component> seal::ReadOnly for Option<&T> {
    #[inline]
    fn get_shared<'w>(
        fetch: &Self::Fetch<'w>,
        row: usize,
        entities: &'w [Entity],
    ) -> Self::Item<'w> {
        <T::Storage as StorageImpl>::ref_opt::<T>(fetch, row, entities)
    }
}

impl<T: Component> ReadOnly for Option<&T> {}

impl<T: Component> seal::Param for Option<&mut T> {
    type Item<'w> = Option<&'w mut T>;
    type Fetch<'w> = <T::Storage as StorageImpl>::Mut<'w, T>;
    const PROBES: bool = false;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>(Role::Optional));
    }

    fn init<'w>(binder: &mut Binder<'w>) -> Self::Fetch<'w> {
        <T::Storage as StorageImpl>::init_mut::<T>(binder.take_sparse())
    }

    #[inline]
    fn set_table<'w>(fetch: &mut Self::Fetch<'w>, binder: &mut Binder<'w>) {
        let (col, len) = (binder.take_table(), binder.len());
        <T::Storage as StorageImpl>::set_mut::<T>(fetch, col, len);
    }

    #[inline]
    fn passes(_: &mut Self::Fetch<'_>, _: usize, _: &[Entity]) -> bool {
        true
    }

    #[inline]
    fn get<'a, 'w: 'a>(
        fetch: &'a mut Self::Fetch<'w>,
        row: usize,
        entities: &'a [Entity],
    ) -> Self::Item<'a> {
        <T::Storage as StorageImpl>::mut_opt::<T>(fetch, row, entities)
    }
}

impl<T: Component> QueryParam for Option<&mut T> {}

impl seal::Param for Entity {
    type Item<'w> = Entity;
    type Fetch<'w> = ();
    const PROBES: bool = false;

    fn types(_: &mut Vec<seal::TypeEntry>) {}

    fn init<'w>(_: &mut Binder<'w>) -> Self::Fetch<'w> {}

    #[inline]
    fn set_table<'w>((): &mut Self::Fetch<'w>, _: &mut Binder<'w>) {}

    #[inline]
    fn passes((): &mut Self::Fetch<'_>, _: usize, _: &[Entity]) -> bool {
        true
    }

    #[inline]
    fn get<'a, 'w: 'a>((): &'a mut (), row: usize, entities: &'a [Entity]) -> Self::Item<'a> {
        entities[row]
    }
}

impl QueryParam for Entity {}

impl seal::ReadOnly for Entity {
    #[inline]
    fn get_shared<'w>((): &Self::Fetch<'w>, row: usize, entities: &'w [Entity]) -> Self::Item<'w> {
        entities[row]
    }
}

impl ReadOnly for Entity {}

macro_rules! impl_tuple_param {
    ($($p:ident),+) => {
        #[allow(non_snake_case)]
        impl<$($p: QueryParam),+> seal::Param for ($($p,)+) {
            type Item<'w> = ($($p::Item<'w>,)+);
            type Fetch<'w> = ($($p::Fetch<'w>,)+);
            const PROBES: bool = $($p::PROBES)||+;

            fn types(out: &mut Vec<seal::TypeEntry>) {
                $($p::types(out);)+
            }

            fn init<'w>(binder: &mut Binder<'w>) -> Self::Fetch<'w> {
                ($($p::init(binder),)+)
            }

            #[inline]
            fn set_table<'w>(fetch: &mut Self::Fetch<'w>, binder: &mut Binder<'w>) {
                let ($($p,)+) = fetch;
                $($p::set_table($p, binder);)+
            }

            #[inline]
            fn passes(fetch: &mut Self::Fetch<'_>, row: usize, entities: &[Entity]) -> bool {
                let ($($p,)+) = fetch;
                $($p::passes($p, row, entities))&&+
            }

            #[inline]
            fn get<'a, 'w: 'a>(
                fetch: &'a mut Self::Fetch<'w>,
                row: usize,
                entities: &'a [Entity],
            ) -> Self::Item<'a> {
                let ($($p,)+) = fetch;
                ($($p::get($p, row, entities),)+)
            }
        }

        impl<$($p: QueryParam),+> QueryParam for ($($p,)+) {}

        #[allow(non_snake_case)]
        impl<$($p: ReadOnly),+> seal::ReadOnly for ($($p,)+) {
            #[inline]
            fn get_shared<'w>(
                fetch: &Self::Fetch<'w>,
                row: usize,
                entities: &'w [Entity],
            ) -> Self::Item<'w> {
                let ($($p,)+) = fetch;
                ($($p::get_shared($p, row, entities),)+)
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
    type Fetch<'w> = ();
    const PROBES: bool = false;

    fn types(_: &mut Vec<seal::TypeEntry>) {}

    fn init<'w>(_: &mut Binder<'w>) -> Self::Fetch<'w> {}

    #[inline]
    fn passes((): &mut Self::Fetch<'_>, _: usize, _: &[Entity]) -> bool {
        true
    }
}

impl Filter for () {}

impl<T: Component> seal::Filter for With<T> {
    type Fetch<'w> = <T::Storage as StorageImpl>::Ref<'w, T>;
    const PROBES: bool = <T::Storage as StorageImpl>::PROBES;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>(Role::Required));
    }

    fn init<'w>(binder: &mut Binder<'w>) -> Self::Fetch<'w> {
        <T::Storage as StorageImpl>::init_ref::<T>(binder.take_sparse())
    }

    #[inline]
    fn passes(fetch: &mut Self::Fetch<'_>, row: usize, entities: &[Entity]) -> bool {
        <T::Storage as StorageImpl>::ref_has::<T>(fetch, row, entities)
    }
}

impl<T: Component> Filter for With<T> {}

impl<T: Component> seal::Filter for Without<T> {
    type Fetch<'w> = <T::Storage as StorageImpl>::Ref<'w, T>;
    const PROBES: bool = <T::Storage as StorageImpl>::PROBES;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>(Role::Excluded));
    }

    fn init<'w>(binder: &mut Binder<'w>) -> Self::Fetch<'w> {
        <T::Storage as StorageImpl>::init_ref::<T>(binder.take_sparse())
    }

    #[inline]
    fn passes(fetch: &mut Self::Fetch<'_>, row: usize, entities: &[Entity]) -> bool {
        // A table type is excluded at archetype level, so its rows always pass.
        !<T::Storage as StorageImpl>::PROBES
            || !<T::Storage as StorageImpl>::ref_has::<T>(fetch, row, entities)
    }
}

impl<T: Component> Filter for Without<T> {}

impl<F: Filter, G: Filter> seal::Filter for (F, G) {
    type Fetch<'w> = (F::Fetch<'w>, G::Fetch<'w>);
    const PROBES: bool = F::PROBES || G::PROBES;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        F::types(out);
        G::types(out);
    }

    fn init<'w>(binder: &mut Binder<'w>) -> Self::Fetch<'w> {
        (F::init(binder), G::init(binder))
    }

    #[inline]
    fn passes(fetch: &mut Self::Fetch<'_>, row: usize, entities: &[Entity]) -> bool {
        F::passes(&mut fetch.0, row, entities) && G::passes(&mut fetch.1, row, entities)
    }
}

impl<F: Filter, G: Filter> Filter for (F, G) {}

/// What a run needs to match archetypes: every type the query mentions with its component id,
/// parameters first, and how many of them are parameters (the rest are filters). One allocation.
struct Plan {
    entries: Vec<seal::TypeEntry>,
    params: usize,
    /// A required type was never registered, so nothing can match.
    dead: bool,
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
        let mut dead = false;
        for e in &mut entries {
            e.component = world.component_id_of(e.id);
            e.set = e.component.and_then(|c| world.component_info(c).sparse);
            dead |= e.role == Role::Required && e.component.is_none();
        }
        Self {
            entries,
            params,
            dead,
        }
    }

    /// Whether the archetype has every required table type and none of the excluded ones. Both
    /// query types and `matched_archetypes` use this one function.
    fn matches(&self, archetype: &Archetype) -> bool {
        !self.dead && self.entries.iter().all(|e| e.passes(archetype))
    }

    /// The sparse types the run probes per row, with their roles: the ones a row consults,
    /// from the same list the run uses.
    #[cfg(test)]
    fn probed(&self) -> Vec<(crate::archetype::ComponentId, Role)> {
        self.entries
            .iter()
            .filter(|e| e.kind == crate::component::StorageKind::Sparse)
            .filter_map(|e| e.component.map(|c| (c, e.role)))
            .collect()
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

/// A read-only query, borrowed from [`World::query`]. Add filters with [`with`](Self::with)
/// and [`without`](Self::without), then run it with [`for_each`](Self::for_each),
/// [`iter`](Self::iter) or a `for` loop.
///
/// # Examples
///
/// ```
/// use spike_ecs_hybrid::{Component, Entity, SparseStorage, TableStorage, World};
///
/// struct Position(f32);
/// struct Frozen;
/// impl Component for Position { type Storage = TableStorage; }
/// impl Component for Frozen { type Storage = SparseStorage; }
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
    pub fn with<T: Component>(self) -> Query<'w, Q, (F, With<T>)> {
        Query {
            world: self.world,
            marker: PhantomData,
        }
    }

    /// Keeps only entities that do not have a `T`.
    pub fn without<T: Component>(self) -> Query<'w, Q, (F, Without<T>)> {
        Query {
            world: self.world,
            marker: PhantomData,
        }
    }

    /// Calls `f` for each matching entity.
    ///
    /// # Panics
    ///
    /// Panics with a `Query::for_each:` message if a type appears twice among the parameters
    /// and filters.
    #[track_caller]
    pub fn for_each(self, mut f: impl FnMut(Q::Item<'w>)) {
        let world: &'w World = self.world;
        let plan = Plan::new::<Q, F>(world, "Query::for_each");
        if plan.dead {
            return;
        }
        let (archetypes, sets) = world.parts();
        let mut binder = Binder::new();
        binder.fill_sparse_shared(sets, &plan.entries);
        let mut qf = Q::init(&mut binder);
        let mut ff = F::init(&mut binder);
        for archetype in archetypes {
            if archetype.table.len() == 0 || !plan.matches(archetype) {
                continue;
            }
            binder.fill_tables_shared(archetype, &plan.entries, plan.params);
            let (entities, len) = (binder.entities(), binder.len());
            Q::set_table(&mut qf, &mut binder);
            for row in 0..len {
                if (Q::PROBES || F::PROBES)
                    && !(Q::passes(&mut qf, row, entities) && F::passes(&mut ff, row, entities))
                {
                    continue;
                }
                f(Q::get_shared(&qf, row, entities));
            }
        }
    }

    /// An iterator over the matching entities.
    ///
    /// # Panics
    ///
    /// Panics with a `Query::iter:` message under the same conditions as
    /// [`for_each`](Self::for_each).
    #[track_caller]
    pub fn iter(&self) -> QueryIter<'w, Q, F> {
        self.start("Query::iter")
    }

    #[track_caller]
    fn start(&self, op: &str) -> QueryIter<'w, Q, F> {
        let world: &'w World = self.world;
        let plan = Plan::new::<Q, F>(world, op);
        let (archetypes, sets) = world.parts();
        let remaining = archetypes
            .iter()
            .filter(|a| plan.matches(a))
            .map(|a| a.table.len())
            .sum();
        let mut binder = Binder::new();
        if !plan.dead {
            binder.fill_sparse_shared(sets, &plan.entries);
        }
        let qf = Q::init(&mut binder);
        let ff = F::init(&mut binder);
        QueryIter {
            archetypes,
            plan,
            binder,
            qf,
            ff,
            entities: &[],
            next_archetype: 0,
            row: 0,
            len: 0,
            remaining,
        }
    }

    /// The ids of the archetypes a run would match, in id order, rows or not, for tests.
    #[cfg(test)]
    pub(crate) fn matched_archetypes(&self) -> Vec<crate::archetype::ArchetypeId> {
        let plan = Plan::new::<Q, F>(self.world, "Query::matched_archetypes");
        matched(self.world.parts().0, &plan)
    }

    /// The sparse types a run probes per row, with their roles, for tests.
    #[cfg(test)]
    pub(crate) fn probed(&self) -> Vec<(crate::archetype::ComponentId, Role)> {
        Plan::new::<Q, F>(self.world, "Query::probed").probed()
    }
}

impl<'w, Q: ReadOnly, F: Filter> IntoIterator for Query<'w, Q, F> {
    type Item = Q::Item<'w>;
    type IntoIter = QueryIter<'w, Q, F>;

    /// # Panics
    ///
    /// Panics with a `Query::into_iter:` message under the same conditions as
    /// [`Query::for_each`].
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
    /// Panics with a `Query::into_iter:` message under the same conditions as
    /// [`Query::for_each`].
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
    binder: Binder<'w>,
    qf: Q::Fetch<'w>,
    ff: F::Fetch<'w>,
    /// The current archetype's entities.
    entities: &'w [Entity],
    next_archetype: usize,
    row: usize,
    len: usize,
    /// Rows of the matched archetypes not yet visited (the current one included).
    remaining: usize,
}

impl<Q: ReadOnly, F: Filter> QueryIter<'_, Q, F> {
    /// Opens the next matched archetype that has rows. Returns whether there was one.
    fn open_next(&mut self) -> bool {
        while let Some(archetype) = self.archetypes.get(self.next_archetype) {
            self.next_archetype += 1;
            if archetype.table.len() > 0 && self.plan.matches(archetype) {
                self.binder
                    .fill_tables_shared(archetype, &self.plan.entries, self.plan.params);
                self.entities = self.binder.entities();
                self.len = self.binder.len();
                self.row = 0;
                Q::set_table(&mut self.qf, &mut self.binder);
                return true;
            }
        }
        false
    }
}

impl<'w, Q: ReadOnly, F: Filter> Iterator for QueryIter<'w, Q, F> {
    type Item = Q::Item<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            while self.row < self.len {
                let row = self.row;
                self.row += 1;
                self.remaining -= 1;
                if (Q::PROBES || F::PROBES)
                    && !(Q::passes(&mut self.qf, row, self.entities)
                        && F::passes(&mut self.ff, row, self.entities))
                {
                    continue;
                }
                return Some(Q::get_shared(&self.qf, row, self.entities));
            }
            if !self.open_next() {
                return None;
            }
        }
    }

    /// Exact for a query with no required or excluded sparse type; otherwise `(0, Some(n))`
    /// with `n` the rows of the matched archetypes not yet visited.
    fn size_hint(&self) -> (usize, Option<usize>) {
        if Q::PROBES || F::PROBES {
            (0, Some(self.remaining))
        } else {
            (self.remaining, Some(self.remaining))
        }
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
/// use spike_ecs_hybrid::{Component, SparseStorage, TableStorage, World};
///
/// struct Position(f32);
/// struct Velocity(f32);
/// impl Component for Position { type Storage = TableStorage; }
/// impl Component for Velocity { type Storage = SparseStorage; }
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
    pub fn with<T: Component>(self) -> QueryMut<'w, Q, (F, With<T>)> {
        QueryMut {
            world: self.world,
            marker: PhantomData,
        }
    }

    /// Keeps only entities that do not have a `T`.
    pub fn without<T: Component>(self) -> QueryMut<'w, Q, (F, Without<T>)> {
        QueryMut {
            world: self.world,
            marker: PhantomData,
        }
    }

    /// Calls `f` for each matching entity.
    ///
    /// # Panics
    ///
    /// Panics with a `QueryMut::for_each:` message if a type appears twice among the parameters
    /// and filters.
    #[track_caller]
    pub fn for_each(self, mut f: impl FnMut(Q::Item<'_>)) {
        let world: &'w mut World = self.world;
        let plan = Plan::new::<Q, F>(world, "QueryMut::for_each");
        if plan.dead {
            return;
        }
        let (archetypes, sets) = world.parts_mut();
        let mut binder = Binder::new();
        binder.fill_sparse_exclusive(sets, &plan.entries);
        let mut qf = Q::init(&mut binder);
        let mut ff = F::init(&mut binder);
        for archetype in archetypes {
            if archetype.table.len() == 0 || !plan.matches(archetype) {
                continue;
            }
            binder.fill_tables_exclusive(archetype, &plan.entries, plan.params);
            let (entities, len) = (binder.entities(), binder.len());
            Q::set_table(&mut qf, &mut binder);
            for row in 0..len {
                if (Q::PROBES || F::PROBES)
                    && !(Q::passes(&mut qf, row, entities) && F::passes(&mut ff, row, entities))
                {
                    continue;
                }
                f(Q::get(&mut qf, row, entities));
            }
        }
    }

    /// The ids of the archetypes a run would match, in id order, rows or not, for tests.
    #[cfg(test)]
    pub(crate) fn matched_archetypes(&self) -> Vec<crate::archetype::ArchetypeId> {
        let plan = Plan::new::<Q, F>(self.world, "QueryMut::matched_archetypes");
        matched(self.world.parts().0, &plan)
    }

    /// The sparse types a run probes per row, with their roles, for tests.
    #[cfg(test)]
    pub(crate) fn probed(&self) -> Vec<(crate::archetype::ComponentId, Role)> {
        Plan::new::<Q, F>(self.world, "QueryMut::probed").probed()
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
    use std::collections::BTreeSet;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use super::seal::Role;
    use crate::component::components;
    use crate::{Entity, World};

    #[derive(Debug, Clone, PartialEq)]
    struct A(u32);
    #[derive(Debug, Clone, PartialEq)]
    struct B(u32);
    #[derive(Debug, Clone, PartialEq)]
    struct S(u32);
    #[derive(Debug, Clone, PartialEq)]
    struct R(u32);
    /// A table type that is never inserted.
    struct Y;
    /// A sparse type that is never inserted.
    struct Z;
    components!(TableStorage: A, B, Y);
    components!(SparseStorage: S, R, Z);

    /// Archetypes: 0 empty, 1 (A), 2 (A, B). `S` and `R` live in their sets.
    /// e0 nothing; e1 A; e2 A B; e3 A S; e4 S; e5 A B S R; e6 A B S.
    fn fixture() -> (World, [Entity; 7]) {
        let mut w = World::new();
        let e0 = w.spawn();
        let e1 = w.spawn_with((A(1),));
        let e2 = w.spawn_with((A(2), B(20)));
        let e3 = w.spawn_with((A(3), S(30)));
        let e4 = w.spawn_with((S(40),));
        let e5 = w.spawn_with((A(5), B(50), S(50), R(500)));
        let e6 = w.spawn_with((A(6), B(60), S(60)));
        (w, [e0, e1, e2, e3, e4, e5, e6])
    }

    /// Walks `it` to its end, checking the bounds `size_hint` gives with sparse filters.
    fn walk<Q: super::ReadOnly, F: super::Filter>(mut it: super::QueryIter<'_, Q, F>, kept: usize) {
        assert_eq!(it.size_hint(), (0, Some(5)));
        let mut yielded = 0;
        while it.next().is_some() {
            yielded += 1;
            let (lo, hi) = it.size_hint();
            assert_eq!(lo, 0);
            assert!(hi.expect("an upper bound") <= 5 - yielded);
        }
        assert_eq!(yielded, kept);
        assert_eq!(it.size_hint(), (0, Some(0)));
    }

    fn panic_text(f: impl FnOnce()) -> String {
        let payload = catch_unwind(AssertUnwindSafe(f)).expect_err("the call should panic");
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_string()))
            .expect("a string panic payload")
    }

    fn sorted<T: Ord>(mut v: Vec<T>) -> Vec<T> {
        v.sort();
        v
    }

    #[test]
    fn required_types_select_entities() {
        let (w, _) = fixture();
        let mut got = Vec::new();
        w.query::<(&A, &S)>()
            .for_each(|(a, s)| got.push((a.0, s.0)));
        assert_eq!(sorted(got), [(3, 30), (5, 50), (6, 60)]);
        let mut got = Vec::new();
        w.query::<(&B, &R)>()
            .for_each(|(b, r)| got.push((b.0, r.0)));
        assert_eq!(got, [(50, 500)]);
        let mut got = Vec::new();
        w.query::<(&A, &B)>()
            .for_each(|(a, b)| got.push((a.0, b.0)));
        assert_eq!(sorted(got), [(2, 20), (5, 50), (6, 60)]);
    }

    #[test]
    fn option_yields_some_exactly_when_present() {
        let (w, _) = fixture();
        let mut got = Vec::new();
        w.query::<(&A, Option<&B>)>()
            .for_each(|(a, b)| got.push((a.0, b.map(|b| b.0))));
        assert_eq!(
            sorted(got),
            [
                (1, None),
                (2, Some(20)),
                (3, None),
                (5, Some(50)),
                (6, Some(60))
            ]
        );
        let mut got = Vec::new();
        w.query::<(&A, Option<&S>)>()
            .for_each(|(a, s)| got.push((a.0, s.map(|s| s.0))));
        assert_eq!(
            sorted(got),
            [
                (1, None),
                (2, None),
                (3, Some(30)),
                (5, Some(50)),
                (6, Some(60))
            ]
        );
    }

    #[test]
    fn sparse_option_is_none_when_absent() {
        let (mut w, _) = fixture();
        // A registered set the entity is not in, and a type never registered.
        let mut got = Vec::new();
        w.query::<(&A, Option<&R>)>()
            .for_each(|(a, r)| got.push((a.0, r.map(|r| r.0))));
        assert_eq!(
            sorted(got),
            [(1, None), (2, None), (3, None), (5, Some(500)), (6, None)]
        );
        let mut n = 0;
        w.query::<(&A, Option<&Z>)>().for_each(|(_, z)| {
            assert!(z.is_none());
            n += 1;
        });
        assert_eq!(n, 5);
        let mut n = 0;
        w.query_mut::<(&mut A, Option<&mut Z>)>()
            .for_each(|(_, z)| {
                assert!(z.is_none());
                n += 1;
            });
        assert_eq!(n, 5);
        // A world where no sparse type was ever registered.
        let mut fresh = World::new();
        fresh.spawn_with((A(1),));
        let mut seen = Vec::new();
        fresh
            .query::<Option<&S>>()
            .for_each(|s| seen.push(s.is_some()));
        assert_eq!(seen, [false]);
    }

    #[test]
    fn with_and_without_filters_chain() {
        let (w, e) = fixture();
        let ids = |q: Vec<Entity>| q.into_iter().map(Entity::index).collect::<BTreeSet<_>>();
        let idx = |list: &[usize]| list.iter().map(|i| e[*i].index()).collect::<BTreeSet<_>>();
        // A table `With`, a sparse `Without`.
        let got: Vec<Entity> = w
            .query::<Entity>()
            .with::<A>()
            .without::<S>()
            .iter()
            .collect();
        assert_eq!(ids(got), idx(&[1, 2]));
        // A sparse `With`, a table `Without`.
        let got: Vec<Entity> = w
            .query::<Entity>()
            .with::<S>()
            .without::<A>()
            .iter()
            .collect();
        assert_eq!(ids(got), idx(&[4]));
        let got: Vec<Entity> = w
            .query::<Entity>()
            .with::<S>()
            .without::<B>()
            .iter()
            .collect();
        assert_eq!(ids(got), idx(&[3, 4]));
        let got: Vec<Entity> = w
            .query::<Entity>()
            .with::<A>()
            .without::<R>()
            .iter()
            .collect();
        assert_eq!(ids(got), idx(&[1, 2, 3, 6]));
        // Both sparse.
        let got: Vec<Entity> = w
            .query::<Entity>()
            .with::<S>()
            .without::<R>()
            .iter()
            .collect();
        assert_eq!(ids(got), idx(&[3, 4, 6]));
        // The same through for_each.
        let mut got = Vec::new();
        w.query::<Entity>()
            .with::<S>()
            .without::<R>()
            .for_each(|x| got.push(x));
        assert_eq!(ids(got), idx(&[3, 4, 6]));
    }

    #[test]
    fn sparse_with_alone_drives_a_query() {
        let (w, e) = fixture();
        let q = w.query::<Entity>().with::<S>();
        // No table type is required, so every archetype is visited, the empty one included.
        assert_eq!(q.matched_archetypes().len(), w.archetype_count());
        let got: BTreeSet<u32> = q.iter().map(Entity::index).collect();
        let want: BTreeSet<u32> = [3, 4, 5, 6].iter().map(|i| e[*i].index()).collect();
        assert_eq!(got, want);
    }

    #[test]
    fn entity_parameter_yields_the_entity() {
        let (w, _) = fixture();
        let mut n = 0;
        w.query::<(Entity, &A)>().for_each(|(e, a)| {
            assert_eq!(w.get::<A>(e), Some(a));
            n += 1;
        });
        assert_eq!(n, 5);
        w.query::<(Entity, &S)>().for_each(|(e, s)| {
            assert_eq!(w.get::<S>(e), Some(s));
        });
    }

    #[test]
    fn nested_tuple_works() {
        let (mut w, e) = fixture();
        let mut seen = Vec::new();
        w.query_mut::<(Entity, (&mut A, &S, &B, Option<&mut R>))>()
            .for_each(|(ent, (a, s, b, r))| {
                a.0 += 1000;
                assert_eq!(s.0, b.0);
                let had_r = r.is_some();
                if let Some(r) = r {
                    r.0 += 1;
                }
                seen.push((ent.index(), had_r));
            });
        assert_eq!(
            sorted(seen),
            sorted(vec![(e[5].index(), true), (e[6].index(), false)])
        );
        assert_eq!(w.get::<A>(e[5]), Some(&A(1005)));
        assert_eq!(w.get::<A>(e[6]), Some(&A(1006)));
        assert_eq!(w.get::<A>(e[3]), Some(&A(3)));
        assert_eq!(w.get::<R>(e[5]), Some(&R(501)));
    }

    #[test]
    fn never_inserted_required_type_matches_nothing() {
        let (mut w, _) = fixture();
        let mut n = 0;
        w.query::<&Y>().for_each(|_| n += 1);
        w.query::<(&A, &Z)>().for_each(|_| n += 1);
        w.query::<&A>().with::<Y>().for_each(|_| n += 1);
        w.query::<&A>().with::<Z>().for_each(|_| n += 1);
        w.query_mut::<&mut Y>().for_each(|_| n += 1);
        w.query_mut::<(&mut A, &mut Z)>().for_each(|_| n += 1);
        assert_eq!(n, 0);
        assert_eq!(w.query::<&Y>().matched_archetypes(), []);
        assert_eq!(w.query::<&Z>().matched_archetypes(), []);
        assert!(w.query::<&Z>().iter().next().is_none());
        assert!(w.query::<&Y>().iter().next().is_none());
    }

    #[test]
    fn mutable_query_writes_are_visible() {
        let (mut w, e) = fixture();
        w.query_mut::<&mut S>().for_each(|s| s.0 += 1);
        assert_eq!(w.get::<S>(e[3]), Some(&S(31)));
        assert_eq!(w.get::<S>(e[4]), Some(&S(41)));
        assert_eq!(w.get::<S>(e[5]), Some(&S(51)));
        w.query_mut::<(&mut B, &mut S)>().for_each(|(b, s)| {
            b.0 += 1;
            s.0 += 1;
        });
        assert_eq!(w.get::<B>(e[5]), Some(&B(51)));
        assert_eq!(w.get::<S>(e[5]), Some(&S(52)));
        assert_eq!(w.get::<S>(e[3]), Some(&S(31)));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn iter_and_for_each_agree() {
        let (w, _) = fixture();
        let mut by_each = Vec::new();
        w.query::<(Entity, &A, Option<&S>)>()
            .without::<R>()
            .for_each(|(e, a, s)| by_each.push((e.index(), a.0, s.map(|s| s.0))));
        let q = w.query::<(Entity, &A, Option<&S>)>().without::<R>();
        let by_iter: Vec<_> = q
            .iter()
            .map(|(e, a, s)| (e.index(), a.0, s.map(|s| s.0)))
            .collect();
        let mut by_ref = Vec::new();
        for (e, a, s) in &q {
            by_ref.push((e.index(), a.0, s.map(|s| s.0)));
        }
        let mut by_value = Vec::new();
        for (e, a, s) in q {
            by_value.push((e.index(), a.0, s.map(|s| s.0)));
        }
        assert_eq!(by_each.len(), 4);
        assert_eq!(by_each, by_iter);
        assert_eq!(by_each, by_ref);
        assert_eq!(by_each, by_value);
    }

    #[test]
    fn size_hint_is_exact_without_sparse_filters() {
        let (w, _) = fixture();
        let mut it = w.query::<&A>().iter();
        assert_eq!(it.size_hint(), (5, Some(5)));
        it.next();
        assert_eq!(it.size_hint(), (4, Some(4)));
        assert_eq!(it.by_ref().count(), 4);
        assert_eq!(it.size_hint(), (0, Some(0)));
        assert!(it.next().is_none());
        assert!(it.next().is_none(), "fused");
        // An optional sparse parameter does not probe per row, so the hint stays exact.
        let it = w.query::<(&A, Option<&S>)>().iter();
        assert_eq!(it.size_hint(), (5, Some(5)));
        // A table filter is archetype-level.
        let it = w.query::<&A>().without::<B>().iter();
        assert_eq!(it.size_hint(), (2, Some(2)));
    }

    #[test]
    fn size_hint_bounds_the_count_with_them() {
        let (w, _) = fixture();
        // `A` has 5 rows; `.with::<S>()` keeps 3 of them and `.without::<R>()` keeps 4.
        walk(w.query::<&A>().with::<S>().iter(), 3);
        walk(w.query::<&A>().without::<R>().iter(), 4);
    }

    #[test]
    fn repeated_type_panics() {
        let (mut w, _) = fixture();
        let m = panic_text(|| w.query::<(&A, &A)>().for_each(|_| {}));
        assert!(
            m.starts_with("Query::for_each:") && m.contains("appears twice"),
            "{m}"
        );
        let m = panic_text(|| w.query::<(&S, Option<&S>)>().for_each(|_| {}));
        assert!(
            m.starts_with("Query::for_each:") && m.contains("appears twice"),
            "{m}"
        );
        let m = panic_text(|| drop(w.query::<(&A, &A)>().iter()));
        assert!(
            m.starts_with("Query::iter:") && m.contains("appears twice"),
            "{m}"
        );
        let m = panic_text(|| drop(w.query::<(&S, &S)>().iter()));
        assert!(
            m.starts_with("Query::iter:") && m.contains("appears twice"),
            "{m}"
        );
        let m = panic_text(|| drop(w.query::<(&A, &A)>().into_iter()));
        assert!(
            m.starts_with("Query::into_iter:") && m.contains("appears twice"),
            "{m}"
        );
        let m = panic_text(|| drop(w.query::<(&S, &S)>().into_iter()));
        assert!(
            m.starts_with("Query::into_iter:") && m.contains("appears twice"),
            "{m}"
        );
        let m = panic_text(|| w.query_mut::<(&mut A, &A)>().for_each(|_| {}));
        assert!(
            m.starts_with("QueryMut::for_each:") && m.contains("appears twice"),
            "{m}"
        );
        let m = panic_text(|| w.query_mut::<(&mut S, Option<&mut S>)>().for_each(|_| {}));
        assert!(
            m.starts_with("QueryMut::for_each:") && m.contains("appears twice"),
            "{m}"
        );
        assert!(m.contains("query::tests::S"), "names the type: {m}");
    }

    #[test]
    fn a_type_in_a_parameter_and_a_filter_panics() {
        let (mut w, _) = fixture();
        let m = panic_text(|| w.query::<&A>().without::<A>().for_each(|_| {}));
        assert!(
            m.starts_with("Query::for_each:") && m.contains("appears twice"),
            "{m}"
        );
        let m = panic_text(|| w.query::<&S>().without::<S>().for_each(|_| {}));
        assert!(
            m.starts_with("Query::for_each:") && m.contains("appears twice"),
            "{m}"
        );
        let m = panic_text(|| w.query::<Option<&S>>().with::<S>().for_each(|_| {}));
        assert!(
            m.starts_with("Query::for_each:") && m.contains("appears twice"),
            "{m}"
        );
        let m = panic_text(|| w.query_mut::<&mut B>().with::<B>().for_each(|_| {}));
        assert!(
            m.starts_with("QueryMut::for_each:") && m.contains("appears twice"),
            "{m}"
        );
        let m = panic_text(|| w.query_mut::<&mut R>().without::<R>().for_each(|_| {}));
        assert!(
            m.starts_with("QueryMut::for_each:") && m.contains("appears twice"),
            "{m}"
        );
    }

    #[test]
    fn query_without_a_required_type_visits_every_live_entity() {
        let (w, e) = fixture();
        assert_eq!(w.query::<Entity>().iter().count(), w.len());
        let mut some = 0;
        let mut total = 0;
        w.query::<Option<&S>>().for_each(|s| {
            total += 1;
            some += usize::from(s.is_some());
        });
        assert_eq!((total, some), (7, 4));
        let got: BTreeSet<u32> = w
            .query::<Entity>()
            .without::<S>()
            .iter()
            .map(Entity::index)
            .collect();
        let want: BTreeSet<u32> = [0, 1, 2].iter().map(|i| e[*i].index()).collect();
        assert_eq!(got, want);
        let got: BTreeSet<u32> = w
            .query::<Entity>()
            .without::<A>()
            .iter()
            .map(Entity::index)
            .collect();
        let want: BTreeSet<u32> = [0, 4].iter().map(|i| e[*i].index()).collect();
        assert_eq!(got, want, "the empty archetype's entities are visited");
    }

    #[test]
    fn matched_and_probed_report_the_run() {
        let (w, _) = fixture();
        let ids = |v: Vec<crate::archetype::ArchetypeId>| {
            v.into_iter()
                .map(crate::archetype::ArchetypeId::index)
                .collect::<Vec<_>>()
        };
        // A table-only query has no probe.
        let q = w.query::<(&A, &B)>();
        assert_eq!(q.probed(), []);
        assert_eq!(ids(q.matched_archetypes()), [2]);
        let q = w.query::<&A>().without::<B>();
        assert_eq!(q.probed(), []);
        assert_eq!(ids(q.matched_archetypes()), [1]);
        let q = w.query::<Entity>().without::<B>();
        assert_eq!(ids(q.matched_archetypes()), [0, 1]);
        // Sparse types are probed and play no part in matching.
        let (s, r) = (
            w.component_id::<S>().unwrap(),
            w.component_id::<R>().unwrap(),
        );
        let q = w.query::<(&A, Option<&S>)>().without::<R>();
        assert_eq!(q.probed(), [(s, Role::Optional), (r, Role::Excluded)]);
        assert_eq!(ids(q.matched_archetypes()), [1, 2]);
        let q = w.query::<&S>();
        assert_eq!(q.probed(), [(s, Role::Required)]);
        // The mutable query reports the same way.
        let mut w = w;
        let q = w.query_mut::<&mut A>().with::<S>().without::<Y>();
        assert_eq!(q.probed(), [(s, Role::Required)]);
        assert_eq!(
            ids(q.matched_archetypes()),
            [1, 2],
            "an unregistered excluded type is ignored"
        );
    }
}
