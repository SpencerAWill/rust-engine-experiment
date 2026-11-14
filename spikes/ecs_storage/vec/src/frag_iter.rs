//! The `frag_iter` workload on a `Vec` of structs.

use engine::prelude::Prng;
use spike_ecs_harness::frag_iter::{self as rules, Data, FragIter, LETTERS, Letter};
use spike_ecs_harness::{Params, Workload};

/// `frag_iter` with the letter as an enum field: `26 * count` `(Letter, Data)` pairs.
#[derive(Clone, Debug)]
pub struct VecFragIter {
    pub(crate) entities: Vec<(Letter, Data)>,
}

impl Workload for VecFragIter {
    type Spec = FragIter;
    const BACKEND: &'static str = "vec_aos";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let population = u64::from(LETTERS) * params.count as u64;
        let entities = (0..population)
            .map(|i| rules::spawn(&prng, i, params.count))
            .collect();
        Self { entities }
    }

    fn step(&mut self) {
        for (_, data) in &mut self.entities {
            rules::update(data);
        }
    }

    fn checksum(&self) -> u64 {
        self.entities.iter().fold(0, |sum, (letter, data)| {
            sum.wrapping_add(rules::hash(letter, data))
        })
    }
}
