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

    /// What a storage kind does for a component type. One call per operation, so the kind is
    /// chosen at compile time and nothing per entity branches on it.
    pub trait Storage: 'static {
        /// The kind this marker stands for.
        const KIND: StorageKind;
        /// `World::insert` for a live `entity`.
        fn insert<T: Component>(world: &mut World, entity: Entity, value: T) -> Option<T>;
        /// `World::remove`.
        fn remove<T: Component>(world: &mut World, entity: Entity) -> Option<T>;
        /// `World::get`.
        fn get<T: Component>(world: &World, entity: Entity) -> Option<&T>;
        /// `World::get_mut`.
        fn get_mut<T: Component>(world: &mut World, entity: Entity) -> Option<&mut T>;
    }

    impl Storage for TableStorage {
        const KIND: StorageKind = StorageKind::Table;

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
    }

    impl Storage for SparseStorage {
        const KIND: StorageKind = StorageKind::Sparse;

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
    }
}

/// Writes one `impl Component` per type, all with the same storage marker:
/// `components!(TableStorage: Position, Velocity);`.
#[cfg(test)]
macro_rules! components {
    ($storage:ident: $($t:ty),+ $(,)?) => {
        $( impl $crate::Component for $t { type Storage = $crate::$storage; } )+
    };
}
#[cfg(test)]
pub(crate) use components;
