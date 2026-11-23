//! The raw operations of [`World`]: components registered at run time from a layout, stored as
//! bytes in byte columns beside the typed ones (M3.6).

use core::fmt;

use super::{ComponentInfo, ComponentKind, Location, World};
use crate::Entity;
use crate::archetype::{ArchetypeId, ComponentId};
use crate::raw::RawLayout;
use crate::table::Table;

/// Why a raw operation was rejected. The world is unchanged after each of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawError {
    /// The entity was despawned.
    DeadEntity(Entity),
    /// No component with this id is registered in the world.
    UnknownId(ComponentId),
    /// The id names a Rust type's component, which the raw operations do not serve.
    NotRaw(ComponentId),
    /// A value's length is not the layout's size.
    WrongSize {
        /// The component.
        id: ComponentId,
        /// The layout's size.
        expected: usize,
        /// The length of the slice given.
        got: usize,
    },
    /// The id was listed twice in one `spawn_raw`.
    Duplicate(ComponentId),
}

impl fmt::Display for RawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::DeadEntity(e) => write!(
                f,
                "entity {e:?} was despawned; raw operations need a live entity"
            ),
            Self::UnknownId(id) => write!(
                f,
                "component id {} is not registered in this world; register it with \
                 World::register_raw first",
                id.index()
            ),
            Self::NotRaw(id) => write!(
                f,
                "component id {} is a Rust type's; use the typed API",
                id.index()
            ),
            Self::WrongSize { id, expected, got } => write!(
                f,
                "component id {} holds {expected}-byte values but {got} bytes were given; \
                 pass exactly {expected} bytes",
                id.index()
            ),
            Self::Duplicate(id) => write!(
                f,
                "component id {} is listed twice; list each component once",
                id.index()
            ),
        }
    }
}

impl core::error::Error for RawError {}

impl World {
    /// Registers a component known only by its layout and returns its id. Every call returns a
    /// new id, even for a name and layout already registered; `name` is for `Debug` output and
    /// is not looked up.
    pub fn register_raw(&mut self, name: impl Into<Box<str>>, layout: RawLayout) -> ComponentId {
        let id = self.next_component_id();
        self.components.push(ComponentInfo {
            kind: ComponentKind::Raw(layout),
            name: name.into(),
        });
        id
    }

    /// The layout of raw component `id`.
    pub(crate) fn raw_layout(&self, id: ComponentId) -> Result<RawLayout, RawError> {
        match self.components.get(id.index()).map(|c| &c.kind) {
            None => Err(RawError::UnknownId(id)),
            Some(ComponentKind::Rust { .. }) => Err(RawError::NotRaw(id)),
            Some(ComponentKind::Raw(layout)) => Ok(*layout),
        }
    }

    /// `id`'s layout, if `bytes` is the right length for it.
    fn raw_value_layout(&self, id: ComponentId, bytes: &[u8]) -> Result<RawLayout, RawError> {
        let layout = self.raw_layout(id)?;
        if bytes.len() == layout.size() {
            Ok(layout)
        } else {
            Err(RawError::WrongSize {
                id,
                expected: layout.size(),
                got: bytes.len(),
            })
        }
    }

