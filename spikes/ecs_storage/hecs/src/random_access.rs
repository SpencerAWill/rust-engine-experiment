//! The `random_access` workload on a `hecs` `World` (M3.21 spec §4.3).

use engine::prelude::{Prng, Vec2};
use hecs::{Entity, World};
use spike_ecs_harness::random_access::{
    self as rules, Lifetime, Position, RandomAccess, Slot, SpawnId, Target, Velocity,
};
use spike_ecs_harness::{Params, Workload, dt};

/// `random_access` on a `hecs` [`World`]: pass A reads each target's position through a
/// `world.view::<&Position>()` (`hecs`'s random-access path) created once per pass, pass B
/// writes the results back with `query_mut`.
///
/// The harness types are stored unwrapped. The roster of slot handles is the backend's own, as
/// in every other backend of this workload.
pub struct HecsRandomAccess {
    world: World,
    /// `roster[s]` is the entity in slot `s`.
    roster: Vec<Entity>,
    prng: Prng,
    next_id: u64,
    count: usize,
    dt: f32,
    scratch: Vec<(Vec2, Option<Entity>)>,
    expired: Vec<(Entity, u32)>,
}

impl HecsRandomAccess {
    /// Spawns the next spawn index into `slot`, targeting the roster as it stands now.
    fn respawn(&mut self, slot: u32) {
        let (id, position, velocity, lifetime, target_slot) =
            rules::spawn(&self.prng, self.next_id, self.count);
        self.next_id += 1;
        let target = Target {
            slot: target_slot,
            entity: self.roster[target_slot as usize],
        };
        let entity = self
            .world
            .spawn((id, Slot(slot), position, velocity, lifetime, target));
        self.roster[slot as usize] = entity;
    }
}

impl core::fmt::Debug for HecsRandomAccess {
    /// `hecs::World` has no `Debug`, so this shows the entity count only.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HecsRandomAccess")
            .field("entities", &self.world.len())
            .finish_non_exhaustive()
    }
}

impl Workload for HecsRandomAccess {
    type Spec = RandomAccess;
    const BACKEND: &'static str = "hecs";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        let mut roster = Vec::with_capacity(params.count);
        for i in 0..params.count as u64 {
            let (id, position, velocity, lifetime, target_slot) =
                rules::spawn(&prng, i, params.count);
            let slot = u32::try_from(i).expect("count fits in u32");
            // The handle is patched below, once every slot is filled.
            let target = Target {
                slot: target_slot,
                entity: Entity::DANGLING,
            };
            roster.push(world.spawn((id, Slot(slot), position, velocity, lifetime, target)));
        }
        let roster_ref = &roster;
        world
            .query_mut::<&mut Target<Entity>>()
            .into_iter()
            .for_each(|target| {
                target.entity = roster_ref[target.slot as usize];
            });
        Self {
            world,
            roster,
            prng,
            next_id: params.count as u64,
            count: params.count,
            dt: dt(),
            scratch: Vec::with_capacity(params.count),
            expired: Vec::with_capacity(params.count),
        }
    }

    fn step(&mut self) {
        // Pass A: read only. Every lookup sees the start-of-step positions. The view is created
        // once per pass; `ViewBorrow::get` then reads the entity table and the archetype's column.
        self.scratch.clear();
        let scratch = &mut self.scratch;
        let roster = &self.roster;
        {
            let view = self.world.view::<&Position>();
            self.world
                .query::<(&Position, &Target<Entity>)>()
                .iter()
                .for_each(|(p, t)| {
                    let (q, retarget) = if let Some(q) = view.get(t.entity) {
                        (q, None)
                    } else {
                        let h = roster[t.slot as usize];
                        (
                            view.get(h).expect("the roster holds live entities"),
                            Some(h),
                        )
                    };
                    scratch.push((rules::steer(p, q).0, retarget));
                });
        }

        // Pass B: write, in the same order.
        self.expired.clear();
        let expired = &mut self.expired;
        let dt = self.dt;
        let mut next = self.scratch.iter();
        self.world
            .query_mut::<(
                Entity,
                &Slot,
                &mut Position,
                &mut Velocity,
                &mut Target<Entity>,
                &mut Lifetime,
            )>()
            .into_iter()
            .for_each(|(e, slot, position, velocity, target, lifetime)| {
                let &(v, retarget) = next.next().expect("one scratch entry per entity");
                *velocity = Velocity(v);
                if let Some(h) = retarget {
                    target.entity = h;
                }
                rules::integrate(position, velocity, lifetime, dt);
                if rules::is_expired(lifetime) {
                    expired.push((e, slot.0));
                }
            });

        // Despawn, then refill the freed slots in ascending slot order.
        self.expired.sort_unstable_by_key(|&(_, slot)| slot);
        for i in 0..self.expired.len() {
            let entity = self.expired[i].0;
            let _ = self.world.despawn(entity);
        }
        for i in 0..self.expired.len() {
            let slot = self.expired[i].1;
            self.respawn(slot);
        }
    }

    fn checksum(&self) -> u64 {
        self.world
            .query::<(
                &SpawnId,
                &Slot,
                &Position,
                &Velocity,
                &Lifetime,
                &Target<Entity>,
            )>()
            .iter()
            .fold(
                0_u64,
                |sum, (id, slot, position, velocity, lifetime, target)| {
                    sum.wrapping_add(rules::hash(id, slot, position, velocity, lifetime, target))
                },
            )
    }
}
