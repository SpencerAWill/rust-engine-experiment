//! The `simple_iter` workload on the dense `World`.

use engine::prelude::Prng;
use spike_ecs_harness::simple_iter::{
    self as rules, Matrix, Position, Rotation, SimpleIter, Velocity,
};
use spike_ecs_harness::{Params, Workload};

use crate::{QueryMut, World};

/// `simple_iter` on a dense-array [`World`].
#[derive(Debug)]
pub struct DenseSimpleIter {
    world: World,
}

/// The step's query, shared with the test hook so the two cannot drift.
fn step_query(world: &mut World) -> QueryMut<'_, (&mut Position, &Velocity)> {
    world.query_mut()
}

#[cfg(test)]
impl DenseSimpleIter {
    /// The world, for tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }

    /// The sets the step's query joins (required, excluded), for tests.
    pub(crate) fn step_joined(&mut self) -> (Vec<crate::query::Joined>, Vec<crate::query::Joined>) {
        step_query(&mut self.world).joined()
    }
}

impl Workload for DenseSimpleIter {
    type Spec = SimpleIter;
    const BACKEND: &'static str = "dense";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            let (matrix, position, rotation, velocity) = rules::spawn(&prng, i);
            let e = world.spawn();
            world.insert::<Matrix>(e, matrix);
            world.insert::<Position>(e, position);
            world.insert::<Rotation>(e, rotation);
            world.insert::<Velocity>(e, velocity);
        }
        Self { world }
    }

    fn step(&mut self) {
        step_query(&mut self.world)
            .for_each(|(position, velocity)| rules::update(position, velocity));
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        self.world
            .query::<(&Position, &Velocity)>()
            .for_each(|(position, velocity)| {
                sum = sum.wrapping_add(rules::hash(position, velocity));
            });
        sum
    }
}