    /// Creates an entity with the raw components in `components`, placed straight into the
    /// table for exactly those ids, in any order, and written in one operation. An empty list
    /// makes an entity with no components. A new set of ids creates one archetype; a known set
    /// creates none.
    ///
    /// # Errors
    ///
    /// Returns [`RawError::UnknownId`], [`RawError::NotRaw`], [`RawError::WrongSize`] or
    /// [`RawError::Duplicate`] for the first entry that is wrong. Every entry is checked before
    /// an archetype is looked up or an entity allocated, so the world is unchanged.
    ///
    /// # Panics
    ///
    /// Panics with a `World::spawn_raw:` message if the world holds the most entities it can
    /// (2³¹−2 entity slots in use or retired). `len`, the archetypes and every existing entity
    /// are unchanged.
    #[track_caller]
    pub fn spawn_raw(&mut self, components: &[(ComponentId, &[u8])]) -> Result<Entity, RawError> {
        for &(id, bytes) in components {
            self.raw_value_layout(id, bytes)?;
        }
        let mut sorted = components.to_vec();
        sorted.sort_unstable_by_key(|(id, _)| *id);
        if let Some(pair) = sorted.windows(2).find(|pair| pair[0].0 == pair[1].0) {
            return Err(RawError::Duplicate(pair[0].0));
        }
        let Some(entity) = self.entities.try_allocate() else {
            panic!(
                "World::spawn_raw: the world holds the most entities it can (every entity slot \
                 is in use or retired); despawn entities to free slots"
            )
        };
        let ids: Box<[ComponentId]> = sorted.iter().map(|(id, _)| *id).collect();
        let archetype = self.find_or_create(ids);
        let table = &mut self.archetypes[archetype.index()].table;
        let row = table.len();
        table.push_raw_row(entity, &sorted);
        self.set_location(entity, archetype, row);
        self.live += 1;
        Ok(entity)
    }

    /// Stores `bytes` as the value of raw component `id` of `entity`. If the entity had none,
    /// it moves to the archetype with `id` added; otherwise the value is replaced in place.
    /// Returns whether a value was replaced.
    ///
    /// # Errors
    ///
    /// Returns [`RawError::DeadEntity`], [`RawError::UnknownId`], [`RawError::NotRaw`] or
    /// [`RawError::WrongSize`], checked in that order, with the world unchanged.
    pub fn insert_raw(
        &mut self,
        entity: Entity,
        id: ComponentId,
        bytes: &[u8],
    ) -> Result<bool, RawError> {
        if !self.entities.is_alive(entity) {
            return Err(RawError::DeadEntity(entity));
        }
        self.raw_value_layout(id, bytes)?;
        let Location { archetype, row } = self.location_of(entity);
        let (from, row) = (archetype.index(), row as usize);
        if let Some(col) = self.archetypes[from].column_of(id) {
            self.archetypes[from].table.overwrite_raw(row, col, bytes);
            return Ok(true);
        }
        let to = self.archetype_after_adding(archetype, id).index();
        let (src, dst) = self.two_archetypes(from, to);
        let swapped = Table::move_row_adding_raw(
            &mut src.table,
            &src.components,
            row,
            &mut dst.table,
            &dst.components,
            bytes,
        );
        let new_row = dst.table.len() - 1;
        self.finish_move(entity, ArchetypeId::new(to), new_row, swapped, row);
        Ok(false)
    }

    /// Removes raw component `id` from `entity`, moving it to the archetype with `id` removed.
    /// Returns `false` if the entity had none.
    ///
    /// # Errors
    ///
    /// Returns [`RawError::DeadEntity`], [`RawError::UnknownId`] or [`RawError::NotRaw`],
    /// checked in that order, with the world unchanged.
    pub fn remove_raw(&mut self, entity: Entity, id: ComponentId) -> Result<bool, RawError> {
        if !self.entities.is_alive(entity) {
            return Err(RawError::DeadEntity(entity));
        }
        self.raw_layout(id)?;
        let Location { archetype, row } = self.location_of(entity);
        let (from, row) = (archetype.index(), row as usize);
        if self.archetypes[from].column_of(id).is_none() {
            return Ok(false);
        }
        let to = self.archetype_after_removing(archetype, id).index();
        let (src, dst) = self.two_archetypes(from, to);
        let swapped = Table::move_row_removing_raw(
            &mut src.table,
            &src.components,
            row,
            &mut dst.table,
            &dst.components,
        );
        let new_row = dst.table.len() - 1;
        self.finish_move(entity, ArchetypeId::new(to), new_row, swapped, row);
        Ok(true)
    }

