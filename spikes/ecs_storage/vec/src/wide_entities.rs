//! The `wide_entities` workload on a `Vec` of structs.

use engine::prelude::Prng;
use spike_ecs_harness::wide_entities::{self as rules, Extra, Wide, WideEntities};
use spike_ecs_harness::{Params, Workload};

/// `wide_entities` with `Extra` as an `Option` field: the reference, not a storage model, so no
/// value moves.
#[derive(Clone, Debug)]
pub struct VecWideEntities {
    pub(crate) entities: Vec<(Wide, Option<Extra>)>,
}

impl VecWideEntities {
    /// One step, calling `between` after the insert pass and before the remove pass.
    pub(crate) fn step_with<F: FnMut(&[(Wide, Option<Extra>)])>(&mut self, mut between: F) {
        for (wide, _) in &mut self.entities {
            rules::update(&mut wide.w0, &mut wide.w7, &mut wide.w14, &mut wide.w21);
        }
        // Separate passes: every insert happens before the first removal.
        for (wide, extra) in &mut self.entities {
            *extra = Some(rules::make_extra(&wide.w0));
        }
        between(&self.entities);
        for (_, extra) in &mut self.entities {
            *extra = None;
        }
    }
}

impl Workload for VecWideEntities {
    type Spec = WideEntities;
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
        self.entities.iter().fold(0, |sum, (wide, extra)| {
            sum.wrapping_add(rules::hash(wide, extra.as_ref()))
        })
    }
}
