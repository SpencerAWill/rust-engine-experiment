//! Group declarations (queries come in later steps).

use core::any::TypeId;

use crate::world::{ComponentId, World};

/// Implementation details of the sealed traits; unnameable outside the crate.
pub(crate) mod seal {
    use core::any::TypeId;

    use crate::world::{ComponentId, World};

    /// The member types of a group, and how to create their storages.
    pub trait Group {
        /// The `TypeId` and name of each member, in declaration order.
        fn types() -> Vec<(TypeId, &'static str)>;
        /// Ensures a storage exists for each member and returns their ids in order.
        fn register(world: &mut World) -> Vec<ComponentId>;
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
