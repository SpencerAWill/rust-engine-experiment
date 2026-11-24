# M3.21 plan: random_access workload for the ECS storage harness

- **Status:** Accepted (2026-11-24)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. Progress and surprises go in [log.md](log.md).

## 1. Approach

The harness module comes first, then the reference, so every later step has something to conform to. Each prototype gets its `Lookup` and its `random_access` backend in one step, because the backend is `Lookup`'s first user: `archetype` first (it carries the reused-index test of R4), then `sparse_set`, `dense`, `hybrid`. Then the two baselines, bench registration, documents, the allocation count (ecs-2) and the informal run last, on the final code (specs-4).

Choices the spec leaves to implementation, fixed here:

- **Names:** module `random_access` in each crate; types `VecRandomAccess`, `ArchetypeRandomAccess`, `SparseSetRandomAccess`, `DenseRandomAccess`, `HybridRandomAccess`, `BevyRandomAccess`, `HecsRandomAccess`; `BACKEND` = `"vec_aos"`, `"archetype"`, `"sparse_set"`, `"dense"`, `"hybrid"`, `"bevy_ecs"`, `"hecs"`.
- **Harness signatures:** `pub fn steer(position: &Position, target: &Position) -> Velocity`; `pub fn integrate(position: &mut Position, velocity: &Velocity, lifetime: &mut Lifetime)`; `pub fn is_expired(lifetime: &Lifetime) -> bool`; `pub fn hash<E>(id: &SpawnId, slot: &Slot, position: &Position, velocity: &Velocity, lifetime: &Lifetime, target: &Target<E>) -> u64`; `pub const GAIN: f32 = 0.5`. `Target<E>` derives `Clone, Copy, Debug, PartialEq, Eq`.
- **`Lookup`:** in each prototype, `pub struct Lookup<'w, T>` in `world.rs` (or a `lookup.rs` beside it), re-exported from the crate root as `World` is; fields private; it holds `&'w World` plus what it resolved, and hands out only `&'w T` (ecs-5: no column, storage or `&mut` accessor leaves through it). `archetype` and `hybrid` cache a `Vec<Option<usize>>` of `T`'s column per archetype, built in `World::lookup`.
- **Scratch:** `scratch: Vec<(Velocity, Option<E>)>` and `expired: Vec<(E, u32)>`, both `with_capacity(count)` in `setup`. Pass B sorts `expired` by slot with `sort_unstable_by_key` (slots are unique) unless the backend's pass B already visits slots in order.
- **Test hooks:** `#[cfg(test)] pub(crate) fn world(&self) -> &World` and `fn roster(&self) -> &[Entity]` on `ArchetypeRandomAccess` and `HybridRandomAccess`, for R4 and the storage test.
- **Zero count:** `setup` with `count` 0 spawns nothing and draws nothing; every backend's test list includes the zero-count case, as the other workloads' do.

## 2. Steps

Every conformance test below is `check_conformance::<VecRandomAccess, X>(Params { count: 1_000, seed: 42 }, 700)`, named `random_access_<backend>_matches_the_reference`.

