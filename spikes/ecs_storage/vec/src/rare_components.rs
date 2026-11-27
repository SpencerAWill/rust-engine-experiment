//! The `rare_components` workload on a `Vec` of structs.

use engine::prelude::Prng;
use spike_ecs_harness::rare_components::{self as rules, Common, Rare, RareComponents};
use spike_ecs_harness::{Params, Workload};

/// `rare_components` with `Rare` as an `Option` field: the reference, not a storage model. The
/// step walks every row and updates those that have a `Rare`.
#[derive(Clone, Debug)]
pub struct VecRareComponents {
    pub(crate) entities: Vec<(Common, Option<Rare>)>,
}

impl Workload for VecRareComponents {
    type Spec = RareComponents;
    const BACKEND: &'static str = "vec_aos";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let entities = (0..params.count as u64)
            .map(|i| rules::spawn(&prng, i))
            .collect();
        Self { entities }
    }

    fn step(&mut self) {
        for (common, rare) in &mut self.entities {
            if let Some(rare) = rare {
                rules::update(common, rare);
            }
        }
    }

    fn checksum(&self) -> u64 {
        self.entities.iter().fold(0, |sum, (common, rare)| {
            sum.wrapping_add(rules::hash(common, rare.as_ref()))
        })
    }
}
