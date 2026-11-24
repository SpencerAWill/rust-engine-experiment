//! [`Lookup`]: a read-only view of one component type by entity handle (M3.21 spec §4.6).

use core::fmt;

use super::World;
use crate::Entity;
use crate::table::column_slice;

impl World {
    /// A read-only view of every entity's `T`, with `T` resolved once.
    ///
    /// The type's id and, for each archetype, the column that holds it are looked up here, so
    /// [`Lookup::get`] does neither. A `T` that was never registered gives a view that finds
    /// nothing.
    ///
    /// ```
    /// use spike_ecs_archetype::World;
    ///
    /// struct Hp(u32);
    ///
    /// let mut world = World::new();
    /// let entity = world.spawn_with((Hp(7),));
    /// let hp = world.lookup::<Hp>();
    /// assert_eq!(hp.get(entity).map(|h| h.0), Some(7));
    /// ```
    #[must_use]
    pub fn lookup<T: 'static>(&self) -> Lookup<'_, T> {
        let columns = match self.component_id::<T>() {
            None => Vec::new(),
            Some(id) => self
                .archetypes()
                .iter()
                .map(|archetype| {
                    archetype
                        .column_of(id)
                        .map(|col| column_slice::<T>(archetype.table.column(col)))
                })
                .collect(),
        };
        Lookup {
            world: self,
            columns,
        }
    }
}

/// Reads one component type by entity handle; made by [`World::lookup`].
///
/// It borrows the [`World`], so no archetype is created while it lives. Unlike a `World`
/// lookup it is `Clone` but not `Copy`: it owns the per-archetype column cache.
pub struct Lookup<'w, T> {
    world: &'w World,
    /// The slice of `T`s in each archetype's table, indexed by archetype; `None` where the
    /// archetype has no `T`. Empty if `T` was never registered.
    columns: Vec<Option<&'w [T]>>,
}

impl<T> Clone for Lookup<'_, T> {
    fn clone(&self) -> Self {
        Self {
            world: self.world,
            columns: self.columns.clone(),
        }
    }
}

impl<T> fmt::Debug for Lookup<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Lookup")
            .field("archetypes", &self.columns.len())
            .finish_non_exhaustive()
    }
}

impl<'w, T: 'static> Lookup<'w, T> {
    /// `entity`'s `T`, or `None` if the entity is despawned (including a stale handle whose
    /// index is reused) or has no `T`.
    #[must_use]
    pub fn get(&self, entity: Entity) -> Option<&'w T> {
        if !self.world.entities.is_alive(entity) {
            return None;
        }
        let location = self.world.location_of(entity);
        let column = (*self.columns.get(location.archetype.index())?)?;
        column.get(location.row as usize)
    }
}
