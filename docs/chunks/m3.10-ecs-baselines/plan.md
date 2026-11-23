# M3.10 plan: bevy_ecs and hecs baselines as harness backends

- **Status:** Accepted (2026-11-23)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. Progress and surprises go in [log.md](log.md).

## 1. Approach

The two crates are built one after the other, bevy first, because its wrappers, cached `QueryState`s and `clear_trackers` make it the harder one. The sparse variant is a separate step, so its storage tests land with it. Then hecs, then the `deny.toml` guard, which needs both crates to exist so that neither wrapper is unused. Then bench registration and the documents. The informal run comes last, on the final code (specs-4).

Choices the spec leaves to implementation, fixed here:

- **Module layout:** one module per workload in each crate, named as in the harness. In `spike_ecs_bevy`, `add_remove.rs` holds `BevyAddRemove` and `BevySparseAddRemove`, and `agents.rs` holds `BevyAgents` and `BevySparseAgents`. Code shared between a table type and its sparse twin MAY be one private generic function, provided both types keep their own `BACKEND` and storage.
- **Wrappers:** `spike_ecs_bevy::Table<T>` and `spike_ecs_bevy::Sparse<T>` in `lib.rs`, both `pub`, with `#[derive(Component, Clone, Copy, Debug)]` and `T: Send + Sync + 'static`.
- **Agent newtypes:** `AgentId(pub u64)`, `Velocity(pub Vec2)` and `Ttl(pub u32)`, defined in each crate's `agents.rs`, as the prototypes do.
- **Test hooks:** under `#[cfg(test)]`, a `pub(crate) fn world(&self) -> &World` on each bevy workload type that the R4 and R5 tests need.
- **Manifests:** as spec §4.1. `spike_ecs_vec` is a dev-dependency; `proptest` is not needed.

## 2. Steps

### Step 1: `spike_ecs_bevy` with the seven `bevy_ecs` backends (R1, R3, R4, R6)

