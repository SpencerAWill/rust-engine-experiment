//! [`Lookup`]: a read-only view of one component type by entity handle (M3.21 spec §4.6).

use core::fmt;

use crate::component::StorageKind;
use crate::sparse::SparseSet;
use crate::table::column_slice;
use crate::{Component, Entity, World};

impl World {
    /// A read-only view of every entity's `T`, with `T` resolved once.
    ///
    /// For a table type the type's id and, for each archetype, the column that holds it are
    /// looked up here; for a sparse type, its set. [`Lookup::get`] then does neither. A `T`
    /// that was never registered gives a view that finds nothing.
    ///
    /// ```
    /// use spike_ecs_hybrid::{Component, TableStorage, World};
    ///
    /// struct Hp(u32);
    /// impl Component for Hp { type Storage = TableStorage; }
    ///
    /// let mut world = World::new();
    /// let entity = world.spawn_with((Hp(7),));
    /// let hp = world.lookup::<Hp>();
    /// assert_eq!(hp.get(entity).map(|h| h.0), Some(7));
    /// ```
    #[must_use]
    pub fn lookup<T: Component>(&self) -> Lookup<'_, T> {
        let source = match self.component_id::<T>() {
            None => Source::Absent,
            Some(id) => match self.component_info(id).kind {
                StorageKind::Table => Source::Table(
                    self.parts()
                        .0
                        .iter()
                        .map(|archetype| {
                            archetype
                                .column_of(id)
                                .map(|col| column_slice::<T>(archetype.table.column(col)))
                        })
                        .collect(),
                ),
                StorageKind::Sparse => Source::Sparse(self.sparse_set::<T>(id)),
            },
        };
        Lookup {
            world: self,
            source,
        }
    }
}

/// Where a [`Lookup`] finds its values, fixed when it is made.
enum Source<'w, T> {
    /// The type was never registered.
    Absent,
    /// A table type: the slice of `T`s in each archetype's table, indexed by archetype; `None`
    /// where the archetype has no `T`.
    Table(Vec<Option<&'w [T]>>),
    /// A sparse type: its set.
    Sparse(&'w SparseSet<T>),
}

/// Reads one component type by entity handle; made by [`World::lookup`].
///
/// It borrows the [`World`], so no archetype is created and no row moves while it lives. It
/// hands out `&T` only.
pub struct Lookup<'w, T> {
    world: &'w World,
    source: Source<'w, T>,
}

impl<T> fmt::Debug for Lookup<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Lookup").finish_non_exhaustive()
    }
}

impl<'w, T> Lookup<'w, T> {
    /// `entity`'s `T`, or `None` if the entity is despawned (including a stale handle whose
    /// index is reused) or has no `T`.
    ///
    /// A table type is found by the entity's location and the cached column of its archetype,
    /// after a liveness check; a sparse set compares the full handle, so it needs none.
    #[must_use]
    pub fn get(&self, entity: Entity) -> Option<&'w T> {
        match &self.source {
            Source::Absent => None,
            Source::Table(columns) => {
                if !self.world.is_alive(entity) {
                    return None;
                }
                let location = self.world.location_of(entity);
                let column = (*columns.get(location.archetype.index())?)?;
                column.get(location.row as usize)
            }
            Source::Sparse(set) => set.get(entity),
        }
    }
}
