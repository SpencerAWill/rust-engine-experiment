//! Typed tuple queries and group declarations.
//!
//! A query names its parameters (`&T`, `&mut T`, `Option<&T>`, `Entity`, tuples of those) and
//! its filters (`.with::<T>()`, `.without::<T>()`). Running it walks the smallest storage
//! among the required types and probes the others (the view path; spec section 4.5).

use core::any::TypeId;
use core::fmt;
use core::marker::PhantomData;

use crate::Entity;
use crate::world::{ComponentId, World};

/// Implementation details of the sealed traits; unnameable outside the crate.
pub(crate) mod seal {
    use core::any::{Any, TypeId};

    use crate::Entity;
    use crate::storage::{ErasedStorage, SparseIndex, SparseSet};
    use crate::world::{ComponentId, World};

    /// The member types of a group, and how to create their storages.
    pub trait Group {
        /// The `TypeId` and name of each member, in declaration order.
        fn types() -> Vec<(TypeId, &'static str)>;
        /// Ensures a storage exists for each member and returns their ids in order.
        fn register(world: &mut World) -> Vec<ComponentId>;
    }

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
    }

    impl TypeEntry {
        /// An entry for `T`.
        pub fn of<T: 'static>(role: Role) -> Self {
            Self {
                id: TypeId::of::<T>(),
                name: core::any::type_name::<T>(),
                role,
            }
        }
    }

    /// A storage handed to one parameter or filter of a running query.
    pub enum Slot<'w> {
        /// From a read-only query.
        Shared(&'w dyn ErasedStorage),
        /// From a mutable query.
        Mut(&'w mut dyn ErasedStorage),
    }

    impl core::fmt::Debug for Slot<'_> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                Slot::Shared(s) => write!(f, "Shared({})", s.type_name()),
                Slot::Mut(s) => write!(f, "Mut({})", s.type_name()),
            }
        }
    }

    impl<'w> Slot<'w> {
        /// The storage as a shared `SparseSet<T>`.
        pub fn into_ref<T: 'static>(self) -> &'w SparseSet<T> {
            let storage: &'w dyn ErasedStorage = match self {
                Slot::Shared(s) => s,
                Slot::Mut(s) => s,
            };
            let any: &'w dyn Any = storage.as_any();
            any.downcast_ref()
                .expect("a component id names the storage of its own type")
        }

        /// The storage as an exclusive `SparseSet<T>`.
        pub fn into_mut<T: 'static>(self) -> &'w mut SparseSet<T> {
            match self {
                Slot::Mut(s) => s
                    .as_any_mut()
                    .downcast_mut()
                    .expect("a component id names the storage of its own type"),
                Slot::Shared(_) => unreachable!("a read-only query has no `&mut` parameter"),
            }
        }
    }

    /// The storages a query run needs, one slot per type in declaration order (parameters,
    /// then filters), handed out by `next` as the fetches are bound.
    #[derive(Debug)]
    pub struct Slots<'w> {
        storages: Vec<Option<Slot<'w>>>,
        cursor: usize,
        /// Which slots are read by loop index instead of probed.
        direct: Vec<bool>,
        /// On the group path, the group's length: direct columns are cut to it.
        limit: Option<usize>,
    }

    impl<'w> Slots<'w> {
        /// Slots that borrow the storages shared.
        pub fn shared(world: &'w World, ids: &[Option<ComponentId>], how: &Access) -> Self {
            let storages = world.storages();
            let slots = ids
                .iter()
                .map(|id| id.map(|c| Slot::Shared(&*storages[c.index()])))
                .collect();
            Self {
                storages: slots,
                cursor: 0,
                direct: how.direct.clone(),
                limit: how.limit,
            }
        }

        /// Slots that borrow the storages exclusive: one pass over the storage list hands each
        /// `&mut` to its slot, so no two slots alias and no `unsafe` is needed.
        pub fn exclusive(world: &'w mut World, ids: &[Option<ComponentId>], how: &Access) -> Self {
            let mut slots: Vec<Option<Slot<'w>>> = ids.iter().map(|_| None).collect();
            for (k, storage) in world.storages_mut().iter_mut().enumerate() {
                if let Some(j) = ids
                    .iter()
                    .position(|id| id.map(ComponentId::index) == Some(k))
                {
                    slots[j] = Some(Slot::Mut(&mut **storage));
                }
            }
            Self {
                storages: slots,
                cursor: 0,
                direct: how.direct.clone(),
                limit: how.limit,
            }
        }

        /// The next slot, and whether it is read by loop index.
        pub fn next(&mut self) -> (Option<Slot<'w>>, bool) {
            let n = self.cursor;
            self.cursor += 1;
            (self.storages[n].take(), self.direct[n])
        }

        /// The group length that direct columns are cut to, on the group path.
        pub fn limit(&self) -> Option<usize> {
            self.limit
        }
    }

    /// How a query run reads its columns: which are direct, and the group length on the group
    /// path.
    #[derive(Debug)]
    pub struct Access {
        /// One flag per type, in declaration order.
        pub direct: Vec<bool>,
        /// The group length, on the group path.
        pub limit: Option<usize>,
    }

    /// A storage's fields, borrowed separately. `direct` marks a column whose position is the
    /// loop index and is never probed: the driver on the view path, every member on the group
    /// path.
    #[derive(Debug)]
    pub struct Col<'w, V> {
        /// The sparse index.
        pub sparse: &'w SparseIndex,
        /// The packed entities.
        pub entities: &'w [Entity],
        /// The packed values (`&[T]`, `&mut [T]`, or `()` for a filter).
        pub data: V,
        /// Whether this column drives the loop.
        pub direct: bool,
    }

    /// Column data that can be cut to a prefix.
    pub trait Trim {
        /// Keeps the first `n` values.
        fn trim(&mut self, n: usize);
    }

    impl Trim for () {
        fn trim(&mut self, _n: usize) {}
    }

    impl<T> Trim for &[T] {
        fn trim(&mut self, n: usize) {
            *self = &self[..n];
        }
    }

    impl<T> Trim for &mut [T] {
        fn trim(&mut self, n: usize) {
            let all = core::mem::take(self);
            *self = &mut all[..n];
        }
    }

    impl<'w, V: Trim> Col<'w, V> {
        /// A column over a storage's split fields. On the group path a direct column is re-sliced
        /// to the group's length, so its slices line up index for index.
        pub fn new(
            (sparse, mut entities, mut data): (&'w SparseIndex, &'w [Entity], V),
            direct: bool,
            limit: Option<usize>,
        ) -> Self {
            if direct && let Some(n) = limit {
                entities = &entities[..n];
                data.trim(n);
            }
            Self {
                sparse,
                entities,
                data,
                direct,
            }
        }
    }

    impl<V> Col<'_, V> {
        /// The position of `entity`: the loop index when direct, otherwise a probe that reads
        /// the sparse slot alone (the entity is live by S3, so a present slot is its own).
        #[inline]
        pub fn pos(&self, entity: Entity, index: usize) -> Option<usize> {
            if self.direct {
                Some(index)
            } else {
                self.sparse.get(entity.index()).map(|p| p as usize)
            }
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
        /// Takes this parameter's slots, in the same order as `types`.
        fn bind<'w>(slots: &mut Slots<'w>) -> Self::Fetch<'w>;
        /// The driver's entities, if this parameter holds the driver.
        fn driven<'w>(fetch: &Self::Fetch<'w>) -> Option<&'w [Entity]>;
        /// The item for `entity` at driver position `index`, or `None` if it does not match.
        fn get<'a>(
            fetch: &'a mut Self::Fetch<'_>,
            entity: Entity,
            index: usize,
        ) -> Option<Self::Item<'a>>;
    }

    /// `get` for read-only parameters, which can hand out items that outlive the fetch borrow.
    pub trait ReadOnly: Param {
        /// As `Param::get`, with the lifetime of the world borrow.
        fn get_shared<'w>(
            fetch: &Self::Fetch<'w>,
            entity: Entity,
            index: usize,
        ) -> Option<Self::Item<'w>>;
    }

    /// The fetch machinery of a query filter.
    pub trait Filter {
        /// Per-run state.
        type Fetch<'w>;
        /// Appends the component types this filter mentions.
        fn types(out: &mut Vec<TypeEntry>);
        /// Takes this filter's slots.
        fn bind<'w>(slots: &mut Slots<'w>) -> Self::Fetch<'w>;
        /// The driver's entities, if this filter holds the driver.
        fn driven<'w>(fetch: &Self::Fetch<'w>) -> Option<&'w [Entity]>;
        /// Whether `entity` at driver position `index` passes.
        fn passes(fetch: &Self::Fetch<'_>, entity: Entity, index: usize) -> bool;
    }
}

