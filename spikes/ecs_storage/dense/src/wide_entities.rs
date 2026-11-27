//! The `wide_entities` workload on the dense `World`.
//!
//! Each entity is spawned empty and given its 24 components by 24 `insert`s in type order. The
//! insert and remove passes run outside any query and use `World::insert` and `World::remove`
//! directly, as `add_remove` does.

use engine::prelude::Prng;
use spike_ecs_harness::wide_entities::{
    self as rules, Extra, W0, W1, W2, W3, W4, W5, W6, W7, W8, W9, W10, W11, W12, W13, W14, W15,
    W16, W17, W18, W19, W20, W21, W22, W23, Wide, WideEntities,
};
use spike_ecs_harness::{Params, Workload};

use crate::{Entity, Query, With, World};

/// `wide_entities` on a dense-array [`World`]: every tick moves each 24-type entity to 25 types
/// (adding `Extra`) and back.
#[derive(Debug)]
pub struct DenseWideEntities {
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
impl DenseWideEntities {
    /// The entities the last step removed `Extra` from, for tests.
    pub(crate) fn removed_last_step(&self) -> &[Entity] {
        &self.with_extra
    }
}

impl Workload for DenseWideEntities {
    type Spec = WideEntities;
    const BACKEND: &'static str = "dense";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            let w = rules::spawn(&prng, i);
            let e = world.spawn();
            world.insert(e, w.w0);
            world.insert(e, w.w1);
            world.insert(e, w.w2);
            world.insert(e, w.w3);
            world.insert(e, w.w4);
            world.insert(e, w.w5);
            world.insert(e, w.w6);
            world.insert(e, w.w7);
            world.insert(e, w.w8);
            world.insert(e, w.w9);
            world.insert(e, w.w10);
            world.insert(e, w.w11);
            world.insert(e, w.w12);
            world.insert(e, w.w13);
            world.insert(e, w.w14);
            world.insert(e, w.w15);
            world.insert(e, w.w16);
            world.insert(e, w.w17);
            world.insert(e, w.w18);
            world.insert(e, w.w19);
            world.insert(e, w.w20);
            world.insert(e, w.w21);
            world.insert(e, w.w22);
            world.insert(e, w.w23);
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
