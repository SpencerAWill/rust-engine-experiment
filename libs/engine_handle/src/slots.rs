//! The slot table shared by [`HandlePool`](crate::HandlePool) and the allocators: generations,
//! the LIFO free list, retirement on generation exhaustion, and the slot limit.
//!
//! The table knows nothing about values. An occupied slot carries one `u32` of caller data (its
//! `link`): the pool stores the value's dense position there, the lean allocator stores `0`.

use alloc::collections::TryReserveError;
use alloc::vec::Vec;
use core::num::NonZeroU32;

/// The most slots a pool can hold: indices `0..MAX_SLOTS`.
///
/// A slot is 8 bytes: a generation and one `u32` that encodes its [`SlotState`].
/// That `u32` spends its top bit on "vacant", and reserves the two largest values as
/// sentinels (`NO_FREE` and `RETIRED`), which caps indices at `0x7FFF_FFFD`.
pub(crate) const MAX_SLOTS: u32 = 0x7FFF_FFFE;

/// Top bit of `Slot::link`: set for vacant and retired slots, clear for occupied ones.
pub(crate) const VACANT_BIT: u32 = 1 << 31;
/// Vacant slot at the end of the free list.
const NO_FREE: u32 = u32::MAX;
/// Retired slot.
const RETIRED: u32 = u32::MAX - 1;

#[derive(Debug, Clone)]
pub(crate) struct Slot {
    /// For an occupied slot, the live generation. For a vacant slot, the
    /// generation the next occupant receives. Retired slots stay at `u32::MAX`.
    generation: NonZeroU32,
    /// Packed [`SlotState`]; see [`Slot::state`].
    link: u32,
}

impl Slot {
    /// Decodes the packed state. Occupied slots have the top bit clear.
    #[inline]
    pub(crate) fn state(&self) -> SlotState {
        match self.link {
            RETIRED => SlotState::Retired,
            NO_FREE => SlotState::Vacant { next_free: None },
            link if link & VACANT_BIT != 0 => SlotState::Vacant {
                next_free: Some(link & !VACANT_BIT),
            },
            dense => SlotState::Occupied { dense },
        }
    }

