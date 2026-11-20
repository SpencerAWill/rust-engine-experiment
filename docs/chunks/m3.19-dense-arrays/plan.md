# M3.19 plan: Dense per-entity component arrays

- **Status:** Accepted (2026-11-20)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. Progress and surprises go in [log.md](log.md).

## 1. Approach

Eight steps, one commit each, built bottom up as M3.3 and M3.4 were: columns and a `World` with `insert`/`remove`/`despawn` and the model test; queries; `Commands`; three workload commits; the bench registrations; the documents. M3.3's and M3.4's crates (`spikes/ecs_storage/sparse_set/`, `spikes/ecs_storage/archetype/`) are this repository's own code: the builder MAY adapt their `query.rs`, `commands.rs`, `tests.rs` and workload modules (M3.3's workload modules are the closer fit: same insert-per-component spawning), and SHOULD read their plans' §5 Deviations and logs first, which record the compiler and lint traps they met.

Choices the spec leaves to implementation:

- **One type per workload**, `pub struct DenseX { world: World, … }` with private fields, `BACKEND = "dense"`. Workload modules are `pub mod`.
- **`Entity` and `EntityMarker`** live in `lib.rs`, as in M3.3.
- **Module homes.** `ComponentId(u32)` in `world.rs`; `DenseColumn<T>`, `ValuesMut<'a, T>` and `ErasedColumn` in `column.rs`. Items the seal traits mention are `pub` inside private modules (unnameable outside the crate), because `pub(crate)` trips `private_interfaces` (M3.3 plan §5, M3.4 plan §5).
- **`ErasedColumn` method names** must not collide with `Any::type_id`: `value_type_id`, `value_type_name`, plus `remove_index`, `present`, `present_count`, `as_any`, `as_any_mut`, and `check(&self) -> Result<(), String>` for D1 (so its test can corrupt the private fields inside `column.rs`).
- **Per-index tables.** `World { allocator, len, entities: Vec<Entity>, alive: HierarchicalBitSet, ids: BTreeMap<TypeId, ComponentId>, columns: Vec<Box<dyn ErasedColumn>> }`. `spawn` writes `entities[index]` (growing it) and sets `alive`.
- **`Query` holds its sets.** `Query<'w, Q, F>` has `world: &'w World`, `required: Vec<&'w HierarchicalBitSet>`, `excluded: Vec<&'w HierarchicalBitSet>`, `alive: [&'w HierarchicalBitSet; 1]` (the fallback of spec §4.5 step 3, so the fallback needs no allocation), and `empty: bool` (a required type has no column, so the run yields nothing and builds no `Join`). `World::query` resolves `Q`'s required types; `.with::<T>()` pushes to `required` (or sets `empty`), `.without::<T>()` pushes to `excluded` if `T` has a column. `iter(&self)` joins `&self.required`, or `&self.alive` if `required` is empty; `QueryIter` holds `Option<Join<'q>>` (`None` when `empty`) and `Q`'s fetch.
- **`QueryMut` resolves at run.** `for_each` destructures the `World` into its fields, collects `columns.iter_mut().map(Some)` into a `Vec<Option<&mut Box<dyn ErasedColumn>>>`, and each parameter `take`s its own column by `ComponentId` (a second `take` of the same column cannot happen: step 2's repeated-type check runs first). `&mut T` calls `split_mut` and pushes the presence set to `required`; `&T` turns its `&mut Box` into `&Box` and pushes `present()`; `Without<T>` takes its column shared and pushes to `excluded`. The grill's stub (log.md, 2026-11-20) built exactly this on 1.100.
- **The required-fetch panic** (spec §4.5 step 4) reads "dense query: `<T>` has no value at index <i> although its presence bit is set (invariant D1)"; it is an internal-bug panic, not a documented one.
- **`joined()`** returns `(Vec<Joined>, Vec<Joined>)` with `#[cfg(test)] pub(crate) enum Joined { Alive, Component(ComponentId) }`, computed by the same resolution functions the run uses (lesson ecs-1).
- **Panic messages** name the operation first: "World::insert called on a despawned entity …", "World::spawn: …", "`<T>` appears twice in the query" prefixed by the method (`Query::for_each:`, `Query::iter:`, `QueryMut::for_each:`). Types by `core::any::type_name::<T>()`, entities by `Debug`.
- **Invariant messages** contain the invariant's name (`"D1"`, `"D2"`, `"W1"`, `"W2"`), so a test can check which one fired. `check_invariants` checks D2, then each column's D1, then W2, then W1.