### Step 1: the harness module (R1)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/harness/src/random_access.rs` (new), `harness/src/lib.rs` (`pub mod random_access;`, crate doc: eight workloads, `random_access`'s rules in the M3.21 spec §4.2).
- **Test first:** in `random_access.rs`'s test module, `spawn_follows_the_draw_order` (for `i` in `[0, 1, 999]` and `count` 1,000, `spawn` equals direct draws from `prng.stream(i)` in §4.2's order, velocity zero) and `steer_and_integrate_follow_the_spec_order` (hand-computed values for one entity, `DT` from `dt()`). Fails first: the module does not exist (E0432).
- **Change:** spec §4.2's components, `RandomAccess` with its `NAME` and `SIZES`, and the functions above, each documented.
- **Verify:** `cargo nextest run -p spike_ecs_harness` passes, with the 2 new tests listed. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add the random_access workload rules to the harness (M3.21)`

### Step 2: the `vec_aos` reference (R2, R4)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/vec/src/random_access.rs` (new), `vec/src/lib.rs`, `vec/src/tests.rs`.
- **Test first:** `random_access_zero_count_is_valid` (the existing `assert_zero_count_is_valid`); `random_access_retargets_within_conformance_ticks`: steps `count` 1,000, seed 42, for 700 ticks, counting each tick the entities whose `Target::entity` changed while their `Target::slot` did not, and asserts the total is above 0 (print it); `random_access_respawns_fill_slots_in_order`: after a step with expiries, the new spawn ids increase with slot. Fails first: the type does not exist.
- **Change:** spec §4.4: `Vec` of structs indexed by slot, `E = u64`, two passes, respawn in place in ascending slot order. No order-independence test: reversing the `Vec` would break the slot index, which is the storage; say so in a comment.
- **Verify:** `cargo nextest run -p spike_ecs_vec` passes; the retarget count is printed with `--no-capture` and is above 0. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add the random_access reference on vec_aos (M3.21)`

### Step 3: `archetype`: `Lookup` and the backend (R3, R4, R5, R10)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/archetype/src/world.rs` (or `src/lookup.rs`), `src/lib.rs`, `src/random_access.rs` (new), `src/tests.rs`.
- **Test first:**
  - `lookup_agrees_with_get`: one test with four asserts, one per spec §4.6 case (live with `T`, live without `T`, despawned, stale with its index reused, the last made by despawning then spawning until `index()` matches with a higher generation), each comparing `world.lookup::<T>().get(e)` with `world.get::<T>(e)`; plus `lookup_of_an_unregistered_type_is_none`.
  - `random_access_archetype_matches_the_reference`, and `random_access_archetype_zero_count_is_valid`.
  - `random_access_sees_a_reused_index` (R4): over 700 ticks at `count` 1,000, at some tick an entity's `Target::entity` is not alive while `roster()[slot]` has the same `index()` and a higher generation.
  - Red check (ecs-6): with the liveness check removed from `Lookup::get`, run `cargo nextest run -p spike_ecs_archetype --no-fail-fast` and record which tests fail in `log.md` (expected: the stale case of `lookup_agrees_with_get` and the conformance test). Restore it.
