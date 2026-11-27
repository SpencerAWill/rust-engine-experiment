//! The `add_remove` workload on a `Vec` of structs.

use engine::prelude::Prng;
use spike_ecs_harness::add_remove::{self as rules, A, AddRemove, B};
use spike_ecs_harness::{Params, Workload};

/// `add_remove` with `B` as an `Option` field.
#[derive(Clone, Debug)]
pub struct VecAddRemove {
    pub(crate) entities: Vec<(A, Option<B>)>,
}

impl VecAddRemove {
    /// One step, calling `between` after the insert pass and before the remove pass.
    pub(crate) fn step_with<F: FnMut(&[(A, Option<B>)])>(&mut self, mut between: F) {
        // Two separate passes: every insert happens before the first removal (spec §4.4.4).
        for (a, b) in &mut self.entities {
            *b = Some(rules::make_b(a));
        }
        between(&self.entities);
        for (_, b) in &mut self.entities {
            *b = None;
        }
    }
}

impl Workload for VecAddRemove {
    type Spec = AddRemove;
    const BACKEND: &'static str = "vec_aos";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let entities = (0..params.count as u64)
            .map(|i| (rules::spawn(&prng, i), None))
            .collect();
        Self { entities }
    }

    fn step(&mut self) {
        self.step_with(|_| {});
    }

    fn checksum(&self) -> u64 {
        self.entities.iter().fold(0, |sum, (a, b)| {
            sum.wrapping_add(rules::hash(a, b.as_ref()))
        })
    }
}
