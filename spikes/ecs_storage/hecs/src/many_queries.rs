//! The `many_queries` workload on a `hecs` `World`: eight archetypes of four types each.

use engine::prelude::Prng;
use hecs::World;
use spike_ecs_harness::many_queries::{self as rules, C0, C1, C2, C3, C4, C5, C6, C7, ManyQueries};
use spike_ecs_harness::{Params, Workload};

/// `many_queries` on a `hecs` [`World`]: one archetype per group, each holding four types.
///
/// Each of the step's 56 queries is a plain `query_mut`, with no prepared query.
pub struct HecsManyQueries {
    world: World,
}

impl core::fmt::Debug for HecsManyQueries {
    /// `hecs::World` has no `Debug`, so this shows the entity count only.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HecsManyQueries")
            .field("entities", &self.world.len())
            .finish_non_exhaustive()
    }
}

/// Defines the two functions that need one arm per group, with the group's four types written
/// out: its spawn bundle in `values` order, and its checksum query.
macro_rules! per_group {
    ($($g:literal => ($a:ident, $b:ident, $c:ident, $d:ident)),+ $(,)?) => {
        /// Spawns the entity's four components as one bundle of its group's types.
        fn spawn_group(world: &mut World, s: &rules::Spawn) {
            let [va, vb, vc, vd] = s.values;
            match s.group {
                $($g => {
                    world.spawn(($a(va), $b(vb), $c(vc), $d(vd)));
                })+
                _ => unreachable!("many_queries: a group is 0 to 7"),
            }
        }

        /// The sum of every entity's hash, one read-only four-type query per group.
        fn hash_all(world: &World) -> u64 {
            let mut sum = 0_u64;
            $(
                for (a, b, c, d) in world.query::<(&$a, &$b, &$c, &$d)>().iter() {
                    sum = sum.wrapping_add(rules::hash($g, [a.0, b.0, c.0, d.0]));
                }
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

impl Workload for HecsManyQueries {
    type Spec = ManyQueries;
    const BACKEND: &'static str = "hecs";

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
                    .into_iter()
                    .for_each(|(w, r)| rules::update(&mut w.0, r.0));
            };
        }
        spike_ecs_harness::for_each_query!(run_query);
    }

    fn checksum(&self) -> u64 {
        hash_all(&self.world)
    }
}
