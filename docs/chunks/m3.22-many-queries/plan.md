# M3.22 plan: many_queries workload for the ECS storage harness

- **Status:** Accepted (2026-11-26)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. Progress and surprises go in [log.md](log.md).

## 1. Approach

The harness module comes first, then the reference, so every later step has something to conform to. Then the four prototypes (`archetype` first, which carries the red check), the two baselines, bench registration, documents, the allocation count (ecs-2) and the informal run last, on the final code (specs-4). One backend per step, as M3.21.

Choices the spec leaves to implementation, fixed here:

- **Names:** module `many_queries` in each crate; types `VecManyQueries`, `ArchetypeManyQueries`, `SparseSetManyQueries`, `DenseManyQueries`, `HybridManyQueries`, `BevyManyQueries`, `HecsManyQueries`; `BACKEND` = `"vec_aos"`, `"archetype"`, `"sparse_set"`, `"dense"`, `"hybrid"`, `"bevy_ecs"`, `"hecs"`.
- **Harness signatures:** `pub struct C0(pub f32)` to `C7`, each deriving `Clone, Copy, Debug, PartialEq`; `pub struct Spawn { pub group: u8, pub values: [f32; 4] }` with the same derives; `pub fn spawn(prng: &Prng, i: u64) -> Spawn`; `pub fn update(write: &mut f32, read: f32)` (`#[inline]`); `pub fn hash(group: u8, values: [f32; 4]) -> u64`; `pub const GROUPS: u8 = 8`; `pub const QUERIES: [(u8, u8); 56]`. `for_each_query!` is `#[macro_export]`, written out as 56 lines of `$m!($crate::many_queries::Cw, $crate::many_queries::Cr);` (spec §4.3), documented with a runnable doctest that collects the expansion.
- **Per-group code** (spawn bundles, the eight checksum queries) is written in each backend with the concrete types, as `frag_iter`'s `per_letter!`; conformance catches a wrong group, since the hash starts with it. No harness macro for groups (the spec's export list has none).
- **`vec_aos`:** `struct Entity { group: u8, values: [f32; 8] }`; a type `t` is present when `(t + 8 - group) % 8 < 4`. The step loops `QUERIES` in order, then every entity.
- **bevy:** `queries: Vec<Box<dyn FnMut(&mut World)>>`, `with_capacity(56)`, filled through `for_each_query!` in `setup`, each closure owning its `QueryState` and running `iter_mut(world).for_each(..)`; `step` runs them in order, then `clear_trackers`. The struct's `Debug` is hand-written (the closures have none). `checksum` uses `try_query` per group, as `simple_iter`.
- **Iteration form (ecs-7):** `for_each` on every prototype query and on bevy; `hecs` `query_mut(..).into_iter().for_each(..)`, as its `simple_iter`.
- **Zero count:** `setup` with `count` 0 spawns nothing; every backend's tests include the zero-count case.

Every conformance test below is `check_conformance::<VecManyQueries, X>(Params { count: 1_000, seed: 42 }, 120)`, named `many_queries_<backend>_matches_the_reference`; every zero-count test is `many_queries_<backend>_zero_count_is_valid`, through the crate's existing zero-count helper.

## 2. Steps

### Step 1: the harness module (R1)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/harness/src/many_queries.rs` (new), `harness/src/lib.rs` (`pub mod many_queries;`; crate doc: nine workloads, `many_queries`'s rules in the M3.22 spec §4.2).
- **Test first:** in `many_queries.rs`'s test module: `spawn_follows_the_draw_order` (for `i` in `[0, 1, 7, 8, 999]`: group `i % 8`, values equal four direct `range(0.0_f32..1.0)` draws from `prng.stream(i)`); `update_wraps_and_keeps_the_order` (hand-computed: `update(0.5, 0.25)` gives `0.875`; `update(0.75, 0.5)` gives `0.25`, the wrap; `update(0.0, 0.0)` gives `0.25`); `queries_are_every_ordered_pair` (`QUERIES` holds 56 distinct pairs, `w != r`, `w` major, `r` minor); `the_macro_expands_in_query_order` (collects `for_each_query!`'s expansion by type name and compares with `QUERIES`, as the grill's stub did). Fails first: the module does not exist (E0432).
- **Change:** spec §4.2 and §4.3, each item documented.
- **Verify:** `cargo nextest run -p spike_ecs_harness` passes with the 4 new tests; `cargo test --doc -p spike_ecs_harness` passes the macro's doctest. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add the many_queries workload rules to the harness (M3.22)`

