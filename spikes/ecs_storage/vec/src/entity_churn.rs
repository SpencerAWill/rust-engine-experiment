//! The `entity_churn` workload on a `Vec` of structs.

use engine::prelude::Prng;
use spike_ecs_harness::entity_churn::{
    self as rules, EntityChurn, Lifetime, Position, SpawnId, Velocity,
};
use spike_ecs_harness::{Params, Workload};

/// One entity: all four components in one struct.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ChurnEntity {
    pub(crate) id: SpawnId,
    pub(crate) position: Position,
    pub(crate) velocity: Velocity,
    pub(crate) lifetime: Lifetime,
}

/// `entity_churn` as an array of structs: removal with `retain`, spawns pushed.
#[derive(Clone, Debug)]
pub struct VecEntityChurn {
    pub(crate) entities: Vec<ChurnEntity>,
    pub(crate) prng: Prng,
    pub(crate) next_id: u64,
    pub(crate) count: usize,
}

impl VecEntityChurn {
    fn spawn_next(&mut self) {
        let (id, position, velocity, lifetime) = rules::spawn(&self.prng, self.next_id);
        self.next_id += 1;
        self.entities.push(ChurnEntity {
            id,
            position,
            velocity,
            lifetime,
        });
    }

    /// Despawns every expired entity, then spawns until `count` are alive.
    pub(crate) fn replace_expired(&mut self) {
        self.entities.retain(|e| !rules::is_expired(&e.lifetime));
        while self.entities.len() < self.count {
            self.spawn_next();
        }
    }
}

impl Workload for VecEntityChurn {
    type Spec = EntityChurn;
    const BACKEND: &'static str = "vec_aos";

    fn setup(params: Params) -> Self {
        let mut w = Self {
            entities: Vec::with_capacity(params.count),
            prng: Prng::from_seed(params.seed),
            next_id: 0,
            count: params.count,
        };
        w.replace_expired();
        w
    }

    fn step(&mut self) {
        for e in &mut self.entities {
            rules::update(&mut e.position, &e.velocity, &mut e.lifetime);
        }
        self.replace_expired();
    }

    fn checksum(&self) -> u64 {
        self.entities.iter().fold(0, |sum, e| {
            sum.wrapping_add(rules::hash(&e.id, &e.position, &e.velocity, &e.lifetime))
        })
    }
}
