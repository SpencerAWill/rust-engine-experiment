# M3.25 plan: rare_components workload for the ECS storage harness

- **Status:** Accepted (2026-11-27)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. Progress and surprises go in [log.md](log.md).

## 1. Approach

The harness module comes first, then the reference, so every later step has something to conform to; then the four prototypes, the two baselines, bench registration, documents, the allocation count (ecs-2) and the informal run last, on the final code (specs-4). One backend per step, as M3.21 to M3.24.

Choices the spec leaves to implementation, fixed here:

- **Names:** module `rare_components` in each crate; types `VecRareComponents`, `ArchetypeRareComponents`, `SparseSetRareComponents`, `DenseRareComponents`, `HybridRareComponents`, `BevyRareComponents`, `HecsRareComponents`; `BACKEND` = `"vec_aos"`, `"archetype"`, `"sparse_set"`, `"dense"`, `"hybrid"`, `"bevy_ecs"`, `"hecs"`.
- **Harness attributes:** `spawn` and `hash` are `#[must_use]`, `update` is `#[inline]`, as `add_remove`'s; the threshold is a private `const RARE_BELOW: f32 = 0.01`.
- **Step shape:** the step's query in a named function the placement tests can reach where the crate has the pattern (ecs-1); no reused buffer is needed (one query, no deferred change). `for_each` on every backend (ecs-7); `hecs` `query_mut::<(&mut Common, &Rare)>().into_iter().for_each(..)`.
- **Conformance constant:** each crate's tests use `const RARE_PARAMS: Params = Params { count: 10_000, seed: 42 }` (spec §4.4), 120 ticks.

Every conformance test is `check_conformance::<VecRareComponents, X>(RARE_PARAMS, 120)`, named `rare_components_<backend>_matches_the_reference`; every zero-count test `rare_components_<backend>_zero_count_is_valid`, through the crate's existing zero-count helper or pattern.

**Red checks (ECS steps 3 to 8)**, each run with `cargo nextest run -p <package> --no-fail-fast`, failures and messages recorded in `log.md`, code restored and `diff`ed against a saved copy (ecs-6):

- **(a)** an empty step body: the conformance test MUST fail at tick 1.
- **(b)** the step's query widened to `(&mut Common, Option<&Rare>)`, calling `update` with `&Rare(0.5)` where `Rare` is absent: the conformance test MUST fail at tick 1.

## 2. Steps

### Step 1: the harness module (R1)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/harness/src/rare_components.rs` (new), `harness/src/lib.rs` (`pub mod rare_components;`; crate doc: the rules of the twelfth, `rare_components`, in the M3.25 spec §4.2).
- **Test first:** in the module's test module: `spawn_follows_the_draw_order` (at seed 42, find by scanning `i` the first rare and the first non-rare entity; for each, replay `prng.stream(i)` by hand: draw 1 is `Common`, draw 2 is `d`, and for the rare one draw 3 is `Rare`; assert `spawn` returns those values and `Some`/`None` as `d < 0.01`; ecs-14); `rare_count_at_seed_42` (`count` 10,000 gives 78 rare entities, counted by replaying draw 2 of each stream, not by calling `spawn`); `update_wraps` (`Common(0.25)`, `Rare(0.5)` gives `0.75`; `Common(0.75)`, `Rare(0.5)` gives `0.75 + 0.5 - 1.0` computed in f32); `hash_sees_common_and_rare` (changing `Common`, adding a `Rare`, or changing its value each changes the hash). Fails first: the module does not exist (E0432).
- **Change:** spec §4.2, each item documented.
- **Verify:** `cargo nextest run -p spike_ecs_harness` passes with the 4 new tests. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add the rare_components workload rules to the harness (M3.25)`

### Step 2: the `vec_aos` reference (R2, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/vec/src/rare_components.rs` (new), `vec/src/lib.rs`, `vec/src/tests.rs`.
- **Test first:** `rare_components_vec_zero_count_is_valid`; `rare_components_checksum_never_repeats` (at `RARE_PARAMS`, 120 ticks: each tick's checksum differs from every earlier tick's, tick 0 included; ecs-13). Fails first: the type does not exist.
- **Change:** spec §4.3's `vec_aos`.
- **Red check:** an empty step body: the never-repeats test MUST fail at tick 1; recorded in `log.md`; restore.
- **Verify:** `cargo nextest run -p spike_ecs_vec` passes. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add the rare_components reference on vec_aos (M3.25)`

### Step 3: `archetype` (R3, R4, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/archetype/src/rare_components.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** conformance and zero-count tests; `rare_components_archetype_places_rare_entities_in_their_own_table` (spec §4.5: `archetype_count()` is 3 after `setup` and after one step, `check_invariants()` `Ok`, and the step query's matched archetypes are exactly one, holding 78 rows at `RARE_PARAMS`). Fails first: the type does not exist.
- **Change:** spec §4.3 for `archetype`.
- **Red checks:** (a), (b) of §1.
- **Verify:** `cargo nextest run -p spike_ecs_archetype` passes. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add rare_components to the archetype prototype (M3.25)`

