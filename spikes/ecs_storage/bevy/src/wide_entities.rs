//! The `wide_entities` workload on a `bevy_ecs` `World`.
//!
//! Every entity is spawned from one bundle of 24 `Table` components, so the entities share one
//! archetype. The passes run outside any query iteration, so they use `World::entity_mut`.

use bevy_ecs::entity::Entity;
use bevy_ecs::query::{QueryState, With};
use bevy_ecs::world::World;
use engine::prelude::Prng;
use spike_ecs_harness::wide_entities::{
    self as rules, Extra, W0, W1, W2, W3, W4, W5, W6, W7, W8, W9, W10, W11, W12, W13, W14, W15,
    W16, W17, W18, W19, W20, W21, W22, W23, Wide, WideEntities,
};
use spike_ecs_harness::{Params, Workload};

use crate::Table;

/// The update pass's query.
type UpdateQuery = QueryState<(
    &'static mut Table<W0>,
    &'static mut Table<W7>,
    &'static mut Table<W14>,
    &'static mut Table<W21>,
)>;

/// The read pass's query.
type ReadQuery = QueryState<(Entity, &'static Table<W0>)>;

/// The remove pass's query.
type RemoveQuery = QueryState<Entity, With<Table<Extra>>>;

/// `wide_entities` on a `bevy_ecs` [`World`]: every tick adds `Extra` to each 24-type entity and
/// removes it again, a table row move each way.
#[derive(Debug)]
pub struct BevyWideEntities {
    world: World,
    update: UpdateQuery,
    read: ReadQuery,
    with_extra_query: RemoveQuery,
    with_w0: Vec<(Entity, f32)>,
    with_extra: Vec<Entity>,
}

#[cfg(test)]
impl BevyWideEntities {
    /// The world, for tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }

    /// The entities the last step removed `Extra` from, for tests.
    pub(crate) fn removed_last_step(&self) -> &[Entity] {
        &self.with_extra
    }
}

impl Workload for BevyWideEntities {
    type Spec = WideEntities;
    const BACKEND: &'static str = "bevy_ecs";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            let w = rules::spawn(&prng, i);
            world.spawn((
                (
                    Table(w.w0),
                    Table(w.w1),
                    Table(w.w2),
                    Table(w.w3),
                    Table(w.w4),
                    Table(w.w5),
                    Table(w.w6),
                    Table(w.w7),
                    Table(w.w8),
                    Table(w.w9),
                    Table(w.w10),
                    Table(w.w11),
                ),
                (
                    Table(w.w12),
                    Table(w.w13),
                    Table(w.w14),
                    Table(w.w15),
                    Table(w.w16),
                    Table(w.w17),
                    Table(w.w18),
                    Table(w.w19),
                    Table(w.w20),
                    Table(w.w21),
                    Table(w.w22),
                    Table(w.w23),
                ),
            ));
        }
        let update = world.query();
        let read = world.query();
        let with_extra_query = world.query_filtered();
        Self {
            world,
            update,
            read,
            with_extra_query,
            with_w0: Vec::with_capacity(params.count),
            with_extra: Vec::with_capacity(params.count),
        }
    }

    fn step(&mut self) {
        self.update
            .iter_mut(&mut self.world)
            .for_each(|(mut a, mut b, mut c, mut d)| {
                rules::update(&mut a.0, &mut b.0, &mut c.0, &mut d.0);
            });

        // Two separate passes: every insert happens before the first removal.
        self.with_w0.clear();
        let pairs = &mut self.with_w0;
        self.read
            .iter(&self.world)
            .for_each(|(e, w0)| pairs.push((e, w0.0.0[0])));
        for &(e, x) in &self.with_w0 {
            self.world
                .entity_mut(e)
                .insert(Table(rules::make_extra(&W0([x]))));
        }

        self.with_extra.clear();
        let removals = &mut self.with_extra;
        self.with_extra_query
            .iter(&self.world)
            .for_each(|e| removals.push(e));
        for &e in &self.with_extra {
            self.world.entity_mut(e).remove::<Table<Extra>>();
        }
        self.world.clear_trackers();
    }

    fn checksum(&self) -> u64 {
        let Some(query) = self.world.try_query::<(
            (
                (&Table<W0>, &Table<W1>, &Table<W2>, &Table<W3>),
                (&Table<W4>, &Table<W5>, &Table<W6>, &Table<W7>),
                (&Table<W8>, &Table<W9>, &Table<W10>, &Table<W11>),
                (&Table<W12>, &Table<W13>, &Table<W14>, &Table<W15>),
            ),
            (
                (&Table<W16>, &Table<W17>, &Table<W18>, &Table<W19>),
                (&Table<W20>, &Table<W21>, &Table<W22>, &Table<W23>),
                Option<&Table<Extra>>,
            ),
        )>() else {
            return 0;
        };
        query
            .iter_manual(&self.world)
            .fold(0_u64, |sum, ((a, b, c, d), (e, f, extra))| {
                let wide = Wide {
                    w0: a.0.0,
                    w1: a.1.0,
                    w2: a.2.0,
                    w3: a.3.0,
                    w4: b.0.0,
                    w5: b.1.0,
                    w6: b.2.0,
                    w7: b.3.0,
                    w8: c.0.0,
                    w9: c.1.0,
                    w10: c.2.0,
                    w11: c.3.0,
                    w12: d.0.0,
                    w13: d.1.0,
                    w14: d.2.0,
                    w15: d.3.0,
                    w16: e.0.0,
                    w17: e.1.0,
                    w18: e.2.0,
                    w19: e.3.0,
                    w20: f.0.0,
                    w21: f.1.0,
                    w22: f.2.0,
                    w23: f.3.0,
                };
                sum.wrapping_add(rules::hash(&wide, extra.map(|x| &x.0)))
            })
    }
}
