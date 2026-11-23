//! The `add_remove` workload on a `hecs` `World`.
//!
//! The passes run outside any query iteration, so they use `World::insert_one` and
//! `World::remove_one` directly.

use engine::prelude::Prng;
use hecs::{Entity, World};
use spike_ecs_harness::add_remove::{self as rules, A, AddRemove, B};
use spike_ecs_harness::{Params, Workload};

/// `add_remove` on a `hecs` [`World`]: every tick moves each entity from `(A)` to `(A, B)` and
/// back, a table row move each way.
pub struct HecsAddRemove {
    world: World,
    with_a: Vec<(Entity, A)>,
    with_b: Vec<Entity>,
}

impl core::fmt::Debug for HecsAddRemove {
    /// `hecs::World` has no `Debug`, so this shows the entity count only.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HecsAddRemove")
            .field("entities", &self.world.len())
            .finish_non_exhaustive()
    }
}

impl Workload for HecsAddRemove {
    type Spec = AddRemove;
    const BACKEND: &'static str = "hecs";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            world.spawn((rules::spawn(&prng, i),));
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
        self.world
            .query_mut::<(Entity, &A)>()
            .into_iter()
            .for_each(|(e, a)| with_a.push((e, *a)));
        for &(e, a) in &self.with_a {
            self.world
                .insert_one(e, rules::make_b(&a))
                .expect("the entity is alive");
        }

        self.with_b.clear();
        let with_b = &mut self.with_b;
        self.world
            .query_mut::<Entity>()
            .with::<&B>()
            .into_iter()
            .for_each(|e| with_b.push(e));
        for &e in &self.with_b {
            self.world.remove_one::<B>(e).expect("the entity has a B");
        }
    }

    fn checksum(&self) -> u64 {
        self.world
            .query::<(&A, Option<&B>)>()
            .iter()
            .fold(0_u64, |sum, (a, b)| sum.wrapping_add(rules::hash(a, b)))
    }
}
