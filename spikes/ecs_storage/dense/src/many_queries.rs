//! The `many_queries` workload on the dense-array `World`: eight groups of four types each.

use engine::prelude::Prng;
use spike_ecs_harness::many_queries::{self as rules, C0, C1, C2, C3, C4, C5, C6, C7, ManyQueries};
use spike_ecs_harness::{Params, Workload};

use crate::World;

/// `many_queries` on a dense-array [`World`] joining presence sets, one array per type.
///
/// Each of the step's 56 queries joins the presence sets of its two types and fetches at the
/// indices the join yields.
#[derive(Debug)]
pub struct DenseManyQueries {
    world: World,
}

/// Defines the two functions that need one arm per group, with the group's four types written
/// out: its spawn inserts in `values` order, and its checksum query.
macro_rules! per_group {
    ($($g:literal => ($a:ident, $b:ident, $c:ident, $d:ident)),+ $(,)?) => {
        /// Spawns an entity and inserts its four components of its group's types.
        fn spawn_group(world: &mut World, s: &rules::Spawn) {
            let [va, vb, vc, vd] = s.values;
            match s.group {
                $($g => {
                    let entity = world.spawn();
                    world.insert(entity, $a(va));
                    world.insert(entity, $b(vb));
                    world.insert(entity, $c(vc));
                    world.insert(entity, $d(vd));
                })+
                _ => unreachable!("many_queries: a group is 0 to 7"),
            }
        }

        /// The sum of every entity's hash, one read-only four-type query per group.
        fn hash_all(world: &World) -> u64 {
            let mut sum = 0_u64;
            $(
                world.query::<(&$a, &$b, &$c, &$d)>().for_each(|(a, b, c, d)| {
                    sum = sum.wrapping_add(rules::hash($g, [a.0, b.0, c.0, d.0]));
                });
            )+
            sum
        }
    };
}

per_group!(
    0 => (C0, C1, C2, C3),
    1 => (C1, C2, C3, C4),
    2 => (C2, C3, C4, C5),
    3 => (C3, C4, C5, C6),
    4 => (C4, C5, C6, C7),
    5 => (C5, C6, C7, C0),
    6 => (C6, C7, C0, C1),
    7 => (C7, C0, C1, C2),
);

impl Workload for DenseManyQueries {
    type Spec = ManyQueries;
    const BACKEND: &'static str = "dense";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            spawn_group(&mut world, &rules::spawn(&prng, i));
        }
        Self { world }
    }

    fn step(&mut self) {
        let world = &mut self.world;
        macro_rules! run_query {
            ($write:ty, $read:ty) => {
                world
                    .query_mut::<(&mut $write, &$read)>()
                    .for_each(|(w, r)| rules::update(&mut w.0, r.0));
            };
        }
        spike_ecs_harness::for_each_query!(run_query);
    }

    fn checksum(&self) -> u64 {
        hash_all(&self.world)
    }
}