/// A tuple of 2 to 4 component types that [`WorldBuilder::group`](crate::WorldBuilder::group)
/// can declare as an owning group. Sealed: it is implemented for tuples only.
pub trait GroupSpec: seal::Group {}

macro_rules! impl_group_spec {
    ($($t:ident),+) => {
        impl<$($t: 'static),+> seal::Group for ($($t,)+) {
            fn types() -> Vec<(TypeId, &'static str)> {
                vec![$((TypeId::of::<$t>(), core::any::type_name::<$t>())),+]
            }
            fn register(world: &mut World) -> Vec<ComponentId> {
                vec![$(world.id_or_create::<$t>()),+]
            }
        }
        impl<$($t: 'static),+> GroupSpec for ($($t,)+) {}
    };
}

impl_group_spec!(A, B);
impl_group_spec!(A, B, C);
impl_group_spec!(A, B, C, D);

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

const REQUIRED: &str = "a required type has a storage, or the query would have matched nothing";

impl<T: 'static> seal::Param for &T {
    type Item<'w> = &'w T;
    type Fetch<'w> = seal::Col<'w, &'w [T]>;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>(seal::Role::Required));
    }

    fn bind<'w>(slots: &mut seal::Slots<'w>) -> Self::Fetch<'w> {
        let (slot, direct) = slots.next();
        seal::Col::new(
            slot.expect(REQUIRED).into_ref::<T>().split(),
            direct,
            slots.limit(),
        )
    }

    fn driven<'w>(fetch: &Self::Fetch<'w>) -> Option<&'w [Entity]> {
        fetch.direct.then_some(fetch.entities)
    }

    #[inline]
    fn get<'a, 'w>(
        fetch: &'a mut seal::Col<'w, &'w [T]>,
        entity: Entity,
        index: usize,
    ) -> Option<Self::Item<'a>> {
        let p = fetch.pos(entity, index)?;
        Some(&fetch.data[p])
    }
}

