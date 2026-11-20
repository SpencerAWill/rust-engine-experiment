//! The `frag_iter` workload on the dense `World`: one component type per letter.

use engine::prelude::Prng;
use spike_ecs_harness::frag_iter::{
    self as rules, A, B, C, D, Data, E, F, FragIter, G, H, I, J, K, L, LETTERS, Letter, M, N, O, P,
    Q, R, S, T, U, V, W, X, Y, Z,
};
use spike_ecs_harness::{Params, Workload};

use crate::{Entity, QueryMut, World};

/// `frag_iter` on a dense-array [`World`] with each letter stored as its own component type.
#[derive(Debug)]
pub struct DenseFragIter {
    world: World,
}

/// Defines the two functions that need one arm per letter type.
macro_rules! per_letter {
    ($($name:ident = $index:literal),+ $(,)?) => {
        /// Inserts the letter's payload as the letter's own component type.
        fn insert_letter(world: &mut World, entity: Entity, letter: Letter) {
            match letter {
                $(Letter::$name(x) => {
                    world.insert(entity, x);
                })+
            }
        }

        /// The sum of every entity's hash, one `(letter, Data)` query per letter type.
        fn hash_all(world: &World) -> u64 {
            let mut sum = 0_u64;
            $(
                world.query::<(&$name, &Data)>().for_each(|(l, data)| {
                    let letter = Letter::from_index($index, l.0);
                    sum = sum.wrapping_add(rules::hash(&letter, data));
                });
            )+
            sum
        }
    };
}

per_letter!(
    A = 0,
    B = 1,
    C = 2,
    D = 3,
    E = 4,
    F = 5,
    G = 6,
    H = 7,
    I = 8,
    J = 9,
    K = 10,
    L = 11,
    M = 12,
    N = 13,
    O = 14,
    P = 15,
    Q = 16,
    R = 17,
    S = 18,
    T = 19,
    U = 20,
    V = 21,
    W = 22,
    X = 23,
    Y = 24,
    Z = 25,
);

/// The step's query, shared with the test hook so the two cannot drift.
fn step_query(world: &mut World) -> QueryMut<'_, &mut Data> {
    world.query_mut()
}

#[cfg(test)]
impl DenseFragIter {
    /// The world, for tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }

    /// The sets the step's query joins (required, excluded), for tests.
    pub(crate) fn step_joined(&mut self) -> crate::query::JoinedSets {
        step_query(&mut self.world).joined()
    }
}

impl Workload for DenseFragIter {
    type Spec = FragIter;
    const BACKEND: &'static str = "dense";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        let population = u64::from(LETTERS) * params.count as u64;
        for i in 0..population {
            let (letter, data) = rules::spawn(&prng, i, params.count);
            let entity = world.spawn();
            insert_letter(&mut world, entity, letter);
            world.insert(entity, data);
        }
        Self { world }
    }

    fn step(&mut self) {
        step_query(&mut self.world).for_each(rules::update);
    }

    fn checksum(&self) -> u64 {
        hash_all(&self.world)
    }
}
