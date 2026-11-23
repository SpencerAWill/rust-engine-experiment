//! The `add_remove` workload on runtime-registered components in byte columns, through the two
//! access paths of the M3.6 spec §4.4: [`BytesCopyAddRemove`] decodes and encodes each value
//! through its [`Codec`], [`BytesCastAddRemove`] casts each column to `[f32]` (`A` and `B` are
//! `f32` newtypes, so no mirror type is needed).
//!
//! The passes mirror [`crate::add_remove::ArchetypeAddRemove`]: the same two collect-then-apply
//! passes, outside any query, with `insert_raw` and `remove_raw` in place of `insert` and
//! `remove` (spec section 4.7).

use engine::prelude::Prng;
use spike_ecs_harness::add_remove::{self as rules, A, AddRemove, B};
use spike_ecs_harness::{Params, Workload};

use crate::codec::{Codec, f32_codec};
use crate::{ComponentId, Entity, RawQuery, World};

f32_codec!(A, B);

/// The read pass's query, shared with the test hook so the two cannot drift.
fn read_query(a: ComponentId) -> RawQuery {
    RawQuery::new().read(a)
}

/// The remove pass's query, shared with the test hook so the two cannot drift.
fn remove_query(b: ComponentId) -> RawQuery {
    RawQuery::new().with(b)
}

/// The checksum's first query: `A` without `B`.
fn checksum_without_query(a: ComponentId, b: ComponentId) -> RawQuery {
    RawQuery::new().read(a).without(b)
}

/// The checksum's second query: `A` and `B`.
fn checksum_with_query(a: ComponentId, b: ComponentId) -> RawQuery {
    RawQuery::new().read(a).read(b)
}

/// The world, the two component ids, the four queries and the two scratch lists both backends
/// share.
#[derive(Debug)]
struct Base {
    world: World,
    b: ComponentId,
    read: RawQuery,
    remove: RawQuery,
    checksum_without: RawQuery,
    checksum_with: RawQuery,
    with_a: Vec<(Entity, f32)>,
    with_b: Vec<Entity>,
}

impl Base {
    /// Registers `A` and `B` and spawns entities `0..count`, each with its `A`'s bytes from
    /// `bytes`.
    fn new(params: Params, bytes: impl Fn(A) -> [u8; 4]) -> Self {
        let mut world = World::new();
        let a = world.register_raw("A", A::LAYOUT);
        let b = world.register_raw("B", B::LAYOUT);
        let prng = Prng::from_seed(params.seed);
        for i in 0..params.count as u64 {
            let value = bytes(rules::spawn(&prng, i));
            world
                .spawn_raw(&[(a, &value)])
                .expect("the id is registered raw and listed once, with its layout's size");
        }
        Self {
            world,
            b,
            read: read_query(a),
            remove: remove_query(b),
            checksum_without: checksum_without_query(a, b),
            checksum_with: checksum_with_query(a, b),
            with_a: Vec::with_capacity(params.count),
            with_b: Vec::with_capacity(params.count),
        }
    }

    /// The remove pass: collects the entities with `B`, then removes it from each.
    fn remove_pass(&mut self) {
        self.with_b.clear();
        let with_b = &mut self.with_b;
        self.world
            .query_raw(&self.remove)
            .expect("the remove pass's id is registered raw")
            .for_each_table(|table| with_b.extend_from_slice(table.entities()));
        for &e in &self.with_b {
            self.world
                .remove_raw(e, self.b)
                .expect("the entity is alive and B is registered raw");
        }
    }

    /// The insert pass: inserts a `B` made from each collected `A`, with `bytes` encoding it.
    /// Every insert happens before the first removal (spec 4.4.4).
    fn insert_pass(&mut self, bytes: impl Fn(B) -> [u8; 4]) {
        for &(e, a) in &self.with_a {
            self.world
                .insert_raw(e, self.b, &bytes(rules::make_b(&A(a))))
                .expect("the entity is alive and B is registered raw, with its layout's size");
        }
    }
}

/// `add_remove` on byte columns, copying each value in and out through its [`Codec`].
#[derive(Debug)]
pub struct BytesCopyAddRemove {
    base: Base,
}

/// `add_remove` on byte columns, casting each column to `[f32]`.
#[derive(Debug)]
pub struct BytesCastAddRemove {
    base: Base,
}

/// Encodes a value into a buffer of its layout's size, which every codec here makes 4 bytes.
fn encoded<T: Codec>(value: &T) -> [u8; 4] {
    let mut bytes = [0; 4];
    value.encode(&mut bytes);
    bytes
}

