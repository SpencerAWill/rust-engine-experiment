//! [`Lookup`]: a read-only view of one component type by entity handle (M3.21 spec §4.6).

use core::fmt;

use engine::genid::GenIdAllocator;

use crate::column::DenseColumn;
use crate::{Entity, EntityMarker, World};

impl World {
    /// A read-only view of every entity's `T`, with `T`'s column resolved once.
    ///
    /// The type's column is looked up here, so [`Lookup::get`] does not. A `T` that was never
    /// inserted gives a view that finds nothing.
    ///
    /// ```
    /// use spike_ecs_dense::World;
    ///
    /// struct Hp(u32);
    ///
    /// let mut world = World::new();
    /// let entity = world.spawn();
    /// world.insert(entity, Hp(7));
    /// let hp = world.lookup::<Hp>();
    /// assert_eq!(hp.get(entity).map(|h| h.0), Some(7));
    /// ```
    #[must_use]
    pub fn lookup<T: 'static>(&self) -> Lookup<'_, T> {
        Lookup {
            allocator: self.allocator(),
            column: self.column::<T>(),
        }
    }
}

/// Reads one component type by entity handle; made by [`World::lookup`].
///
/// It borrows the [`World`], so nothing changes while it lives.
pub struct Lookup<'w, T> {
    /// Tells live handles from stale ones.
    allocator: &'w GenIdAllocator<EntityMarker>,
    /// The column of `T`s, or `None` if `T` was never inserted.
    column: Option<&'w DenseColumn<T>>,
}

impl<T> fmt::Debug for Lookup<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Lookup").finish_non_exhaustive()
    }
}

impl<T> Clone for Lookup<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Lookup<'_, T> {}

impl<'w, T> Lookup<'w, T> {
    /// `entity`'s `T`, or `None` if the entity is despawned (including a stale handle whose
    /// index is reused) or has no `T`.
    ///
    /// A column slot is reused after a despawn, so liveness (the generation) is checked
    /// before the slot is read.
    #[must_use]
    pub fn get(&self, entity: Entity) -> Option<&'w T> {
        if !self.allocator.is_alive(entity) {
            return None;
        }
        self.column?.get(entity.index() as usize)
    }
}
