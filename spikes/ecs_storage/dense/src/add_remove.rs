//! The `add_remove` workload on the dense `World`.
//!
//! The passes run outside any query, so they use `World::insert` and `World::remove` directly
//! rather than a command buffer.

use engine::prelude::Prng;
use spike_ecs_harness::add_remove::{self as rules, A, AddRemove, B};
use spike_ecs_harness::{Params, Workload};

use crate::{Entity, Query, With, World};

/// `add_remove` on a dense-array [`World`].
#[derive(Debug)]
pub struct DenseAddRemove {
    world: World,
    with_a: Vec<(Entity, f32)>,
    with_b: Vec<Entity>,
}

/// The read pass's query, shared with the test hook so the two cannot drift.
fn read_query(world: &World) -> Query<'_, (Entity, &A)> {
    world.query()
}

/// The removal pass's query, shared with the test hook so the two cannot drift.
fn remove_query(world: &World) -> Query<'_, Entity, ((), With<B>)> {
    world.query().with::<B>()
}

#[cfg(test)]
impl DenseAddRemove {
    /// The world, for tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }

    /// The sets the read pass's query joins (required, excluded), for tests.
    pub(crate) fn read_joined(&self) -> (Vec<crate::query::Joined>, Vec<crate::query::Joined>) {
        read_query(&self.world).joined()
    }

    /// The sets the removal pass's query joins (required, excluded), for tests.
    pub(crate) fn remove_joined(&self) -> (Vec<crate::query::Joined>, Vec<crate::query::Joined>) {
        remove_query(&self.world).joined()
    }
}

impl Workload for DenseAddRemove {
    type Spec = AddRemove;
    const BACKEND: &'static str = "dense";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
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
        // Two separate passes: every insert happens before the first removal.
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