- **Change:** spec §4.6 for `archetype`, then `ArchetypeRandomAccess` per §4.3 with `for_each` in both passes. Compare `Lookup::get`'s per-call work with `World::get` (ecs-7): no `component_id`, no `column_of` search.
- **Verify:** `cargo nextest run -p spike_ecs_archetype` passes. `grep -n "component_id\|column_of" ` over the body of `Lookup::get` finds nothing. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add Lookup and random_access to the archetype prototype (M3.21)`

### Step 4: `sparse_set` (R3, R5, R10)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/sparse_set/src/world.rs` (or `src/lookup.rs`), `src/lib.rs`, `src/random_access.rs` (new), `src/tests.rs`.
- **Test first:** `lookup_agrees_with_get` and `lookup_of_an_unregistered_type_is_none` as step 3; `random_access_sparse_set_matches_the_reference`; `random_access_sparse_set_zero_count_is_valid`. Fails first: `lookup` does not exist.
- **Change:** spec §4.6 (`Lookup` holds the `&SparseSet<T>`, or `None`), then the backend on the view path (no group).
- **Verify:** `cargo nextest run -p spike_ecs_sparse_set` passes. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add Lookup and random_access to the sparse_set prototype (M3.21)`

### Step 5: `dense` (R3, R5, R10)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/dense/src/world.rs` (or `src/lookup.rs`), `src/lib.rs`, `src/random_access.rs` (new), `src/tests.rs`.
- **Test first:** as step 4, for `dense`. Fails first: `lookup` does not exist.
- **Change:** spec §4.6 (`Lookup` holds the allocator and the `&DenseColumn<T>`, or `None`), then the backend. Pass B visits indices in ascending order, which is not slot order, so `expired` is sorted.
- **Verify:** `cargo nextest run -p spike_ecs_dense` passes. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add Lookup and random_access to the dense prototype (M3.21)`

### Step 6: `hybrid` (R3, R5, R10)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/hybrid/src/world.rs` (or `src/lookup.rs`), `src/lib.rs`, `src/harness_components.rs` (the six types, `Target<crate::Entity>` included, as `TableStorage`), `src/random_access.rs` (new), `src/tests.rs`.
- **Test first:** `lookup_agrees_with_get` for a table type and for a sparse type (the four cases each), `lookup_of_an_unregistered_type_is_none`; `random_access_hybrid_matches_the_reference`; `random_access_hybrid_zero_count_is_valid`; `random_access_hybrid_registers_no_sparse_set` (ecs-11): after setup and 5 steps, the world has no sparse set for any of the six types. Fails first: `lookup` does not exist.
- **Change:** spec §4.6 for both storage kinds, then the backend.
- **Verify:** `cargo nextest run -p spike_ecs_hybrid` passes. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add Lookup and random_access to the hybrid prototype (M3.21)`

### Step 7: `bevy_ecs` (R3, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/bevy/src/random_access.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** `random_access_bevy_matches_the_reference`; `random_access_bevy_clears_removals_each_step` (as M3.10's R4 tests, for `Table<Lifetime>`). Fails first: the type does not exist.
- **Change:** spec §4.3 for bevy: pass A and pass B `QueryState`s and the `QueryState<&Table<Position>>` for `get_manual`, all built in `setup`; `iter_manual(..).for_each` in pass A, `iter_mut(..).for_each` in pass B; `clear_trackers` at the end of `step`; the backend's own alive count, as `BevyEntityChurn`.
- **Verify:** `cargo nextest run -p spike_ecs_bevy` passes. `grep -n "world.get\b\|\.get::<" spikes/ecs_storage/bevy/src/random_access.rs` finds no `World::get` in `step`. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add random_access to the bevy_ecs baseline (M3.21)`

### Step 8: `hecs` (R3, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/hecs/src/random_access.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** `random_access_hecs_matches_the_reference`. Fails first: the type does not exist.
- **Change:** spec §4.3 for hecs: pass A `world.query::<(&Position, &Target<Entity>)>()` with `world.view::<&Position>()` beside it; pass B `query_mut`; despawn and spawn through `World` after pass B; `for_each` in both passes (ecs-7).
- **Verify:** `cargo nextest run -p spike_ecs_hecs` passes. `grep -nE "PreparedQuery|CommandBuffer|world\.get" spikes/ecs_storage/hecs/src/random_access.rs` finds nothing. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add random_access to the hecs baseline (M3.21)`

### Step 9: bench registration (R6)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/bench_compare/benches/compare.rs`, `spikes/ecs_storage/bench_cache/benches/cache.rs`.
- **Test first:** none runnable here: the cache bench's size assertion (`prepared`) checks that each listed size is in `Spec::SIZES`, at run time under Valgrind, which this Mac lacks; it does not catch a missing size. The `cache_bench!` lines are checked by reading them against `SIZES` (review).
- **Change:** seven `bench::<…>` lines and seven `cache_bench!` lines in a `random_access` group, spec §4.5.
- **Verify:** `cargo bench -p spike_ecs_bench_compare --no-run` and `cargo bench -p spike_ecs_bench_cache --no-run` rc 0; `grep -c 'RandomAccess' spikes/ecs_storage/bench_compare/benches/compare.rs` counts the seven `bench::<…>` lines (plus imports), and step 12's run lists 21 `spike/ecs/random_access/` ids. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): register random_access in the spike benches (M3.21)`

### Step 10: documents (R7)