### Step 2: the `vec_aos` reference (R2, R1's checks)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/vec/src/many_queries.rs` (new), `vec/src/lib.rs`, `vec/src/tests.rs`.
- **Test first:** `many_queries_vec_zero_count_is_valid`; `many_queries_match_counts_follow_the_distance` (at `count` 1,000, for each of the 56 queries, the number of entities it visits is 375, 250, 125 or 0 for `d` = `(r - w) mod 8` in {1, 7}, {2, 6}, {3, 5}, {4}; through a `#[cfg(test)]` hook that counts per query); `many_queries_still_changes_at_the_last_tick` (at `count` 1,000, seed 42, the checksum after tick 120 differs from the one after tick 119, and so does each of ticks 2 to 120 from the one before). Fails first: the type does not exist.
- **Change:** spec §4.4's `vec_aos`.
- **Verify:** `cargo nextest run -p spike_ecs_vec` passes. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add the many_queries reference on vec_aos (M3.22)`

### Step 3: `archetype` (R3, R4)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/archetype/src/many_queries.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** the conformance and zero-count tests. Fails first: the type does not exist.
- **Change:** spec §4.4 for the prototypes: the guide's `run_query!` callback through `for_each_query!` in `step`; eight `spawn_with` bundles by group in `setup`; eight read-only four-type queries in `checksum`.
- **Red check (ecs-6, ecs-11):** make `step` return early once it has run 60 times (a counter in the struct); `cargo nextest run -p spike_ecs_archetype --no-fail-fast` must fail the conformance test at tick 61, which shows conformance still compares changing state late in the run. Record the failing tests and message in `log.md`; restore.
- **Verify:** `cargo nextest run -p spike_ecs_archetype` passes. `grep -c "for_each_query!" spikes/ecs_storage/archetype/src/many_queries.rs` is at least 1, and `grep -nE "query_mut::<\(&mut C[0-7]" spikes/ecs_storage/archetype/src/many_queries.rs` finds nothing (no hand-written query). `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add many_queries to the archetype prototype (M3.22)`

### Step 4: `sparse_set` (R3, R4)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/sparse_set/src/many_queries.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** the conformance and zero-count tests. Fails first: the type does not exist.
- **Change:** as step 3, on the view path (no group).
- **Verify:** `cargo nextest run -p spike_ecs_sparse_set` passes; the two greps of step 3 on this crate's file. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add many_queries to the sparse_set prototype (M3.22)`

### Step 5: `dense` (R3, R4)

- [ ] **Who:** builder
- **Files:** `spikes/ecs_storage/dense/src/many_queries.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** the conformance and zero-count tests. Fails first: the type does not exist.
- **Change:** as step 3.
- **Verify:** `cargo nextest run -p spike_ecs_dense` passes; the two greps of step 3 on this crate's file. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add many_queries to the dense prototype (M3.22)`

### Step 6: `hybrid` (R3, R4)

- [ ] **Who:** builder
- **Files:** `spikes/ecs_storage/hybrid/src/harness_components.rs` (`many_queries::C0` to `C7` as `TableStorage`; module doc: nine workloads), `src/many_queries.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** the conformance and zero-count tests; `many_queries_hybrid_registers_no_sparse_set` (ecs-11, as `random_access_hybrid_registers_no_sparse_set`): after setup and 5 steps, the world has no sparse set for any of the eight types. Fails first: the type does not exist.
- **Change:** as step 3.
- **Verify:** `cargo nextest run -p spike_ecs_hybrid` passes; the two greps of step 3 on this crate's file. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add many_queries to the hybrid prototype (M3.22)`

### Step 7: `bevy_ecs` (R3, R4)

- [ ] **Who:** builder
- **Files:** `spikes/ecs_storage/bevy/src/many_queries.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** the conformance test; `many_queries_bevy_holds_every_query` (ecs-11: the 8 empty queries change no checksum, so a `#[cfg(test)]` hook asserts the closure `Vec` has 56 entries after `setup`). Fails first: the type does not exist.
- **Change:** §1's bevy shape.
- **Verify:** `cargo nextest run -p spike_ecs_bevy` passes. `grep -nE "world\.query|try_query" spikes/ecs_storage/bevy/src/many_queries.rs` shows them only in `setup`, the `for_each_query!` callback and `checksum`, none in `step`. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add many_queries to the bevy_ecs baseline (M3.22)`

### Step 8: `hecs` (R3, R4)

- [ ] **Who:** builder
- **Files:** `spikes/ecs_storage/hecs/src/many_queries.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** the conformance test. Fails first: the type does not exist.
- **Change:** spec §4.4 for `hecs`: `query_mut` per query through `for_each_query!`.
- **Verify:** `cargo nextest run -p spike_ecs_hecs` passes. `grep -nE "PreparedQuery|CommandBuffer" spikes/ecs_storage/hecs/src/many_queries.rs` finds nothing. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add many_queries to the hecs baseline (M3.22)`

### Step 9: bench registration (R5)

- [ ] **Who:** builder
- **Files:** `spikes/ecs_storage/bench_compare/benches/compare.rs`, `spikes/ecs_storage/bench_cache/benches/cache.rs`.
- **Test first:** none runnable here: the cache bench's size check runs only under Valgrind and does not catch a missing size (M3.21 log). The `cache_bench!` lines are checked against `SIZES` by reading them.
- **Change:** seven `bench::<…>` lines; seven `cache_bench!` lines with `[100, 1_000, 10_000]` in a `many_queries` group added to `main!`.
- **Verify:** `cargo bench -p spike_ecs_bench_compare --no-run` and `cargo bench -p spike_ecs_bench_cache --no-run` rc 0; `grep -c "bench::<.*ManyQueries>" spikes/ecs_storage/bench_compare/benches/compare.rs` is 7 and `grep -c "ManyQueries,$" spikes/ecs_storage/bench_cache/benches/cache.rs` is 7 (or each `cache_bench!` line counted by reading if formatting differs); step 12's run lists 21 `spike/ecs/many_queries/` ids. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): register many_queries in the spike benches (M3.22)`