impl<T: 'static> QueryParam for &T {}

impl<T: 'static> seal::ReadOnly for &T {
    #[inline]
    fn get_shared<'w>(
        fetch: &Self::Fetch<'w>,
        entity: Entity,
        index: usize,
    ) -> Option<Self::Item<'w>> {
        let data: &'w [T] = fetch.data;
        Some(&data[fetch.pos(entity, index)?])
    }
}

impl<T: 'static> ReadOnly for &T {}

impl<T: 'static> seal::Param for &mut T {
    type Item<'w> = &'w mut T;
    type Fetch<'w> = seal::Col<'w, &'w mut [T]>;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>(seal::Role::Required));
    }

    fn bind<'w>(slots: &mut seal::Slots<'w>) -> Self::Fetch<'w> {
        let (slot, direct) = slots.next();
        seal::Col::new(
            slot.expect(REQUIRED).into_mut::<T>().split_mut(),
            direct,
            slots.limit(),
        )
    }

    fn driven<'w>(fetch: &Self::Fetch<'w>) -> Option<&'w [Entity]> {
        fetch.direct.then_some(fetch.entities)
    }

    #[inline]
    fn get<'a, 'w>(
        fetch: &'a mut seal::Col<'w, &'w mut [T]>,
        entity: Entity,
        index: usize,
    ) -> Option<Self::Item<'a>> {
        let p = fetch.pos(entity, index)?;
        Some(&mut fetch.data[p])
    }
}

impl<T: 'static> QueryParam for &mut T {}

impl<T: 'static> seal::Param for Option<&T> {
    type Item<'w> = Option<&'w T>;
    type Fetch<'w> = Option<seal::Col<'w, &'w [T]>>;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>(seal::Role::Optional));
    }

    fn bind<'w>(slots: &mut seal::Slots<'w>) -> Self::Fetch<'w> {
        let (slot, _) = slots.next();
        slot.map(|s| {
            let (sparse, entities, data) = s.into_ref::<T>().split();
            seal::Col {
                sparse,
                entities,
                data,
                direct: false,
            }
        })
    }

    fn driven<'w>(_fetch: &Self::Fetch<'w>) -> Option<&'w [Entity]> {
        None
    }

    #[inline]
    fn get<'a, 'w>(
        fetch: &'a mut Option<seal::Col<'w, &'w [T]>>,
        entity: Entity,
        index: usize,
    ) -> Option<Self::Item<'a>> {
        Some(
            fetch
                .as_ref()
                .and_then(|c| Some(&c.data[c.pos(entity, index)?])),
        )
    }
}

impl<T: 'static> QueryParam for Option<&T> {}

impl<T: 'static> seal::ReadOnly for Option<&T> {
    #[inline]
    fn get_shared<'w>(
        fetch: &Self::Fetch<'w>,
        entity: Entity,
        index: usize,
    ) -> Option<Self::Item<'w>> {
        Some(fetch.as_ref().and_then(|c| {
            let data: &'w [T] = c.data;
            Some(&data[c.pos(entity, index)?])
        }))
    }
}

impl<T: 'static> ReadOnly for Option<&T> {}

impl<T: 'static> seal::Param for Option<&mut T> {
    type Item<'w> = Option<&'w mut T>;
    type Fetch<'w> = Option<seal::Col<'w, &'w mut [T]>>;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>(seal::Role::Optional));
    }

    fn bind<'w>(slots: &mut seal::Slots<'w>) -> Self::Fetch<'w> {
        let (slot, _) = slots.next();
        slot.map(|s| {
            let (sparse, entities, data) = s.into_mut::<T>().split_mut();
            seal::Col {
                sparse,
                entities,
                data,
                direct: false,
            }
        })
    }

    fn driven<'w>(_fetch: &Self::Fetch<'w>) -> Option<&'w [Entity]> {
        None
    }

    #[inline]
    fn get<'a, 'w>(
        fetch: &'a mut Option<seal::Col<'w, &'w mut [T]>>,
        entity: Entity,
        index: usize,
    ) -> Option<Self::Item<'a>> {
        Some(fetch.as_mut().and_then(|c| {
            let p = c.pos(entity, index)?;
            Some(&mut c.data[p])
        }))
    }
}

