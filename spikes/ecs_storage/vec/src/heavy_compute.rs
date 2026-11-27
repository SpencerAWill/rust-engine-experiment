//! The `heavy_compute` workload on a `Vec` of structs.

use engine::prelude::Prng;
use spike_ecs_harness::heavy_compute::{
    self as rules, HeavyCompute, Matrix, Position, Rotation, Velocity,
};
use spike_ecs_harness::{Params, Workload};

/// One entity: all four components in one struct.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HeavyEntity {
    pub(crate) matrix: Matrix,
    pub(crate) position: Position,
    pub(crate) rotation: Rotation,
    /// Read by the step (it moves the translation).
    pub(crate) velocity: Velocity,
}

/// `heavy_compute` as an array of structs.
#[derive(Clone, Debug)]
pub struct VecHeavyCompute {
    pub(crate) entities: Vec<HeavyEntity>,
}

impl Workload for VecHeavyCompute {
    type Spec = HeavyCompute;
    const BACKEND: &'static str = "vec_aos";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut entities = Vec::with_capacity(params.count);
        for i in 0..params.count as u64 {
            let (matrix, position, rotation, velocity) = rules::spawn(&prng, i);
            entities.push(HeavyEntity {
                matrix,
                position,
                rotation,
                velocity,
            });
        }
        Self { entities }
    }

    fn step(&mut self) {
        for e in &mut self.entities {
            rules::update(&mut e.matrix, &mut e.position, &e.rotation, &e.velocity);
        }
    }

    fn checksum(&self) -> u64 {
        self.entities.iter().fold(0, |sum, e| {
            sum.wrapping_add(rules::hash(&e.position, &e.matrix))
        })
    }
}
