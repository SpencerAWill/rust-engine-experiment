//! The `wide_entities` workload on a `hecs` `World`.
//!
//! `hecs` tuple bundles stop at 15 types, so each entity is spawned from an [`EntityBuilder`]
//! holding its 24 values; the entities share one archetype. The passes run outside any query
//! iteration, so they use `World::insert_one` and `World::remove_one` directly.

use engine::prelude::Prng;
use hecs::{Entity, EntityBuilder, World};
use spike_ecs_harness::wide_entities::{
    self as rules, Extra, W0, W1, W2, W3, W4, W5, W6, W7, W8, W9, W10, W11, W12, W13, W14, W15,
    W16, W17, W18, W19, W20, W21, W22, W23, Wide, WideEntities,
};
use spike_ecs_harness::{Params, Workload};

/// `wide_entities` on a `hecs` [`World`]: every tick adds `Extra` to each 24-type entity and
/// removes it again, a table row move each way.
pub struct HecsWideEntities {
    world: World,
    with_w0: Vec<(Entity, f32)>,
    with_extra: Vec<Entity>,
}

impl core::fmt::Debug for HecsWideEntities {
    /// `hecs::World` has no `Debug`, so this shows the entity count only.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HecsWideEntities")
            .field("entities", &self.world.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
impl HecsWideEntities {
    /// The world, for tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }

    /// The entities the last step removed `Extra` from, for tests.
    pub(crate) fn removed_last_step(&self) -> &[Entity] {
        &self.with_extra
    }
}

impl Workload for HecsWideEntities {
    type Spec = WideEntities;
    const BACKEND: &'static str = "hecs";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        let mut builder = EntityBuilder::new();
        for i in 0..params.count as u64 {
            let w = rules::spawn(&prng, i);
            builder.add(w.w0);
            builder.add(w.w1);
            builder.add(w.w2);
            builder.add(w.w3);
            builder.add(w.w4);
            builder.add(w.w5);
            builder.add(w.w6);
            builder.add(w.w7);
            builder.add(w.w8);
            builder.add(w.w9);
            builder.add(w.w10);
            builder.add(w.w11);
            builder.add(w.w12);
            builder.add(w.w13);
            builder.add(w.w14);
            builder.add(w.w15);
            builder.add(w.w16);
            builder.add(w.w17);
            builder.add(w.w18);
            builder.add(w.w19);
            builder.add(w.w20);
            builder.add(w.w21);
            builder.add(w.w22);
            builder.add(w.w23);
            world.spawn(builder.build());
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
            .into_iter()
            .for_each(|(a, b, c, d)| rules::update(a, b, c, d));

        // Two separate passes: every insert happens before the first removal.
        self.with_w0.clear();
        let pairs = &mut self.with_w0;
        self.world
            .query_mut::<(Entity, &W0)>()
            .into_iter()
            .for_each(|(e, w0)| pairs.push((e, w0.0[0])));
        for &(e, x) in &self.with_w0 {
            self.world
                .insert_one(e, rules::make_extra(&W0([x])))
                .expect("the entity is alive");
        }

        self.with_extra.clear();
        let removals = &mut self.with_extra;
        self.world
            .query_mut::<Entity>()
            .with::<&Extra>()
            .into_iter()
            .for_each(|e| removals.push(e));
        for &e in &self.with_extra {
            self.world
                .remove_one::<Extra>(e)
                .expect("the entity has an Extra");
        }
    }

    fn checksum(&self) -> u64 {
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
            .iter()
            .fold(0_u64, |sum, ((a, b, c, d), (e, f, extra))| {
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
                sum.wrapping_add(rules::hash(&wide, extra))
            })
    }
}
