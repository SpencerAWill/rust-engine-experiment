//! The `frag_iter` workload on a `hecs` `World`: one component type per letter.

use engine::prelude::Prng;
use hecs::World;
use spike_ecs_harness::frag_iter::{
    self as rules, A, B, C, D, Data, E, F, FragIter, G, H, I, J, K, L, LETTERS, Letter, M, N, O, P,
    Q, R, S, T, U, V, W, X, Y, Z,
};
use spike_ecs_harness::{Params, Workload};

/// `frag_iter` on a `hecs` [`World`]: one archetype per letter type, each holding `(L, Data)`.
pub struct HecsFragIter {
    world: World,
}

/// Defines the functions that need one arm per letter type.
macro_rules! per_letter {
    ($($name:ident = $index:literal),+ $(,)?) => {
        /// Spawns the letter's payload and the data as one entity `(L, Data)`.
        fn spawn_letter(world: &mut World, letter: Letter, data: Data) {
            match letter {
                $(Letter::$name(x) => {
                    world.spawn((x, data));
                })+
            }
        }

        /// The sum of every entity's hash, one `(letter, Data)` query per letter type.
        fn hash_all(world: &World) -> u64 {
            let mut sum = 0_u64;
            $(
                for (l, data) in world.query::<(&$name, &Data)>().iter() {
                    let letter = Letter::from_index($index, l.0);
                    sum = sum.wrapping_add(rules::hash(&letter, data));
                }
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

impl core::fmt::Debug for HecsFragIter {
    /// `hecs::World` has no `Debug`, so this shows the entity count only.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HecsFragIter")
            .field("entities", &self.world.len())
            .finish_non_exhaustive()
    }
}

impl Workload for HecsFragIter {
    type Spec = FragIter;
    const BACKEND: &'static str = "hecs";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        let population = u64::from(LETTERS) * params.count as u64;
        for i in 0..population {
            let (letter, data) = rules::spawn(&prng, i, params.count);
            spawn_letter(&mut world, letter, data);
        }
        Self { world }
    }

    fn step(&mut self) {
        for data in self.world.query_mut::<&mut Data>() {
            rules::update(data);
        }
    }

    fn checksum(&self) -> u64 {
        hash_all(&self.world)
    }
}
