//! The `simple_iter` workload on a `Vec` of structs.

use engine::prelude::Prng;
use spike_ecs_harness::simple_iter::{
    self as rules, Matrix, Position, Rotation, SimpleIter, Velocity,
};
use spike_ecs_harness::{Params, Workload};

/// One entity: all four components in one struct.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SimpleIterEntity {
    pub(crate) matrix: Matrix,
    pub(crate) position: Position,
    pub(crate) rotation: Rotation,
    pub(crate) velocity: Velocity,
}

/// `simple_iter` as an array of structs.
#[derive(Clone, Debug)]
pub struct VecSimpleIter {
    pub(crate) entities: Vec<SimpleIterEntity>,
}

impl Workload for VecSimpleIter {
    type Spec = SimpleIter;
    const BACKEND: &'static str = "vec_aos";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut entities = Vec::with_capacity(params.count);
        for i in 0..params.count as u64 {
            let (matrix, position, rotation, velocity) = rules::spawn(&prng, i);
            entities.push(SimpleIterEntity {
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
            rules::update(&mut e.position, &e.velocity);
        }
    }

    fn checksum(&self) -> u64 {
        self.entities.iter().fold(0, |sum, e| {
            sum.wrapping_add(rules::hash(&e.position, &e.velocity))
        })
    }
}
