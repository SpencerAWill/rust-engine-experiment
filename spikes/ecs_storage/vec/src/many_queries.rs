//! The `many_queries` workload on a `Vec` of structs (M3.22 spec §4.4).

use engine::prelude::Prng;
use spike_ecs_harness::many_queries::{self as rules, GROUPS, ManyQueries, QUERIES};
use spike_ecs_harness::{Params, Workload};

/// One entity: its group and a slot for each of the eight component types.
///
/// Only four slots are present, those of types `t` with `(t + 8 - group) % 8 < 4`; the others
/// are never read or written.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Entity {
    group: u8,
    values: [f32; 8],
}

impl Entity {
    /// Whether the entity has component type `t`.
    #[inline]
    fn has(&self, t: u8) -> bool {
        (t + GROUPS - self.group) % GROUPS < 4
    }
}

/// `many_queries` as an array of structs, each query a loop over every entity that checks for
/// its two types.
///
/// The best case for this workload is a hand-written loop with no per-query setup, so it
/// is the floor the ECS backends are read against.
#[derive(Clone, Debug)]
pub struct VecManyQueries {
    entities: Vec<Entity>,
}

impl VecManyQueries {
    /// One step, calling `on_visit(query_index)` for each entity a query updates.
    ///
    /// `step` passes a closure that does nothing, so it compiles away.
    #[inline]
    fn step_with<F: FnMut(usize)>(&mut self, mut on_visit: F) {
        for (q, &(write, read)) in QUERIES.iter().enumerate() {
            let (w, r) = (usize::from(write), usize::from(read));
            for e in &mut self.entities {
                if e.has(write) && e.has(read) {
                    let read_value = e.values[r];
                    rules::update(&mut e.values[w], read_value);
                    on_visit(q);
                }
            }
        }
    }

    /// One step that also counts, per query, the entities it updated.
    #[cfg(test)]
    pub(crate) fn step_counting(&mut self, counts: &mut [usize; 56]) {
        self.step_with(|q| counts[q] += 1);
    }
}

impl Workload for VecManyQueries {
    type Spec = ManyQueries;
    const BACKEND: &'static str = "vec_aos";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut entities = Vec::with_capacity(params.count);
        for i in 0..params.count as u64 {
            let s = rules::spawn(&prng, i);
            let mut values = [0.0; 8];
            for (k, &v) in s.values.iter().enumerate() {
                values[(usize::from(s.group) + k) % 8] = v;
            }
            entities.push(Entity {
                group: s.group,
                values,
            });
        }
        Self { entities }
    }

    fn step(&mut self) {
        self.step_with(|_| {});
    }

    fn checksum(&self) -> u64 {
        self.entities.iter().fold(0, |sum, e| {
            let g = usize::from(e.group);
            let values = core::array::from_fn(|k| e.values[(g + k) % 8]);
            sum.wrapping_add(rules::hash(e.group, values))
        })
    }
}
