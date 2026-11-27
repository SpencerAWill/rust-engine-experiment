# M3.26 plan: workload checks that can fail for frag_iter, heavy_compute and add_remove

- **Status:** Accepted (2026-11-26)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. Progress and surprises go in [log.md](log.md).

## 1. Approach

Rules first, each in one commit with every caller, so the workspace builds at every commit: `frag_iter` touches only the harness and `vec_aos` tests; `heavy_compute`'s new `update` signature breaks all eight callers, so they change together. Then the `add_remove` tests, reference first, then the ECS backends by crate. Documents next, the informal run last, on the final code after review (specs-4).

Choices the spec leaves to implementation, fixed here:

- **History tests** (spec §4.4) are named `<workload>_checksum_never_repeats`, in `vec/src/tests.rs`: setup at the conformance parameters (`frag_iter` `Params { count: 20, seed: 42 }`, `heavy_compute` `Params { count: 1_000, seed: 42 }`), collect the checksum at tick 0 and after each of 120 steps, and assert each is absent from the earlier ones, with a message naming both ticks. Written first, each fails on the old rule, which is the spec's old-rule red check (record the message in `log.md`).
- **`heavy_compute::update`**: `pub fn update(matrix: &mut Matrix, position: &mut Position, rotation: &Rotation, velocity: &Velocity)`; the translation step is `matrix.0.w_axis += Vec4::from((velocity.0 * dt(), 0.0))` or the per-row form of spec §4.2, whichever reads better (both are multiply then add per row; `w` gains `+ 0.0`, exact).
- **`add_remove` accessors** (spec §4.3): `#[cfg(test)] pub(crate) fn removed_last_step(&self) -> &[Entity]` on each backend struct (on `Base` for the byte-column pair, forwarded by both), returning the stored removal list as a slice (ecs-5: no `&mut`, no `&Vec`). The test, `add_remove_<backend>_inserts_on_every_entity_each_step`, runs 120 steps at `PARAMS` (count 1,000, seed 42) and after each asserts `removed_last_step().len() == PARAMS.count` and that the list has no duplicate (collected into a `BTreeSet` of the handle, or of `to_bits()` where the handle has no `Ord`), with the tick in each message.
- **`add_remove` red checks** (spec §4.3, ecs-6, ecs-11): per backend, (a) delete the insert loop; (b) keep the insert only when a step counter is 0. Run `cargo nextest run -p <package> --no-fail-fast`; (a) must fail the new test at tick 1, (b) at tick 2. Record each failing test list and message in `log.md`, restore with `git checkout -- <file>`, delete any `proptest-regressions/` written.
- **`vec_aos` hook** (spec §4.4): `fn step_with<F: FnMut(&[(A, Option<B>)])>(&mut self, between: F)` runs the insert loop, calls `between(&self.entities)`, then the remove loop; `step` calls `self.step_with(|_| {})`. Adjust the element type to the struct's actual field.

## 2. Steps

