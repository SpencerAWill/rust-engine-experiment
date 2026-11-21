//! Bundles: a flat tuple of component values, of either storage kind, that `World::spawn_with`
//! places in one go: table elements as one row of the entity's final table, sparse elements
//! each into its set. Adapted from the archetype prototype's bundles (M3.4).

use core::any::TypeId;

use crate::Entity;
use crate::archetype::{ArchetypeId, ComponentId};
use crate::component::{Component, StorageKind, seal as storage_seal};
use crate::sparse::{ErasedSparseSet, SparseSet};
use crate::table::RowWriter;
use crate::world::World;

/// Implementation details of the sealed trait; unnameable outside the crate.
pub(crate) mod seal {
    use core::any::TypeId;

    use crate::Entity;
    use crate::archetype::ComponentId;
    use crate::component::StorageKind;
    use crate::sparse::ErasedSparseSet;
    use crate::table::RowWriter;
    use crate::world::World;

    /// What the world needs from a bundle.
    pub trait Bundle {
        /// The `TypeId` and name of each element, in tuple order.
        fn types() -> Vec<(TypeId, &'static str)>;
        /// Registers each element type, and returns the id and kind of each in tuple order.
        fn register(world: &mut World) -> Vec<(ComponentId, StorageKind)>;
        /// Stores each element: a table element through `writer` onto its column
        /// (`columns[k]` is the column of the `k`-th table element), a sparse element into its
        /// set (`sparse[k]` indexes `sets` for the `k`-th sparse element).
        fn write(
            self,
            entity: Entity,
            writer: &mut RowWriter<'_>,
            columns: &[usize],
            sets: &mut [Box<dyn ErasedSparseSet>],
            sparse: &[usize],
        );
    }
}

/// A tuple of 1 to 8 [`Component`] values, of either storage kind, for [`World::spawn_with`].
/// Sealed: it is implemented for flat tuples of components only. Bundles do not nest: in
/// `(A, (B, C))` the second element is one component of the tuple type `(B, C)`, if that is a
/// component at all.
pub trait Bundle: seal::Bundle + 'static {}

/// What the world caches for one bundle type (invariant B1).
#[derive(Debug)]
pub(crate) struct BundleInfo {
    /// The archetype with exactly the bundle's table element types.
    pub(crate) archetype: ArchetypeId,
    /// `columns[k]` is the column, in that archetype's table, of the `k`-th table element.
    pub(crate) columns: Box<[usize]>,
    /// `sparse[k]` is the set of the `k`-th sparse element.
    pub(crate) sparse: Box<[usize]>,
    /// All element types in tuple order, kept so `check_invariants` can verify the rest.
    pub(crate) types: Box<[TypeId]>,
}

/// The ids of `ids`, ascending. Used to find the bundle's archetype.
pub(crate) fn sorted(ids: &[ComponentId]) -> Box<[ComponentId]> {
    let mut v = ids.to_vec();
    v.sort_unstable();
    v.into_boxed_slice()
}

/// Stores one element; `columns` and `sparse` are read at the running counters `c` and `s`.
fn write_one<T: Component>(
    value: T,
    entity: Entity,
    writer: &mut RowWriter<'_>,
    columns: &[usize],
    sets: &mut [Box<dyn ErasedSparseSet>],
    sparse: &[usize],
    (c, s): (&mut usize, &mut usize),
) {
    match <T::Storage as storage_seal::Storage>::KIND {
        StorageKind::Table => {
            writer.push(columns[*c], value);
            *c += 1;
        }
        StorageKind::Sparse => {
            sets[sparse[*s]]
                .as_any_mut()
                .downcast_mut::<SparseSet<T>>()
                .expect("S4: a set holds the type its component id names")
                .insert(entity, value);
            *s += 1;
        }
    }
}

macro_rules! impl_bundle {
    ($($t:ident $i:tt),+) => {
        impl<$($t: Component),+> seal::Bundle for ($($t,)+) {
            fn types() -> Vec<(TypeId, &'static str)> {
                vec![$((TypeId::of::<$t>(), core::any::type_name::<$t>())),+]
            }
            fn register(world: &mut World) -> Vec<(ComponentId, StorageKind)> {
                vec![$((
                    world.register::<$t>(),
                    <$t::Storage as storage_seal::Storage>::KIND,
                )),+]
            }
            fn write(
                self,
                entity: Entity,
                writer: &mut RowWriter<'_>,
                columns: &[usize],
                sets: &mut [Box<dyn ErasedSparseSet>],
                sparse: &[usize],
            ) {
                let (mut c, mut s) = (0, 0);
                $(write_one(self.$i, entity, writer, columns, sets, sparse, (&mut c, &mut s));)+
            }
        }
        impl<$($t: Component),+> Bundle for ($($t,)+) {}
    };
}

impl_bundle!(A 0);
impl_bundle!(A 0, B 1);
impl_bundle!(A 0, B 1, C 2);
impl_bundle!(A 0, B 1, C 2, D 3);
impl_bundle!(A 0, B 1, C 2, D 3, E 4);
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5);
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5, G 6);
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7);
