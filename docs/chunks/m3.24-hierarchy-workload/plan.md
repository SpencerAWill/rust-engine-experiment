# M3.24 plan: hierarchy workload for the ECS storage harness

- **Status:** Accepted (2026-11-27)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. Progress and surprises go in [log.md](log.md).

## 1. Approach

The harness module first, then the reference, so every later step has something to conform to; then the four prototypes, the two baselines, bench registration, documents, the allocation count (ecs-2) and the informal run last, on the final code (specs-4). One backend per step, as M3.21 to M3.23, each ECS step with its own red checks. The grill's stub (log.md) already ran the harness rules, the `vec_aos` reference and the `archetype` level passes; steps 1 to 3 may start from its shape, rewritten to the repository's rules.

Choices the spec leaves to implementation, fixed here:

- **Names:** module `hierarchy` in each crate; types `VecHierarchy`, `ArchetypeHierarchy`, `SparseSetHierarchy`, `DenseHierarchy`, `HybridHierarchy`, `BevyHierarchy`, `HecsHierarchy`; `BACKEND` = `"vec_aos"`, `"archetype"`, `"sparse_set"`, `"dense"`, `"hybrid"`, `"bevy_ecs"`, `"hecs"`.
- **Harness signatures:** `pub const LEVELS: u32 = 8`; `pub const SPIN: f32 = 0.05`; `pub struct Node { pub id: u64, pub level: u32, pub parent: Option<u64>, pub local: LocalTransform }` (`Clone, Copy, Debug, PartialEq`); `pub struct Forest<'p>`, an `Iterator<Item = Node>` (`Debug`); `pub fn forest(prng: &Prng, count: usize) -> Forest<'_>`; `#[inline] pub fn spin(local: &mut LocalTransform)`; `#[inline] #[must_use] pub fn root(local: &LocalTransform) -> GlobalTransform`; `#[inline] #[must_use] pub fn place(parent: &GlobalTransform, local: &LocalTransform) -> GlobalTransform`; `#[must_use] pub fn hash(id: &SpawnId, level: u32, parent: Option<u64>, local: &LocalTransform, global: &GlobalTransform) -> u64`. `Depth1` to `Depth8` from one private `macro_rules!`, each with its own doc comment naming its level.
- **Step shape on every ECS backend:** the update query, the root query, then one private generic function per backend, `fn level<D>(..)` (the bound each crate needs: `'static`, `Component`, or `Table<D>` on bevy), called for `Depth2` to `Depth8` in order, holding the read and write pass of spec §4.3 with one `scratch: Vec<GlobalTransform>` field, `with_capacity(count)` in `setup`. Iteration by `for_each` (ecs-7); `hecs` `query_mut(..).into_iter().for_each(..)`. `setup` spawns through an eight-arm `match` on `node.level` (the bundle differs per depth type), keeps a `Vec<Entity>` of handles by spawn id for the parent links (dropped at the end of `setup`), then calls the same root and level functions the step calls.
- **`bevy_ecs`:** the eight levels need eight distinct `QueryState` pairs: a field per depth (`levels: [LevelQueries; 7]` is impossible across types), so a private struct `Levels` with one read and one write `QueryState` per depth, built in `setup` after the spawns, plus the lookup `QueryState<&Table<GlobalTransform>>`.
- **`checksum`:** one filtered read query per depth (the level is the depth's number), each item `(&SpawnId, &LocalTransform, &GlobalTransform, Option<&Parent<E>>)` (nested as each crate's arity needs), and the parent's `SpawnId` through `lookup::<SpawnId>()` (prototypes), `view::<&SpawnId>()` (`hecs`) or `World::get` (`bevy_ecs`): not measured.
- **Zero count:** `setup` with `count` 0 spawns nothing and the checksum is 0; each backend's tests include the zero-count case through the crate's existing helper or a test named `hierarchy_<backend>_zero_count_is_valid`.

Every conformance test below is `check_conformance::<VecHierarchy, X>(Params { count: 1_000, seed: 42 }, 120)`, named `hierarchy_<backend>_matches_the_reference`.

