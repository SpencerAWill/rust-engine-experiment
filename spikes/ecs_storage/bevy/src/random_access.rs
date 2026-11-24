//! The `random_access` workload on a `bevy_ecs` `World` (M3.21 spec §4.3).

use bevy_ecs::entity::Entity;
use bevy_ecs::query::QueryState;
use bevy_ecs::world::World;
use engine::prelude::{Prng, Vec2};
use spike_ecs_harness::random_access::{
    self as rules, Lifetime, Position, RandomAccess, Slot, SpawnId, Target, Velocity,
};
use spike_ecs_harness::{Params, Workload, dt};

use crate::Table;

/// Pass A: the entity's own position and its target.
type PassA = QueryState<(&'static Table<Position>, &'static Table<Target<Entity>>)>;

/// Pass B: everything the write pass touches.
type PassB = QueryState<(
    Entity,
    &'static Table<Slot>,
    &'static mut Table<Position>,
    &'static mut Table<Velocity>,
    &'static mut Table<Target<Entity>>,
    &'static mut Table<Lifetime>,
)>;

/// The lookup: any entity's position, read by handle.
type PositionLookup = QueryState<&'static Table<Position>>;

/// `random_access` on a `bevy_ecs` [`World`]: pass A reads each target's position through a
/// `QueryState<&Table<Position>>` with `get_manual`, pass B writes the results back.
///
/// The count of live entities is the roster's length, which is the backend's own count (a bevy
/// `World` holds an entity of its own).
#[derive(Debug)]
pub struct BevyRandomAccess {
    world: World,
    pass_a: PassA,
    pass_b: PassB,
    lookup: PositionLookup,
    /// `roster[s]` is the entity in slot `s`.
    roster: Vec<Entity>,
    prng: Prng,
    next_id: u64,
    count: usize,
    dt: f32,
    scratch: Vec<(Vec2, Option<Entity>)>,
    expired: Vec<(Entity, u32)>,
}

#[cfg(test)]
impl BevyRandomAccess {
    /// The world, for tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }

    /// The spawn index the next respawn takes, for tests.
    pub(crate) fn next_id(&self) -> u64 {
        self.next_id
    }

    /// For each target whose handle is not alive, the spawn index of the entity now in the
    /// roster at that target's slot, for tests.
    pub(crate) fn stale_target_occupants(&self) -> Vec<u64> {
        let mut occupants = Vec::new();
        let Some(query) = self.world.try_query::<&Table<Target<Entity>>>() else {
            return occupants;
        };
        for t in query.iter_manual(&self.world) {
            if self.world.get_entity(t.0.entity).is_err() {
                let occupant = self.roster[t.0.slot as usize];
                let id = self.world.get::<Table<SpawnId>>(occupant);
                occupants.push(id.expect("the roster holds live entities").0.0);
            }
        }
        occupants
    }
}

impl BevyRandomAccess {
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
            .spawn((
                Table(id),
                Table(Slot(slot)),
                Table(position),
                Table(velocity),
                Table(lifetime),
                Table(target),
            ))
            .id();
        self.roster[slot as usize] = entity;
    }
}

impl Workload for BevyRandomAccess {
    type Spec = RandomAccess;
    const BACKEND: &'static str = "bevy_ecs";

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
                entity: Entity::PLACEHOLDER,
            };
            roster.push(
                world
                    .spawn((
                        Table(id),
                        Table(Slot(slot)),
                        Table(position),
                        Table(velocity),
                        Table(lifetime),
                        Table(target),
                    ))
                    .id(),
            );
        }
        let pass_a = world.query();
        let mut pass_b: PassB = world.query();
        let lookup = world.query();
        let roster_ref = &roster;
        pass_b
            .iter_mut(&mut world)
            .for_each(|(_, _, _, _, mut target, _)| {
                target.0.entity = roster_ref[target.0.slot as usize];
            });
        Self {
            world,
            pass_a,
            pass_b,
            lookup,
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
        // Pass A: read only. Every lookup sees the start-of-step positions. The `QueryState`s
        // were built in `setup`; every archetype exists by then (the spawns after setup reuse
        // the one archetype), so `update_archetypes` is not called here.
        self.scratch.clear();
        let scratch = &mut self.scratch;
        let roster = &self.roster;
        let world = &self.world;
        let lookup = &self.lookup;
        self.pass_a.iter_manual(world).for_each(|(p, t)| {
            let (q, retarget) = if let Ok(q) = lookup.get_manual(world, t.0.entity) {
                (q, None)
            } else {
                let h = roster[t.0.slot as usize];
                (
                    lookup
                        .get_manual(world, h)
                        .expect("the roster holds live entities"),
                    Some(h),
                )
            };
            scratch.push((rules::steer(&p.0, &q.0).0, retarget));
        });

        // Pass B: write, in the same order.
        self.expired.clear();
        let expired = &mut self.expired;
        let dt = self.dt;
        let mut next = self.scratch.iter();
        self.pass_b.iter_mut(&mut self.world).for_each(
            |(e, slot, mut position, mut velocity, mut target, mut lifetime)| {
                let &(v, retarget) = next.next().expect("one scratch entry per entity");
                velocity.0 = Velocity(v);
                if let Some(h) = retarget {
                    target.0.entity = h;
                }
                rules::integrate(&mut position.0, &velocity.0, &mut lifetime.0, dt);
                if rules::is_expired(&lifetime.0) {
                    expired.push((e, slot.0.0));
                }
            },
        );

        // Despawn, then refill the freed slots in ascending slot order.
        self.expired.sort_unstable_by_key(|&(_, slot)| slot);
        for i in 0..self.expired.len() {
            let entity = self.expired[i].0;
            self.world.despawn(entity);
        }
        for i in 0..self.expired.len() {
            let slot = self.expired[i].1;
            self.respawn(slot);
        }
        self.world.clear_trackers();
    }

    fn checksum(&self) -> u64 {
        let Some(query) = self.world.try_query::<(
            (&Table<SpawnId>, &Table<Slot>),
            (&Table<Position>, &Table<Velocity>),
            (&Table<Lifetime>, &Table<Target<Entity>>),
        )>() else {
            return 0;
        };
        query.iter_manual(&self.world).fold(
            0_u64,
            |sum, ((id, slot), (position, velocity), (lifetime, target))| {
                sum.wrapping_add(rules::hash(
                    &id.0,
                    &slot.0,
                    &position.0,
                    &velocity.0,
                    &lifetime.0,
                    &target.0,
                ))
            },
        )
    }
}
