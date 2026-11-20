//! The `simple_iter` workload on the sparse-set `World`, ungrouped and with an owning group.

use engine::prelude::Prng;
use spike_ecs_harness::simple_iter::{
    self as rules, Matrix, Position, Rotation, SimpleIter, Velocity,
};
use spike_ecs_harness::{Params, Workload};

use crate::World;

/// `simple_iter` on a [`World`]; `GROUPED` declares the `(Position, Velocity)` owning group.
#[derive(Debug)]
pub struct SimpleIterOn<const GROUPED: bool> {
    world: World,
}

/// `simple_iter` on a sparse-set `World` without groups.
pub type SparseSetSimpleIter = SimpleIterOn<false>;
/// `simple_iter` on a sparse-set `World` with the `(Position, Velocity)` group.
pub type GroupedSimpleIter = SimpleIterOn<true>;

#[cfg(test)]
impl<const GROUPED: bool> SimpleIterOn<GROUPED> {
    /// Whether the step's query takes the group path, for tests.
    pub(crate) fn uses_group(&self) -> bool {
        self.world.query::<(&Position, &Velocity)>().uses_group()
    }
}

impl<const GROUPED: bool> Workload for SimpleIterOn<GROUPED> {
    type Spec = SimpleIter;
    const BACKEND: &'static str = if GROUPED {
        "sparse_set_grouped"
    } else {
        "sparse_set"
    };

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = if GROUPED {
            World::builder().group::<(Position, Velocity)>().build()
        } else {
            World::new()
        };
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
        self.world
            .query_mut::<(&mut Position, &Velocity)>()
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