    /// The bytes of raw component `id` of `entity`, or `None` if the entity is despawned, the
    /// id is unknown or a Rust type's, or the entity has none.
    #[must_use]
    pub fn get_raw(&self, entity: Entity, id: ComponentId) -> Option<&[u8]> {
        if !self.entities.is_alive(entity) {
            return None;
        }
        self.raw_layout(id).ok()?;
        let Location { archetype, row } = self.location_of(entity);
        let archetype = &self.archetypes[archetype.index()];
        let col = archetype.column_of(id)?;
        archetype.table.raw_value(row as usize, col)
    }
}

#[cfg(test)]
mod tests {
    use core::any::TypeId;

    use engine::genid::GenIdAllocator;

    use super::super::*;
    use super::*;

    #[derive(Debug, PartialEq)]
    struct T(u32);

    fn lay(size: usize, align: usize) -> RawLayout {
        RawLayout::new(size, align).unwrap()
    }

    /// `size` bytes that differ with `seed` and with the position.
    fn pattern(size: usize, seed: u8) -> Vec<u8> {
        (0..size)
            .map(|i| seed.wrapping_add(u8::try_from(i).unwrap()))
            .collect()
    }

    fn location(w: &World, e: Entity) -> Location {
        w.locations[e.index() as usize]
    }

    fn message(payload: &(dyn core::any::Any + Send)) -> &str {
        payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or_default()
    }

    /// A world with a 4-byte raw component `a` and an 8-byte one `b`.
    fn world_ab() -> (World, ComponentId, ComponentId) {
        let mut w = World::new();
        let a = w.register_raw("a", lay(4, 4));
        let b = w.register_raw("b", lay(8, 4));
        (w, a, b)
    }

    /// A world whose only entity holds the typed `T`, and `T`'s id.
    fn world_with_typed() -> (World, Entity, ComponentId) {
        let mut w = World::new();
        let e = w.spawn();
        w.insert(e, T(1));
        let t = w.component_id::<T>().unwrap();
        (w, e, t)
    }

