//! The `heavy_compute` workload on a `bevy_ecs` `World`.

use bevy_ecs::query::QueryState;
use bevy_ecs::world::World;
use engine::prelude::Prng;
use spike_ecs_harness::heavy_compute::{self as rules, HeavyCompute, Matrix, Position, Rotation};
use spike_ecs_harness::{Params, Workload};

use crate::Table;

/// The step's query.
type StepQuery = QueryState<(
    &'static mut Table<Matrix>,
    &'static mut Table<Position>,
    &'static Table<Rotation>,
)>;

/// `heavy_compute` on a `bevy_ecs` [`World`]: one table holding every entity.
#[derive(Debug)]
pub struct BevyHeavyCompute {
    world: World,
    step: StepQuery,
}

impl Workload for BevyHeavyCompute {
    type Spec = HeavyCompute;
    const BACKEND: &'static str = "bevy_ecs";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            let (matrix, position, rotation, velocity) = rules::spawn(&prng, i);
            world.spawn((
                Table(matrix),
                Table(position),
                Table(rotation),
                Table(velocity),
            ));
        }
        let step = world.query();
        Self { world, step }
    }

    fn step(&mut self) {
        self.step
            .iter_mut(&mut self.world)
            .for_each(|(mut matrix, mut position, rotation)| {
                rules::update(&mut matrix.0, &mut position.0, &rotation.0);
            });
        self.world.clear_trackers();
    }

    fn checksum(&self) -> u64 {
        let Some(query) = self.world.try_query::<(&Table<Position>, &Table<Matrix>)>() else {
            return 0;
        };
        query
            .iter_manual(&self.world)
            .fold(0_u64, |sum, (position, matrix)| {
                sum.wrapping_add(rules::hash(&position.0, &matrix.0))
            })
    }
}
