//! The `many_queries` workload on a `bevy_ecs` `World` (M3.22 spec §4.4).

use std::fmt;

use bevy_ecs::query::QueryState;
use bevy_ecs::world::World;
use engine::prelude::Prng;
use spike_ecs_harness::many_queries::{self as rules, C0, C1, C2, C3, C4, C5, C6, C7, ManyQueries};
use spike_ecs_harness::{Params, Workload};

use crate::Table;

/// One of the step's queries: it owns its `QueryState` and runs it over the world.
type QueryFn = Box<dyn FnMut(&mut World)>;

/// `many_queries` on a `bevy_ecs` [`World`]: eight archetypes of four types each.
///
/// Each of the step's 56 queries is a `QueryState` built in `setup`, so a step pays bevy's cached
/// per-query matching and no query construction. No archetype is created after `setup`.
pub struct BevyManyQueries {
    world: World,
    queries: Vec<QueryFn>,
}

impl BevyManyQueries {
    /// The number of queries `setup` built, for a test that the empty ones were not dropped.
    #[cfg(test)]
    pub(crate) fn query_count(&self) -> usize {
        self.queries.len()
    }
}

impl fmt::Debug for BevyManyQueries {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BevyManyQueries")
            .field("world", &self.world)
            .field("queries", &self.queries.len())
            .finish()
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
                    world.spawn((Table($a(va)), Table($b(vb)), Table($c(vc)), Table($d(vd))));
                })+
                _ => unreachable!("many_queries: a group is 0 to 7"),
            }
        }

        /// The sum of every entity's hash, one read-only four-type query per group.
        fn hash_all(world: &World) -> u64 {
            let mut sum = 0_u64;
            $(
                if let Some(query) =
                    world.try_query::<(&Table<$a>, &Table<$b>, &Table<$c>, &Table<$d>)>()
                {
                    query.iter_manual(world).for_each(|(a, b, c, d)| {
                        sum = sum.wrapping_add(rules::hash($g, [a.0.0, b.0.0, c.0.0, d.0.0]));
                    });
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

impl Workload for BevyManyQueries {
    type Spec = ManyQueries;
    const BACKEND: &'static str = "bevy_ecs";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            spawn_group(&mut world, &rules::spawn(&prng, i));
        }
        let mut queries: Vec<QueryFn> = Vec::with_capacity(56);
        macro_rules! build_query {
            ($write:ty, $read:ty) => {{
                let mut state: QueryState<(&'static mut Table<$write>, &'static Table<$read>)> =
                    world.query();
                queries.push(Box::new(move |world: &mut World| {
                    state
                        .iter_mut(world)
                        .for_each(|(mut w, r)| rules::update(&mut w.0.0, r.0.0));
                }));
            }};
        }
        spike_ecs_harness::for_each_query!(build_query);
        Self { world, queries }
    }

    fn step(&mut self) {
        for query in &mut self.queries {
            query(&mut self.world);
        }
        self.world.clear_trackers();
    }

    fn checksum(&self) -> u64 {
        hash_all(&self.world)
    }
}
