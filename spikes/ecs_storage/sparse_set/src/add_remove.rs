//! The `add_remove` workload on the sparse-set `World`, ungrouped and with an owning group.
//!
//! The passes run outside any query, so they use `World::insert` and `World::remove` directly
//! rather than a command buffer (spec section 4.7).

use engine::prelude::Prng;
use spike_ecs_harness::add_remove::{self as rules, A, AddRemove, B};
use spike_ecs_harness::{Params, Workload};

use crate::{Entity, Query, With, World};

/// `add_remove` on a [`World`]; `GROUPED` declares the `(A, B)` owning group.
#[derive(Debug)]
pub struct AddRemoveOn<const GROUPED: bool> {
    world: World,
    with_a: Vec<(Entity, f32)>,
    with_b: Vec<Entity>,
}

/// `add_remove` on a sparse-set `World` without groups.
pub type SparseSetAddRemove = AddRemoveOn<false>;
/// `add_remove` on a sparse-set `World` with the `(A, B)` group.
pub type GroupedAddRemove = AddRemoveOn<true>;

/// The read pass's query, shared with the test hook so the two cannot drift.
fn read_query(world: &World) -> Query<'_, (Entity, &A)> {
    world.query()
}

/// The remove pass's query, shared with the test hook so the two cannot drift.
fn remove_query(world: &World) -> Query<'_, Entity, ((), With<B>)> {
    world.query().with::<B>()
}

#[cfg(test)]
impl<const GROUPED: bool> AddRemoveOn<GROUPED> {
    /// Whether the read pass's query takes the group path, for tests.
    pub(crate) fn uses_group_for_read_pass(&self) -> bool {
        read_query(&self.world).uses_group()
    }

    /// Whether the removal pass's query takes the group path, for tests.
    pub(crate) fn uses_group_for_remove_pass(&self) -> bool {
        remove_query(&self.world).uses_group()
    }

    /// The entities the last step removed `B` from, for tests.
    pub(crate) fn removed_last_step(&self) -> &[Entity] {
        &self.with_b
    }
}

impl<const GROUPED: bool> Workload for AddRemoveOn<GROUPED> {
    type Spec = AddRemove;
    const BACKEND: &'static str = if GROUPED {
        "sparse_set_grouped"
    } else {
        "sparse_set"
    };

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = if GROUPED {
            World::builder().group::<(A, B)>().build()
        } else {
            World::new()
        };
        for i in 0..params.count as u64 {
            let e = world.spawn();
            world.insert::<A>(e, rules::spawn(&prng, i));
        }
        Self {
            world,
            with_a: Vec::with_capacity(params.count),
            with_b: Vec::with_capacity(params.count),
        }
    }

    fn step(&mut self) {
        // Two separate passes: every insert happens before the first removal (spec 4.4.4).
        self.with_a.clear();
        let with_a = &mut self.with_a;
        read_query(&self.world).for_each(|(e, a)| with_a.push((e, a.0)));
        for &(e, a) in &self.with_a {
            self.world.insert::<B>(e, rules::make_b(&A(a)));
        }

        self.with_b.clear();
        let with_b = &mut self.with_b;
        remove_query(&self.world).for_each(|e| with_b.push(e));
        for &e in &self.with_b {
            self.world.remove::<B>(e);
        }
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        self.world
            .query::<(&A, Option<&B>)>()
            .for_each(|(a, b)| sum = sum.wrapping_add(rules::hash(a, b)));
        sum
    }
}