impl<T: 'static> QueryParam for Option<&mut T> {}

impl seal::Param for Entity {
    type Item<'w> = Entity;
    type Fetch<'w> = ();

    fn types(_: &mut Vec<seal::TypeEntry>) {}

    fn bind<'w>(_: &mut seal::Slots<'w>) -> Self::Fetch<'w> {}

    fn driven<'w>(_fetch: &Self::Fetch<'w>) -> Option<&'w [Entity]> {
        None
    }

    #[inline]
    fn get(_fetch: &mut (), entity: Entity, _index: usize) -> Option<Self::Item<'_>> {
        Some(entity)
    }
}

impl QueryParam for Entity {}

impl seal::ReadOnly for Entity {
    #[inline]
    fn get_shared<'w>(_fetch: &(), entity: Entity, _index: usize) -> Option<Self::Item<'w>> {
        Some(entity)
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

            fn driven<'w>(fetch: &Self::Fetch<'w>) -> Option<&'w [Entity]> {
                let ($($p,)+) = fetch;
                None$(.or_else(|| $p::driven($p)))+
            }

            #[inline]
            fn get<'a>(
                fetch: &'a mut ($($p::Fetch<'_>,)+),
                entity: Entity,
                index: usize,
            ) -> Option<Self::Item<'a>> {
                let ($($p,)+) = fetch;
                Some(($($p::get($p, entity, index)?,)+))
            }
        }

        impl<$($p: QueryParam),+> QueryParam for ($($p,)+) {}

        #[allow(non_snake_case)]
        impl<$($p: ReadOnly),+> seal::ReadOnly for ($($p,)+) {
            #[inline]
            fn get_shared<'w>(
                fetch: &Self::Fetch<'w>,
                entity: Entity,
                index: usize,
            ) -> Option<Self::Item<'w>> {
                let ($($p,)+) = fetch;
                Some(($($p::get_shared($p, entity, index)?,)+))
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

    fn types(_: &mut Vec<seal::TypeEntry>) {}

    fn bind<'w>(_: &mut seal::Slots<'w>) -> Self::Fetch<'w> {}

    fn driven<'w>(_fetch: &Self::Fetch<'w>) -> Option<&'w [Entity]> {
        None
    }

    #[inline]
    fn passes(_fetch: &Self::Fetch<'_>, _entity: Entity, _index: usize) -> bool {
        true
    }
}

impl Filter for () {}

impl<T: 'static> seal::Filter for With<T> {
    type Fetch<'w> = seal::Col<'w, ()>;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>(seal::Role::Required));
    }

    fn bind<'w>(slots: &mut seal::Slots<'w>) -> Self::Fetch<'w> {
        let (slot, direct) = slots.next();
        let (sparse, entities, _) = slot.expect(REQUIRED).into_ref::<T>().split();
        seal::Col::new((sparse, entities, ()), direct, slots.limit())
    }

    fn driven<'w>(fetch: &Self::Fetch<'w>) -> Option<&'w [Entity]> {
        fetch.direct.then_some(fetch.entities)
    }

    #[inline]
    fn passes(fetch: &Self::Fetch<'_>, entity: Entity, index: usize) -> bool {
        fetch.pos(entity, index).is_some()
    }
}

impl<T: 'static> Filter for With<T> {}

impl<T: 'static> seal::Filter for Without<T> {
    type Fetch<'w> = Option<&'w crate::storage::SparseIndex>;

    fn types(out: &mut Vec<seal::TypeEntry>) {
        out.push(seal::TypeEntry::of::<T>(seal::Role::Excluded));
    }

    fn bind<'w>(slots: &mut seal::Slots<'w>) -> Self::Fetch<'w> {
        let (slot, _) = slots.next();
        slot.map(|s| s.into_ref::<T>().split().0)
    }

    fn driven<'w>(_fetch: &Self::Fetch<'w>) -> Option<&'w [Entity]> {
        None
    }

    #[inline]
    fn passes(fetch: &Self::Fetch<'_>, entity: Entity, _: usize) -> bool {
        fetch.is_none_or(|sparse| sparse.get(entity.index()).is_none())
    }
}

impl<T: 'static> Filter for Without<T> {}

impl<F: Filter, G: Filter> seal::Filter for (F, G) {
    type Fetch<'w> = (F::Fetch<'w>, G::Fetch<'w>);

    fn types(out: &mut Vec<seal::TypeEntry>) {
        F::types(out);
        G::types(out);
    }

