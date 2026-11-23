//! The `simple_iter` workload on a `bevy_ecs` `World`.

use bevy_ecs::query::QueryState;
use bevy_ecs::world::World;
use engine::prelude::Prng;
use spike_ecs_harness::simple_iter::{self as rules, Position, SimpleIter, Velocity};
use spike_ecs_harness::{Params, Workload};

use crate::Table;

/// The step's query.
type StepQuery = QueryState<(&'static mut Table<Position>, &'static Table<Velocity>)>;

/// `simple_iter` on a `bevy_ecs` [`World`]: one table holding every entity.
#[derive(Debug)]
pub struct BevySimpleIter {
    world: World,
    step: StepQuery,
}

impl Workload for BevySimpleIter {
    type Spec = SimpleIter;
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
        for (mut position, velocity) in self.step.iter_mut(&mut self.world) {
            rules::update(&mut position.0, &velocity.0);
        }
        self.world.clear_trackers();
    }

    fn checksum(&self) -> u64 {
        let Some(query) = self
            .world
            .try_query::<(&Table<Position>, &Table<Velocity>)>()
        else {
            return 0;
        };
        query
            .iter_manual(&self.world)
            .fold(0_u64, |sum, (position, velocity)| {
                sum.wrapping_add(rules::hash(&position.0, &velocity.0))
            })
    }
}
