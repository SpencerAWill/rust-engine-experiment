//! The raw query: a query over runtime component ids that hands out one table's byte columns at
//! a time, so both access paths (copy through a codec, cast to a `Pod` slice) loop over a whole
//! column with no per-row call into the query (M3.6).
//!
//! A run owns one slot buffer, cleared and refilled for each matching table, so it allocates
//! nothing per table; the slot buffer is its only allocation, and the run borrows the query
//! rather than copying it. Its setup depends on the query, not on how many components are
//! registered.

use core::fmt;

use crate::archetype::{Archetype, ComponentId};
use crate::raw::{RawColumn, RawColumnMut};
use crate::{Entity, World};

/// How a fetch reaches its column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Access {
    Read,
    Write,
}

/// A query over raw components, by id: fetches, which the table view hands out, and filters.
///
/// Fetch `k` is the `k`-th [`read`](Self::read) or [`write`](Self::write) call. Run it with
/// [`World::query_raw`] or, if it writes, [`World::query_raw_mut`].
#[derive(Debug, Clone, Default)]
pub struct RawQuery {
    fetches: Vec<(ComponentId, Access)>,
    with: Vec<ComponentId>,
    without: Vec<ComponentId>,
}

impl RawQuery {
    /// A query with no fetches and no filters: every non-empty archetype.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Fetches `id`'s column for reading, as the next fetch.
    #[must_use]
    pub fn read(mut self, id: ComponentId) -> Self {
        self.fetches.push((id, Access::Read));
        self
    }

    /// Fetches `id`'s column for writing in place, as the next fetch.
    #[must_use]
    pub fn write(mut self, id: ComponentId) -> Self {
        self.fetches.push((id, Access::Write));
        self
    }

    /// Matches only archetypes that have `id`, without fetching it.
    #[must_use]
    pub fn with(mut self, id: ComponentId) -> Self {
        self.with.push(id);
        self
    }

    /// Matches only archetypes that do not have `id`.
    #[must_use]
    pub fn without(mut self, id: ComponentId) -> Self {
        self.without.push(id);
        self
    }

    /// Every id the query names: fetches, then `with`, then `without`.
    fn ids(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.fetches
            .iter()
            .map(|(id, _)| *id)
            .chain(self.with.iter().copied())
            .chain(self.without.iter().copied())
    }

    /// Whether `archetype` has every fetched and `with` id and no `without` id. The one
    /// matching function: runs and the test hook both use it.
    fn matches(&self, archetype: &Archetype) -> bool {
        let has = |id: &ComponentId| archetype.column_of(*id).is_some();
        self.fetches.iter().all(|(id, _)| has(id))
            && self.with.iter().all(has)
            && !self.without.iter().any(has)
    }
}

/// Why a raw query was rejected before it visited any table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawQueryError {
    /// No component with this id is registered in the world.
    UnknownId(ComponentId),
    /// The id names a Rust type's component, which the raw query does not serve.
    NotRaw(ComponentId),
    /// The id is named twice, as fetches, filters or one of each.
    Repeated(ComponentId),
    /// The id is a write fetch in a [`World::query_raw`] query.
    WriteInReadQuery(ComponentId),
}

impl fmt::Display for RawQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::UnknownId(id) => write!(
                f,
                "component id {} is not registered in this world; register it with \
                 World::register_raw first",
                id.index()
            ),
            Self::NotRaw(id) => write!(
                f,
                "component id {} is a Rust type's; use the typed API",
                id.index()
            ),
            Self::Repeated(id) => write!(
                f,
                "component id {} is named twice in one raw query; name each component once",
                id.index()
            ),
            Self::WriteInReadQuery(id) => write!(
                f,
                "component id {} is a write fetch; run queries that write with \
                 World::query_raw_mut",
                id.index()
            ),
        }
    }
}

impl core::error::Error for RawQueryError {}