**Red checks (ECS steps 3 to 8)**, each run with `cargo nextest run -p <package> --no-fail-fast`, failures and messages recorded in `log.md`, code restored and diffed against a saved copy (ecs-6, spec §4.5):

- **(a)** stale parent: run all seven read passes before any write pass (seven scratch buffers, local to the red check): the conformance test MUST fail.
- **(b)** drop the `Depth8` level call: the conformance test MUST fail.
- **(c)** skip `spin`: the conformance test MUST fail at tick 1.

## 2. Steps

### Step 1: the harness module (R1)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/harness/src/hierarchy.rs` (new), `harness/src/lib.rs` (`pub mod hierarchy;`; crate doc: the eleventh, `hierarchy`, in the M3.24 spec §4.2).
- **Test first:** in the module's test module: `forest_links_parents_first` (at `count` 1,000 and 100,000, seed 42: ids `0..count` in order; each parent id lower than its child's, with level one less; roots have `None` and level 1; every level from 1 to 8 has at least one node, none above 8); `forest_follows_the_draw_order` (node 0 at seed 42: depth, translation x and y in the root ranges, rotation, scale x and y, then the child count if depth > 1, equal to direct draws from `prng.stream(0)`; and the first non-root node's translation draws in `-16.0..16.0`); `spin_wraps_and_keeps_the_order` (0.0 gives 0.05; a rotation within 0.05 of `PI` gives `r - TAU` computed in f32; `PI - 0.05` exactly is checked against the `> PI` compare as written); `root_and_place_compose` (`root` equals `to_world(DVec2::ZERO)`; `place` equals `parent.0.mul_transform(local.0)`); `hash_sees_every_field` (changing each of id, level, parent, each local field and each global field in turn changes the hash; `None` and `Some(u64::MAX - 1)` differ). Fails first: the module does not exist (E0432).
- **Change:** spec §4.2, each item documented.
- **Verify:** `cargo nextest run -p spike_ecs_harness` passes with the 5 new tests. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add the hierarchy workload rules to the harness (M3.24)`

### Step 2: the `vec_aos` reference (R2, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/vec/src/hierarchy.rs` (new), `vec/src/lib.rs`, `vec/src/tests.rs`.
- **Test first:** `hierarchy_vec_zero_count_is_valid`; `hierarchy_checksum_never_repeats_and_every_global_changes` (spec §4.5, ecs-13: at the conformance parameters, 120 ticks: each tick's checksum differs from every earlier tick's, tick 0 included, and every row's `GlobalTransform` differs from its value at the previous tick; a `#[cfg(test)]` accessor returns the rows' globals). Fails first: the type does not exist (E0583 or E0432).
- **Change:** spec §4.4.
- **Red check:** skip `spin` in `step`: the changing-state test MUST fail; recorded; restored.
- **Verify:** `cargo nextest run -p spike_ecs_vec` passes. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add the hierarchy reference on vec_aos (M3.24)`

### Step 3: `archetype` (R3, R4, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/archetype/src/hierarchy.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** conformance and zero-count tests; `hierarchy_archetype_holds_one_table_per_level` (spec §4.5: `archetype_count()` is 9 after `setup`, after one step and after ten; `check_invariants()` `Ok`). Fails first: the type does not exist.
- **Change:** spec §4.3 with `World::lookup::<GlobalTransform>()` once per level.
- **Red checks:** (a), (b), (c) of §1.
- **Verify:** `cargo nextest run -p spike_ecs_archetype` passes. `grep -n "\.get::<\|get_mut" spikes/ecs_storage/archetype/src/hierarchy.rs` finds nothing outside `checksum` and `#[cfg(test)]` code. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add hierarchy to the archetype prototype (M3.24)`

### Step 4: `sparse_set` (R3, R4, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/sparse_set/src/hierarchy.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** conformance and zero-count tests. Fails first: the type does not exist.
- **Change:** spec §4.3; views only, no group.
- **Red checks:** (a), (b), (c) of §1.
- **Verify:** `cargo nextest run -p spike_ecs_sparse_set` passes; the same grep as step 3 on this file. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add hierarchy to the sparse_set prototype (M3.24)`

### Step 5: `dense` (R3, R4, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/dense/src/hierarchy.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** conformance and zero-count tests. Fails first: the type does not exist.
- **Change:** as step 4.
- **Red checks:** (a), (b), (c) of §1.
- **Verify:** `cargo nextest run -p spike_ecs_dense` passes; the same grep. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add hierarchy to the dense prototype (M3.24)`

### Step 6: `hybrid` (R3, R4, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/hybrid/src/harness_components.rs` (every `hierarchy` type in the `TableStorage` list, `Parent<crate::Entity>` as `random_access::Target<crate::Entity>`; module doc: eleven workloads), `src/hierarchy.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** conformance and zero-count tests; `hierarchy_hybrid_holds_one_table_per_level` (as step 3's). Fails first: the type does not exist.
- **Change:** spec §4.3.
- **Red checks:** (a), (b), (c) of §1.
- **Verify:** `cargo nextest run -p spike_ecs_hybrid` passes; the same grep. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add hierarchy to the hybrid prototype (M3.24)`

### Step 7: `bevy_ecs` (R3, R4, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/bevy/src/hierarchy.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** conformance and zero-count tests. Fails first: the type does not exist.
- **Change:** spec §4.3 for `bevy_ecs`; every type under `Table<T>`; the `QueryState`s of §1 built in `setup`; `clear_trackers()` at the end of `step`.
- **Red checks:** (a), (b), (c) of §1.
- **Verify:** `cargo nextest run -p spike_ecs_bevy` passes. `grep -nE "world\.query|query_filtered|try_query|\.get::<" spikes/ecs_storage/bevy/src/hierarchy.rs` shows them only in `setup` and `checksum`. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add hierarchy to the bevy_ecs baseline (M3.24)`

### Step 8: `hecs` (R3, R4, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/hecs/src/hierarchy.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** conformance and zero-count tests. Fails first: the type does not exist.
- **Change:** spec §4.3 for `hecs`: `world.view::<&GlobalTransform>()` once per level beside `world.query::<..>().with::<&D>()`; `query_mut` for the other passes.
- **Red checks:** (a), (b), (c) of §1.
- **Verify:** `cargo nextest run -p spike_ecs_hecs` passes. `grep -nE "PreparedQuery|CommandBuffer|\.get::<" spikes/ecs_storage/hecs/src/hierarchy.rs` finds nothing outside `checksum`. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add hierarchy to the hecs baseline (M3.24)`

### Step 9: bench registration (R6)

- [ ] **Who:** builder
- **Files:** `spikes/ecs_storage/bench_compare/benches/compare.rs`, `spikes/ecs_storage/bench_cache/benches/cache.rs`.
- **Test first:** none runnable here (the cache bench's size check runs only under Valgrind; M3.21 log). The `cache_bench!` lines are checked against `SIZES` by reading them.
- **Change:** seven `bench::<…>` lines, each after the backend's `wide_entities` line; seven `cache_bench!` lines with `[1_000, 10_000, 100_000]` in a `hierarchy` group added to `main!`.
- **Verify:** `cargo bench -p spike_ecs_bench_compare --no-run` and `cargo bench -p spike_ecs_bench_cache --no-run` rc 0; `grep -c "bench::<.*Hierarchy>" spikes/ecs_storage/bench_compare/benches/compare.rs` is 7, and the seven `cache_bench!` invocations naming `Hierarchy` counted by reading. Step 12's run lists 21 `spike/ecs/hierarchy/` ids. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): register hierarchy in the spike benches (M3.24)`

### Step 10: documents (R7)

- [ ] **Who:** main session
- **Files:** `spikes/ecs_storage/README.md` (status-line date; the rules sentence links the M3.24 spec §4.2; the harness row "eleven workloads"; the workload list and the backends sentence; one sentence on `hierarchy` with its caveat, the one `sin_cos` per non-root entity every row holds (docs-3)), `spikes/README.md` (Contents row), `CHANGELOG.md` (one entry, the same caveat); the plan tick.
- **Test first:** `grep -rnE "ten (shared )?workloads|10 workloads" spikes/ CHANGELOG.md` finds the stale text (specs-1: the paths are named).
- **Change:** spec §4.6's document items.
- **Verify:** `cargo xtask check-docs` rc 0; `typos docs/chunks/m3.24-hierarchy-workload spikes/ecs_storage/README.md spikes/README.md` clean (docs-4); the grep finds nothing except history entries in `CHANGELOG.md` and correct ordinal lists, each read.
- **Commit:** `docs(spikes): document the hierarchy workload (M3.24)`

### Step 11: allocation count (R5, ecs-2)

- [ ] **Who:** main session, in a scratchpad crate outside the workspace (own `[workspace]`, the pinned `rust-toolchain.toml`, the repo's `Cargo.lock`, a counting `#[global_allocator]` over `alloc` and `realloc`, the spike crates by path, release)
- **Files:** `log.md` only.
- **Test first:** none (a measurement); the pass criterion is spec §8's R5 row. Control in the same binary: one run of each prototype's read query and write query of one level, counted alone, so the expected per-step count is (update + root + 7 × (read + write)) runs.
- **Change:** count allocations per `step` for the seven backends' `hierarchy` at `count` 1,000, seed 42, 100 steps after 10 warm-up steps; min and max; the control counts; record with the full commit hash (specs-4, docs-4). A count above the expected sum, or one that varies between steps, is a finding: explained (said how it is known, specs-6) or flagged for the caveats.
- **Verify:** the log entry exists, names the commit, and states for each backend whether the criterion holds.
- **Commit:** `docs: record hierarchy allocation counts (M3.24)`

### Step 12: the informal run (R8)

- [ ] **Who:** main session
- **Files:** `log.md`.
- **Test first:** none.
- **Change:** `/bench-run` with `cargo xtask spike-bench ecs --save m3.24-informal` on the final code commit (after review fixes; specs-4). Before quoting, compare siblings (ecs-9): `hybrid` with `archetype` (the same tables, so within noise is expected); `sparse_set` with `dense`; `hecs` and `bevy_ecs` with `archetype`; and the run's `random_access` and `simple_iter` rows with M3.23's `m3.23-informal`, so a disturbed run shows. Caveats per spec §4.6, each said as measured or expected and unmeasured (specs-6, docs-3), with step 11's counts. Record commit, conditions, power log and the `hierarchy` table in `log.md`; the pull request and the comment on #102 (stage 7, once the pull request exists) carry them (specs-2).
- **Verify:** the log entry names the commit and the power-log check.
- **Commit:** `docs: record the hierarchy informal run (M3.24)`

## 3. Final verification

`cargo xtask ci` on the final code commit, with nothing else running `cargo`; the spec's §9 Done-when list ticked with evidence.

## 4. Lessons

- **ecs-2** followed: step 11, from a scratchpad crate, with a same-binary control.
- **ecs-6** followed: the red checks of §1 run with `--no-fail-fast`; they target conformance, which is where a propagation-order bug shows (no storage bug is in scope).
- **ecs-7** followed: `for_each` everywhere; the lookup resolves the type once per level on every backend (spec §4.3), and each step's grep checks no per-entity `get` crept in.
- **ecs-9** followed: step 12 compares siblings before quoting.
- **ecs-11** followed: the state the checksum cannot see is the archetype layout (one table per level), asserted by the table-placement tests of steps 3 and 6; the checksum sees every other value.
- **ecs-13** followed: simulated in the grill (log.md), and kept as step 2's test.
- **docs-3** followed: step 10's README and CHANGELOG sentences carry the `sin_cos` caveat.
- **docs-4** followed: `typos` in step 10, full hashes in steps 11 and 12.
- **specs-2** followed: the allocation count and the informal run each have a step.
- **specs-3** followed: the table-placement test is named per backend that has tables (steps 3 and 6).
- **specs-4** followed: steps 11 and 12 run on the final code commit, or are re-run after later code changes.
- **specs-6** followed: steps 11 and 12 say how each cause is known.

## 5. Deviations

- Step 7: the `bevy_ecs` tests are `hierarchy_bevy_matches_the_reference` and `hierarchy_bevy_zero_count_is_valid`, the crate's existing `<workload>_bevy_*` naming, not §1's `hierarchy_bevy_ecs_*`.
