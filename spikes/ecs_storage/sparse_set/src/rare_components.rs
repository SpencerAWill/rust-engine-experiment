//! The `rare_components` workload on the sparse-set `World`, ungrouped.
//!
//! Each entity is spawned empty, given `Common` by `insert`, and then `Rare` for the rare ones;
//! there is no group. The step's query drives from the smaller `Rare` set, so it visits only the
//! rare entities.

use engine::prelude::Prng;
use spike_ecs_harness::rare_components::{self as rules, Common, Rare, RareComponents};
use spike_ecs_harness::{Params, Workload};

use crate::{QueryMut, World};

/// `rare_components` on a sparse-set [`World`]: `Rare` lives in a sparse set of its own, so the
/// step's query walks only the rare entities.
#[derive(Debug)]
pub struct SparseSetRareComponents {
    world: World,
}

/// The step's query.
fn step_query(world: &mut World) -> QueryMut<'_, (&mut Common, &Rare)> {
    world.query_mut()
}

impl Workload for SparseSetRareComponents {
    type Spec = RareComponents;
    const BACKEND: &'static str = "sparse_set";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            let (common, rare) = rules::spawn(&prng, i);
            let e = world.spawn();
            world.insert(e, common);
            if let Some(rare) = rare {
                world.insert(e, rare);
            }
        }
        Self { world }
    }

    fn step(&mut self) {
        step_query(&mut self.world).for_each(|(common, rare)| rules::update(common, rare));
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        self.world
            .query::<(&Common, Option<&Rare>)>()
            .for_each(|(common, rare)| sum = sum.wrapping_add(rules::hash(common, rare)));
        sum
    }
}