impl World {
    /// Checks `query`'s ids: each registered and raw, none named twice, and, if
    /// `allow_write` is false, no write fetch. Allocates nothing.
    fn check_raw_query(&self, query: &RawQuery, allow_write: bool) -> Result<(), RawQueryError> {
        for id in query.ids() {
            self.raw_layout(id).map_err(|e| match e {
                crate::RawError::NotRaw(id) => RawQueryError::NotRaw(id),
                // `raw_layout` fails only with these two.
                _ => RawQueryError::UnknownId(id),
            })?;
        }
        for (i, id) in query.ids().enumerate() {
            if query.ids().take(i).any(|earlier| earlier == id) {
                return Err(RawQueryError::Repeated(id));
            }
        }
        match query.fetches.iter().find(|(_, a)| *a == Access::Write) {
            Some((id, _)) if !allow_write => Err(RawQueryError::WriteInReadQuery(*id)),
            _ => Ok(()),
        }
    }

    /// A run of `query` that only reads.
    ///
    /// # Errors
    ///
    /// Returns a [`RawQueryError`], before any table is visited, for an unknown or non-raw id,
    /// an id named twice, or a write fetch (use [`World::query_raw_mut`]).
    #[must_use = "a run does nothing until for_each_table"]
    pub fn query_raw<'q>(&self, query: &'q RawQuery) -> Result<RawRun<'_, 'q>, RawQueryError> {
        self.check_raw_query(query, false)?;
        Ok(RawRun {
            archetypes: self.archetypes(),
            query,
            slots: Vec::with_capacity(query.fetches.len()),
        })
    }

    /// A run of `query` that may write.
    ///
    /// # Errors
    ///
    /// Returns a [`RawQueryError`], before any table is visited, for an unknown or non-raw id
    /// or an id named twice.
    #[must_use = "a run does nothing until for_each_table"]
    pub fn query_raw_mut<'q>(
        &mut self,
        query: &'q RawQuery,
    ) -> Result<RawRunMut<'_, 'q>, RawQueryError> {
        self.check_raw_query(query, true)?;
        Ok(RawRunMut {
            archetypes: self.archetypes_mut(),
            query,
            slots: Vec::with_capacity(query.fetches.len()),
        })
    }
}

/// Whether column `index` of an archetype with these `components` is one of `query`'s fetches.
/// Only fetched columns are downcast, so an unfetched typed column is never touched.
fn is_fetched(components: &[ComponentId], query: &RawQuery, index: usize) -> bool {
    let id = components[index];
    query.fetches.iter().any(|(fetched, _)| *fetched == id)
}

/// One fetch's column for the table being visited.
#[derive(Debug)]
enum Slot<'w> {
    /// Not filled yet; never seen by a workload.
    Vacant,
    Read(RawColumn<'w>),
    /// `None` once taken.
    Write(Option<RawColumnMut<'w>>),
}

/// A read-only run of a [`RawQuery`], from [`World::query_raw`].
#[derive(Debug)]
pub struct RawRun<'w, 'q> {
    archetypes: &'w [Archetype],
    query: &'q RawQuery,
    slots: Vec<Slot<'w>>,
}

impl<'w> RawRun<'w, '_> {
    /// Calls `f` once for each matching archetype that has rows, in creation order.
    pub fn for_each_table(self, mut f: impl FnMut(RawTable<'_, 'w>)) {
        let Self {
            archetypes,
            query,
            mut slots,
        } = self;
        for archetype in archetypes {
            if archetype.table.len() == 0 || !query.matches(archetype) {
                continue;
            }
            let (entities, columns) = archetype
                .table
                .raw_columns(|i| is_fetched(&archetype.components, query, i));
            slots.clear();
            slots.resize_with(query.fetches.len(), || Slot::Vacant);
            for (index, view) in columns {
                let id = archetype.components[index];
                // Each column belongs to at most one fetch, since a query names an id once.
                if let Some(k) = query.fetches.iter().position(|(fetched, _)| *fetched == id) {
                    slots[k] = Slot::Read(view);
                }
            }
            f(RawTable {
                entities,
                slots: &mut slots,
            });
        }
    }

    /// The ids of the archetypes the run matches, rows or not, for tests.
    #[cfg(test)]
    pub(crate) fn matched_archetypes(&self) -> Vec<crate::archetype::ArchetypeId> {
        matched(self.archetypes, self.query)
    }
}

/// The ids of the archetypes `query` matches, in creation order.
#[cfg(test)]
fn matched(archetypes: &[Archetype], query: &RawQuery) -> Vec<crate::archetype::ArchetypeId> {
    archetypes
        .iter()
        .enumerate()
        .filter(|(_, a)| query.matches(a))
        .map(|(i, _)| crate::archetype::ArchetypeId::new(i))
        .collect()
}