- [ ] **Who:** builder
- **Files:** `spikes/ecs_storage/bevy/Cargo.toml`, `src/lib.rs`, `src/simple_iter.rs`, `src/frag_iter.rs`, `src/heavy_compute.rs`, `src/add_remove.rs`, `src/entity_churn.rs`, `src/sprites.rs`, `src/agents.rs`, `src/tests.rs` (new); `xtask/src/coverage.rs` (`spike_ecs_bevy` in `SPIKE_PACKAGES` and in `test_args_match_the_spec`'s expected list); `Cargo.lock`.
- **Test first:** in `tests.rs`, one conformance test per workload against the `vec_aos` type of the same workload, with `Params { count: 1_000, seed: 42 }` and 120 ticks (1,300 for `agents`): `simple_iter_bevy_matches_the_reference`, `frag_iter_bevy_matches_the_reference` (count 20), `heavy_compute_bevy_matches_the_reference`, `add_remove_bevy_matches_the_reference`, `entity_churn_bevy_matches_the_reference`, `sprites_bevy_matches_the_reference`, `agents_bevy_matches_the_reference`. Then the R4 tests, `add_remove_bevy_clears_removals_each_step`, `entity_churn_bevy_clears_removals_each_step` and `agents_bevy_clears_removals_each_step`. Each runs 3 steps, then asserts that `world().removed::<Table<B>>()` (or `<Table<Lifetime>>`, `<Table<Seeker>>` respectively) is empty. Red check: with `clear_trackers` left out of `add_remove`'s `step`, its test fails, and with 1,000 entities the count is 3,000. Record the output, then restore the call.
- **Change:** spec §4.3 to §4.5 for the `bevy_ecs` column: `QueryState`s built in `setup`, changes decided in a query collected and applied in recorded order, `clear_trackers` at the end of every `step`, the backend's own alive counter, no query of `Entity` alone. `lib.rs` docs as spec §4.7, last item.
- **Verify:** `cargo nextest run -p spike_ecs_bevy` shows all 10 tests passing. `cargo xtask ci --quick` rc 0. `grep -rnE 'Schedule|IntoSystem|App\b|insert_batch|spawn_batch' spikes/ecs_storage/bevy/src` finds nothing (specs-1). `grep -n spike_ecs_bevy xtask/src/coverage.rs` finds both entries.
- **Commit:** `feat(spikes): add the bevy_ecs baseline backends (M3.10)`

### Step 2: `bevy_ecs_sparse` (R3, R4, R5)

- [ ] **Who:** builder
- **Files:** `spikes/ecs_storage/bevy/src/lib.rs` (`Sparse<T>` if step 1 left it out), `src/add_remove.rs`, `src/agents.rs`, `src/tests.rs`.
- **Test first:**
  - Conformance: `add_remove_bevy_sparse_matches_the_reference` and `agents_bevy_sparse_matches_the_reference`, as in step 1.
  - R4: `add_remove_bevy_sparse_clears_removals_each_step` and `agents_bevy_sparse_clears_removals_each_step`.
  - R5, one test per backend: `bevy_storage_is_table_for_every_type` covers the step-1 types after `setup` and one step of `add_remove` and `agents`. `bevy_sparse_storage_is_sparse_for_b_and_seeker_only` reads each registered component's `StorageType` through `World::components()`. Both look up the component ids by type.
  - R5 caveat: `bevy_sparse_insert_moves_archetype_not_table`. On a `BevySparseAddRemove` world after `setup`, insert `Sparse<B>` on one entity through the world. Assert that `World::storages().tables.len()` is unchanged and that the entity's archetype id differs from before.
  - Fails first: the types do not exist (E0432).
- **Change:** spec §4.5 for `bevy_ecs_sparse`. `B` and `Seeker` are `Sparse<…>`; every other type is `Table<…>`.
- **Verify:** `cargo nextest run -p spike_ecs_bevy` shows all 17 tests passing. `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): add the bevy_ecs_sparse baseline backend (M3.10)`

### Step 3: `spike_ecs_hecs` (R1, R3, R5, R6)

- [ ] **Who:** builder
- **Files:** `spikes/ecs_storage/hecs/Cargo.toml`, `src/lib.rs`, the seven workload modules, `src/tests.rs` (new); `xtask/src/coverage.rs` (`spike_ecs_hecs` in both lists); `Cargo.lock`.
- **Test first:** seven conformance tests named `<workload>_hecs_matches_the_reference`, with the parameters of step 1. They fail first because the package does not exist.
- **Change:** spec §4.3 to §4.5 for `hecs`: `query_mut`/`query` per step, never `PreparedQuery`; a `CommandBuffer` kept in the workload for changes decided inside a query (`entity_churn`, `agents`), run with `run_on`; harness types stored as they are, except the agent newtypes.
- **Verify:** `cargo nextest run -p spike_ecs_hecs` shows 7 tests passing. `cargo xtask ci --quick` rc 0. `grep -rnE 'PreparedQuery|spawn_batch|Schedule' spikes/ecs_storage/hecs/src` finds nothing. `grep -n spike_ecs_hecs xtask/src/coverage.rs` finds both entries.
- **Commit:** `feat(spikes): add the hecs baseline backend (M3.10)`

### Step 4: the dependency guard (R2)

- [ ] **Who:** main session (configuration)
- **Files:** `deny.toml`; `log.md`.
- **Test first:** before the change, temporarily add `hecs = { version = "0.11.2", default-features = false, features = ["std"] }` to `spikes/ecs_storage/harness/Cargo.toml` and run `cargo deny check bans`. It passes, because nothing bans `hecs` yet.
- **Change:** the `deny` list of spec §4.2. Then, with the temporary dependency still in place, `cargo deny check bans` MUST fail with `crate 'hecs = 0.11.2' is explicitly banned`. Revert the manifest and `Cargo.lock` (`git diff --stat` shows only `deny.toml`), and record both outputs in `log.md` with the commit (specs-2, specs-4).
- **Verify:** `cargo xtask deny` rc 0, with no `unused-wrapper` or `banned` diagnostics. `git diff --stat` shows `deny.toml` and `log.md` only.
- **Commit:** `build: let only the baseline crates depend on bevy_ecs and hecs (M3.10)`

### Step 5: bench registration (R7)

- [ ] **Who:** builder
- **Files:** `spikes/ecs_storage/bench_compare/Cargo.toml`, `benches/compare.rs`, `spikes/ecs_storage/bench_cache/Cargo.toml`, `benches/cache.rs`; `Cargo.lock`.
- **Test first:** n/a: registration has no behaviour of its own. Before the change, `cargo bench -p spike_ecs_bench_compare --bench compare -- --list 2>/dev/null | grep -cE '/(bevy_ecs|bevy_ecs_sparse|hecs)/'` prints 0.
- **Change:** dev-dependencies on both crates in both bench crates. Add 16 `bench::<…>(c);` lines in `compare.rs`. In `cache.rs`, add 16 `cache_bench!` lines named `<workload>_bevy_ecs`, `<workload>_bevy_ecs_sparse` and `<workload>_hecs`, with each workload's `Spec::SIZES` (as M3.19's plan step 7 lists them), each named in its workload's group.
- **Verify:** the `--list` count above is 33: 14 ids each for `bevy_ecs` and `hecs` (3 + 1 + 1 + 3 + 3 + 1 + 2 sizes) and 5 for `bevy_ecs_sparse`. `cargo bench -p spike_ecs_bench_compare --bench compare -- --test` rc 0. `cargo xtask bench-build` rc 0. `cargo xtask ci --quick` rc 0.
- **Commit:** `bench(spikes): register the bevy_ecs and hecs baselines (M3.10)`

