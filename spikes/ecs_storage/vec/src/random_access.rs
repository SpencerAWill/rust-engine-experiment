//! The `random_access` workload on a `Vec` of structs (M3.21 spec §4.4).

use engine::prelude::{Prng, Vec2};
use spike_ecs_harness::random_access::{
    self as rules, Lifetime, Position, RandomAccess, Slot, SpawnId, Target, Velocity,
};
use spike_ecs_harness::{Params, Workload, dt};

/// One entity: all six components in one struct. Its index in the `Vec` is its slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RaEntity {
    pub(crate) id: SpawnId,
    pub(crate) slot: Slot,
    pub(crate) position: Position,
    pub(crate) velocity: Velocity,
    pub(crate) lifetime: Lifetime,
    pub(crate) target: Target<u64>,
}

/// `random_access` as an array of structs indexed by slot.
///
/// The handle is the spawn id (`E = u64`): a lookup by `Target { slot, entity }` succeeds when
/// `entities[slot].id == entity`, the check a generational index makes. The `Vec` is the roster.
///
/// Nothing here reverses the storage to test order independence: the position in the `Vec` is
/// the slot, so reversing it would break the slot index, which is the storage itself.
#[derive(Clone, Debug)]
pub struct VecRandomAccess {
    pub(crate) entities: Vec<RaEntity>,
    pub(crate) prng: Prng,
    pub(crate) next_id: u64,
    count: usize,
    dt: f32,
    scratch: Vec<(Vec2, Option<u64>)>,
    expired: Vec<(u64, u32)>,
}

impl VecRandomAccess {
    /// Spawns the next spawn index into `slot`, targeting the roster as it stands now.
    fn respawn(&mut self, slot: u32) {
        let (id, position, velocity, lifetime, target_slot) =
            rules::spawn(&self.prng, self.next_id, self.count);
        self.next_id += 1;
        let target = Target {
            slot: target_slot,
            entity: self.entities[target_slot as usize].id.0,
        };
        self.entities[slot as usize] = RaEntity {
            id,
            slot: Slot(slot),
            position,
            velocity,
            lifetime,
            target,
        };
    }
}

impl Workload for VecRandomAccess {
    type Spec = RandomAccess;
    const BACKEND: &'static str = "vec_aos";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut entities = Vec::with_capacity(params.count);
        for i in 0..params.count as u64 {
            let (id, position, velocity, lifetime, target_slot) =
                rules::spawn(&prng, i, params.count);
            entities.push(RaEntity {
                id,
                slot: Slot(u32::try_from(i).expect("count fits in u32")),
                position,
                velocity,
                lifetime,
                // Patched below, once every slot is filled.
                target: Target {
                    slot: target_slot,
                    entity: 0,
                },
            });
        }
        for i in 0..entities.len() {
            let slot = entities[i].target.slot as usize;
            entities[i].target.entity = entities[slot].id.0;
        }
        Self {
            entities,
            prng,
            next_id: params.count as u64,
            count: params.count,
            dt: dt(),
            scratch: Vec::with_capacity(params.count),
            expired: Vec::with_capacity(params.count),
        }
    }

    fn step(&mut self) {
        // Pass A: read only. Every lookup sees the start-of-step positions.
        self.scratch.clear();
        for e in &self.entities {
            let t = &e.target;
            let found = &self.entities[t.slot as usize];
            let (q, retarget) = if found.id.0 == t.entity {
                (found.position, None)
            } else {
                // Stale: the roster entry for the slot is the live occupant, which is `found`.
                (found.position, Some(found.id.0))
            };
            let v = rules::steer(&e.position, &q);
            self.scratch.push((v.0, retarget));
        }

        // Pass B: write, in the same order.
        self.expired.clear();
        for (e, &(v, retarget)) in self.entities.iter_mut().zip(&self.scratch) {
            e.velocity = Velocity(v);
            if let Some(h) = retarget {
                e.target.entity = h;
            }
            rules::integrate(&mut e.position, &e.velocity, &mut e.lifetime, self.dt);
            if rules::is_expired(&e.lifetime) {
                self.expired.push((e.id.0, e.slot.0));
            }
        }

        // Respawn in place, in ascending slot order (the pass visits slots in order).
        for i in 0..self.expired.len() {
            let (_, slot) = self.expired[i];
            self.respawn(slot);
        }
    }

    fn checksum(&self) -> u64 {
        self.entities.iter().fold(0, |sum, e| {
            sum.wrapping_add(rules::hash(
                &e.id,
                &e.slot,
                &e.position,
                &e.velocity,
                &e.lifetime,
                &e.target,
            ))
        })
    }
}
