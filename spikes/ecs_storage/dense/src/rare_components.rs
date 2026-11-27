//! The `rare_components` workload on the dense `World`.
//!
//! Each entity is spawned empty, given `Common` by `insert`, and then `Rare` for the rare ones.
//! The step's query joins the `Common` and `Rare` presence sets (`HierarchicalBitSet`s). It yields
//! only the rare entities, but descends only where both sets have a bit, so it skips just the
//! regions with no `Rare`: at 1% about half the 64-entity leaf words still hold one.

use engine::prelude::Prng;
use spike_ecs_harness::rare_components::{self as rules, Common, Rare, RareComponents};
use spike_ecs_harness::{Params, Workload};

use crate::{QueryMut, World};

/// `rare_components` on a dense-array [`World`]: `Rare` has an array and presence set of its own;
/// the module docs say what the step's join skips.
#[derive(Debug)]
pub struct DenseRareComponents {
    world: World,
}

/// The step's query.
fn step_query(world: &mut World) -> QueryMut<'_, (&mut Common, &Rare)> {
    world.query_mut()
}

impl Workload for DenseRareComponents {
    type Spec = RareComponents;
    const BACKEND: &'static str = "dense";

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
