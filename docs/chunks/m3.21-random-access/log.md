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
- 2026-11-24 decision (maintainer) `hybrid` keeps all six `random_access` components in tables: nothing is added or removed; a sparse-`Position` variant is out of scope (#105).
- 2026-11-24 decision (maintainer) The lookup resolves the type once per pass on every backend: the four prototypes gain a read-only `World::lookup::<T>() -> Lookup` (spec §4.6), bevy uses a held `QueryState` with `get_manual`, hecs `World::view`. Amends the "no new prototype API" non-goal for this view only. Name `Lookup` (not `View`, which `sparse_set` uses for its query path) is the main session's call.
- 2026-11-24 decision (maintainer) Spec approved; status Accepted.
- 2026-11-24 done Lessons (`lessons-researcher`, registered agent): ecs-1, 2, 5, 6, 7, 9, 11; docs-1, 3; specs-1, 2, 3, 4, 6. All followed (plan §4).
- 2026-11-24 decision Spec §4.3 corrected without changing what is built: `hecs` stores the harness types unwrapped, so its view is `World::view::<&Position>()`, not `&Table<Position>`.
- 2026-11-24 surprise The cache bench's size check (`prepared`) asserts only that each listed size is in `Spec::SIZES`, at run time under Valgrind; a missing size passes. Plan step 9 checks the lines by review.
- 2026-11-24 done Plan written and accepted: 12 steps; agents dispatched by registered name (`builder`, reviewers).
- 2026-11-24 done Step 1: `spike_ecs_harness::random_access` added (components, `spawn`, `steer`, `integrate`, `is_expired`, `hash`, `GAIN`); 2 tests pass; first failure was E0425 (`spawn` not found).
- 2026-11-24 decision `spawn` panics on `count` 0 or above `u32::MAX` (documented, `#[track_caller]`, no `try_` twin: harness code, setup never calls it with 0).
- 2026-11-24 lesson? clippy assign_op_pattern rejects `p.0 = p.0 + v * dt`; write `p.0 += v * dt` (same f32 ops, so the spec's order is kept).
- 2026-11-24 decision (review fix) `integrate` takes `dt: f32` as its last argument; backends compute `dt()` once in setup.
- 2026-11-24 done Step 2: `spike_ecs_vec::random_access::VecRandomAccess` (vec_aos, E = u64, two passes); 3 tests pass; first failure was E0583 (module file not found). Retargets over 700 ticks at count 1000, seed 42: 1689.
- 2026-11-24 lesson? clippy denies `print_stdout` and `cast_possible_truncation` in spikes: a test that prints needs `#[allow(clippy::print_stdout, reason = ...)]`; use `u32::try_from(i).expect(..)` for slots.
- 2026-11-24 done Step 3: `archetype` gains `World::lookup`/`Lookup` (`src/world/lookup.rs`) and `random_access::ArchetypeRandomAccess`; 181 tests pass, `cargo xtask ci --quick` rc 0. First failure of the new tests was E0432 (`crate::random_access` missing) and E0599 (no `lookup`).
- 2026-11-24 surprise `World` already had a private `lookup(&mut self, &[ComponentId])` (the `by_components` reader), so `World::lookup::<T>()` collided (E0624). Renamed the private one to `find_by_components`; no behaviour change.
- 2026-11-24 decision `Lookup` is `Clone` and `Debug` but not `Copy` (spec §4.6 says `Copy`): its per-archetype column cache is a `Vec<Option<&'w [T]>>`, so that `get` does no `column_of` search. Spec wording needs the amendment.
- 2026-11-24 done Red check (ecs-6): liveness check removed from `Lookup::get`, `cargo nextest run -p spike_ecs_archetype --no-fail-fast`: failed `tests::lookup::lookup_agrees_with_get` and `tests::workloads::random_access_archetype_matches_the_reference` (2 of 181); `random_access_sees_a_reused_index` does not fail (it checks the workload, not `get`). Check restored.
- 2026-11-24 lesson? `GenId::from_bits` is index in the high half, generation in the low half: `from_bits(1)` is index 0, generation 1; `1 << 32` is invalid (generation 0).