    fn bind<'w>(slots: &mut seal::Slots<'w>) -> Self::Fetch<'w> {
        (F::bind(slots), G::bind(slots))
    }

    fn driven<'w>(fetch: &Self::Fetch<'w>) -> Option<&'w [Entity]> {
        F::driven(&fetch.0).or_else(|| G::driven(&fetch.1))
    }

    #[inline]
    fn passes(fetch: &Self::Fetch<'_>, entity: Entity, index: usize) -> bool {
        F::passes(&fetch.0, entity, index) && G::passes(&fetch.1, entity, index)
    }
}

impl<F: Filter, G: Filter> Filter for (F, G) {}

/// Which storage each type of a query lives in, and how the run reads them.
struct Plan {
    ids: Vec<Option<ComponentId>>,
    access: seal::Access,
    /// The group the run iterates by prefix, if any (read by tests).
    #[cfg(test)]
    group: Option<usize>,
}

impl Plan {
    /// Checks the query's types and looks them up. Returns `None` if a required type has no
    /// storage, so the query matches nothing. `op` names the running method in panic messages.
    /// Both `Query` and `QueryMut` choose their path here, so they cannot disagree.
    #[track_caller]
    fn new<Q: QueryParam, F: Filter>(world: &World, op: &str) -> Option<Self> {
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
        assert!(
            entries.iter().any(|e| e.role == seal::Role::Required),
            "{op}: this prototype needs at least one `&T`, `&mut T` or `.with::<T>()` in a query; `{}` has none",
            core::any::type_name::<Q>()
        );
        let ids: Vec<Option<ComponentId>> =
            entries.iter().map(|e| world.component_id(e.id)).collect();
        let required = || {
            entries
                .iter()
                .zip(&ids)
                .enumerate()
                .filter(|(_, (e, _))| e.role == seal::Role::Required)
        };
        if required().any(|(_, (_, id))| id.is_none()) {
            return None;
        }
        let required_ids: Vec<ComponentId> = required().filter_map(|(_, (_, id))| *id).collect();
        if let Some(group) = world.select_group(&required_ids) {
            // Every member is required, so it has a slot; each is read by loop index up to the
            // group's length. The driver is any of them (their entity prefixes are equal; the
            // first member's is the one the spec names).
            let first = world.group_first(group);
            debug_assert!(ids.contains(&Some(first)));
            let direct = ids
                .iter()
                .map(|id| id.is_some_and(|c| world.group_has(group, c)))
                .collect();
            return Some(Self {
                ids,
                access: seal::Access {
                    direct,
                    limit: Some(world.group_len(group)),
                },
                #[cfg(test)]
                group: Some(group),
            });
        }
        let driver = required()
            .min_by_key(|(_, (_, id))| id.map_or(0, |c| world.storages()[c.index()].len()))
            .map(|(j, _)| j)
            .expect("the check above found a required type");
        let mut direct = vec![false; ids.len()];
        direct[driver] = true;
        Some(Self {
            ids,
            access: seal::Access {
                direct,
                limit: None,
            },
            #[cfg(test)]
            group: None,
        })
    }

    /// The slot of the driver.
    #[cfg(test)]
    fn driver(&self) -> usize {
        self.access
            .direct
            .iter()
            .position(|&d| d)
            .expect("a plan has a driver")
    }
}

/// A read-only query, borrowed from [`World::query`]. Add filters with [`with`](Self::with)
/// and [`without`](Self::without), then run it with [`for_each`](Self::for_each),
/// [`iter`](Self::iter) or a `for` loop.
///
/// # Examples
///
/// ```
/// use spike_ecs_sparse_set::{Entity, World};
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
pub struct Query<'w, Q: ReadOnly, F: Filter = ()> {
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
    /// Panics if a type appears twice among the parameters and filters, or if none of them is
    /// required (`&T`, `&mut T` or `.with::<T>()`).
    #[track_caller]
    pub fn for_each(self, mut f: impl FnMut(Q::Item<'w>)) {
        for item in self.start("Query::for_each") {
            f(item);
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
        let Some(plan) = Plan::new::<Q, F>(world, op) else {
            return QueryIter { state: None };
        };
        let mut slots = seal::Slots::shared(world, &plan.ids, &plan.access);
        let qf = Q::bind(&mut slots);
        let ff = F::bind(&mut slots);
        let entities = Q::driven(&qf)
            .or_else(|| F::driven(&ff))
            .expect("the driver is a required parameter or filter");
        QueryIter {
            state: Some(IterState {
                qf,
                ff,
                entities,
                next: 0,
            }),
        }
    }

    /// The group a run would iterate by prefix, for tests.
    #[cfg(test)]
    pub(crate) fn selected_group(&self) -> Option<usize> {
        Plan::new::<Q, F>(self.world, "Query::selected_group")?.group
    }

    /// Whether a run would take the group path, for tests.
    #[cfg(test)]
    pub(crate) fn uses_group(&self) -> bool {
        self.selected_group().is_some()
    }

    /// The length of the driver's storage, for tests.
    #[cfg(test)]
    pub(crate) fn driver_len(&self) -> Option<usize> {
        let plan = Plan::new::<Q, F>(self.world, "Query::driver_len")?;
        let id = plan.ids[plan.driver()]?;
        Some(self.world.storages()[id.index()].len())
    }
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
        self.iter()
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

struct IterState<'w, Q: ReadOnly, F: Filter> {
    qf: Q::Fetch<'w>,
    ff: F::Fetch<'w>,
    entities: &'w [Entity],
    next: usize,
}

/// The iterator of a [`Query`]: yields one item per matching entity.
#[must_use = "iterators do nothing unless consumed"]
pub struct QueryIter<'w, Q: ReadOnly, F: Filter> {
    state: Option<IterState<'w, Q, F>>,
}