    fn assert_world(w: &World, len: usize, archetypes: usize) {
        assert_eq!(w.len(), len);
        assert_eq!(w.archetype_count(), archetypes);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn register_raw_returns_a_new_id_every_call() {
        let mut w = World::new();
        let layout = lay(4, 4);
        let first = w.register_raw("same", layout);
        let second = w.register_raw("same", layout);
        let third = w.register_raw(String::from("other"), lay(16, 16));
        assert_ne!(first, second);
        assert_ne!(second, third);
        assert_ne!(first, third);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn raw_and_rust_ids_never_coincide() {
        let mut w = World::new();
        let r0 = w.register_raw("r0", lay(4, 4));
        let e = w.spawn();
        w.insert(e, T(1));
        let t = w.component_id::<T>().unwrap();
        let r1 = w.register_raw("r1", lay(4, 4));
        assert!(r0 != t && r1 != t && r0 != r1);
        assert_eq!(w.ids.len(), 1, "the TypeId map holds only Rust types");
        assert_eq!(w.component_id_of(TypeId::of::<T>()), Some(t));
    }

    #[test]
    fn not_raw_spawn_raw() {
        let (mut w, e, t) = world_with_typed();
        let (len, count) = (w.len(), w.archetype_count());
        assert_eq!(w.spawn_raw(&[(t, &[0u8; 4][..])]), Err(RawError::NotRaw(t)));
        assert_world(&w, len, count);
        assert_eq!(w.get::<T>(e), Some(&T(1)));
    }

    #[test]
    fn not_raw_insert_raw() {
        let (mut w, e, t) = world_with_typed();
        let (len, count) = (w.len(), w.archetype_count());
        assert_eq!(w.insert_raw(e, t, &[0u8; 4]), Err(RawError::NotRaw(t)));
        assert_world(&w, len, count);
        assert_eq!(w.get::<T>(e), Some(&T(1)));
    }

    #[test]
    fn not_raw_remove_raw() {
        let (mut w, e, t) = world_with_typed();
        let (len, count) = (w.len(), w.archetype_count());
        assert_eq!(w.remove_raw(e, t), Err(RawError::NotRaw(t)));
        assert_world(&w, len, count);
        assert_eq!(w.get::<T>(e), Some(&T(1)));
    }

    #[test]
    fn not_raw_get_raw_is_none() {
        let (w, e, t) = world_with_typed();
        assert_eq!(w.get_raw(e, t), None);
        assert_world(&w, 1, 2);
    }

    #[test]
    fn spawn_raw_places_the_entity_in_its_final_archetype() {
        let (mut w, a, b) = world_ab();
        let e = w
            .spawn_raw(&[(b, &[2u8; 8][..]), (a, &[1u8; 4][..])])
            .unwrap();
        assert_world(&w, 1, 2);
        let at = location(&w, e);
        assert_eq!(at.archetype, ArchetypeId::new(1));
        assert_eq!(at.row, 0);
        assert_eq!(&*w.archetypes[1].components, &[a, b]);
        assert_eq!(w.get_raw(e, a), Some(&[1u8; 4][..]));
        assert_eq!(w.get_raw(e, b), Some(&[2u8; 8][..]));
    }

    #[test]
    fn spawn_raw_creates_one_archetype_for_a_new_set_and_none_for_a_known_one() {
        let (mut w, a, b) = world_ab();
        w.spawn_raw(&[(a, &[1u8; 4][..]), (b, &[2u8; 8][..])])
            .unwrap();
        assert_world(&w, 1, 2);
        w.spawn_raw(&[(b, &[3u8; 8][..]), (a, &[4u8; 4][..])])
            .unwrap();
        assert_world(&w, 2, 2);
        w.spawn_raw(&[(a, &[5u8; 4][..])]).unwrap();
        assert_world(&w, 3, 3);
    }

    #[test]
    fn spawn_raw_sorts_its_entries() {
        let mut w = World::new();
        let ids = [
            w.register_raw("x", lay(4, 4)),
            w.register_raw("y", lay(8, 4)),
            w.register_raw("z", lay(2, 2)),
        ];
        let values = [pattern(4, 10), pattern(8, 20), pattern(2, 30)];
        let e = w
            .spawn_raw(&[
                (ids[2], &values[2][..]),
                (ids[0], &values[0][..]),
                (ids[1], &values[1][..]),
            ])
            .unwrap();
        for (id, value) in ids.iter().zip(&values) {
            assert_eq!(w.get_raw(e, *id), Some(&value[..]));
        }
        assert_world(&w, 1, 2);
    }

    #[test]
    fn spawn_raw_with_no_components() {
        let mut w = World::new();
        let e = w.spawn_raw(&[]).unwrap();
        assert_world(&w, 1, 1);
        assert_eq!(location(&w, e).archetype, ArchetypeId::EMPTY);
        assert!(w.is_alive(e));
    }

    #[test]
    fn insert_raw_adds_and_moves() {
        let (mut w, a, b) = world_ab();
        let e = w.spawn();
        assert_eq!(w.insert_raw(e, a, &[1u8; 4]), Ok(false));
        assert_world(&w, 1, 2);
        let first = location(&w, e).archetype;
        assert_ne!(first, ArchetypeId::EMPTY);
        assert_eq!(w.insert_raw(e, b, &[2u8; 8]), Ok(false));
        assert_world(&w, 1, 3);
        assert_ne!(location(&w, e).archetype, first);
        assert_eq!(w.get_raw(e, a), Some(&[1u8; 4][..]));
        assert_eq!(w.get_raw(e, b), Some(&[2u8; 8][..]));
    }

    #[test]
    fn insert_raw_replaces_in_place() {
        let (mut w, a, _) = world_ab();
        let e = w.spawn();
        assert_eq!(w.insert_raw(e, a, &[1u8; 4]), Ok(false));
        assert_eq!(w.insert_raw(e, a, &[9u8; 4]), Ok(true));
        assert_eq!(w.get_raw(e, a), Some(&[9u8; 4][..]));
        assert_world(&w, 1, 2);
    }

    #[test]
    fn replacing_a_raw_value_does_not_move_the_entity() {
        let (mut w, a, _) = world_ab();
        let e = w.spawn();
        let other = w.spawn();
        w.insert_raw(e, a, &[1u8; 4]).unwrap();
        w.insert_raw(other, a, &[2u8; 4]).unwrap();
        let (at, count) = (location(&w, e), w.archetype_count());
        assert_eq!(w.insert_raw(e, a, &[7u8; 4]), Ok(true));
        assert_eq!(location(&w, e), at);
        assert_eq!(w.archetype_count(), count);
        assert_eq!(w.get_raw(e, a), Some(&[7u8; 4][..]));
        assert_eq!(w.get_raw(other, a), Some(&[2u8; 4][..]));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn remove_raw_moves_and_drops() {
        let (mut w, a, b) = world_ab();
        let e = w
            .spawn_raw(&[(a, &[1u8; 4][..]), (b, &[2u8; 8][..])])
            .unwrap();
        assert_eq!(w.remove_raw(e, a), Ok(true));
        assert_eq!(w.get_raw(e, a), None);
        assert_eq!(w.get_raw(e, b), Some(&[2u8; 8][..]));
        assert_world(&w, 1, 3);
        assert_eq!(w.remove_raw(e, b), Ok(true));
        assert_eq!(location(&w, e).archetype, ArchetypeId::EMPTY);
        assert_world(&w, 1, 3);
    }

    #[test]
    fn remove_raw_of_a_missing_component_is_false() {
        let (mut w, a, _) = world_ab();
        let e = w.spawn();
        assert_eq!(w.remove_raw(e, a), Ok(false));
        assert_world(&w, 1, 1);
    }

    #[test]
    fn get_raw_returns_the_bytes() {
        let (mut w, a, b) = world_ab();
        let e = w.spawn_raw(&[(a, &[1u8; 4][..])]).unwrap();
        assert_eq!(w.get_raw(e, a), Some(&[1u8; 4][..]));
        assert_eq!(w.get_raw(e, b), None);
    }

    #[test]
    fn get_raw_is_none_for_each_missing_case() {
        let (mut w, a, b) = world_ab();
        let live = w.spawn_raw(&[(a, &[1u8; 4][..])]).unwrap();
        let dead = w.spawn();
        w.despawn(dead);
        assert_eq!(w.get_raw(dead, a), None, "a dead entity");
        assert_eq!(w.get_raw(live, ComponentId::new(99)), None, "an unknown id");
        assert_eq!(w.get_raw(live, b), None, "a missing component");
        let (w2, e2, t) = world_with_typed();
        assert_eq!(w2.get_raw(e2, t), None, "a Rust type's id");
    }

    #[test]
    fn dead_entity_is_rejected_cold() {
        let (mut w, a, _) = world_ab();
        let e = w.spawn();
        w.despawn(e);
        assert_eq!(w.insert_raw(e, a, &[1u8; 4]), Err(RawError::DeadEntity(e)));
        assert_world(&w, 0, 1);
        let (mut w, a, _) = world_ab();
        let e = w.spawn();
        w.despawn(e);
        assert_eq!(w.remove_raw(e, a), Err(RawError::DeadEntity(e)));
        assert_world(&w, 0, 1);
    }

    #[test]
    fn unknown_id_is_rejected_cold() {
        let unknown = ComponentId::new(9);
        let (mut w, a, _) = world_ab();
        assert_eq!(
            w.spawn_raw(&[(a, &[1u8; 4][..]), (unknown, &[0u8; 4][..])]),
            Err(RawError::UnknownId(unknown))
        );
        assert_world(&w, 0, 1);
        assert_eq!(w.spawn().index(), 0, "no entity slot was taken");
        let (mut w, _, _) = world_ab();
        let e = w.spawn();
        assert_eq!(
            w.insert_raw(e, unknown, &[0u8; 4]),
            Err(RawError::UnknownId(unknown))
        );
        assert_world(&w, 1, 1);
        assert_eq!(w.remove_raw(e, unknown), Err(RawError::UnknownId(unknown)));
        assert_world(&w, 1, 1);
    }

    #[test]
    fn wrong_size_is_rejected_cold() {
        let (mut w, a, b) = world_ab();
        let wrong = RawError::WrongSize {
            id: b,
            expected: 8,
            got: 3,
        };
        assert_eq!(
            w.spawn_raw(&[(a, &[1u8; 4][..]), (b, &[0u8; 3][..])]),
            Err(wrong)
        );
        assert_world(&w, 0, 1);
        assert_eq!(w.spawn().index(), 0, "no entity slot was taken");
        let (mut w, a, _) = world_ab();
        let e = w.spawn();
        assert_eq!(
            w.insert_raw(e, a, &[1, 2, 3]),
            Err(RawError::WrongSize {
                id: a,
                expected: 4,
                got: 3
            })
        );
        assert_world(&w, 1, 1);
        assert_eq!(location(&w, e).archetype, ArchetypeId::EMPTY);
    }

    #[test]
    fn duplicate_is_rejected_cold() {
        let (mut w, a, b) = world_ab();
        assert_eq!(
            w.spawn_raw(&[(a, &[1u8; 4][..]), (b, &[2u8; 8][..]), (a, &[3u8; 4][..])]),
            Err(RawError::Duplicate(a))
        );
        assert_world(&w, 0, 1);
        assert_eq!(w.spawn().index(), 0, "no entity slot was taken");
    }

    #[test]
    fn a_failed_cold_spawn_raw_leaves_the_world_unchanged() {
        let (mut w, a, _) = world_ab();
        w.entities = GenIdAllocator::with_fixed_capacity(0);
        let failed = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
            w.spawn_raw(&[(a, &[1u8; 4][..])])
        }));
        let payload = failed.unwrap_err();
        assert!(
            message(&*payload).starts_with("World::spawn_raw:"),
            "got: {}",
            message(&*payload)
        );
        assert_world(&w, 0, 1);
    }