    #[inline]
    pub(crate) fn set_state(&mut self, state: SlotState) {
        self.link = match state {
            SlotState::Occupied { dense } => {
                debug_assert!(dense < MAX_SLOTS, "dense position {dense} out of range");
                dense
            }
            SlotState::Vacant { next_free: None } => NO_FREE,
            SlotState::Vacant {
                next_free: Some(next),
            } => {
                debug_assert!(next < MAX_SLOTS, "free link {next} out of range");
                VACANT_BIT | next
            }
            SlotState::Retired => RETIRED,
        };
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SlotState {
    /// Holds the value at position `dense` of the dense arrays.
    Occupied { dense: u32 },
    /// Free; links to the next free slot (LIFO stack).
    Vacant { next_free: Option<u32> },
    /// Generation exhausted: never reused, so a stale handle can never become valid again.
    Retired,
}

/// Generation-checked slots with a LIFO free list.
#[derive(Debug, Clone)]
pub(crate) struct SlotTable {
    slots: Vec<Slot>,
    free_head: Option<u32>,
    limit: u32,
    /// Slots retired for good: neither occupied nor ever handed out again.
    retired: usize,
}

impl SlotTable {
    pub(crate) const fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_head: None,
            limit: MAX_SLOTS,
            retired: 0,
        }
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: Vec::with_capacity(capacity),
            ..Self::new()
        }
    }

    /// A table that holds at most `limit` slots, for testing the full path.
    #[cfg(test)]
    pub(crate) fn with_limit(limit: u32) -> Self {
        Self {
            limit,
            ..Self::new()
        }
    }

    /// Ensures capacity for `total` slots in all, counting those already created.
    pub(crate) fn try_reserve_slots(&mut self, total: usize) -> Result<(), TryReserveError> {
        self.slots
            .try_reserve(total.saturating_sub(self.slots.len()))
    }

    /// Slots the table can hold without reallocating.
    #[cfg(test)]
    pub(crate) fn capacity(&self) -> usize {
        self.slots.capacity()
    }

    /// The most slots this table will create.
    pub(crate) const fn limit(&self) -> u32 {
        self.limit
    }

    /// Takes a free slot (most recently freed first), or creates one, and marks it occupied
    /// with `link`. Returns its index and live generation, or `None` at the slot limit.
    pub(crate) fn acquire(&mut self, link: u32) -> Option<(u32, NonZeroU32)> {
        debug_assert!(link < MAX_SLOTS, "link {link} out of range");
        if let Some(index) = self.free_head {
            let slot = &mut self.slots[index as usize];
            let SlotState::Vacant { next_free } = slot.state() else {
                debug_assert!(false, "free list points at non-vacant slot {index}");
                return None;
            };
            self.free_head = next_free;
            slot.link = link;
            return Some((index, slot.generation));
        }
        if self.slots.len() >= self.limit as usize {
            return None;
        }
        let index = to_u32(self.slots.len());
        self.slots.push(Slot {
            generation: NonZeroU32::MIN,
            link,
        });
        Some((index, NonZeroU32::MIN))
    }

    /// The index and generation the next [`acquire`](Self::acquire) will return, without
    /// changing anything, or `None` at the slot limit.
    pub(crate) fn peek_next(&self) -> Option<(u32, NonZeroU32)> {
        if let Some(index) = self.free_head {
            Some((index, self.slots[index as usize].generation))
        } else if self.slots.len() < self.limit as usize {
            Some((to_u32(self.slots.len()), NonZeroU32::MIN))
        } else {
            None
        }
    }

    /// How many more slots can be acquired: the limit minus the occupied (`live`) and retired
    /// ones.
    pub(crate) fn obtainable(&self, live: usize) -> usize {
        (self.limit as usize).saturating_sub(live + self.retired)
    }

    /// Acquires exactly `n` slots, as `n` calls to [`acquire`](Self::acquire) would (free list
    /// first, then fresh slots in ascending order), calling `emit` with each.
    ///
    /// `n` must not exceed [`obtainable`](Self::obtainable); the caller checks it first.
    pub(crate) fn acquire_exactly(
        &mut self,
        n: usize,
        link: u32,
        mut emit: impl FnMut(u32, NonZeroU32),
    ) {
        debug_assert!(link < MAX_SLOTS, "link {link} out of range");
        let mut remaining = n;
        while remaining > 0 {
            let Some(index) = self.free_head else { break };
            let slot = &mut self.slots[index as usize];
            let SlotState::Vacant { next_free } = slot.state() else {
                debug_assert!(false, "free list points at non-vacant slot {index}");
                break;
            };
            self.free_head = next_free;
            slot.link = link;
            emit(index, slot.generation);
            remaining -= 1;
        }
        debug_assert!(
            self.slots.len() + remaining <= self.limit as usize,
            "acquire_exactly: {n} slots exceed what is obtainable"
        );
        let start = self.slots.len();
        self.slots.reserve(remaining);
        self.slots.extend(core::iter::repeat_n(
            Slot {
                generation: NonZeroU32::MIN,
                link,
            },
            remaining,
        ));
        for index in start..start + remaining {
            emit(to_u32(index), NonZeroU32::MIN);
        }
    }

    /// The raw link of slot `index` if its generation is `generation`, else `None`.
    ///
    /// The link is only meaningful for an occupied slot. Vacant and retired links are at least
    /// [`VACANT_BIT`], which exceeds any dense length, so a caller that range-checks the link
    /// against its own storage needs no separate state test.
    #[inline]
    pub(crate) fn lookup(&self, index: u32, generation: NonZeroU32) -> Option<u32> {
        let slot = self.slots.get(index as usize)?;
        (slot.generation == generation).then_some(slot.link)
    }

    /// Re-points occupied slot `index` at a new link (the pool's swap-remove).
    pub(crate) fn set_link(&mut self, index: u32, link: u32) {
        self.slots[index as usize].set_state(SlotState::Occupied { dense: link });
    }

    /// Releases occupied slot `index`: bumps its generation and pushes it on the free list, or
    /// retires it if the generation is exhausted.
    pub(crate) fn release(&mut self, index: u32) {
        let free_head = self.free_head;
        let slot = &mut self.slots[index as usize];
        if let Some(next) = slot.generation.checked_add(1) {
            slot.generation = next;
            slot.set_state(SlotState::Vacant {
                next_free: free_head,
            });
            self.free_head = Some(index);
        } else {
            slot.set_state(SlotState::Retired);
            self.retired += 1;
        }
    }

    /// Releases every occupied slot (bump or retire) and rebuilds the free list so that
    /// successive acquisitions yield ascending indices.
    pub(crate) fn release_all(&mut self) {
        for slot in &mut self.slots {
            if let SlotState::Occupied { .. } = slot.state() {
                if let Some(next) = slot.generation.checked_add(1) {
                    slot.generation = next;
                    slot.set_state(SlotState::Vacant { next_free: None });
                } else {
                    slot.set_state(SlotState::Retired);
                    self.retired += 1;
                }
            }
        }
        self.free_head = None;
        for (index, slot) in self.slots.iter_mut().enumerate().rev() {
            if let SlotState::Vacant { .. } = slot.state() {
                slot.set_state(SlotState::Vacant {
                    next_free: self.free_head,
                });
                self.free_head = Some(to_u32(index));
            }
        }
    }

    /// State of slot `index`, for invariant checks.
    #[cfg(any(test, feature = "test-invariants"))]
    pub(crate) fn state(&self, index: u32) -> SlotState {
        self.slots[index as usize].state()
    }

    /// Generation of slot `index`, for invariant checks.
    #[cfg(any(test, feature = "test-invariants"))]
    pub(crate) fn generation(&self, index: u32) -> NonZeroU32 {
        self.slots[index as usize].generation
    }

    /// Overwrites a slot's generation, for exhaustion tests.
    #[cfg(test)]
    pub(crate) fn set_generation(&mut self, index: u32, generation: NonZeroU32) {
        self.slots[index as usize].generation = generation;
    }

    /// Checks the table's own invariants and returns the number of occupied slots.
    ///
    /// # Panics
    ///
    /// Panics on the first violation: a retired slot not at the maximum generation, or a free
    /// list that cycles or misses a vacant slot.
    #[cfg(any(test, feature = "test-invariants"))]
    #[track_caller]
    pub(crate) fn assert_invariants(&self) -> usize {
        let mut occupied = 0;
        let mut vacant = 0;
        for (index, slot) in self.slots.iter().enumerate() {
            match slot.state() {
                SlotState::Occupied { .. } => occupied += 1,
                SlotState::Vacant { .. } => vacant += 1,
                SlotState::Retired => {
                    assert_eq!(
                        slot.generation,
                        NonZeroU32::MAX,
                        "retired slot {index} is not at MAX"
                    );
                }
            }
        }
        let mut visited = 0;
        let mut cursor = self.free_head;
        while let Some(index) = cursor {
            let SlotState::Vacant { next_free } = self.slots[index as usize].state() else {
                panic!("free list reaches non-vacant slot {index}");
            };
            visited += 1;
            assert!(
                visited <= vacant,
                "free list has a cycle or visits a slot twice"
            );
            cursor = next_free;
        }
        assert_eq!(visited, vacant, "free list misses vacant slots");
        let retired = self
            .slots
            .iter()
            .filter(|slot| matches!(slot.state(), SlotState::Retired))
            .count();
        assert_eq!(retired, self.retired, "retired count is out of sync");
        occupied
    }
}

