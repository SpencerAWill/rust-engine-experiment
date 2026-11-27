//! The `heavy_compute` workload on the sparse-set `World`, ungrouped and with an owning group.

use engine::prelude::Prng;
use spike_ecs_harness::heavy_compute::{
    self as rules, HeavyCompute, Matrix, Position, Rotation, Velocity,
};
use spike_ecs_harness::{Params, Workload};

use crate::{QueryMut, World};

/// `heavy_compute` on a [`World`]; `GROUPED` declares the `(Matrix, Position, Rotation, Velocity)` group.
#[derive(Debug)]
pub struct HeavyComputeOn<const GROUPED: bool> {
    world: World,
}

/// `heavy_compute` on a sparse-set `World` without groups.
pub type SparseSetHeavyCompute = HeavyComputeOn<false>;
/// `heavy_compute` on a sparse-set `World` with the `(Matrix, Position, Rotation, Velocity)` group.
pub type GroupedHeavyCompute = HeavyComputeOn<true>;

/// The step's query, shared with the test hook so the two cannot drift.
fn step_query(
    world: &mut World,
) -> QueryMut<'_, (&mut Matrix, &mut Position, &Rotation, &Velocity)> {
    world.query_mut()
}

#[cfg(test)]
impl<const GROUPED: bool> HeavyComputeOn<GROUPED> {
    /// Whether the step's query takes the group path, for tests.
    pub(crate) fn uses_group(&mut self) -> bool {
        step_query(&mut self.world).uses_group()
    }
}

impl<const GROUPED: bool> Workload for HeavyComputeOn<GROUPED> {
    type Spec = HeavyCompute;
    const BACKEND: &'static str = if GROUPED {
        "sparse_set_grouped"
    } else {
        "sparse_set"
    };

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = if GROUPED {
            World::builder()
                .group::<(Matrix, Position, Rotation, Velocity)>()
                .build()
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
        step_query(&mut self.world).for_each(|(matrix, position, rotation, velocity)| {
            rules::update(matrix, position, rotation, velocity);
        });
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        self.world
            .query::<(&Position, &Matrix)>()
            .for_each(|(position, matrix)| {
                sum = sum.wrapping_add(rules::hash(position, matrix));
            });
        sum
    }
}