    #[test]
    fn despawn_drops_raw_values() {
        let (mut w, a, b) = world_ab();
        let es: Vec<Entity> = (0..3u8)
            .map(|i| {
                w.spawn_raw(&[(a, &[i; 4][..]), (b, &[i + 10; 8][..])])
                    .unwrap()
            })
            .collect();
        assert!(w.despawn(es[0]));
        assert_eq!(w.get_raw(es[2], a), Some(&[2u8; 4][..]));
        assert_eq!(w.archetypes[1].table.len(), 2);
        assert!(w.despawn(es[2]));
        assert!(w.despawn(es[1]));
        assert_eq!(w.archetypes[1].table.len(), 0);
        assert_eq!(w.archetypes[1].table.column(0).len(), 0);
        assert_world(&w, 0, 2);
    }

    #[test]
    fn raw_toggle_creates_no_archetype_after_the_first() {
        let (mut w, a, b) = world_ab();
        let e = w.spawn();
        let cycle = |w: &mut World, e: Entity| {
            w.insert_raw(e, a, &[1u8; 4]).unwrap();
            w.insert_raw(e, b, &[2u8; 8]).unwrap();
            w.remove_raw(e, b).unwrap();
            w.remove_raw(e, a).unwrap();
        };
        cycle(&mut w, e);
        let count = w.archetype_count();
        assert_eq!(count, 3);
        for _ in 0..10 {
            cycle(&mut w, e);
        }
        assert_eq!(w.archetype_count(), count);
        assert_eq!(location(&w, e).archetype, ArchetypeId::EMPTY);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn raw_toggle_reads_the_edge_cache() {
        let (mut w, a, b) = world_ab();
        let cycle = |w: &mut World, e: Entity| {
            w.insert_raw(e, a, &[1u8; 4]).unwrap();
            w.insert_raw(e, b, &[2u8; 8]).unwrap();
            w.remove_raw(e, b).unwrap();
            w.remove_raw(e, a).unwrap();
        };
        let first = w.spawn();
        cycle(&mut w, first);
        let (lookups, count) = (w.lookups, w.archetype_count());
        assert!(
            lookups > 0,
            "the cold transitions must have looked archetypes up"
        );
        let second = w.spawn();
        cycle(&mut w, second);
        assert_eq!(w.lookups, lookups);
        assert_eq!(w.archetype_count(), count);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn mixed_world_is_seen_by_both_kinds() {
        let (mut w, a, _) = world_ab();
        let both = w.spawn();
        w.insert(both, T(5));
        w.insert_raw(both, a, &[6u8; 4]).unwrap();
        let typed_only = w.spawn();
        w.insert(typed_only, T(7));
        let raw_only = w.spawn_raw(&[(a, &[8u8; 4][..])]).unwrap();
        let mut seen: Vec<(Entity, u32)> = w
            .query::<(Entity, &T)>()
            .into_iter()
            .map(|(e, t)| (e, t.0))
            .collect();
        seen.sort_by_key(|(e, _)| e.to_bits());
        assert_eq!(seen, [(both, 5), (typed_only, 7)]);
        assert_eq!(w.get::<T>(both), Some(&T(5)));
        assert_eq!(w.get_raw(both, a), Some(&[6u8; 4][..]));
        assert_eq!(w.get::<T>(raw_only), None);
        assert_eq!(w.get_raw(raw_only, a), Some(&[8u8; 4][..]));
        assert_eq!(w.get_raw(typed_only, a), None);
        assert_eq!(w.remove::<T>(both), Some(T(5)));
        assert_eq!(w.get_raw(both, a), Some(&[6u8; 4][..]));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn stale_id_does_not_reach_the_reused_slot_raw() {
        let (mut w, a, _) = world_ab();
        let old = w.spawn_raw(&[(a, &[1u8; 4][..])]).unwrap();
        assert!(w.despawn(old));
        let new = w.spawn();
        assert_eq!(new.index(), old.index(), "the slot is reused");
        assert_ne!(old, new);
        assert_eq!(w.get_raw(old, a), None);
        assert_eq!(
            w.insert_raw(old, a, &[9u8; 4]),
            Err(RawError::DeadEntity(old))
        );
        assert_eq!(w.remove_raw(old, a), Err(RawError::DeadEntity(old)));
        assert_eq!(
            w.get_raw(new, a),
            None,
            "the new entity has none of the old values"
        );
        assert_world(&w, 1, 2);
    }

    #[test]
    fn raw_remove_fixes_the_moved_row() {
        let (mut w, a, b) = world_ab();
        let es: Vec<Entity> = (0..4u8)
            .map(|i| {
                w.spawn_raw(&[(a, &[i; 4][..]), (b, &[i + 10; 8][..])])
                    .unwrap()
            })
            .collect();
        // A middle row: the last entity is swapped into its place.
        assert_eq!(w.remove_raw(es[1], a), Ok(true));
        assert_eq!(location(&w, es[3]).row, 1);
        assert_eq!(w.get_raw(es[3], a), Some(&[3u8; 4][..]));
        assert_eq!(w.get_raw(es[3], b), Some(&[13u8; 8][..]));
        assert_eq!(w.get_raw(es[1], b), Some(&[11u8; 8][..]));
        assert_eq!(w.check_invariants(), Ok(()));
        // The last row: nothing is swapped.
        assert_eq!(w.remove_raw(es[2], b), Ok(true));
        assert_eq!(location(&w, es[3]).row, 1);
        assert_eq!(w.get_raw(es[0], a), Some(&[0u8; 4][..]));
        assert_eq!(w.get_raw(es[3], b), Some(&[13u8; 8][..]));
        assert_eq!(w.get_raw(es[2], a), Some(&[2u8; 4][..]));
        assert_eq!(w.check_invariants(), Ok(()));
        // Despawn a middle row (es[3] is at row 1 of three), then the last.
        assert!(w.despawn(es[3]));
        assert_eq!(location(&w, es[0]).row, 0);
        assert_eq!(w.get_raw(es[0], b), Some(&[10u8; 8][..]));
        assert!(w.despawn(es[0]));
        assert_eq!(w.check_invariants(), Ok(()));
        assert_eq!(w.len(), 2);
    }

    #[test]
    fn mixed_layouts_move_together() {
        let mut w = World::new();
        let ids = [
            w.register_raw("x", lay(1, 1)),
            w.register_raw("y", lay(12, 4)),
            w.register_raw("z", lay(16, 16)),
        ];
        let sizes = [1, 12, 16];
        let value = |e: usize, r: usize| pattern(sizes[r], u8::try_from(e * 3 + r).unwrap() * 7);
        let es: Vec<Entity> = (0..4)
            .map(|e| {
                let vs: Vec<Vec<u8>> = (0..3).map(|r| value(e, r)).collect();
                let entries: Vec<(ComponentId, &[u8])> = ids
                    .iter()
                    .copied()
                    .zip(vs.iter().map(Vec::as_slice))
                    .collect();
                w.spawn_raw(&entries).unwrap()
            })
            .collect();
        // model[e][r]
        let mut model: Vec<[Option<Vec<u8>>; 3]> = (0..4)
            .map(|e| core::array::from_fn(|r| Some(value(e, r))))
            .collect();
        let check = |w: &World, model: &Vec<[Option<Vec<u8>>; 3]>, live: &[bool]| {
            for e in 0..4 {
                for r in 0..3 {
                    let want = if live[e] {
                        model[e][r].as_deref()
                    } else {
                        None
                    };
                    assert_eq!(w.get_raw(es[e], ids[r]), want, "entity {e} component {r}");
                }
            }
            assert_eq!(w.check_invariants(), Ok(()));
        };
        let mut live = [true; 4];
        check(&w, &model, &live);
        assert_eq!(w.remove_raw(es[0], ids[1]), Ok(true));
        model[0][1] = None;
        check(&w, &model, &live);
        let new = pattern(12, 99);
        assert_eq!(w.insert_raw(es[0], ids[1], &new), Ok(false));
        model[0][1] = Some(new);
        check(&w, &model, &live);
        assert_eq!(w.remove_raw(es[1], ids[0]), Ok(true));
        model[1][0] = None;
        assert_eq!(w.remove_raw(es[2], ids[2]), Ok(true));
        model[2][2] = None;
        check(&w, &model, &live);
        assert!(w.despawn(es[3]));
        live[3] = false;
        check(&w, &model, &live);
        assert_eq!(w.remove_raw(es[2], ids[1]), Ok(true));
        model[2][1] = None;
        assert_eq!(w.insert_raw(es[2], ids[2], &pattern(16, 5)), Ok(false));
        model[2][2] = Some(pattern(16, 5));
        check(&w, &model, &live);
    }

    #[test]
    fn raw_errors_say_what_to_do() {
        let id = ComponentId::new(3);
        let text = RawError::NotRaw(id).to_string();
        assert!(text.contains("typed API"), "got: {text}");
        let text = RawError::UnknownId(id).to_string();
        assert!(
            text.contains('3') && text.contains("register_raw"),
            "got: {text}"
        );
        let text = RawError::WrongSize {
            id,
            expected: 8,
            got: 3,
        }
        .to_string();
        assert!(text.contains('8') && text.contains('3'), "got: {text}");
        let text = RawError::Duplicate(id).to_string();
        assert!(text.contains("twice"), "got: {text}");
        let (mut w, a, _) = world_ab();
        let e = w.spawn();
        w.despawn(e);
        let err = w.remove_raw(e, a).unwrap_err();
        assert!(err.to_string().contains("despawned"), "got: {err}");
        let boxed: Box<dyn std::error::Error> = Box::new(err);
        assert!(boxed.to_string().contains("despawned"));
    }
}