### Step 4: `sparse_set` (R3, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/sparse_set/src/rare_components.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** conformance and zero-count tests. Fails first: the type does not exist.
- **Change:** spec §4.3: `spawn`, `insert` of `Common`, then of `Rare` for the rare ones; no group.
- **Red checks:** (a), (b) of §1.
- **Verify:** `cargo nextest run -p spike_ecs_sparse_set` passes. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add rare_components to the sparse_set prototype (M3.25)`

### Step 5: `dense` (R3, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/dense/src/rare_components.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** conformance and zero-count tests. Fails first: the type does not exist.
- **Change:** as step 4.
- **Red checks:** (a), (b) of §1.
- **Verify:** `cargo nextest run -p spike_ecs_dense` passes. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add rare_components to the dense prototype (M3.25)`

### Step 6: `hybrid` (R3, R4, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/hybrid/src/harness_components.rs` (`rare_components::Common` and `rare_components::Rare` in the `TableStorage` list; module doc: twelve workloads), `src/rare_components.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** conformance and zero-count tests; `rare_components_hybrid_places_rare_entities_in_their_own_table` (as step 3's); `rare_components_hybrid_stores_both_in_tables` (ecs-11, as `wide_entities_hybrid_stores_extra_sparse`: both types' `StorageKind` is the table kind, and no sparse set exists after a step). Fails first: the type does not exist.
- **Change:** spec §4.3 for `hybrid`.
- **Red checks:** (a), (b) of §1; and (c): move `Rare` to the `SparseStorage` list: the placement and storage tests MUST fail.
- **Verify:** `cargo nextest run -p spike_ecs_hybrid` passes. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add rare_components to the hybrid prototype (M3.25)`

### Step 7: `bevy_ecs` (R3, R5)

- [ ] **Who:** builder
- **Files:** `spikes/ecs_storage/bevy/src/rare_components.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** conformance and zero-count tests; `rare_components_bevy_stores_rare_in_a_table` (spec §4.5, as `wide_entities_bevy_stores_extra_in_a_table`: after a step `Table<Rare>` is registered and in an archetype, and `Sparse<Rare>` was never registered). Fails first: the type does not exist.
- **Change:** spec §4.3 for `bevy_ecs`: the step's `QueryState` built in `setup`; `clear_trackers()` at the end of `step`.
- **Red checks:** (a), (b) of §1; and (c): spawn rare entities with `Sparse(rare)` (and query it): the storage test MUST fail.
- **Verify:** `cargo nextest run -p spike_ecs_bevy` passes. `grep -nE "world\.query|query_filtered|try_query" spikes/ecs_storage/bevy/src/rare_components.rs` shows them only in `setup` and `checksum`. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add rare_components to the bevy_ecs baseline (M3.25)`

### Step 8: `hecs` (R3, R5)

- [ ] **Who:** builder
- **Files:** `spikes/ecs_storage/hecs/src/rare_components.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Test first:** conformance and zero-count tests. Fails first: the type does not exist.
- **Change:** spec §4.3 for `hecs`.
- **Red checks:** (a), (b) of §1.
- **Verify:** `cargo nextest run -p spike_ecs_hecs` passes. `grep -nE "PreparedQuery|CommandBuffer" spikes/ecs_storage/hecs/src/rare_components.rs` finds nothing. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add rare_components to the hecs baseline (M3.25)`

### Step 9: bench registration (R6)

- [ ] **Who:** builder
- **Files:** `spikes/ecs_storage/bench_compare/benches/compare.rs`, `spikes/ecs_storage/bench_cache/benches/cache.rs`.
- **Test first:** none runnable here (the cache bench's size check runs only under Valgrind; M3.21 log). The `cache_bench!` lines are checked against `SIZES` by reading them.
- **Change:** seven `bench::<…>` lines, each after the backend's `hierarchy` line; seven `cache_bench!` lines with `[10_000, 100_000, 1_000_000]` in a `rare_components` group added to `main!`; imports merged into each crate's `use` group.
- **Verify:** `cargo bench -p spike_ecs_bench_compare --no-run` and `cargo bench -p spike_ecs_bench_cache --no-run` rc 0; `grep -c "bench::<.*RareComponents>" spikes/ecs_storage/bench_compare/benches/compare.rs` is 7, and the seven `cache_bench!` invocations naming `RareComponents` are counted by reading. Step 12's run lists 21 `spike/ecs/rare_components/` ids. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): register rare_components in the spike benches (M3.25)`

