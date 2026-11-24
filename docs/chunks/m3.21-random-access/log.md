# M3.21 log: random_access workload for the ECS storage harness

- **Status:** Living (2026-11-24).

<!-- Append-only. One or two lines per entry, written when it happens, not at the end:
- <date> <tag> <text>
Tags: done, decision, surprise, dead-end, lesson? (a candidate lesson for stage 6; see docs/lessons/README.md). -->

- 2026-11-24 done Picked by `/chunk next`: M3.21 (#106), order 11, the lowest unblocked Scheduled chunk in M3; no scheduled chores. Branch `m3.21-random-access-workload` pushed.
- 2026-11-24 surprise None of the four prototype `World`s can read one entity's component by handle while a mutable query over others runs: `query_mut` borrows the whole `World`, and no `QueryMut::get` or split borrow exists. `World::get` and `query` take `&self`, so a read-only pass with lookups works today.
- 2026-11-24 decision (maintainer) Two passes on every backend, baselines included: pass A a read-only query plus `World::get(target)` per entity into a reused scratch `Vec`; pass B a mutable query that writes the scratch back in the same order and integrates. No new prototype API; the lookup is the only cost that differs.
- 2026-11-24 decision (maintainer) Retarget to the slot's new occupant: the workload keeps `n` slots (a roster of handles), a respawn takes the dead entity's slot, `Target` holds (slot, handle), and a failed lookup re-reads the roster and reads the new occupant the same step. No draws after spawn.
- 2026-11-24 done Grill cold read (`spec-reader`, registered agent): 3 questions. Stub crate on the pinned toolchain compiled pass A (`query` with `World::get` inside) and pass B (nested tuples; prototype queries take at most four elements) against `archetype`.
- 2026-11-24 decision Main session's calls from the cold read: a respawn's `Target` takes `roster[t]` as it stands just before the spawn (no patch; a self or same-step slot is stale and resolves next step to the same entity); R5 binds the workload's own buffers only, not the prototypes' recorded per-query allocations; the guide sample uses nested tuples.
- 2026-11-24 surprise Every prototype `World::get` resolves the component by a `BTreeMap<TypeId, ComponentId>` lookup on each call, and so does bevy's `World::get`; bevy's `QueryState::get_manual` and hecs's `World::view` resolve it once. A per-row type lookup is not a storage cost.