impl Workload for BytesCopyAddRemove {
    type Spec = AddRemove;
    const BACKEND: &'static str = "archetype_bytes_copy";

    fn setup(params: Params) -> Self {
        Self {
            base: Base::new(params, |a| encoded(&a)),
        }
    }

    fn step(&mut self) {
        let base = &mut self.base;
        base.with_a.clear();
        let with_a = &mut base.with_a;
        base.world
            .query_raw(&base.read)
            .expect("the read pass's id is registered raw")
            .for_each_table(|table| {
                let entities = table.entities();
                for (&e, row) in entities.iter().zip(table.column(0).rows()) {
                    with_a.push((e, A::decode(row).0));
                }
            });
        base.insert_pass(|b| encoded(&b));
        base.remove_pass();
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        let base = &self.base;
        base.world
            .query_raw(&base.checksum_without)
            .expect("the checksum's ids are registered raw")
            .for_each_table(|table| {
                for row in table.column(0).rows() {
                    sum = sum.wrapping_add(rules::hash(&A::decode(row), None));
                }
            });
        base.world
            .query_raw(&base.checksum_with)
            .expect("the checksum's ids are registered raw")
            .for_each_table(|table| {
                let (a, b) = (table.column(0), table.column(1));
                for (a, b) in a.rows().zip(b.rows()) {
                    sum = sum.wrapping_add(rules::hash(&A::decode(a), Some(&B::decode(b))));
                }
            });
        sum
    }
}

impl Workload for BytesCastAddRemove {
    type Spec = AddRemove;
    const BACKEND: &'static str = "archetype_bytes_cast";

    fn setup(params: Params) -> Self {
        Self {
            base: Base::new(params, |a| a.0.to_ne_bytes()),
        }
    }

    fn step(&mut self) {
        let base = &mut self.base;
        base.with_a.clear();
        let with_a = &mut base.with_a;
        base.world
            .query_raw(&base.read)
            .expect("the read pass's id is registered raw")
            .for_each_table(|table| {
                let values = table
                    .column(0)
                    .cast::<f32>()
                    .expect("A's layout matches f32");
                with_a.extend(table.entities().iter().copied().zip(values.iter().copied()));
            });
        base.insert_pass(|b| b.0.to_ne_bytes());
        base.remove_pass();
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        let base = &self.base;
        base.world
            .query_raw(&base.checksum_without)
            .expect("the checksum's ids are registered raw")
            .for_each_table(|table| {
                let a = table
                    .column(0)
                    .cast::<f32>()
                    .expect("A's layout matches f32");
                for &a in a {
                    sum = sum.wrapping_add(rules::hash(&A(a), None));
                }
            });
        base.world
            .query_raw(&base.checksum_with)
            .expect("the checksum's ids are registered raw")
            .for_each_table(|table| {
                let a = table
                    .column(0)
                    .cast::<f32>()
                    .expect("A's layout matches f32");
                let b = table
                    .column(1)
                    .cast::<f32>()
                    .expect("B's layout matches f32");
                for (&a, &b) in a.iter().zip(b) {
                    sum = sum.wrapping_add(rules::hash(&A(a), Some(&B(b))));
                }
            });
        sum
    }
}

/// The two backends' hooks for tests: the same query functions the passes use (ecs-1).
macro_rules! test_hooks {
    ($($backend:ty),*) => {$(
        #[cfg(test)]
        impl $backend {
            /// The archetypes the read pass's query matches, for tests.
            pub(crate) fn read_archetypes(&self) -> Vec<crate::archetype::ArchetypeId> {
                self.base
                    .world
                    .query_raw(&self.base.read)
                    .expect("the read pass's id is registered raw")
                    .matched_archetypes()
            }

            /// The archetypes the removal pass's query matches, for tests.
            pub(crate) fn remove_archetypes(&self) -> Vec<crate::archetype::ArchetypeId> {
                self.base
                    .world
                    .query_raw(&self.base.remove)
                    .expect("the remove pass's id is registered raw")
                    .matched_archetypes()
            }

            /// The number of archetypes in the world, for tests.
            pub(crate) fn archetype_count(&self) -> usize {
                self.base.world.archetype_count()
            }

            /// The world, for invariant checks in tests.
            pub(crate) fn world(&self) -> &World {
                &self.base.world
            }
        }
    )*};
}

test_hooks!(BytesCopyAddRemove, BytesCastAddRemove);
