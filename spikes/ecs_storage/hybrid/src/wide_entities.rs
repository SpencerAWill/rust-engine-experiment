//! The `wide_entities` workload on the hybrid `World`.
//!
//! Each entity is spawned with one `spawn_with` of the 24-tuple, so the entities live in one
//! 24-type archetype of table types. `Extra` is a sparse type, so the insert and remove passes
//! (outside any query, with `World::insert` and `World::remove`, as `add_remove` does) touch a
//! sparse set and move no row.

use engine::prelude::Prng;
use spike_ecs_harness::wide_entities::{
    self as rules, Extra, W0, W1, W2, W3, W4, W5, W6, W7, W8, W9, W10, W11, W12, W13, W14, W15,
    W16, W17, W18, W19, W20, W21, W22, W23, Wide, WideEntities,
};
use spike_ecs_harness::{Params, Workload};

use crate::{Entity, Query, With, World};

/// `wide_entities` on the hybrid [`World`]: every tick adds `Extra` to each 24-type entity's
/// sparse set and removes it again; no entity changes archetype.
#[derive(Debug)]
pub struct HybridWideEntities {
    world: World,
    with_w0: Vec<(Entity, f32)>,
    with_extra: Vec<Entity>,
}

/// The read pass's query, shared with the test hook so the two cannot drift.
fn read_query(world: &World) -> Query<'_, (Entity, &W0)> {
    world.query()
}

/// The remove pass's query, shared with the test hook so the two cannot drift.
fn remove_query(world: &World) -> Query<'_, Entity, ((), With<Extra>)> {
    world.query().with::<Extra>()
}

#[cfg(test)]
impl HybridWideEntities {
    /// The number of archetypes in the world, for tests.
    pub(crate) fn archetype_count(&self) -> usize {
        self.world.archetype_count()
    }

    /// The world, for invariant checks in tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }

    /// The entities the last step removed `Extra` from, for tests.
    pub(crate) fn removed_last_step(&self) -> &[Entity] {
        &self.with_extra
    }
}

impl Workload for HybridWideEntities {
    type Spec = WideEntities;
    const BACKEND: &'static str = "hybrid";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            let w = rules::spawn(&prng, i);
            world.spawn_with((
                w.w0, w.w1, w.w2, w.w3, w.w4, w.w5, w.w6, w.w7, w.w8, w.w9, w.w10, w.w11, w.w12,
                w.w13, w.w14, w.w15, w.w16, w.w17, w.w18, w.w19, w.w20, w.w21, w.w22, w.w23,
            ));
        }
        Self {
            world,
            with_w0: Vec::with_capacity(params.count),
            with_extra: Vec::with_capacity(params.count),
        }
    }

    fn step(&mut self) {
        self.world
            .query_mut::<(&mut W0, &mut W7, &mut W14, &mut W21)>()
            .for_each(|(a, b, c, d)| rules::update(a, b, c, d));

        // Two separate passes: every insert happens before the first removal.
        self.with_w0.clear();
        let with_w0 = &mut self.with_w0;
        read_query(&self.world).for_each(|(e, w0)| with_w0.push((e, w0.0[0])));
        for &(e, x) in &self.with_w0 {
            self.world.insert::<Extra>(e, rules::make_extra(&W0([x])));
        }

        self.with_extra.clear();
        let with_extra = &mut self.with_extra;
        remove_query(&self.world).for_each(|e| with_extra.push(e));
        for &e in &self.with_extra {
            self.world.remove::<Extra>(e);
        }
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        self.world
            .query::<(
                (
                    (&W0, &W1, &W2, &W3),
                    (&W4, &W5, &W6, &W7),
                    (&W8, &W9, &W10, &W11),
                    (&W12, &W13, &W14, &W15),
                ),
                (
                    (&W16, &W17, &W18, &W19),
                    (&W20, &W21, &W22, &W23),
                    Option<&Extra>,
                ),
            )>()
            .for_each(|((a, b, c, d), (e, f, extra))| {
                let wide = Wide {
                    w0: *a.0,
                    w1: *a.1,
                    w2: *a.2,
                    w3: *a.3,
                    w4: *b.0,
                    w5: *b.1,
                    w6: *b.2,
                    w7: *b.3,
                    w8: *c.0,
                    w9: *c.1,
                    w10: *c.2,
                    w11: *c.3,
                    w12: *d.0,
                    w13: *d.1,
                    w14: *d.2,
                    w15: *d.3,
                    w16: *e.0,
                    w17: *e.1,
                    w18: *e.2,
                    w19: *e.3,
                    w20: *f.0,
                    w21: *f.1,
                    w22: *f.2,
                    w23: *f.3,
                };
                sum = sum.wrapping_add(rules::hash(&wide, extra));
            });
        sum
    }
}
