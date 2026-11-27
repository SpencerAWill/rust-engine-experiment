//! Bundles: a flat tuple of component values that `World::spawn_with` places straight into the
//! entity's final table.

use core::any::TypeId;

use crate::archetype::{ArchetypeId, ComponentId};
use crate::table::RowWriter;
use crate::world::World;

/// Implementation details of the sealed trait; unnameable outside the crate.
pub(crate) mod seal {
    use core::any::TypeId;

    use crate::archetype::ComponentId;
    use crate::table::RowWriter;
    use crate::world::World;

    /// What the world needs from a bundle.
    pub trait Bundle {
        /// The `TypeId` and name of each element, in tuple order.
        fn types() -> Vec<(TypeId, &'static str)>;
        /// Registers each element type, and returns the ids in tuple order.
        fn register(world: &mut World) -> Vec<ComponentId>;
        /// Pushes each element through `writer` onto its column (`columns[k]` is the column of
        /// the `k`-th element).
        fn write(self, writer: &mut RowWriter<'_>, columns: &[usize]);
    }
}

/// A tuple of 1 to 24 component values, for [`World::spawn_with`]. Sealed: it is implemented for
/// flat tuples of `'static` types only. Bundles do not nest: in `(A, (B, C))` the second element
/// is one component of the tuple type `(B, C)`.
pub trait Bundle: seal::Bundle + 'static {}

/// What the world caches for one bundle type (invariant B1).
#[derive(Debug)]
pub(crate) struct BundleInfo {
    /// The archetype with exactly the bundle's element types.
    pub(crate) archetype: ArchetypeId,
    /// `columns[k]` is the column, in that archetype's table, of the `k`-th element.
    pub(crate) columns: Box<[usize]>,
    /// The element types in tuple order, kept so `check_invariants` can verify the rest.
    pub(crate) types: Box<[TypeId]>,
}

/// The ids of `ids`, ascending. Used to find the bundle's archetype.
pub(crate) fn sorted(ids: &[ComponentId]) -> Box<[ComponentId]> {
    let mut v = ids.to_vec();
    v.sort_unstable();
    v.into_boxed_slice()
}

macro_rules! impl_bundle {
    ($($t:ident $i:tt),+) => {
        impl<$($t: 'static),+> seal::Bundle for ($($t,)+) {
            fn types() -> Vec<(TypeId, &'static str)> {
                vec![$((TypeId::of::<$t>(), core::any::type_name::<$t>())),+]
            }
            fn register(world: &mut World) -> Vec<ComponentId> {
                vec![$(world.register::<$t>()),+]
            }
            fn write(self, writer: &mut RowWriter<'_>, columns: &[usize]) {
                $(writer.push(columns[$i], self.$i);)+
            }
        }
        impl<$($t: 'static),+> Bundle for ($($t,)+) {}
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
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8);
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9);
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10);
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11);
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12);
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13);
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14);
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15);
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15, Q 16);
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15, Q 16, R 17);
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15, Q 16, R 17, S 18);
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15, Q 16, R 17, S 18, T 19);
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15, Q 16, R 17, S 18, T 19, U 20);
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15, Q 16, R 17, S 18, T 19, U 20, V 21);
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15, Q 16, R 17, S 18, T 19, U 20, V 21, W 22);
impl_bundle!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11, M 12, N 13, O 14, P 15, Q 16, R 17, S 18, T 19, U 20, V 21, W 22, X 23);