/// A run of a [`RawQuery`] that may write, from [`World::query_raw_mut`].
#[derive(Debug)]
pub struct RawRunMut<'w, 'q> {
    archetypes: &'w mut [Archetype],
    query: &'q RawQuery,
    slots: Vec<Slot<'w>>,
}

impl<'w> RawRunMut<'w, '_> {
    /// The ids of the archetypes the run matches, rows or not, for tests.
    #[cfg(test)]
    pub(crate) fn matched_archetypes(&self) -> Vec<crate::archetype::ArchetypeId> {
        matched(self.archetypes, self.query)
    }

    /// Calls `f` once for each matching archetype that has rows, in creation order.
    pub fn for_each_table(self, mut f: impl FnMut(RawTable<'_, 'w>)) {
        let Self {
            archetypes,
            query,
            mut slots,
        } = self;
        for archetype in archetypes {
            if archetype.table.len() == 0 || !query.matches(archetype) {
                continue;
            }
            let components = &archetype.components;
            let (entities, columns) = archetype
                .table
                .raw_columns_mut(|i| is_fetched(components, query, i));
            slots.clear();
            slots.resize_with(query.fetches.len(), || Slot::Vacant);
            // Each column belongs to at most one fetch, since a query names an id once.
            for (index, view) in columns {
                let id = components[index];
                if let Some(k) = query.fetches.iter().position(|(fetched, _)| *fetched == id) {
                    slots[k] = match query.fetches[k].1 {
                        Access::Read => Slot::Read(view.into_shared()),
                        Access::Write => Slot::Write(Some(view)),
                    };
                }
            }
            f(RawTable {
                entities,
                slots: &mut slots,
            });
        }
    }
}

/// One matching table: its entities and the columns of the query's fetches.
///
/// The views [`column`](Self::column) and [`column_mut`](Self::column_mut) return live as long
/// as the table borrow `'w`, not the call, so a workload holds several at once.
#[derive(Debug)]
pub struct RawTable<'a, 'w> {
    entities: &'w [Entity],
    slots: &'a mut [Slot<'w>],
}

impl<'w> RawTable<'_, 'w> {
    /// The entities of the table's rows, in row order.
    #[must_use]
    pub fn entities(&self) -> &'w [Entity] {
        self.entities
    }

    /// The number of rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Whether the table has no rows. A run never visits such a table.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// The column of read fetch `k`.
    ///
    /// # Panics
    ///
    /// Panics if the query has no fetch `k` or fetch `k` is a write fetch; take that one with
    /// [`column_mut`](Self::column_mut). Either is a bug in the workload.
    #[track_caller]
    #[must_use]
    pub fn column(&self, k: usize) -> RawColumn<'w> {
        match self.slots.get(k) {
            Some(Slot::Read(column)) => *column,
            Some(Slot::Write(_)) => {
                panic!("RawTable::column: fetch {k} is a write fetch; take it with column_mut")
            }
            Some(Slot::Vacant) => unreachable!("a matched archetype has a column for every fetch"),
            None => panic!(
                "RawTable::column: fetch {k} does not exist; the query has {} fetches",
                self.slots.len()
            ),
        }
    }

    /// The column of write fetch `k`, once for this table.
    ///
    /// # Panics
    ///
    /// Panics if the query has no fetch `k`, fetch `k` is a read fetch (take it with
    /// [`column`](Self::column)), or it was already taken for this table. Each is a bug in the
    /// workload.
    #[track_caller]
    #[must_use]
    pub fn column_mut(&mut self, k: usize) -> RawColumnMut<'w> {
        let fetches = self.slots.len();
        match self.slots.get_mut(k) {
            Some(Slot::Write(taken)) => taken.take().unwrap_or_else(|| {
                panic!("RawTable::column_mut: fetch {k} was already taken for this table")
            }),
            Some(Slot::Read(_)) => {
                panic!("RawTable::column_mut: fetch {k} is a read fetch; take it with column")
            }
            Some(Slot::Vacant) => unreachable!("a matched archetype has a column for every fetch"),
            None => panic!(
                "RawTable::column_mut: fetch {k} does not exist; the query has {fetches} fetches"
            ),
        }
    }
}
