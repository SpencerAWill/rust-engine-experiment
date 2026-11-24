//! The `frag_iter` workload on a `bevy_ecs` `World`: one component type per letter.

use bevy_ecs::query::QueryState;
use bevy_ecs::world::World;
use engine::prelude::Prng;
use spike_ecs_harness::frag_iter::{
    self as rules, A, B, C, D, Data, E, F, FragIter, G, H, I, J, K, L, LETTERS, Letter, M, N, O, P,
    Q, R, S, T, U, V, W, X, Y, Z,
};
use spike_ecs_harness::{Params, Workload};

use crate::Table;

/// `frag_iter` on a `bevy_ecs` [`World`]: one archetype per letter type, each holding
/// `(Table<L>, Table<Data>)`.
#[derive(Debug)]
pub struct BevyFragIter {
    world: World,
    step: QueryState<&'static mut Table<Data>>,
}

/// Defines the functions that need one arm per letter type.
macro_rules! per_letter {
    ($($name:ident = $index:literal),+ $(,)?) => {
        /// Spawns the letter's payload and the data as one entity `(Table<L>, Table<Data>)`.
        fn spawn_letter(world: &mut World, letter: Letter, data: Data) {
            match letter {
                $(Letter::$name(x) => {
                    world.spawn((Table(x), Table(data)));
                })+
            }
        }

        /// The sum of every entity's hash, one `(letter, Data)` query per letter type.
        fn hash_all(world: &World) -> u64 {
            let mut sum = 0_u64;
            $(
                if let Some(query) = world.try_query::<(&Table<$name>, &Table<Data>)>() {
                    for (l, data) in query.iter_manual(world) {
                        let letter = Letter::from_index($index, l.0.0);
                        sum = sum.wrapping_add(rules::hash(&letter, &data.0));
                    }
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

impl Workload for BevyFragIter {
    type Spec = FragIter;
    const BACKEND: &'static str = "bevy_ecs";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        let population = u64::from(LETTERS) * params.count as u64;
        for i in 0..population {
            let (letter, data) = rules::spawn(&prng, i, params.count);
            spawn_letter(&mut world, letter, data);
        }
        let step = world.query();
        Self { world, step }
    }

    fn step(&mut self) {
        self.step
            .iter_mut(&mut self.world)
            .for_each(|mut data| rules::update(&mut data.0));
        self.world.clear_trackers();
    }

    fn checksum(&self) -> u64 {
        hash_all(&self.world)
    }
}