### Step 6: documents (R9)

- [ ] **Who:** main session
- **Files:** `spikes/ecs_storage/README.md`, `spikes/README.md`, `CONTRIBUTING.md`, `CHANGELOG.md`.
- **Test first:** `grep -n "bevy/" spikes/ecs_storage/README.md` and `grep -n "M3.10" CHANGELOG.md` find nothing.
- **Change:** spec §4.7. The rule that a baseline crate may take a competing library is written once, in `spikes/README.md` Rules. `CONTRIBUTING.md` links to it instead of restating it (docs-1). Before writing, grep `docs/` and `.claude/` for "dev-dependencies" to find other copies of the ADR 0002 wording. The CHANGELOG entry keeps spec §4.5's expectations as "expected, unmeasured" (docs-3).
- **Verify:** both greps above find the new text. `cargo xtask doc` rc 0. `cargo xtask check-docs` rc 0.
- **Commit:** `docs: describe the ECS baseline backends (M3.10)`

## 3. Final verification

1. **Before the run (ecs-7):** read each baseline step against the `archetype` workload of the same name. Look for an extra collect, an extra pass, or a per-step allocation that the prototype does not have and the spec does not ask for. Write findings in `log.md`.
2. **Full check:** `cargo xtask ci` on the final code commit, with nothing else running `cargo`.
3. **The informal run (R10):** `/bench-run` with `cargo xtask spike-bench ecs`, on the final code commit (specs-4). Record the commit, the power log and the table of baseline rows beside `archetype`, `sparse_set` and `hybrid` in `log.md` (specs-2). Before anything is quoted, compare each baseline's iteration numbers with `archetype`'s (ecs-9). Where a row departs from §4.5's expectations, the log says whether the cause was measured or is expected and unmeasured (specs-6).
4. **Done when:** the spec's Done-when checklist.

## 4. Lessons

- **ecs-2:** deviated. The spec defers no per-query state of our own. The baselines' allocations are their libraries' and are outside what the spec measures. The one per-run cost the grill found, hecs's cache lock and lookup, is recorded in spec §4.4 as a caveat, not counted.
- **ecs-6:** deviated, n/a. This chunk writes no storage of its own. The red checks are the R4 removal tests (step 1), aimed at the code they cover.
- **ecs-7:** followed. See final verification, item 1.
- **ecs-9:** followed in part. The baselines fetch no columns themselves, so the fetch rule is n/a. The comparison with `archetype` before quoting applies (final verification, item 3).
- **docs-1:** followed (step 6).
- **docs-2:** followed. The counts 16 (implementations), 17 (bevy tests) and 33 (Criterion ids) appear in steps 2 and 5. A change to either is grepped across this plan.
- **docs-3:** followed (step 6).
- **specs-1:** followed. Each grep names its paths (steps 1 and 3).
- **specs-2:** followed. The R2 negative check (step 4) and the run's power log (final verification) are named steps.
- **specs-3:** followed. Each test is named (steps 1 to 3).
- **specs-4:** followed. The run and the full `ci` are on the final code commit.
- **specs-6:** followed (final verification, item 3).

## 5. Deviations

None yet.
