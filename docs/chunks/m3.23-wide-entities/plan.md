# M3.23 plan: wide_entities workload for the ECS storage harness

- **Status:** Accepted (2026-11-26)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. Progress and surprises go in [log.md](log.md).

## 1. Approach

The harness module comes first, then the bundle arity on the two prototypes that need it, then the reference, so every later step has something to conform to. Then the four prototypes, the two baselines, bench registration, documents, the allocation count (ecs-2) and the informal run last, on the final code (specs-4). One backend per step, as M3.21 and M3.22; each ECS step carries its own red checks (M3.26: the steps are separate code).

Choices the spec leaves to implementation, fixed here:

- **Names:** module `wide_entities` in each crate; types `VecWideEntities`, `ArchetypeWideEntities`, `SparseSetWideEntities`, `DenseWideEntities`, `HybridWideEntities`, `BevyWideEntities`, `HecsWideEntities`; `BACKEND` = `"vec_aos"`, `"archetype"`, `"sparse_set"`, `"dense"`, `"hybrid"`, `"bevy_ecs"`, `"hecs"`.
- **Harness signatures:** `pub struct W0(pub [f32; 1])` to `pub struct W23(pub [f32; 16])` and `pub struct Wide { pub w0: W0, …, pub w23: W23 }`, all from one private `macro_rules!` (as the grill's stub, log.md), each type with its own doc comment naming its size; `pub struct Extra(pub f32)`; all deriving `Clone, Copy, Debug, PartialEq`. `pub fn spawn(prng: &Prng, i: u64) -> Wide`; `#[inline] pub fn update(a: &mut W0, b: &mut W7, c: &mut W14, d: &mut W21)`; `#[inline] pub fn make_extra(w0: &W0) -> Extra`; `pub fn hash(wide: &Wide, extra: Option<&Extra>) -> u64`.
- **Step shape on every ECS backend:** as its `add_remove` (spec §4.3), with field names `with_w0: Vec<(Entity, f32)>` (the insert pass's pairs, `W0`'s first element after the update) and `with_extra: Vec<Entity>` (the remove pass's list), both `with_capacity(count)` in `setup`; the read and remove queries in named functions shared with the test accessors, as `add_remove`'s `read_query` and `remove_query` (ecs-1). Iteration by `for_each` (ecs-7); `hecs` `query_mut(..).into_iter().for_each(..)`.
- **Test accessor:** `#[cfg(test)] pub(crate) fn removed_last_step(&self) -> &[Entity]` returning `with_extra`, as M3.26; the test calls each crate's existing `assert_inserts_on_every_entity_each_step` helper (or the crate's equivalent) at `Params { count: 1_000, seed: 42 }`, 120 ticks.
- **Zero count:** `setup` with `count` 0 spawns nothing; every backend's tests include the zero-count case through the crate's existing helper.

Every conformance test below is `check_conformance::<VecWideEntities, X>(Params { count: 1_000, seed: 42 }, 120)`, named `wide_entities_<backend>_matches_the_reference`; every zero-count test `wide_entities_<backend>_zero_count_is_valid`; every per-step test `wide_entities_<backend>_inserts_on_every_entity_each_step`.

**Red checks (ECS steps 4 to 9)**, each run with `cargo nextest run -p <package> --no-fail-fast`, failures and messages recorded in `log.md`, code restored and diffed against a saved copy (ecs-6, ecs-11):

- **(a)** delete the insert pass's `insert` loop: the per-step test MUST fail at tick 1.
- **(b)** insert only when the step counter is 1: the per-step test MUST fail at tick 2.
- **(c)** make the update pass skip `W21` (call a copy of the rule on three of the four): the conformance test MUST fail at tick 1.

## 2. Steps

### Step 1: the harness module (R1)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/harness/src/wide_entities.rs` (new), `harness/src/lib.rs` (`pub mod wide_entities;`; crate doc: ten workloads, `wide_entities`'s rules in the M3.23 spec §4.2).
- **Test first:** in the module's test module: `sizes_follow_the_cycle` (`size_of::<Wk>()` is `4 * [1, 2, 3, 4, 8, 16][k % 6]` for all 24, and `size_of::<Wide>()` is 544); `spawn_follows_the_draw_order` (for `i` in `[0, 1, 999]`: the 136 values, read in type then element order, equal 136 direct `range(0.0_f32..1.0)` draws from `prng.stream(i)`); `update_wraps_and_keeps_the_order` (an element at `0.25` gives `0.868_034` with no wrap; at `0.5` gives `0.5 + 0.618_034 - 1.0` computed in f32, the wrap; the other 20 types are not arguments, so only the four change); `make_extra_reads_w0`; `hash_sees_every_type` (changing one element of each of the 24 types in turn changes the hash, and `Some(&Extra)` hashes differently from `None`). Fails first: the module does not exist (E0432).
- **Change:** spec §4.2, each item documented.
- **Verify:** `cargo nextest run -p spike_ecs_harness` passes with the 5 new tests. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add the wide_entities workload rules to the harness (M3.23)`

### Step 2: bundles of 24 on `archetype` and `hybrid` (R4, first half)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/archetype/src/bundle.rs`, `spikes/ecs_storage/hybrid/src/bundle.rs`, each crate's tests (the existing bundle or world test module).
- **Test first:** in each crate, `spawn_with_takes_24_types`: a 24-tuple of distinct test-local types (from a test-local macro) spawns into exactly one new archetype (`archetype_count()` 2) and `check_invariants()` is `Ok`, for two spawns (the second hits the cached bundle). Fails first: the tuple does not implement `Bundle` (E0277).
- **Change:** `impl_bundle!` lines for 9 to 24 types, same macro; "1 to 8" becomes "1 to 24" in each trait's doc. No other change.
- **Verify:** `cargo nextest run -p spike_ecs_archetype -p spike_ecs_hybrid` passes. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): take bundles of up to 24 types on archetype and hybrid (M3.23)`

### Step 3: the `vec_aos` reference (R2, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/vec/src/wide_entities.rs` (new), `vec/src/lib.rs`, `vec/src/tests.rs`.
- **Test first:** `wide_entities_vec_zero_count_is_valid`; `wide_entities_checksum_never_repeats` (at the conformance parameters, 120 ticks: each tick's checksum differs from every earlier tick's, tick 0 included; ecs-13); `wide_entities_vec_inserts_between_the_passes` (spec §4.5: a `#[cfg(test)]` method runs the step's loop with a closure that asserts every row holds `Some(make_extra(&wide.w0))`, after each of 120 steps). Fails first: the type does not exist.
- **Change:** spec §4.3's `vec_aos`; the step's loop generic over the between-passes closure, the measured `step` passing a no-op, as `VecAddRemove::step_with`.
- **Red check:** delete the insert pass: the hook test MUST fail; recorded in `log.md`; restore.
- **Verify:** `cargo nextest run -p spike_ecs_vec` passes. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add the wide_entities reference on vec_aos (M3.23)`

### Step 4: `archetype` (R3, R4, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/archetype/src/wide_entities.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** conformance, zero-count and per-step tests; `wide_entities_archetype_places_entities_in_one_archetype` (spec §4.4: after `setup` `archetype_count()` is 2, after one step 3, after ten steps still 3; `check_invariants()` `Ok`). Fails first: the type does not exist.
- **Change:** spec §4.3 for `archetype`; one `spawn_with` of the 24-tuple per entity.
- **Red checks:** (a), (b), (c) of §1.
- **Verify:** `cargo nextest run -p spike_ecs_archetype` passes. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add wide_entities to the archetype prototype (M3.23)`

### Step 5: `sparse_set` (R3, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/sparse_set/src/wide_entities.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** conformance, zero-count and per-step tests. Fails first: the type does not exist.
- **Change:** spec §4.3: `spawn` and 24 `insert`s in type order; no group.
- **Red checks:** (a), (b), (c) of §1.
- **Verify:** `cargo nextest run -p spike_ecs_sparse_set` passes. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add wide_entities to the sparse_set prototype (M3.23)`

### Step 6: `dense` (R3, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/dense/src/wide_entities.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** conformance, zero-count and per-step tests. Fails first: the type does not exist.
- **Change:** as step 5.
- **Red checks:** (a), (b), (c) of §1.
- **Verify:** `cargo nextest run -p spike_ecs_dense` passes. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add wide_entities to the dense prototype (M3.23)`

### Step 7: `hybrid` (R3, R4, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/hybrid/src/harness_components.rs` (`wide_entities::Extra` as `SparseStorage`, `W0` to `W23` as `TableStorage`; module doc: ten workloads, the sparse types now `add_remove::B`, `agents::Seeker` and `wide_entities::Extra`), `src/wide_entities.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** conformance, zero-count and per-step tests; `wide_entities_hybrid_moves_no_row` (spec §4.4: `archetype_count()` 2 after `setup`, after one step and after ten; `check_invariants()` `Ok`); `wide_entities_hybrid_stores_extra_sparse` (ecs-11, as `random_access_hybrid_registers_no_sparse_set`: `Extra`'s storage kind is sparse and each `Wk`'s is table). Fails first: the type does not exist.
- **Change:** spec §4.3 for `hybrid`; setup as step 4.
- **Red checks:** (a), (b), (c) of §1; and (d): move `Extra` to the `TableStorage` list: `wide_entities_hybrid_moves_no_row` and the storage test MUST fail.
- **Verify:** `cargo nextest run -p spike_ecs_hybrid` passes. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add wide_entities to the hybrid prototype (M3.23)`

### Step 8: `bevy_ecs` (R3, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/bevy/src/wide_entities.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** conformance and per-step tests; `wide_entities_bevy_stores_extra_in_a_table` (spec §4.5: after one step the world holds a 25-type archetype (`archetypes()` holds one with `Table<Extra>`) and has never registered `Sparse<Extra>` (`components().get_id(TypeId::of::<Sparse<Extra>>())` is `None`)); `wide_entities_bevy_places_entities_in_one_archetype` (`setup` adds exactly one archetype over `World::new()`'s). Fails first: the type does not exist.
- **Change:** spec §4.3 for `bevy_ecs`: a nested `(12-tuple, 12-tuple)` bundle of `Table<Wk>` (the grill's stub, log.md); three `QueryState`s built in `setup`; `clear_trackers()` at the end of `step`.
- **Red checks:** (a), (b), (c) of §1; and (d): insert `Sparse(make_extra(..))` and remove `Sparse<Extra>`: the storage test MUST fail.
- **Verify:** `cargo nextest run -p spike_ecs_bevy` passes. `grep -nE "world\.query|query_filtered|try_query" spikes/ecs_storage/bevy/src/wide_entities.rs` shows them only in `setup` and `checksum`. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add wide_entities to the bevy_ecs baseline (M3.23)`

### Step 9: `hecs` (R3, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/hecs/src/wide_entities.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** conformance and per-step tests; `wide_entities_hecs_places_entities_in_one_archetype` (after `setup`, `world.archetypes().len()` is 2, the root and the 24-type one, as the stub). Fails first: the type does not exist.
- **Change:** spec §4.3 for `hecs`: an `EntityBuilder` per spawn; `insert_one` and `remove_one`.
- **Red checks:** (a), (b), (c) of §1.
- **Verify:** `cargo nextest run -p spike_ecs_hecs` passes. `grep -nE "PreparedQuery|CommandBuffer" spikes/ecs_storage/hecs/src/wide_entities.rs` finds nothing. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add wide_entities to the hecs baseline (M3.23)`

### Step 10: bench registration (R7)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/bench_compare/benches/compare.rs`, `spikes/ecs_storage/bench_cache/benches/cache.rs`.
- **Test first:** none runnable here (the cache bench's size check runs only under Valgrind; M3.21 log). The `cache_bench!` lines are checked against `SIZES` by reading them.
- **Change:** seven `bench::<…>` lines; seven `cache_bench!` lines with `[1_000, 10_000, 100_000]` in a `wide_entities` group added to `main!`.
- **Verify:** `cargo bench -p spike_ecs_bench_compare --no-run` and `cargo bench -p spike_ecs_bench_cache --no-run` rc 0; `grep -c "bench::<.*WideEntities>" spikes/ecs_storage/bench_compare/benches/compare.rs` is 7, and the seven `cache_bench!` invocations naming `WideEntities` are counted by reading (rustfmt may wrap them; M3.22 deviation). Step 13's run lists 21 `spike/ecs/wide_entities/` ids. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): register wide_entities in the spike benches (M3.23)`

### Step 11: documents (R8)

- [ ] **Who:** main session
- **Files:** `spikes/ecs_storage/README.md` (status-line date; the rules sentence links the M3.23 spec §4.2; the harness row "ten workloads"; the archetype and hybrid rows' `spawn_with` bundles of up to 24 types and `hybrid`'s sparse `Extra`; the workload list; the backends sentence), `spikes/README.md` (Contents row), `CHANGELOG.md` (one entry, as M3.22's); the plan tick.
- **Test first:** `grep -rnE "nine (shared )?workloads|9 workloads|1 to 8" spikes/ CHANGELOG.md` finds the stale text (specs-1: the paths are named).
- **Change:** spec §4.6's document items.
- **Verify:** `cargo xtask check-docs` rc 0; the grep over the same paths finds nothing except the harness crate doc's ordinal list if correct and history entries in `CHANGELOG.md`, each read.
- **Commit:** `docs(spikes): document the wide_entities workload (M3.23)`

### Step 12: allocation count (R6, ecs-2)

- [ ] **Who:** main session, in a scratchpad crate outside the workspace (own `[workspace]`, the pinned `rust-toolchain.toml`, the repo's `Cargo.lock`, a counting `#[global_allocator]` over `alloc` and `realloc`, the spike crates by path, release)
- **Files:** `log.md` only.
- **Test first:** none (a measurement). Expected: `vec_aos` 0 per step; each prototype its per-query-run count times 3 plus whatever its `insert` and `remove` allocate after warm-up, which should match its `add_remove` count at the same size (measured in the same crate as a control); `bevy_ecs` and `hecs` compared with their `add_remove` the same way.
- **Change:** count allocations per `step` for the seven backends' `wide_entities` and `add_remove` at `count` 1,000, seed 42, 100 steps after 10 warm-up steps; min and max; record with the full commit hash (specs-4, docs-4). A `wide_entities` count above its `add_remove` control is a finding for the caveats, explained or flagged.
- **Verify:** the log entry exists and names the commit.
- **Commit:** `docs: record wide_entities allocation counts (M3.23)`

### Step 13: the informal run (R9)

- [ ] **Who:** main session
- **Files:** `log.md`.
- **Test first:** none.
- **Change:** `/bench-run` with `cargo xtask spike-bench ecs --save m3.23-informal` on the final code commit (after review fixes; specs-4). Before quoting, compare siblings (ecs-9): `hybrid` with `sparse_set` (neither moves a row) and with `archetype`; `hecs` and `bevy_ecs` with `archetype`; and the run's `add_remove` rows with M3.26's. Caveats per spec §4.6, each said as measured or expected and unmeasured (specs-6, docs-3), with step 12's counts. Record commit, conditions, power log and the `wide_entities` and `add_remove` tables in `log.md`; the pull request and the comment on #102 (stage 7, once the pull request exists) carry them.
- **Verify:** the log entry names the commit and the power-log check.
- **Commit:** `docs: record the wide_entities informal run (M3.23)`

## 3. Final verification

`typos` over the chunk folder (docs-4), then `cargo xtask ci` with nothing else running `cargo`, and the spec's Done-when checklist.

## 4. Lessons

- **ecs-1:** followed in part: no path test (the workload chooses no storage path), but the per-step accessors read the lists the measured step fills, and the read and remove queries are named functions shared with the tests (§1).
- **ecs-2:** followed: step 12, with each backend's `add_remove` as the control.
- **ecs-4:** not applicable: no new failure path or "unchanged after failure" claim (step 2 adds tuple impls of an existing macro).
- **ecs-5:** not applicable: no new borrow or accessor that hands out storage.
- **ecs-6:** followed: the red checks of §1 run with `--no-fail-fast`; no storage code changes, so no storage red check is needed beyond step 2's test.
- **ecs-7:** followed: §1's step shape and iteration form are each backend's `add_remove`'s.
- **ecs-8:** not applicable: the `Bundle` seal traits have no lifetime GAT.
- **ecs-9:** followed: no new column fetch; step 13's sibling comparison before quoting.
- **ecs-11:** followed: per-step insert tests and red checks (a), (b); `hybrid`'s and `bevy_ecs`'s storage tests and red checks (d).
- **ecs-13:** followed: the simulation is in `log.md` (grill); step 3's never-repeats test.
- **specs-1:** followed: every grep names its paths.
- **specs-2:** followed: steps 12 and 13 are their own steps; red-check records are in each step.
- **specs-3:** followed: each test of spec §4.5 is named in steps 1, 3 to 9.
- **specs-4:** followed: step 13 on the final code; step 12 re-run if review changes a `step`.
- **specs-6:** followed: step 13's caveats.
- **docs-1:** followed: step 11's grep over `spikes/` and `CHANGELOG.md`.
- **docs-3:** followed: step 13; no doc comment ranks a backend.
- **docs-4:** followed: §3, and full hashes in `log.md`.

## 5. Deviations

- Step 2: the tests are in each crate's `world.rs` test module (the plan allowed bundle or world); `archetype`'s `World::spawn_with` doc also said "1 to 8" and now says "1 to 24".
- Step 5: `sparse_set`'s per-step test inlines the loop of the crate's `assert_inserts_on_every_entity_each_step`, which is tied to `AddRemoveOn`; generalising the helper is left to the simplify pass. Steps 6 (`dense`), 7 (`hybrid`) and 9 (`hecs`) the same: none of these crates has a shared helper.
- Step 8: `bevy_ecs` also has a zero-count test (`wide_entities_bevy_zero_count_is_valid`, the crate's `many_queries` pattern); the storage test finds `Table<Extra>` by `component_id::<Table<Extra>>()`.
- Step 9: `hecs` also has a zero-count test (`wide_entities_hecs_zero_count_is_valid`).
