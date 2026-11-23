//! Components and their storage kinds: which storage a type uses is part of its type.

use crate::{Entity, World};

/// A type that can be attached to an entity. [`Component::Storage`] names where its values are
/// kept: [`TableStorage`] or [`SparseStorage`].
///
/// # Examples
///
/// ```
/// use spike_ecs_hybrid::{Component, SparseStorage, TableStorage, World};
///
/// struct Position(f32);
/// struct Stunned;
/// impl Component for Position { type Storage = TableStorage; }
/// impl Component for Stunned { type Storage = SparseStorage; }
///
/// let mut world = World::new();
/// let e = world.spawn();
/// world.insert(e, Position(1.0));
/// world.insert(e, Stunned);
/// assert!(world.get::<Stunned>(e).is_some());
/// ```
pub trait Component: 'static {
    /// The storage kind of this type's values.
    type Storage: Storage;
}

/// A storage kind: implemented by [`TableStorage`] and [`SparseStorage`] only.
pub trait Storage: seal::Storage {}

/// Values live in the archetype table of the entity's set of table types.
#[derive(Debug)]
pub enum TableStorage {}

/// Values live in a sparse set of their own; inserting or removing one moves no row.
#[derive(Debug)]
pub enum SparseStorage {}

impl Storage for TableStorage {}
impl Storage for SparseStorage {}

/// The two storage kinds, as the world records them per registered type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageKind {
    /// An archetype table column.
    Table,
    /// A sparse set.
    Sparse,
}

pub(crate) mod seal {
    use super::{Component, Entity, SparseStorage, StorageKind, TableStorage, World};
    use crate::query::seal::{SparseSlot, TableSlot};
    use crate::sparse::{SparseMut, SparseSet};

    /// What a storage kind does for a component type. One call per operation, so the kind is
    /// chosen at compile time and nothing per entity branches on it.
    ///
    /// The `Ref` and `Mut` types are what a query run holds per parameter. `init_*` binds them
    /// once per run (a sparse set; a table gets an empty slice), `set_*` rebinds the table
    /// parts to a matched archetype's column, and the row functions read one row. A table fetch
    /// ignores `entities`, so a query that names no sparse type reads no entity it does not
    /// yield.
    pub trait Storage: 'static {
        /// The kind this marker stands for.
        const KIND: StorageKind;
        /// Whether a row of this kind needs a membership probe: `true` for sparse sets.
        const PROBES: bool;
        /// A shared fetch of a `T`: a column slice, or a set.
        type Ref<'w, T: 'static>: Copy;
        /// An exclusive fetch of a `T`: a column slice, or a [`SparseMut`].
        type Mut<'w, T: 'static>;

        /// `World::insert` for a live `entity`.
        fn insert<T: Component>(world: &mut World, entity: Entity, value: T) -> Option<T>;
        /// `World::remove`.
        fn remove<T: Component>(world: &mut World, entity: Entity) -> Option<T>;
        /// `World::get`.
        fn get<T: Component>(world: &World, entity: Entity) -> Option<&T>;
        /// `World::get_mut`.
        fn get_mut<T: Component>(world: &mut World, entity: Entity) -> Option<&mut T>;