**Global constraints**

- Every commit passes `cargo xtask ci --quick`. Steps 1 and 7 also pass `cargo xtask deny`, with `deny.toml` unchanged (R11).
- The crate root has `#![forbid(unsafe_code)]`; the package has `[lints] workspace = true`. Never lift, relax or `allow` a CI-enforced attribute, even temporarily in the working tree (M3.4's log). Every public item is documented (R12). Doctests are not required ([spikes/README.md](../../../spikes/README.md)).
- Package `spike_ecs_dense`: dependencies `engine = { workspace = true }` from step 1, `spike_ecs_harness = { path = "../harness" }` from step 4; dev-dependencies `proptest = { workspace = true }` from step 1 and `spike_ecs_vec = { path = "../vec" }` from step 4. No `agents_2d`: the harness re-exports what `agents` needs (M3.4 review).
- Items a later step uses from library code and nothing uses yet are `#[cfg(test)]` until then, never `#[allow(dead_code)]`.
- Conformance: `Params { count: 1_000, seed: 42 }` (`count: 20` for `frag_iter`) and 120 ticks; `agents` 1,000 agents and 1,300 ticks (R8).
- Run `cargo fmt --all` before `ci --quick`. Commits are conventional, end with ` (M3.19)` and the session's attribution line, and stage files by explicit path.

**Review focus** (inputs the spec implies that no single requirement's test pins; each has a test in the step that owns the code)

1. **A stale ID whose index was reused.** After `despawn(a)` and a `spawn()` that returns `b` at the same index: `get::<T>(a)`, `remove::<T>(a)` and `despawn(a)` return `None`, `None` and `false` and leave `b` alone; `insert(a, …)` panics; and `b` has none of `a`'s components. Step 1: `stale_id_does_not_reach_the_reused_slot`.
2. **An index past a column's end.** `get`, `remove` and an `Option<&T>` parameter at an index the column never grew to return `None` and do not grow the column. Steps 1 and 2: `absent_past_the_end_does_not_grow`, `option_past_a_short_column_is_none`.
3. **Replacing a present value.** `insert` of a type the entity has returns the old value and leaves the bit set. Step 1: `replacing_a_value_returns_the_old_one`.
4. **A zero-sized tag** (`Hidden`): it gets a column and matches `With` and `Without`. Step 2: `zero_sized_component_filters`.
5. **A type both queried and excluded.** `query::<&A>().without::<A>()` panics as a repeated type. Step 2: `a_type_in_a_parameter_and_a_filter_panics`.
6. **`with` on a query with no required parameter.** `query::<Entity>().with::<B>()` (the `add_remove` removal query) joins `B`'s set, not `alive`; `query::<Entity>()` joins `alive`. Step 2: `with_replaces_the_alive_fallback`.
7. **Filters on never-registered types.** `.with::<T>()` of a type with no column matches nothing; `.without::<T>()` of one is ignored. Step 2: `filters_on_unregistered_types`.
8. **Order.** After despawns and respawns that reuse low indices, every query yields ascending indices. Step 2: `queries_yield_ascending_indices`; the model test compares sequences, not sets.
9. **`count: 0`.** Every workload sets up empty, has checksum 0 and steps without panicking. Steps 4 to 6: `zero_count_is_valid` for each.
10. **`len` across failed operations.** `despawn` of a dead entity returns `false` and leaves `len` unchanged; a failed `spawn` leaves it unchanged; a skipped command leaves it unchanged. Steps 1 and 3.

## 2. Steps

### Step 1: columns and the `World` (R1, R2 in part, R3, R4, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/dense/Cargo.toml`, `src/lib.rs`, `src/column.rs`, `src/world.rs`, `src/tests.rs` (new); workspace `Cargo.toml` (member); `xtask/src/coverage.rs` (`spike_ecs_dense` in `SPIKE_PACKAGES` and in `test_args_match_the_spec`'s expected `--exclude` list, spec §4.9); `Cargo.lock`.
- **Interfaces produced:**
  - `lib.rs`: `pub struct EntityMarker;`, `pub type Entity = GenId<EntityMarker>;`, `pub use world::{InvariantViolation, World};`. Crate docs say what the crate is, point to the spec, and cite the designs it follows (specs' `VecStorage` and hibitset joins; spec §11), with no code taken (specs-5).
  - `column.rs`: `DenseColumn<T>` with `new`, `insert`, `remove`, `get`, `get_mut`, `present`, `split_mut` (spec §4.3; `split_mut` `#[cfg(test)]` until step 2); `ValuesMut<'a, T>` with `get(&mut self, index) -> Option<&mut T>` only; `ErasedColumn` (§1).
  - `world.rs`: `ComponentId(u32)`; `World` with `new`, `spawn`, `despawn`, `is_alive`, `len`, `is_empty`, `insert`, `remove`, `get`, `get_mut`, `check_invariants` (spec §4.2, §4.4). `Default` calls `new`; `Debug` prints `live` and each column's type name and `present_count`. `InvariantViolation` as M3.3's.
- **Test first** (inline test modules):
  - `column.rs`: `insert_grows_with_none_and_sets_the_bit`; `remove_takes_the_value_and_clears_the_bit`; `absent_past_the_end_does_not_grow` (focus 2); `values_mut_writes_in_place`; `remove_index_drops_the_value` (a `Drop` type counting into an `Rc<Cell<u32>>`); `zero_sized_slots_are_one_byte` (`size_of::<Option<Hidden>>() == 1`, documenting the spec's claim); `d1_violation_is_reported` (one test per direction: a set bit over `None`, and `Some` with a clear bit; the message contains `"D1"`).
  - `world.rs`: `components_are_independent_per_type` (insert and remove of one type leave every other column's slots and bits unchanged; R5); `replacing_a_value_returns_the_old_one` (focus 3); `despawn_clears_every_column` (an entity with values in three columns; each `Drop` runs; no bit left; R5); `stale_id_does_not_reach_the_reused_slot` (focus 1); `len_counts_live_entities` (focus 10); `insert_on_a_despawned_entity_panics` (`#[should_panic(expected = "World::insert called on a despawned entity")]`); `a_failed_spawn_leaves_len_unchanged` (`GenIdAllocator::with_fixed_capacity(1)` assigned to the private field in test code, the first `spawn` after that fails, `catch_unwind`, the message starts with `"World::spawn:"`, `len` and `alive` unchanged; ecs-4); `d2_violation_is_reported`, `w1_violation_is_reported`, `w2_violation_is_reported` (each corrupts one private field); `invariant_violation_displays_its_message`; `world_debug_shows_the_live_count_and_columns`.
  - `tests.rs`: `world_matches_the_model` (proptest). Component types `C0(u32)` to `C3(u32)`; up to 64 operations of `Spawn`, `Despawn(k)`, `Insert(k, type, value)`, `Remove(k, type)`, with `k` choosing among entities spawned so far, live or dead. The model is a `BTreeMap<u32, (Entity, [Option<u32>; 4])>` keyed by entity index for the live entities. After every operation: `check_invariants` is `Ok`; `get` of every type for every entity ever spawned agrees with the model (dead ones `None`); `len` equals the model's size. `Insert` on a dead entity is skipped by the generator, since it panics by design.
  - Fails first: `cargo xtask test` does not compile (the package does not exist).
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes; `cargo xtask deny` passes; `grep -n spike_ecs_dense xtask/src/coverage.rs` finds both entries.
- **Commit:** `feat(spikes): add the dense columns and World (M3.19)`

### Step 2: queries (R2 in part, R6)

- [x] **Who:** builder
- **Files:** `src/query.rs` (new), `src/column.rs`, `src/world.rs`, `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:**
  - `QueryParam`, `ReadOnly`, `Filter` (sealed, one seal each, `Item` on the seal, ecs-3), `With<T>`, `Without<T>` (`PhantomData<fn() -> T>`, `Debug`), `Query<'w, Q, F = ()>`, `QueryMut<'w, Q, F = ()>`, `QueryIter<'q, 'w, Q, F>` (spec §4.5; `#[must_use]`, `Debug`; `QueryIter` `FusedIterator`, `size_hint` as the spec); `IntoIterator for &'q Query<'w, Q, F>`; `World::query`, `World::query_mut`; `#[cfg(test)] joined()` on both query types (§1).
  - Module docs cite the tuple-query design of bevy_ecs and hecs and specs' joins, with no code taken (specs-5).
  - `pub use query::{Filter, Query, QueryIter, QueryMut, QueryParam, ReadOnly, With, Without};`.
- **Test first:**
  - `query.rs`: `required_types_select_entities`; `option_yields_some_exactly_when_present`; `option_past_a_short_column_is_none` (focus 2); `with_and_without_filters_chain`; `with_alone_drives_a_query`; `with_replaces_the_alive_fallback` (focus 6, through `joined()`); `filters_on_unregistered_types` (focus 7); `entity_parameter_yields_the_entity`; `nested_tuple_works` (`(Entity, (&mut A, &B, &C, Option<&D>))` through `query_mut`, writing through `&mut A`); `never_inserted_required_type_matches_nothing`; `mutable_query_writes_are_visible`; `iter_and_for_each_agree` (also `for _ in &query`); `size_hint_bounds_the_count`; `repeated_type_panics` (once each for `Query::for_each`, `Query::iter`, `QueryMut::for_each`, asserting the method prefix); `a_type_in_a_parameter_and_a_filter_panics` (focus 5); `query_without_a_required_type_visits_every_live_entity`, once each for `query::<Entity>()`, `query::<Option<&A>>()` and `query::<Entity>().without::<A>()`, including entities with no components; `zero_sized_component_filters` (focus 4); `queries_yield_ascending_indices` (focus 8).
  - `tests.rs`: `world_matches_the_model` also compares, after every operation, the entity sequences of `query::<(Entity, &C0, &C1)>()`, `query::<(Entity, &C0)>().without::<C2>()`, `query::<(Entity, Option<&C3>)>().with::<C1>()`, `query::<Entity>()` and `query::<Entity>().without::<C0>()` (through both `iter` and `for_each`) with the model's, in the model's index order, and the `Option<&C3>` values; and one `query_mut::<(&mut C0, Option<&mut C1>)>()` that adds 1 to each, mirrored in the model.
  - Fails first: does not compile (`query` and `query_mut` are not defined).
- **Change:** as above. Then count the heap allocations of one query run as built, from a scratchpad binary outside the repository that depends on `spike_ecs_dense` by path and installs a counting `#[global_allocator]` there (never in the crate; ecs-2), for `Query::for_each`, `Query::iter` and `QueryMut::for_each` with one, three and zero required types and with a filter, and write the counts in `log.md` with the commit they ran on (specs-2, specs-4).
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes; `log.md` has the allocation counts; `git status` shows no file outside `spikes/ecs_storage/dense/` and `log.md` changed.
- **Commit:** `feat(spikes): add typed tuple queries over Join to the dense World (M3.19)`

### Step 3: `Commands` (R7)

- [ ] **Who:** builder
- **Files:** `src/commands.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:** `pub struct Commands` and `World::apply(&mut self, &mut Commands)` per spec §4.6 (`impl World { apply }` in `commands.rs`, as M3.3 has it). `Deferred` closures check `is_alive` first; `apply` drains so capacity is kept; `#[cfg(test)] fn capacity`. `Default` and `Debug` (the number of commands). `pub use commands::Commands;`.
- **Test first:** `commands_run_in_recorded_order`; `command_on_a_dead_entity_is_skipped` (no panic, `check_invariants` `Ok`, `len` unchanged; focus 10); `apply_empties_the_buffer_and_keeps_its_capacity`; `commands_change_presence` (a recorded `insert` sets the bit, a recorded `remove` clears it). In `tests.rs`, `world_matches_the_model` gains `DeferInsert(k, type, value)`, `DeferRemove(k, type)`, `DeferDespawn(k)` and `Apply`, with one `Commands` buffer and a model of the recorded commands that applies them in order and skips dead entities (specs-3). Fails first: does not compile.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add a command buffer to the dense World (M3.19)`

### Step 4: `simple_iter`, `frag_iter` and `heavy_compute` (R8 in part)

- [ ] **Who:** builder
- **Files:** `src/simple_iter.rs`, `src/frag_iter.rs`, `src/heavy_compute.rs` (new), `src/lib.rs`, `src/tests.rs`, `Cargo.toml` (`spike_ecs_harness`, dev `spike_ecs_vec`), `Cargo.lock`.
- **Interfaces produced:** `simple_iter::DenseSimpleIter`, `frag_iter::DenseFragIter`, `heavy_compute::DenseHeavyCompute` (spec §4.7). Each step query is one private function used by `step` and by a `#[cfg(test)] pub(crate) fn step_joined(&mut self) -> (Vec<Joined>, Vec<Joined>)` hook (ecs-1). `frag_iter` inserts each letter through a local macro over the 26 `Letter` variants, as M3.3's does; its checksum runs one `query::<(&L, &Data)>()` per letter.
- **Test first** (`tests.rs`): `simple_iter_dense_matches_the_reference`, `frag_iter_dense_matches_the_reference`, `heavy_compute_dense_matches_the_reference`; `<workload>_dense_joins` for each (the step query joins the sets of spec §4.7's table, in parameter order, none excluded); `zero_count_is_valid` for the three (focus 9). Fails first: does not compile.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the iteration workloads to the dense backend (M3.19)`

### Step 5: `add_remove` and `entity_churn` (R8 in part, R9 in part)

- [ ] **Who:** builder
- **Files:** `src/add_remove.rs`, `src/entity_churn.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:** `add_remove::DenseAddRemove` (reused `Vec<(Entity, f32)>` and `Vec<Entity>`; read query `(Entity, &A)`, removal query `query::<Entity>().with::<B>()`, each its own private function with a test hook) and `entity_churn::DenseEntityChurn` (a `Commands`, the `Prng`, `next_id` and the target; respawns with `spawn` and one `insert` per component after `apply` until `world.len()` is the target).
- **Test first** (`tests.rs`): `add_remove_dense_matches_the_reference`, `entity_churn_dense_matches_the_reference`; `add_remove_dense_joins` (read query: A / none; removal query: B / none, not `Alive`); `entity_churn_dense_joins`; `add_remove_keeps_invariants_every_tick` (also asserts no entity has `B` after a tick, so a skipped removal fails it; M3.4's red check 1) and `entity_churn_keeps_invariants_every_tick` (1,000 entities, 120 ticks); `zero_count_is_valid` for the two. Fails first: does not compile.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes. Then the red checks, each a temporary edit, reverted, files byte-identical afterwards (`cmp` against a copy), recorded in `log.md` with the failing test's first lines: (1) skip the `remove::<B>` pass: the `add_remove` conformance test and its invariant test fail; (2) skip `commands.despawn`: the `entity_churn` conformance test fails; (3) respawn one too few: the `entity_churn` conformance test fails; (4) in `World::despawn`, skip clearing the columns: `entity_churn_keeps_invariants_every_tick` fails naming W1.
- **Commit:** `feat(spikes): add the structural workloads to the dense backend (M3.19)`

### Step 6: `sprites` and `agents` (R8, R9)

- [ ] **Who:** builder
- **Files:** `src/sprites.rs`, `src/agents.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:** `sprites::DenseSprites` (owns the draw list, reserved to `count` in `setup`; inserts `Hidden` on hidden entities; move and draw queries each a private function with a hook) and `agents::DenseAgents` (`pub struct AgentId(pub u64)`, `Velocity(pub Vec2)`, `Ttl(pub u32)`, as M3.3's; seeker change recorded before the `despawn`, as M3.3's step does).
- **Test first** (`tests.rs`): `sprites_dense_matches_the_reference`, `agents_dense_matches_the_reference` (1,000 agents, 1,300 ticks); `sprites_dense_joins` (move: Transform2d, Velocity2d / none; draw: Transform2d, Sprite / Hidden); `agents_dense_joins`; `agents_keeps_invariants_every_tick` (1,000 agents, 1,300 ticks); `zero_count_is_valid` for the two. Fails first: does not compile.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the sprite and agent workloads to the dense backend (M3.19)`

### Step 7: bench registrations (R10)

- [ ] **Who:** builder
- **Files:** `spikes/ecs_storage/bench_compare/Cargo.toml`, `benches/compare.rs`, `spikes/ecs_storage/bench_cache/Cargo.toml`, `benches/cache.rs`, `Cargo.lock`.
- **Test first:** n/a for registrations. The check before the change: `cargo bench -p spike_ecs_bench_compare -- --test 2>&1 | grep -c /dense/` prints 0.
- **Change:** dev-dependency `spike_ecs_dense = { path = "../dense" }` in both bench crates. `compare.rs`: one `bench::<…>(c);` per workload (7). `cache.rs`: one `cache_bench!(<workload>_dense, <Type>, [<sizes>]);` per workload with each workload's `Spec::SIZES` (`simple_iter`, `add_remove`, `entity_churn`: `[1_000, 10_000, 100_000]`; `frag_iter`: `[20]`; `heavy_compute`: `[1_000]`; `sprites`: `[100_000]`; `agents`: `[10_000, 100_000]`), each named in its workload's group.
- **Verify:** the same command prints 14 (3 + 1 + 1 + 3 + 3 + 1 + 2); `cargo build --benches -p spike_ecs_bench_cache` builds; the main session checks each new `cache_bench!` line's sizes against the workload's `vec_aos` line and its group (the cache bench's own size assertion does not run without Valgrind, #74); `cargo xtask ci --quick` passes; `cargo xtask deny` passes.
- **Commit:** `feat(spikes): benchmark the dense backend (M3.19)`

### Step 8: documents (R12)

- [ ] **Who:** main session
- **Files:** `spikes/ecs_storage/README.md`, `CHANGELOG.md`.
- **Test first:** n/a for prose. The checks before the change: `grep -n "dense/" spikes/ecs_storage/README.md` and `grep -n "spike_ecs_dense" CHANGELOG.md` find nothing.
- **Change:** spec §4.9: the package row in the spike README; a CHANGELOG entry. Predictions keep their "expected, unmeasured" wording (docs-3). M3.8's roadmap row gains the costs found while building (step 2's allocation count and anything the reviews find) in the close-out commit, with the row's `Done`.
- **Verify:** both greps find their text; `cargo xtask ci` passes in full, with nothing else running `cargo` in the tree.
- **Commit:** `docs: record the dense arrays prototype (M3.19)`

## 3. Final verification

- `cargo xtask ci` passes locally on the final code commit. `cargo xtask bench-regress` is not needed: nothing under `libs/` or `bench/regress/` changes.
- One informal `cargo xtask spike-bench ecs` run under the benchmark run protocol (`/bench-run`: AC power, `caffeinate`, no builds during the run, the power log checked afterwards); its output goes in the pull request, labelled informal. No cache-bench run unless a Valgrind host is available; the pull request says which.
- CI is green on Ubuntu, macOS and Windows with no workflow change (R11).
- The reviews of stage 5 are recorded in `log.md` and the pull request, and the spec's §9 list is ticked with evidence, each naming the commit it ran on (specs-4).

## 4. Lessons

From `lessons-researcher` (2026-11-20).

- **ecs-1** followed: every step and draw query is one private function shared by `step` and its `joined` hook (steps 4 to 6), and `joined()` uses the run's own resolution functions (§1).
- **ecs-2** followed: step 2 counts a query run's allocations from a scratchpad binary, never by a counting allocator in the crate; close-out carries the count into M3.8's row.
- **ecs-3** followed: `type Item<'w>` lives on the seal trait (§1, step 2).
- **ecs-4** followed: `a_failed_spawn_leaves_len_unchanged` fails on the first call and asserts the message (step 1).
- **ecs-5** followed: queries get `ValuesMut` (only `get`), never `&mut [Option<T>]` or `&mut dyn ErasedColumn`'s full surface for values (steps 1, 2).
- **docs-2** followed: when a review fix changes a count (tests, bench ids), the plan is grepped for the old number.
- **docs-3** followed: spec §4.7's predictions keep "expected, unmeasured" in the CHANGELOG, README and M3.8's row (step 8, close-out).
- **specs-1** followed: step 7's and step 8's before-checks name the command and files they search.
- **specs-2** followed: the `log.md` evidence a step asks for (step 2's allocation count, step 5's red checks) is written into that step's own Change or Verify.
- **specs-3** followed: every panic of spec §4.2 and §4.5 and every invariant has its own named test (steps 1 to 3), and the model test's operations are copied from spec §8 into steps 1 to 3, `Apply` included.
- **specs-4** followed: §3's evidence names the commit it ran on.
- **specs-5** (not returned for these paths; followed anyway): steps 1 and 2 cite the designs in the shipped module docs.

## 5. Deviations

- **Step 2, `ComponentId`:** stays `pub(crate)`; no sealed signature names it, and `pub` trips `unreachable_pub` (log.md, 2026-11-20).
- **Step 2, `for` over `&Query`:** panics with its own `Query::into_iter:` prefix, a fourth beside §1's three.
- **Step 2, `joined()`:** names each joined set by its address against the world's own sets, which tests the run's actual sets; `World::component_id` and `present_sets` are `#[cfg(test)]` helpers (log.md, 2026-11-20).