impl<'w, Q: ReadOnly, F: Filter> Iterator for QueryIter<'w, Q, F> {
    type Item = Q::Item<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        let st = self.state.as_mut()?;
        while let Some(&entity) = st.entities.get(st.next) {
            let index = st.next;
            st.next += 1;
            if F::passes(&st.ff, entity, index)
                && let Some(item) = Q::get_shared(&st.qf, entity, index)
            {
                return Some(item);
            }
        }
        None
    }
}

impl<Q: ReadOnly, F: Filter> fmt::Debug for QueryIter<'_, Q, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueryIter")
            .field("position", &self.state.as_ref().map(|s| s.next))
            .finish_non_exhaustive()
    }
}

/// A query that may write, borrowed from [`World::query_mut`]. It is run with
/// [`for_each`](Self::for_each); a safe iterator cannot hand out `&mut` items from probed
/// positions, so there is no `iter`.
///
/// # Examples
///
/// ```
/// use spike_ecs_sparse_set::World;
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
pub struct QueryMut<'w, Q: QueryParam, F: Filter = ()> {
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
    /// Panics if a type appears twice among the parameters and filters, or if none of them is
    /// required (`&T`, `&mut T` or `.with::<T>()`).
    #[track_caller]
    pub fn for_each(self, mut f: impl FnMut(Q::Item<'_>)) {
        let Some(plan) = Plan::new::<Q, F>(self.world, "QueryMut::for_each") else {
            return;
        };
        let mut slots = seal::Slots::exclusive(self.world, &plan.ids, &plan.access);
        let mut qf = Q::bind(&mut slots);
        let ff = F::bind(&mut slots);
        let entities = Q::driven(&qf)
            .or_else(|| F::driven(&ff))
            .expect("the driver is a required parameter or filter");
        for (index, &entity) in entities.iter().enumerate() {
            if F::passes(&ff, entity, index)
                && let Some(item) = Q::get(&mut qf, entity, index)
            {
                f(item);
            }
        }
    }
}

