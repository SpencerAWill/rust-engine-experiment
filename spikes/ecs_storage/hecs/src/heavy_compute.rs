//! The `heavy_compute` workload on a `hecs` `World`.

use engine::prelude::Prng;
use hecs::World;
use spike_ecs_harness::heavy_compute::{self as rules, HeavyCompute, Matrix, Position, Rotation};
use spike_ecs_harness::{Params, Workload};

/// `heavy_compute` on a `hecs` [`World`]: one archetype holding every entity.
pub struct HecsHeavyCompute {
    world: World,
}

impl core::fmt::Debug for HecsHeavyCompute {
    /// `hecs::World` has no `Debug`, so this shows the entity count only.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HecsHeavyCompute")
            .field("entities", &self.world.len())
            .finish_non_exhaustive()
    }
}

impl Workload for HecsHeavyCompute {
    type Spec = HeavyCompute;
    const BACKEND: &'static str = "hecs";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            world.spawn(rules::spawn(&prng, i));
        }
        Self { world }
    }

    fn step(&mut self) {
        self.world
            .query_mut::<(&mut Matrix, &mut Position, &Rotation)>()
            .into_iter()
            .for_each(|(matrix, position, rotation)| rules::update(matrix, position, rotation));
    }

    fn checksum(&self) -> u64 {
        self.world
            .query::<(&Position, &Matrix)>()
            .iter()
            .fold(0_u64, |sum, (position, matrix)| {
                sum.wrapping_add(rules::hash(position, matrix))
            })
    }
}