        /// The shared fetch at the start of a run.
        fn init_ref<T: 'static>(slot: Option<SparseSlot<'_>>) -> Self::Ref<'_, T>;
        /// Rebinds the shared fetch to the matched archetype's column, `len` rows. `required` is
        /// a constant at each call: a required table parameter always gets a column, and the
        /// fetch then has exactly `len` rows, a length the optimizer can see.
        fn set_ref<'w, T: 'static>(
            fetch: &mut Self::Ref<'w, T>,
            col: Option<TableSlot<'w>>,
            len: usize,
            required: bool,
        );
        /// The `T` of a row that must have one.
        fn ref_row<'w, T: 'static>(
            fetch: &Self::Ref<'w, T>,
            row: usize,
            entities: &[Entity],
        ) -> &'w T;
        /// The `T` of a row, if it has one.
        fn ref_opt<'w, T: 'static>(
            fetch: &Self::Ref<'w, T>,
            row: usize,
            entities: &[Entity],
        ) -> Option<&'w T>;
        /// Whether a row has a `T`; only called when `PROBES` is true.
        fn ref_has<T: 'static>(fetch: &Self::Ref<'_, T>, row: usize, entities: &[Entity]) -> bool;

        /// The exclusive fetch at the start of a run.
        fn init_mut<T: 'static>(slot: Option<SparseSlot<'_>>) -> Self::Mut<'_, T>;
        /// Rebinds the exclusive fetch to the matched archetype's column, `len` rows; see
        /// [`set_ref`](Self::set_ref) for `required`.
        fn set_mut<'w, T: 'static>(
            fetch: &mut Self::Mut<'w, T>,
            col: Option<TableSlot<'w>>,
            len: usize,
            required: bool,
        );
        /// The `T` of a row that must have one.
        fn mut_row<'a, T: 'static>(
            fetch: &'a mut Self::Mut<'_, T>,
            row: usize,
            entities: &[Entity],
        ) -> &'a mut T;
        /// The `T` of a row, if it has one.
        fn mut_opt<'a, T: 'static>(
            fetch: &'a mut Self::Mut<'_, T>,
            row: usize,
            entities: &[Entity],
        ) -> Option<&'a mut T>;
        /// Whether a row has a `T`; only called when `PROBES` is true.
        fn mut_has<T: 'static>(
            fetch: &mut Self::Mut<'_, T>,
            row: usize,
            entities: &[Entity],
        ) -> bool;
    }

    /// The panic of a required sparse fetch whose probe passed: an internal bug.
    #[cold]
    #[track_caller]
    fn missing<T>(entity: Entity) -> ! {
        panic!(
            "hybrid query: `{}` has no value for {entity:?} although its probe passed \
             (invariant S1)",
            core::any::type_name::<T>()
        )
    }

    const MISSING: &str =
        "hybrid query: a matched archetype has a column for every required table type";

    impl Storage for TableStorage {
        const KIND: StorageKind = StorageKind::Table;
        const PROBES: bool = false;
        type Ref<'w, T: 'static> = &'w [T];
        type Mut<'w, T: 'static> = &'w mut [T];

        fn insert<T: Component>(world: &mut World, entity: Entity, value: T) -> Option<T> {
            world.table_insert(entity, value)
        }

        fn remove<T: Component>(world: &mut World, entity: Entity) -> Option<T> {
            world.table_remove(entity)
        }

        fn get<T: Component>(world: &World, entity: Entity) -> Option<&T> {
            world.table_get(entity)
        }

        fn get_mut<T: Component>(world: &mut World, entity: Entity) -> Option<&mut T> {
            world.table_get_mut(entity)
        }

        fn init_ref<T: 'static>(_: Option<SparseSlot<'_>>) -> Self::Ref<'_, T> {
            &[]
        }

        #[inline]
        fn set_ref<'w, T: 'static>(
            fetch: &mut Self::Ref<'w, T>,
            col: Option<TableSlot<'w>>,
            len: usize,
            required: bool,
        ) {
            // No `None` arm that yields an empty slice for a required column: that would make
            // the fetch's length `len` or 0, and the row loop could not be vectorized.
            *fetch = if let Some(c) = col {
                &c.into_ref::<T>()[..len]
            } else {
                assert!(!required, "{MISSING}");
                &[]
            };
        }

        #[inline]
        fn ref_row<'w, T: 'static>(fetch: &Self::Ref<'w, T>, row: usize, _: &[Entity]) -> &'w T {
            &fetch[row]
        }

        #[inline]
        fn ref_opt<'w, T: 'static>(
            fetch: &Self::Ref<'w, T>,
            row: usize,
            _: &[Entity],
        ) -> Option<&'w T> {
            let column: &'w [T] = fetch;
            column.get(row)
        }

        #[inline]
        fn ref_has<T: 'static>(_: &Self::Ref<'_, T>, _: usize, _: &[Entity]) -> bool {
            true
        }

        fn init_mut<T: 'static>(_: Option<SparseSlot<'_>>) -> Self::Mut<'_, T> {
            &mut []
        }

        #[inline]
        fn set_mut<'w, T: 'static>(
            fetch: &mut Self::Mut<'w, T>,
            col: Option<TableSlot<'w>>,
            len: usize,
            required: bool,
        ) {
            *fetch = if let Some(c) = col {
                &mut c.into_mut::<T>()[..len]
            } else {
                assert!(!required, "{MISSING}");
                &mut []
            };
        }

        #[inline]
        fn mut_row<'a, T: 'static>(
            fetch: &'a mut Self::Mut<'_, T>,
            row: usize,
            _: &[Entity],
        ) -> &'a mut T {
            &mut fetch[row]
        }

        #[inline]
        fn mut_opt<'a, T: 'static>(
            fetch: &'a mut Self::Mut<'_, T>,
            row: usize,
            _: &[Entity],
        ) -> Option<&'a mut T> {
            fetch.get_mut(row)
        }

        #[inline]
        fn mut_has<T: 'static>(_: &mut Self::Mut<'_, T>, _: usize, _: &[Entity]) -> bool {
            true
        }
    }

    impl Storage for SparseStorage {
        const KIND: StorageKind = StorageKind::Sparse;
        const PROBES: bool = true;
        type Ref<'w, T: 'static> = Option<&'w SparseSet<T>>;
        type Mut<'w, T: 'static> = Option<SparseMut<'w, T>>;

        fn insert<T: Component>(world: &mut World, entity: Entity, value: T) -> Option<T> {
            world.sparse_insert(entity, value)
        }

        fn remove<T: Component>(world: &mut World, entity: Entity) -> Option<T> {
            world.sparse_remove(entity)
        }

        fn get<T: Component>(world: &World, entity: Entity) -> Option<&T> {
            world.sparse_get(entity)
        }

        fn get_mut<T: Component>(world: &mut World, entity: Entity) -> Option<&mut T> {
            world.sparse_get_mut(entity)
        }

        fn init_ref<T: 'static>(slot: Option<SparseSlot<'_>>) -> Self::Ref<'_, T> {
            slot.map(SparseSlot::into_ref::<T>)
        }

        #[inline]
        fn set_ref<'w, T: 'static>(
            _: &mut Self::Ref<'w, T>,
            _: Option<TableSlot<'w>>,
            _: usize,
            _: bool,
        ) {
        }

        #[inline]
        fn ref_row<'w, T: 'static>(
            fetch: &Self::Ref<'w, T>,
            row: usize,
            entities: &[Entity],
        ) -> &'w T {
            let entity = entities[row];
            let set: Option<&'w SparseSet<T>> = *fetch;
            set.and_then(|s| s.get(entity))
                .unwrap_or_else(|| missing::<T>(entity))
        }

        #[inline]
        fn ref_opt<'w, T: 'static>(
            fetch: &Self::Ref<'w, T>,
            row: usize,
            entities: &[Entity],
        ) -> Option<&'w T> {
            let set: Option<&'w SparseSet<T>> = *fetch;
            set.and_then(|s| s.get(entities[row]))
        }

        #[inline]
        fn ref_has<T: 'static>(fetch: &Self::Ref<'_, T>, row: usize, entities: &[Entity]) -> bool {
            fetch.is_some_and(|s| s.index_of(entities[row]).is_some())
        }

        fn init_mut<T: 'static>(slot: Option<SparseSlot<'_>>) -> Self::Mut<'_, T> {
            slot.map(SparseSlot::into_mut::<T>)
        }

        #[inline]
        fn set_mut<'w, T: 'static>(
            _: &mut Self::Mut<'w, T>,
            _: Option<TableSlot<'w>>,
            _: usize,
            _: bool,
        ) {
        }

        #[inline]
        fn mut_row<'a, T: 'static>(
            fetch: &'a mut Self::Mut<'_, T>,
            row: usize,
            entities: &[Entity],
        ) -> &'a mut T {
            let entity = entities[row];
            fetch
                .as_mut()
                .and_then(|s| s.get(entity))
                .unwrap_or_else(|| missing::<T>(entity))
        }

        #[inline]
        fn mut_opt<'a, T: 'static>(
            fetch: &'a mut Self::Mut<'_, T>,
            row: usize,
            entities: &[Entity],
        ) -> Option<&'a mut T> {
            fetch.as_mut().and_then(|s| s.get(entities[row]))
        }

        #[inline]
        fn mut_has<T: 'static>(
            fetch: &mut Self::Mut<'_, T>,
            row: usize,
            entities: &[Entity],
        ) -> bool {
            fetch
                .as_mut()
                .is_some_and(|s| s.get(entities[row]).is_some())
        }
    }
}

/// Writes one `impl Component` per type, all with the same storage marker:
/// `components!(TableStorage: Position, Velocity);`.
macro_rules! components {
    ($storage:ident: $($t:ty),+ $(,)?) => {
        $( impl $crate::Component for $t { type Storage = $crate::$storage; } )+
    };
}
pub(crate) use components;