### Step 10: documents (R6)

- [ ] **Who:** main session
- **Files:** `spikes/ecs_storage/README.md` (status-line date, harness row "nine workloads", workload list, rules link to spec §4.2), `spikes/README.md` (Contents row); the plan tick.
- **Test first:** `grep -rnE "eight workloads|the eighth|8 workloads" spikes/ docs/README.md .claude/` finds the stale counts (docs-1 style: the paths are named).
- **Change:** spec §4.5's document items.
- **Verify:** `cargo xtask check-docs` rc 0; the grep over the same paths finds nothing.
- **Commit:** `docs(spikes): document the many_queries workload (M3.22)`

### Step 11: allocation count (R4, ecs-2)

- [ ] **Who:** main session, in a scratchpad crate outside the workspace (own `[workspace]`, the pinned `rust-toolchain.toml`, the repo's `Cargo.lock`, a counting `#[global_allocator]` over `alloc` and `realloc`, the spike crates by path, release)
- **Files:** `log.md` only.
- **Test first:** none (a measurement). Expected: `vec_aos` 0 per step; `bevy_ecs` 0 (its closures and states are built in `setup`); each prototype its recorded per-query-run count times 56; `hecs` unbroken-down.
- **Change:** count allocations per `step` for the seven backends at `count` 1,000, seed 42, 100 steps after 10 warm-up steps; min and max; record with the full commit hash (specs-4, docs-4).
- **Verify:** the log entry exists and names the commit.
- **Commit:** `docs: record many_queries allocation counts (M3.22)`

### Step 12: the informal run (R7)

- [ ] **Who:** main session
- **Files:** `log.md`.
- **Test first:** none.
- **Change:** `/bench-run` with `cargo xtask spike-bench ecs` on the final code commit (after review fixes; specs-4). Before quoting, compare siblings (ecs-9): `hybrid` with `archetype`, `hecs` with `archetype`, and `simple_iter` rows with M3.21's run. Caveats say measured or expected and unmeasured (specs-6, docs-3), including spec §4.5's cache gap and step 11's allocations. Record commit, conditions, power log and the `many_queries` table in `log.md`; the pull request, the comment on #102 and the pointer on #84 (stage 7, once the pull request exists) carry it.
- **Verify:** the log entry names the commit and the power-log check.
- **Commit:** `docs: record the many_queries informal run (M3.22)`

## 3. Final verification

`typos` over the chunk folder (docs-4), then `cargo xtask ci` with nothing else running `cargo`, and the spec's Done-when checklist.

## 4. Lessons

- **ecs-1:** not applicable: no path test (no storage path is chosen by the workload).
- **ecs-2:** followed: step 11.
- **ecs-6:** followed: step 3's red check, with `--no-fail-fast`.
- **ecs-7:** followed: §1's iteration form, the same as each backend's `simple_iter`.
- **ecs-9:** followed: step 12's sibling check. No new column fetch is added.
- **ecs-11:** followed: the 8 empty queries are guarded by `for_each_query!` (steps 3 to 8's greps) and bevy's 56-entry test (step 7); `hybrid`'s no-sparse-set test (step 6); step 3's red check shows the late ticks are compared.
- **specs-1:** followed: every "finds nothing" grep names its path.
- **specs-2:** followed: steps 11 and 12 are their own steps.
- **specs-3:** followed: step 2's match-count test covers each distance class of spec §4.3.
- **specs-4:** followed: step 12 runs on the final code; step 11 is re-run if review fixes change a step.
- **specs-6:** followed: step 12.
- **docs-3:** followed: step 12.
- **docs-4:** followed: §3, and full hashes in `log.md`.

## 5. Deviations

None yet.