- [x] **Who:** main session
- **Files:** `spikes/ecs_storage/README.md` (status-line date, harness row, workload list, rules link), `spikes/README.md` (Contents row: eight workloads); the plan tick.
- **Test first:** `grep -rn "seven" spikes/README.md spikes/ecs_storage/README.md spikes/ecs_storage/harness/src/lib.rs` finds the stale counts (docs-1: grep the phrase across `docs/` and `.claude/` too).
- **Change:** spec §4.5's document items; the prototypes' `Lookup` in each prototype's row.
- **Verify:** `cargo xtask check-docs` rc 0; the grep finds no stale "seven workloads".
- **Commit:** `docs(spikes): document the random_access workload (M3.21)`

### Step 11: allocation count (R5, ecs-2)

- [ ] **Who:** main session, in a scratchpad crate outside the workspace (its own `[workspace]`, the pinned `rust-toolchain.toml`, a counting `#[global_allocator]`, the spike crates by path)
- **Files:** `log.md` only.
- **Test first:** none (a measurement); the count is checked against expectation: the workload's own code adds 0 allocations per step; each prototype's per-step count equals its recorded per-query-run count times the query runs of both passes (two queries plus one `Lookup`), plus despawn/respawn storage growth.
- **Change:** count allocations per `step` for each of the seven backends at `count` 10,000 after 700 warm-up ticks; record them with the commit (specs-2, specs-4).
- **Verify:** the log entry exists and names the commit.
- **Commit:** `docs: record random_access allocation counts (M3.21)`

### Step 12: the informal run (R8)

- [ ] **Who:** main session
- **Files:** `log.md`.
- **Test first:** none.
- **Change:** `/bench-run` with `cargo xtask spike-bench ecs` on the final code commit (after review fixes; specs-4). Before quoting, compare `hybrid` with `archetype` and each baseline with the prototype nearest its model (ecs-9); any departure from expectation says whether its cause is measured or expected and unmeasured (specs-6, docs-3). Record the commit, conditions, power log and the `random_access` table in `log.md`. The pull request and the comment on #102 (posted in stage 7, after the pull request exists) carry the same table.
- **Verify:** the log entry names the commit and the power-log check.
- **Commit:** `docs: record the random_access informal run (M3.21)`

## 3. Final verification

`cargo xtask ci` with nothing else running `cargo`, and the spec's Done-when checklist.

## 4. Lessons

- **ecs-1:** followed where it applies: the hybrid storage test reads the world the step built, not a copy. No path test is added.
- **ecs-2:** followed: step 11.
- **ecs-5:** followed: `Lookup` hands out `&T` only (§1).
- **ecs-6:** followed: step 3's red check targets the agreement test and uses `--no-fail-fast`.
- **ecs-7:** followed: steps 3, 7 and 8 compare per-call work and use `for_each`.
- **ecs-9:** followed: step 12 compares siblings before quoting. No new per-archetype column fetch is added except `Lookup`'s cached column index, which is per entity, not a slice.
- **ecs-11:** followed: step 6's `registers_no_sparse_set`.
- **docs-1:** followed: step 10's grep.
- **docs-3:** followed: step 12.
- **specs-1:** followed: each "finds nothing" grep names its path.
- **specs-2:** followed: steps 11 and 12 are their own steps.
- **specs-3:** followed: §4.6's four cases are named in each agreement test.
- **specs-4:** followed: step 12 runs on the final code; step 11 is re-run if review fixes change a step.
- **specs-6:** followed: step 12.

## 5. Deviations

- Step 1: `integrate` takes `dt: f32` (computed once in `setup`, as the other workloads do) instead of calling `dt()` per entity; `spawn` panics on `count` 0 (never called then). See log.md.
- Step 3: `Lookup` is not `Copy` on `archetype` (its column cache is a `Vec`); spec §4.6's main-session call amended. `World`'s private `lookup` renamed `find_by_components`. `World::lookup` allocates one `Vec` per pass A (counted in step 11).
- Step 6: hybrid's private `lookup` renamed `find_archetype`; the agreement test is split into a table-type and a sparse-type test; no `roster()` hook (unused there).