### Step 10: documents (R7)

- [ ] **Who:** main session
- **Files:** `spikes/ecs_storage/README.md` (status-line date; the rules sentence links the M3.25 spec §4.2; the harness row "twelve workloads"; the workload list; the backends sentence; one sentence on what the row shows, with spec §4.6's caveats that bear on reading it), `spikes/README.md` (Contents row), `CHANGELOG.md` (one entry, as M3.23's); the plan tick.
- **Test first:** `grep -rnE "eleven (shared )?workloads|11 workloads" spikes/ CHANGELOG.md` finds the stale text (specs-1).
- **Change:** spec §4.6's document items (the harness crate doc and `harness_components.rs` are steps 1 and 6).
- **Verify:** `cargo xtask check-docs` rc 0; the grep over the same paths finds nothing except history entries in `CHANGELOG.md`, each read.
- **Commit:** `docs(spikes): document the rare_components workload (M3.25)`

### Step 11: allocation count (R4, R5, ecs-2)

- [ ] **Who:** main session, in a scratchpad crate outside the workspace (own `[workspace]`, the pinned `rust-toolchain.toml`, the repo's `Cargo.lock`, a counting `#[global_allocator]` over `alloc` and `realloc`, the spike crates by path, release)
- **Files:** `log.md` only.
- **Test first:** none (a measurement). Pass rule (spec §4.5): each backend's per-step count is the same at every step after warm-up and equals its `simple_iter` control's.
- **Change:** count allocations per `step` for the seven backends' `rare_components` and `simple_iter` at `count` 10,000, seed 42, 100 steps after 10 warm-up steps; min and max; record with the full commit hash (specs-4, docs-4). A difference from the control is explained in `log.md` or flagged.
- **Verify:** the log entry exists and names the commit.
- **Commit:** `docs: record rare_components allocation counts (M3.25)`

### Step 12: the informal run (R8)

- [ ] **Who:** main session
- **Files:** `log.md`.
- **Test first:** none.
- **Change:** `/bench-run` with `cargo xtask spike-bench ecs --save m3.25-informal` on the final code commit (after review fixes; specs-4). Before quoting, compare siblings (ecs-9): `hybrid` with `archetype` (both walk one table here), `hecs` and `bevy_ecs` with `archetype`, and this run's `simple_iter` rows with M3.23's `m3.23-informal`. Caveats per spec §4.6, each said as measured or expected and unmeasured (specs-6, docs-3), with step 11's counts. Record commit, conditions, power log and the `rare_components` table in `log.md`; the pull request, the comment on #102 and the comment on #41 (stage 7, once the pull request exists) carry them.
- **Verify:** the log entry names the commit and the power-log check.
- **Commit:** `docs: record the rare_components informal run (M3.25)`

## 3. Final verification

`typos` over the chunk folder (docs-4), then `cargo xtask ci` with nothing else running `cargo`, and the spec's Done-when checklist.

## 4. Lessons

- **ecs-2:** followed: step 11, with each backend's `simple_iter` as the one-query control and the spec's pass rule.
- **ecs-6:** followed: the red checks of §1 run with `--no-fail-fast`; no storage code changes, so no storage red check is needed.
- **ecs-7:** followed: `for_each` on every backend; no hot-path operation is adapted (the step is one query); bevy's `QueryState` built in `setup`.
- **ecs-9:** followed: no new column fetch; step 12's sibling comparison before quoting.
- **ecs-11:** followed: the placement and storage tests of steps 3, 6 and 7, with red checks (c) on `hybrid` and `bevy_ecs`.
- **ecs-13:** followed: the simulation is in `log.md` (brainstorm); step 2's never-repeats test.
- **ecs-14:** followed: step 1's spawn and rare-count tests replay the stream's draws by hand.
- **specs-1:** followed: step 10's grep names its paths.
- **specs-2:** followed: steps 11 and 12 are their own steps; red-check records are in each step.
- **specs-3:** followed: each test of spec §4.5 is named in steps 1 to 7.
- **specs-4:** followed: step 12 on the final code; step 11 re-run if review changes a `step`.
- **specs-6:** followed: step 12's caveats.
- **docs-1:** followed: step 10's grep over `spikes/` and `CHANGELOG.md`.
- **docs-3:** followed: step 10's README and CHANGELOG sentences carry the caveats; step 12.
- **docs-4:** followed: §3, and full hashes in `log.md`.

## 5. Deviations

None yet.