/// Converts a length or position that is bounded by the slot limit (below [`MAX_SLOTS`], which
/// is under `u32::MAX`).
#[inline]
#[expect(
    clippy::cast_possible_truncation,
    reason = "bounded by the u32 slot limit; checked in debug builds"
)]
pub(crate) fn to_u32(n: usize) -> u32 {
    debug_assert!(u32::try_from(n).is_ok(), "{n} exceeds the u32 slot limit");
    n as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserve_slots_makes_room_so_acquiring_never_reallocates() {
        let mut table = SlotTable::new();
        table.try_reserve_slots(100).unwrap();
        let capacity = table.capacity();
        assert!(capacity >= 100);
        for _ in 0..100 {
            assert!(table.acquire(0).is_some());
        }
        assert_eq!(table.capacity(), capacity);
    }

    #[test]
    fn reserve_slots_counts_slots_already_created() {
        let mut table = SlotTable::new();
        for _ in 0..10 {
            let _ = table.acquire(0);
        }
        table.try_reserve_slots(10).unwrap(); // already satisfied: no growth needed
        let capacity = table.capacity();
        table.try_reserve_slots(10).unwrap();
        assert_eq!(table.capacity(), capacity);
    }

    #[test]
    fn slot_is_eight_bytes() {
        // The documented per-slot overhead: a u32 generation plus one u32 of state.
        assert_eq!(size_of::<Slot>(), 8);
    }

    #[test]
    fn slot_state_encoding_roundtrips_at_the_boundaries() {
        let last = MAX_SLOTS - 1; // highest valid slot index and dense position
        let states = [
            SlotState::Occupied { dense: 0 },
            SlotState::Occupied { dense: last },
            SlotState::Vacant { next_free: None },
            SlotState::Vacant { next_free: Some(0) },
            SlotState::Vacant {
                next_free: Some(last),
            },
            SlotState::Retired,
        ];
        for state in states {
            let mut slot = Slot {
                generation: NonZeroU32::MIN,
                link: 0,
            };
            slot.set_state(state);
            assert_eq!(slot.state(), state, "{state:?} did not roundtrip");
        }
        // Every state maps to a distinct encoding.
        let mut links = Vec::new();
        for state in states {
            let mut slot = Slot {
                generation: NonZeroU32::MIN,
                link: 0,
            };
            slot.set_state(state);
            links.push(slot.link);
        }
        links.sort_unstable();
        links.dedup();
        assert_eq!(links.len(), states.len());
    }
}