### Step 1: the `frag_iter` rule (R1, R2)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/harness/src/frag_iter.rs` (`update` and its doc, module doc if it states the rule); `spikes/ecs_storage/vec/src/tests.rs`.
- **Test first:** `frag_iter_checksum_never_repeats` fails on the old rule (tick 26 equals tick 25); `frag_iter_data_follows_the_map` rewritten for the new rule, with one input that wraps (`0.5` gives `0.5 + 0.618_034 - 1.0` computed the same way) and one that does not (`0.25`), fails on the old rule.
- **Change:** spec §4.2, `frag_iter`.
- **Verify:** `cargo nextest run -p spike_ecs_vec` passes; `cargo xtask test` passes (every backend's `frag_iter` conformance, byte-column ones included); `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): make frag_iter's update keep changing (M3.26)`

### Step 2: the `heavy_compute` rule (R1, R2, R4)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/harness/src/heavy_compute.rs`; `heavy_compute.rs` in `vec`, `sparse_set`, `archetype`, `dense`, `hybrid`, `bevy` and `hecs` (`src/`); `spikes/ecs_storage/vec/src/tests.rs`.
- **Test first:** `heavy_compute_checksum_never_repeats` fails on the old rule (a checksum equal to an earlier tick's); `heavy_compute_position_is_the_transformed_rotation` updated so it checks one step against a hand-written application of the new rule (100 inversions, translation `+ velocity * dt()`, then the transform), and fails on the old rule.
- **Change:** spec §4.2, `heavy_compute`: the harness `update`; each backend adds `&Velocity` to its step query and passes it. `sparse_set`: `setup`'s group and the type aliases' and struct's docs become `(Matrix, Position, Rotation, Velocity)`.
- **Verify:** `cargo nextest run -p spike_ecs_vec` passes; `cargo xtask test` passes (`heavy_compute` conformance on all seven, and `grouped_heavy_compute_uses_the_group`); `grep -rn "rules::update" spikes/ecs_storage/*/src/heavy_compute.rs` shows eight call sites, each with four arguments; `cargo xtask ci --quick` rc 0.
- **Commit:** `feat(spikes): move heavy_compute's translation each step (M3.26)`

### Step 3: the `vec_aos` `add_remove` hook (R3, R7)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/vec/src/add_remove.rs`, `vec/src/tests.rs`.
- **Test first:** `add_remove_vec_inserts_on_every_entity_each_step` (through a `#[cfg(test)]` method that calls `step_with` with an asserting closure: every entity holds `Some(make_b(a))`; 120 steps, tick in the message). Fails first: the method does not exist.
- **Change:** §1's hook.
- **Red check:** delete the insert loop; `cargo nextest run -p spike_ecs_vec --no-fail-fast` fails the new test at tick 1; record; restore.
- **Verify:** `cargo nextest run -p spike_ecs_vec` passes; `cargo xtask ci --quick` rc 0.
- **Commit:** `test(spikes): check vec_aos's add_remove insert pass every step (M3.26)`

### Step 4: `add_remove` tests on `sparse_set` and `archetype` (R3, R7)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/sparse_set/src/{add_remove,tests}.rs`; `spikes/ecs_storage/archetype/src/{add_remove,bytes_add_remove,tests}.rs`.
- **Test first:** the §1 test for `sparse_set`, `sparse_set_grouped`, `archetype`, `archetype_bytes_copy` and `archetype_bytes_cast` (five tests). Fails first: `removed_last_step` does not exist.
- **Change:** the §1 accessors.
- **Red checks:** §1 (a) and (b) on each of the five steps (the generic `AddRemoveOn` covers both `sparse_set` backends with one edit; the byte pair shares `Base::insert_pass`, so one edit each covers both, and the log says so).
- **Verify:** `cargo nextest run -p spike_ecs_sparse_set -p spike_ecs_archetype` passes; `cargo xtask ci --quick` rc 0.
- **Commit:** `test(spikes): check the add_remove insert pass every step on sparse_set and archetype (M3.26)`

### Step 5: `add_remove` tests on `hybrid` and `dense` (R3, R7)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/{hybrid,dense}/src/{add_remove,tests}.rs`.
- **Test first, change, red checks:** as step 4, two tests.
- **Verify:** `cargo nextest run -p spike_ecs_hybrid -p spike_ecs_dense` passes; `cargo xtask ci --quick` rc 0.
- **Commit:** `test(spikes): check the add_remove insert pass every step on hybrid and dense (M3.26)`

### Step 6: `add_remove` tests on `bevy_ecs` and `hecs` (R3, R7)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/bevy/src/{add_remove,tests}.rs`; `spikes/ecs_storage/hecs/src/{add_remove,tests}.rs`.
- **Test first, change, red checks:** as step 4, three tests (`bevy_ecs`, `bevy_ecs_sparse`, `hecs`).
- **Verify:** `cargo nextest run -p spike_ecs_bevy -p spike_ecs_hecs` passes; `cargo xtask ci --quick` rc 0.
- **Commit:** `test(spikes): check the add_remove insert pass every step on bevy_ecs and hecs (M3.26)`

### Step 7: documents (R5)

- [x] **Who:** main session
- **Files:** `docs/chunks/m3.2-spike-harness/spec.md` (status line, §4.4.2, §4.4.3, R8, R9's verification row); `spikes/ecs_storage/README.md`; `CHANGELOG.md`.
- **Test first:** the spec's R5 grep, `git grep -n -e '0.5 + 1.0' -e 'converges to 2' -- ':!docs/chunks/'`, finds nothing after steps 1 and 2 (code and its docs), and in `docs/chunks/m3.2-spike-harness/spec.md` matches the rule lines before this step.
- **Change:** spec §4.5.
- **Verify:** the grep matches only amendment notes in the M3.2 spec; `cargo xtask check-docs` rc 0; `typos docs/chunks/m3.26-workload-checks-that-fail` rc 0 (docs-4).
- **Commit:** `docs: amend the frag_iter and heavy_compute rules (M3.26)`

### Step 8: the informal run (R6)

- [ ] **Who:** main session, through `/bench-run`, after stage 5's fixes are in (specs-4)
- **Files:** `docs/chunks/m3.26-workload-checks-that-fail/log.md`.
- **Change:** `cargo xtask spike-bench ecs --save m3.26-informal` on the final code commit; the `frag_iter` and `heavy_compute` medians per backend and size, with the commit hash, machine, power log check and "one run, no noise floor"; before quoting, the sibling check (ecs-7, ecs-9): `hybrid` against `archetype` and `sparse_set_grouped` against `sparse_set` on both workloads, and any gap explained as measured or unmeasured (specs-6). The same rows go in the pull request and a comment on #102 (spec §4.6).
- **Verify:** the run's rc 0 and its id count; `pmset -g log` shows no sleep in the window.
- **Commit:** `docs: record the M3.26 informal run (M3.26)`

## 3. Final verification

`cargo xtask ci` with nothing else running `cargo`; the spec's §9 checklist.

## 4. Lessons

- **ecs-13:** followed (spec §4.1 simulation; steps 1 and 2 history tests).
- **ecs-11:** followed (steps 3 to 6: per-step state tests, red-checked by removing the write).
- **ecs-6:** followed (red checks listed with `--no-fail-fast`; the corrupting edit removes the insert, which only the new tests see).
- **ecs-5:** followed (accessors return `&[Entity]`).
- **ecs-1:** followed (the grouped path test builds the step's own query).
- **ecs-7, ecs-9:** followed in step 8 (sibling comparison before quoting); no hot-path operation changes otherwise.
- **ecs-10:** not applicable: no byte-column move changes (the byte backends gain only a test accessor).
- **specs-1:** followed (step 7's grep names its paths). **specs-2, specs-4:** followed (the informal run is its own step, on the final commit). **specs-3:** followed (one test per backend, named). **specs-6:** followed in step 8.
- **docs-1:** followed (step 7's grep for the old rule's phrases). **docs-3:** followed (no ranking claims; any expectation says "expected, unmeasured"). **docs-4:** followed (step 7's `typos`; full hashes in `log.md`).

## 5. Deviations

- **Step 2, files:** `spikes/ecs_storage/dense/src/tests.rs` also changed: `heavy_compute_dense_joins` lists the step query's types, so it gains `Velocity`. The file list missed it.
- **Step 2, verify:** the `rules::update` grep shows seven call sites, not eight: `sparse_set` and `sparse_set_grouped` share one generic `HeavyComputeOn<GROUPED>` step. Each passes four arguments.
- **Step 6, names:** the bevy tests are `add_remove_bevy_inserts_on_every_entity_each_step` and `add_remove_bevy_sparse_inserts_on_every_entity_each_step`, following that crate's `add_remove_bevy_*` / `add_remove_bevy_sparse_*` prefixes rather than the backend names `bevy_ecs` / `bevy_ecs_sparse`. bevy's two backends have separate steps, so each red check took one edit per backend.
- **Step 3, form:** the test calls the `pub(crate)` `step_with` directly with its asserting closure, rather than through a separate `#[cfg(test)]` method as spec §4.4 words it; the measured `step` is `step_with(|_| {})` either way.
- **Review fix F1:** `grouped_heavy_compute_uses_the_group` also asserts the group matches the step query exactly, since `uses_group()` alone holds for the old three-type group (R4's "shown by its existing path test" could not fail).