impl<Q: QueryParam, F: Filter> QueryMut<'_, Q, F> {
    /// Whether a run would take the group path, for tests.
    #[cfg(test)]
    pub(crate) fn uses_group(&self) -> bool {
        Plan::new::<Q, F>(self.world, "QueryMut::uses_group").is_some_and(|p| p.group.is_some())
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
        let mut n = 0;
        w.query_mut::<(&mut A, &B)>().for_each(|_| n += 1);
        assert_eq!(n, 0);
        // An optional or excluded type with no storage does not block the query.
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
        assert_eq!(by_iter.len(), 4);
        assert_eq!(by_iter, by_for_each);
        assert_eq!(by_iter, by_into);
    }

    #[test]
    #[should_panic(
        expected = "QueryMut::for_each: `spike_ecs_sparse_set::query::tests::A` appears twice in the query"
    )]
    fn repeated_type_panics() {
        let mut w = World::new();
        w.query_mut::<(&mut A, &A)>().for_each(|_| {});
    }

    #[test]
    #[should_panic(
        expected = "Query::iter: `spike_ecs_sparse_set::query::tests::A` appears twice in the query"
    )]
    fn a_type_in_a_parameter_and_a_filter_panics() {
        let w = World::new();
        let _ = w.query::<&A>().without::<A>().iter();
    }

    #[test]
    #[should_panic(
        expected = "Query::for_each: this prototype needs at least one `&T`, `&mut T` or `.with::<T>()` in a query; `"
    )]
    fn query_without_a_required_type_panics_for_entity() {
        let w = World::new();
        w.query::<Entity>().for_each(|_| {});
    }

    #[test]
    #[should_panic(
        expected = "Query::iter: this prototype needs at least one `&T`, `&mut T` or `.with::<T>()` in a query; `"
    )]
    fn query_without_a_required_type_panics_for_option() {
        let w = World::new();
        let _ = w.query::<Option<&A>>().iter();
    }

    #[test]
    #[should_panic(
        expected = "Query::into_iter: this prototype needs at least one `&T`, `&mut T` or `.with::<T>()` in a query; `"
    )]
    fn query_without_a_required_type_panics_for_entity_without() {
        let w = World::new();
        for _ in w.query::<Entity>().without::<A>() {}
    }

    #[test]
    fn driver_is_the_smallest_required_storage() {
        let mut w = World::new();
        for i in 0..100u32 {
            let e = w.spawn();
            w.insert(e, A(i));
            if i < 2 {
                w.insert(e, B(i));
            }
        }
        assert_eq!(w.query::<(&A, &B)>().driver_len(), Some(2));
        assert_eq!(w.query::<(&B, &A)>().driver_len(), Some(2));
        assert_eq!(w.query::<&A>().with::<B>().driver_len(), Some(2));
        assert_eq!(w.query::<(&A, &B)>().iter().count(), 2);
    }

    #[derive(Debug, PartialEq)]
    struct E(u32);

    /// Spawns `n` entities, giving entity `i` the types `has(i)` names (0 = A .. 4 = E).
    fn populate(w: &mut World, n: u32, has: impl Fn(u32, usize) -> bool) -> Vec<Entity> {
        (0..n)
            .map(|i| {
                let e = w.spawn();
                if has(i, 0) {
                    w.insert(e, A(i));
                }
                if has(i, 1) {
                    w.insert(e, B(i));
                }
                if has(i, 2) {
                    w.insert(e, C(i));
                }
                if has(i, 3) {
                    w.insert(e, D(i));
                }
                if has(i, 4) {
                    w.insert(e, E(i));
                }
                e
            })
            .collect()
    }

    #[test]
    fn exact_match_takes_the_group_path() {
        let mut w = World::builder().group::<(A, B)>().build();
        let es = populate(&mut w, 6, |i, t| t == 0 || (t == 1 && i % 2 == 0));
        let q = w.query::<(Entity, &A, &B)>();
        assert!(q.uses_group());
        let got: Vec<Entity> = q.iter().map(|(e, _, _)| e).collect();
        assert_eq!(sorted(got), sorted(vec![es[0], es[2], es[4]]));
        // The order of the members in the query does not matter.
        assert!(w.query::<(&B, &A)>().uses_group());
        assert!(w.query_mut::<(&mut B, &A)>().uses_group());
        // Filters on member types count as required types.
        assert!(w.query::<&A>().with::<B>().uses_group());
    }

    #[test]
    fn superset_takes_the_group_path_and_probes_the_rest() {
        let mut w = World::builder().group::<(A, B)>().build();
        let es = populate(&mut w, 8, |i, t| t < 2 || (t == 2 && i % 3 == 0));
        let q = w.query::<(Entity, &A, &B, &C)>();
        assert!(q.uses_group());
        let got: Vec<(Entity, u32, u32, u32)> =
            q.iter().map(|(e, a, b, c)| (e, a.0, b.0, c.0)).collect();
        let want: Vec<Entity> = [0, 3, 6].iter().map(|&i| es[i]).collect();
        assert_eq!(sorted(got.iter().map(|g| g.0).collect()), sorted(want));
        assert!(got.iter().all(|&(_, a, b, c)| a == b && b == c));
        // The same through a filter, a mutable query, and an optional extra.
        assert!(w.query::<(&A, &B)>().with::<C>().uses_group());
        let mut n = 0;
        w.query_mut::<(&mut A, &B, &C)>().for_each(|(a, b, c)| {
            a.0 += b.0 + c.0;
            n += 1;
        });
        assert_eq!(n, 3);
        assert_eq!(w.get::<A>(es[3]), Some(&A(9)));
        assert_eq!(w.get::<A>(es[1]), Some(&A(1)));
        assert_eq!(w.query::<(&A, &B, Option<&C>)>().iter().count(), 8);
        assert!(w.query::<(&A, &B, Option<&C>)>().uses_group());
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn strict_subset_takes_the_view_path() {
        let mut w = World::builder().group::<(A, B, C)>().build();
        populate(&mut w, 6, |i, t| t == 0 || (t < 3 && i % 2 == 0));
        assert!(!w.query::<&A>().uses_group());
        assert!(!w.query::<(&A, &B)>().uses_group());
        assert!(!w.query_mut::<(&mut A, &C)>().uses_group());
        // An optional or excluded member does not make a query a match.
        assert!(!w.query::<(&A, &B, Option<&C>)>().uses_group());
        assert!(!w.query::<(&A, &B)>().without::<C>().uses_group());
        // A query over a type with no storage matches nothing and does not use a group.
        assert!(!w.query::<(&A, &D)>().uses_group());
        assert_eq!(w.query::<(&A, &B)>().iter().count(), 3);
        assert_eq!(w.query::<&A>().iter().count(), 6);
    }

    #[test]
    fn the_largest_qualifying_group_wins() {
        let mut w = World::builder()
            .group::<(A, B)>()
            .group::<(C, D, E)>()
            .build();
        populate(&mut w, 6, |i, t| t < 2 || i % 2 == 0);
        let q = w.query::<(&A, &B, &C, &D)>().with::<E>();
        assert_eq!(q.selected_group(), Some(1));
        assert!(q.uses_group());
        assert_eq!(q.iter().count(), 3);
        // Only (A, B) qualifies here.
        assert_eq!(w.query::<(&A, &B, &C)>().selected_group(), Some(0));
        // Two groups of equal size qualify: the first declared wins.
        let w2 = World::builder().group::<(A, B)>().group::<(C, D)>().build();
        assert_eq!(w2.query::<(&C, &D, &A, &B)>().selected_group(), Some(0));
        assert_eq!(w2.query::<(&D, &C)>().selected_group(), Some(1));
    }

    #[test]
    fn for_item_in_a_query_reference_agrees_with_iter() {
        let mut w = World::new();
        populate(&mut w, 10, |i, t| t == 0 || (t == 1 && i % 3 == 0));
        let q = w.query::<(Entity, &A, &B)>();
        let by_iter: Vec<(Entity, u32)> = q.iter().map(|(e, a, _)| (e, a.0)).collect();
        let mut by_ref = Vec::new();
        for (e, a, _) in &q {
            by_ref.push((e, a.0));
        }
        assert_eq!(by_iter.len(), 4);
        assert_eq!(by_ref, by_iter);
        // The query is still usable afterwards.
        assert_eq!(q.into_iter().count(), 4);
    }

    mod group_vs_view {
        use proptest::prelude::*;

        use super::{A, B, sorted};
        use crate::{Entity, World};

        #[derive(Debug, Clone)]
        enum Op {
            Spawn,
            Despawn(usize),
            InsertA(usize, u32),
            InsertB(usize, u32),
            RemoveA(usize),
            RemoveB(usize),
        }

        fn op() -> impl Strategy<Value = Op> {
            prop_oneof![
                2 => Just(Op::Spawn),
                1 => any::<usize>().prop_map(Op::Despawn),
                4 => (any::<usize>(), any::<u32>()).prop_map(|(k, v)| Op::InsertA(k, v)),
                4 => (any::<usize>(), any::<u32>()).prop_map(|(k, v)| Op::InsertB(k, v)),
                2 => any::<usize>().prop_map(Op::RemoveA),
                2 => any::<usize>().prop_map(Op::RemoveB),
            ]
        }

        fn apply(w: &mut World, spawned: &mut Vec<Entity>, op: &Op) {
            match *op {
                Op::Spawn => spawned.push(w.spawn()),
                _ if spawned.is_empty() => {}
                Op::Despawn(k) => {
                    w.despawn(spawned[k % spawned.len()]);
                }
                Op::InsertA(k, v) => {
                    let e = spawned[k % spawned.len()];
                    if w.is_alive(e) {
                        w.insert(e, A(v));
                    }
                }
                Op::InsertB(k, v) => {
                    let e = spawned[k % spawned.len()];
                    if w.is_alive(e) {
                        w.insert(e, B(v));
                    }
                }
                Op::RemoveA(k) => {
                    w.remove::<A>(spawned[k % spawned.len()]);
                }
                Op::RemoveB(k) => {
                    w.remove::<B>(spawned[k % spawned.len()]);
                }
            }
        }

        fn visited(w: &World) -> Vec<(u64, u32, u32)> {
            let mut v: Vec<(u64, u32, u32)> = w
                .query::<(Entity, &A, &B)>()
                .iter()
                .map(|(e, a, b)| (e.to_bits(), a.0, b.0))
                .collect();
            v.sort_unstable();
            v
        }

        proptest! {
            #[test]
            fn group_path_and_view_path_visit_the_same_entities(
                ops in prop::collection::vec(op(), 0..64)
            ) {
                let mut grouped = World::builder().group::<(A, B)>().build();
                let mut plain = World::new();
                let (mut sg, mut sp) = (Vec::new(), Vec::new());
                for op in &ops {
                    apply(&mut grouped, &mut sg, op);
                    apply(&mut plain, &mut sp, op);
                    prop_assert!(grouped.query::<(&A, &B)>().uses_group());
                    prop_assert!(!plain.query::<(&A, &B)>().uses_group());
                    prop_assert_eq!(visited(&grouped), visited(&plain));
                    let mut by_mut = Vec::new();
                    grouped
                        .query_mut::<(Entity, &mut A, &B)>()
                        .for_each(|(e, _, _)| by_mut.push(e));
                    prop_assert_eq!(
                        sorted(by_mut).iter().map(|e| e.to_bits()).collect::<Vec<_>>(),
                        visited(&plain).iter().map(|t| t.0).collect::<Vec<_>>()
                    );
                }
            }
        }
    }
}
