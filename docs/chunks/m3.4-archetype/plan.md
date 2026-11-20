# M3.4 plan: Archetype prototype

- **Status:** Accepted (2026-11-20)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. Progress and surprises go in [log.md](log.md).

## 1. Approach

Nine steps, one commit each, built bottom up as M3.3 was: tables, archetypes and a `World` with single-component `insert`/`remove` (together, because an unused `pub(crate)` item fails `clippy -D warnings`); then bundles with the model test; queries; `Commands`; three workload commits; the bench registrations; the documents. M3.3's crate (`spikes/ecs_storage/sparse_set/`) is this repository's own code: the builder MAY adapt its `query.rs`, `commands.rs` and workload modules, and SHOULD read its plan §5 Deviations first, which record the compiler and lint traps it met.

Choices the spec leaves to implementation:

- **One type per workload.** One backend, so no const generic: `pub struct ArchetypeX { world: World, … }` with private fields, `BACKEND = "archetype"`.
- **`Entity` and `EntityMarker`** live in `lib.rs`, as in M3.3.
- **Module homes.** `ComponentId(u32)` and `ArchetypeId(u32)` are in `archetype.rs`; `Column<T>`, `ErasedColumn` and `Table` in `table.rs`. Items the seal traits mention are `pub` inside private modules (unnameable outside the crate), because `pub(crate)` trips `private_interfaces` (M3.3 plan §5).
- **`ErasedColumn` method names** must not collide with `Any::type_id`: use `value_type_id` and `value_type_name`.
- **Moves know the component lists.** `Table` holds no `ComponentId`s, so the move functions take both archetypes' sorted lists and merge-walk them to pair columns, without allocating:
  - `Table::move_row_adding<T: 'static>(src: &mut Table, src_ids: &[ComponentId], row: usize, dst: &mut Table, dst_ids: &[ComponentId], value: T) -> Option<Entity>`;
  - `Table::move_row_removing<T: 'static>(src: &mut Table, src_ids: &[ComponentId], row: usize, dst: &mut Table, dst_ids: &[ComponentId]) -> (T, Option<Entity>)`.
  
  The returned entity is the one swapped into `row` of `src`. A caller borrows the two archetypes with `get_disjoint_mut([src, dst])` and passes their fields.
- **`spawn()` into the empty archetype** uses `Table::push_empty(entity)`, which panics (an internal `unreachable!`-style message, not a documented panic) if the table has columns, so T1 cannot be broken by a misuse.
- **Typed access** by column index: `Table::get<T>(row, col) -> &T`, `get_mut<T>`, `replace<T>(row, col, value) -> T`, each downcasting the column once.
- **The `by_components` counter** for R5 is a `#[cfg(test)] lookups: usize` field on `World`, incremented wherever `by_components` is read; never a static.
- **Bundles.** `seal::Bundle` gives `types() -> Vec<(TypeId, &'static str)>` (used only when a `BundleInfo` is built), `register(world) -> Vec<ComponentId>` (registers each element type with its `new_column`) and `write(self, table, entity, columns: &[usize])` (one typed push per element, then the entity). A warm `spawn_with` looks up the `BundleInfo` and calls `write` without allocating; the `columns` slice is borrowed from the cached `BundleInfo`, which needs the `bundles` map and the archetype table borrowed as separate fields.
- **Query machinery** follows M3.3's shape: `seal::Param` carries `type Item<'w>` (lesson ecs-3), type lists and per-archetype bind functions; `seal::ReadOnly` gives items with the world's lifetime for `QueryIter`; `seal::Filter` gives the type list. Per archetype, each parameter binds to a slice once (spec §4.6 step 4); the row loop fetches by index from slices re-sliced to `..len`. The `QueryMut` split is a reusable `Vec<Option<&mut Box<dyn ErasedColumn>>>` per run, refilled per matched archetype without reallocating.
- **Panic messages** name the operation first: "World::insert called on a despawned entity …", "World::spawn: …", "World::spawn_with: …", "`<T>` appears twice in the bundle", "`<T>` appears twice in the query". Types by `core::any::type_name::<T>()`, entities by `Debug`.
- **Invariant messages** contain the invariant's name (`"T1"`, `"T2"`, `"A1"` to `"A4"`, `"B1"`), so a test can check which one fired.

**Global constraints**

- Every commit passes `cargo xtask ci --quick`. Steps 1 and 8 also pass `cargo xtask deny`, with `deny.toml` unchanged (R12).
- The crate root has `#![forbid(unsafe_code)]`; the package has `[lints] workspace = true`. Every public item is documented (R13). Doctests are not required ([spikes/README.md](../../../spikes/README.md)).
- Package `spike_ecs_archetype`: dependencies `engine = { workspace = true }`, `spike_ecs_harness = { path = "../harness" }` from step 5, `agents_2d = { path = "../../../examples/agents_2d" }` from step 7 (an unused dependency fails clippy); dev-dependencies `proptest = { workspace = true }` from step 2 and `spike_ecs_vec = { path = "../vec" }` from step 5.
- Items a later step uses from library code and nothing uses yet are `#[cfg(test)]` until then, never `#[allow(dead_code)]` (M3.3 plan §5).
- Conformance: `Params { count: 1_000, seed: 42 }` (`count: 20` for `frag_iter`) and 120 ticks; `agents` 1,000 agents and 1,300 ticks (R9).
- Run `cargo fmt --all` before `ci --quick`. Commits are conventional, end with ` (M3.4)` and the session's attribution line, and stage files by explicit path.

**Review focus** (inputs the spec implies that no single requirement's test pins; each has a test in the step that owns the code)

1. **A stale ID whose index was reused.** After `despawn(a)` and a `spawn()` that returns `b` at the same index: `get::<T>(a)`, `remove::<T>(a)` and `despawn(a)` return `None`, `None` and `false` and leave `b` alone; `insert(a, …)` panics. Step 1: `stale_id_does_not_reach_the_reused_slot`.
2. **Moving or removing the last row**, where the swap-remove swaps the row with itself: no entity is reported as swapped, and A1 holds. Step 1: `moving_the_last_row_reports_no_swap`.
3. **Replacing a present value.** `insert` of a type the entity has returns the old value; the entity's location and `archetype_count` do not change. Step 1: `replacing_a_value_does_not_move_the_entity`.
4. **Back to where it came from.** `insert::<B>` then `remove::<B>` returns the entity to its first `ArchetypeId`, not a new archetype with the same list; removing an entity's last component puts it in archetype 0; `remove` of an absent type creates nothing. Step 1: `insert_then_remove_returns_to_the_first_archetype`, `removing_the_last_component_goes_to_the_empty_archetype`, `removing_an_absent_type_creates_no_archetype`.
5. **Bundle element order is not archetype order.** `spawn_with((C, A, B))` stores each value in its own type's column, though the archetype's columns are sorted by `ComponentId`. Step 2: `bundle_values_land_in_their_own_columns`; the model test's bundle shapes include out-of-order ones.
6. **A zero-sized tag in a bundle** (`Hidden`): it gets a column and matches `With` and `Without`. Step 2: `zero_sized_bundle_element_is_a_component`.
7. **A type both queried and excluded.** `query::<&A>().without::<A>()` panics as a repeated type. Step 3: `a_type_in_a_parameter_and_a_filter_panics`.
8. **No required type and an empty archetype.** `query::<Entity>()` counts entities in archetype 0; `query::<Entity>().without::<A>()` skips archetypes with `A`. Step 3: `query_without_a_required_type_visits_every_live_entity`.
9. **`count: 0`.** Every workload sets up empty, has checksum 0 and steps without panicking. Steps 5 to 7: `zero_count_is_valid` for each.
10. **`len` across failed operations.** `despawn` of a dead entity returns `false` and leaves `len` unchanged; a failed `spawn` or `spawn_with` leaves it unchanged; a skipped command leaves it unchanged. Steps 1, 2 and 4.

## 2. Steps

### Step 1: tables, archetypes and the `World` (R1, R2 in part, R3, R4 for T1, T2, A1 to A4, R5)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/archetype/Cargo.toml`, `src/lib.rs`, `src/table.rs`, `src/archetype.rs`, `src/world.rs` (new); `xtask/src/coverage.rs` (`spike_ecs_archetype` in `SPIKE_PACKAGES` and in `test_args_match_the_spec`'s expected `--exclude` list, spec §4.10); `Cargo.lock`.
- **Interfaces produced:**
  - `lib.rs`: `pub struct EntityMarker;`, `pub type Entity = GenId<EntityMarker>;`, `pub use world::{InvariantViolation, World};`. Crate docs say what the crate is, point to the spec, and cite the designs it follows (bevy_ecs's archetype tables and graph, hecs's tables; spec §11), with no code taken (specs-5).
  - `table.rs`: `Column<T>`, `ErasedColumn` (spec §4.3, with `value_type_id`, `value_type_name`), `Table` with `new(columns: Box<[Box<dyn ErasedColumn>]>)`, `len`, `entities`, `column`, `columns_mut`, `push_empty`, `swap_remove`, `move_row_adding`, `move_row_removing`, `get`, `get_mut`, `replace` (§1).
  - `archetype.rs`: `ComponentId(u32)`, `ArchetypeId(u32)` with `EMPTY = ArchetypeId(0)`; `Archetype { components, table, add, remove }` with `column_of(ComponentId) -> Option<usize>` (binary search) and edge get/set.
  - `world.rs`: `World` with `new`, `spawn`, `despawn`, `is_alive`, `len`, `is_empty`, `archetype_count`, `insert`, `remove`, `get`, `get_mut`, `check_invariants` (spec §4.2, §4.4). `Location { archetype, row }` with `NONE`; `ComponentInfo { type_id, name, new_column }`; `pub(crate) fn component_id<T>()` and `register<T: 'static>()`; `#[cfg(test)] lookups`. `Default` calls `new`; `Debug` prints `live` and each archetype's type names and row count (`finish_non_exhaustive` if a field is skipped).
  - `InvariantViolation` as M3.3's (spec §4.2).
- **Test first** (inline test modules):
  - `table.rs`: `swap_remove_reports_the_moved_entity`; `move_row_adding_pairs_columns_by_id`; `move_row_removing_returns_the_value`; `moving_the_last_row_reports_no_swap` (focus 2); `zero_sized_columns_do_not_allocate`; `t1_violation_is_reported`, `t2_violation_is_reported` (each corrupts one private field; the message contains `"T1"` or `"T2"`).
  - `world.rs`: `components_are_independent_per_type`; `insert_moves_to_the_archetype_with_the_type_added`; `insert_then_remove_returns_to_the_first_archetype`, `removing_the_last_component_goes_to_the_empty_archetype`, `removing_an_absent_type_creates_no_archetype` (focus 4); `replacing_a_value_does_not_move_the_entity` (focus 3); `a_warm_transition_uses_the_edge` (the second identical insert and remove leave `lookups` unchanged and create no archetype; R5); `despawn_drops_the_row_and_fixes_the_swapped_location`; `stale_id_does_not_reach_the_reused_slot` (focus 1); `len_counts_live_entities` (focus 10); `insert_on_a_despawned_entity_panics` (`#[should_panic(expected = "World::insert called on a despawned entity")]`); `a_failed_spawn_leaves_len_unchanged` (`GenIdAllocator::with_fixed_capacity(1)` assigned to the private field in test code, `catch_unwind`, message starts with `"World::spawn:"`); `a1_violation_is_reported` to `a4_violation_is_reported`, one each; `invariant_violation_displays_its_message`.
  - Fails first: `cargo xtask test` does not compile (the package does not exist).
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes; `cargo xtask deny` passes; `grep -n spike_ecs_archetype xtask/src/coverage.rs` finds both entries.
- **Commit:** `feat(spikes): add the archetype tables and World (M3.4)`

### Step 2: bundles and the model test (R2 in part, R4 for B1, R5, R6)

- [x] **Who:** builder
- **Files:** `src/bundle.rs` (new), `src/world.rs`, `src/lib.rs`, `src/tests.rs` (new), `Cargo.toml` (`proptest`), `Cargo.lock`.
- **Interfaces produced:** `pub trait Bundle: seal::Bundle + 'static` for flat tuples of 1 to 8 `'static` types (one macro); `BundleInfo { archetype, columns }`; `World::bundles: BTreeMap<TypeId, BundleInfo>`; `World::spawn_with<B: Bundle>(&mut self, bundle: B) -> Entity` (`#[track_caller]`, `# Panics` for a repeated type and a full allocator); `check_invariants` checks B1. `pub use bundle::Bundle;`.
- **Test first:**
  - `world.rs` or `bundle.rs`: `spawn_with_creates_exactly_one_archetype` (`(A, B, C)` on a fresh world: `archetype_count` is 2; R6); `a_warm_spawn_with_does_not_look_up_by_components` (`lookups` unchanged on the second spawn of the same bundle type); `bundle_values_land_in_their_own_columns` (focus 5); `zero_sized_bundle_element_is_a_component` (focus 6; `get::<Hidden>` is `Some`); `bundles_of_one_and_eight_types_work`; `repeated_type_in_a_bundle_panics` (expected message names the type and "appears twice in the bundle"); `a_failed_spawn_with_leaves_len_unchanged` (message starts with `"World::spawn_with:"`; focus 10); `b1_violation_is_reported`.
  - `tests.rs`: `world_matches_the_model` (proptest). Component types `C0(u32)` to `C3(u32)`; up to 64 operations of `Spawn`, `SpawnWith(shape, values)` over the shapes `(C0,)`, `(C0, C1)`, `(C2, C0)`, `(C3, C1, C0)`, `Despawn(k)`, `Insert(k, type, value)`, `Remove(k, type)`, with `k` choosing among entities spawned so far, live or dead. The model is a `BTreeMap<u64, [Option<u32>; 4]>` keyed by `Entity::to_bits`, plus the live set. After every operation: `check_invariants` is `Ok`; `get` of every type for every entity ever spawned agrees with the model; `len` equals the live count. `Insert` on a dead entity is skipped by the generator, since it panics by design.
  - Fails first: does not compile (`spawn_with` and `Bundle` are not defined).
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): spawn bundles straight into their archetype (M3.4)`

### Step 3: queries (R2 in part, R7)

- [ ] **Who:** builder
- **Files:** `src/query.rs` (new), `src/world.rs`, `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:**
  - `QueryParam`, `ReadOnly`, `Filter` (sealed, one seal each, `Item` on `seal::Param`), `With<T>`, `Without<T>` (`PhantomData<fn() -> T>`, `Debug`), `Query<'w, Q, F = ()>`, `QueryMut<'w, Q, F = ()>`, `QueryIter<'w, Q, F>` (spec §4.6; `#[must_use]`, `Debug`; `QueryIter` `FusedIterator` with exact `size_hint`); `IntoIterator` for `Query` and `&Query`; `World::query` and `World::query_mut`. `#[cfg(test)] pub(crate) fn matched_archetypes(&self) -> Vec<ArchetypeId>` on both query types, built by the same matching function the run uses.
  - Module docs cite the tuple-query design of bevy_ecs and hecs, with no code taken (specs-5).
  - `pub use query::{Filter, Query, QueryIter, QueryMut, QueryParam, ReadOnly, With, Without};`.
- **Test first:**
  - `query.rs`: `required_types_select_entities`; `option_yields_some_exactly_when_present` (one archetype with the type, one without); `with_and_without_filters_chain`; `with_alone_drives_a_query`; `entity_parameter_yields_the_entity`; `nested_tuple_works` (`(Entity, (&mut A, &B, &C, Option<&D>))` through `query_mut`, writing through `&mut A`); `never_inserted_required_type_matches_nothing`; `mutable_query_writes_are_visible`; `iter_for_each_and_into_iter_agree` (also `for _ in &query`); `size_hint_is_exact_across_archetypes`; `repeated_type_panics`; `a_type_in_a_parameter_and_a_filter_panics` (focus 7); `query_without_a_required_type_visits_every_live_entity`, once each for `query::<Entity>()`, `query::<Option<&A>>()` and `query::<Entity>().without::<A>()` (focus 8); `matched_archetypes_includes_empty_matches` (an archetype emptied by `remove` still matches); `archetypes_are_visited_in_id_order`.
  - `tests.rs`: `world_matches_the_model` also compares, after every operation, the entity sets of `query::<(Entity, &C0, &C1)>()`, `query::<(Entity, &C0)>().without::<C2>()`, `query::<(Entity, Option<&C3>)>().with::<C1>()`, `query::<Entity>()` and `query::<Entity>().without::<C0>()` with the model's, as sorted `Vec<u64>` of `to_bits`.
  - Fails first: does not compile.
- **Change:** as above. Count the heap allocations of one query run as built and write the count in `log.md` (lesson ecs-2); §6 of the spec says none per matched archetype.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add typed tuple queries to the archetype World (M3.4)`

### Step 4: `Commands` (R8)

- [ ] **Who:** builder
- **Files:** `src/commands.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:** `pub struct Commands` and `World::apply(&mut self, &mut Commands)` per spec §4.7 (`impl World { apply }` in `commands.rs`, as M3.3 has it). `Deferred` closures check `is_alive` first; `apply` drains so capacity is kept; `#[cfg(test)] fn capacity`. `Default` and `Debug` (the number of commands). `pub use commands::Commands;`.
- **Test first:** `commands_run_in_recorded_order`; `command_on_a_dead_entity_is_skipped` (no panic, `check_invariants` `Ok`, `len` unchanged; focus 10); `apply_empties_the_buffer_and_keeps_its_capacity`; `commands_move_entities_between_archetypes` (a recorded `insert` lands the entity in the archetype with the type added). In `tests.rs`, `world_matches_the_model` gains `DeferInsert(k, type, value)`, `DeferRemove(k, type)`, `DeferDespawn(k)` and `Apply`, with one `Commands` buffer and a model of the recorded commands that applies them in order and skips dead entities (lesson specs-3). Fails first: does not compile.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add a command buffer to the archetype World (M3.4)`

### Step 5: `simple_iter`, `frag_iter` and `heavy_compute` (R9 in part)

- [ ] **Who:** builder
- **Files:** `src/simple_iter.rs`, `src/frag_iter.rs`, `src/heavy_compute.rs` (new), `src/lib.rs`, `src/tests.rs`, `Cargo.toml` (`spike_ecs_harness`, dev `spike_ecs_vec`), `Cargo.lock`.
- **Interfaces produced:** `simple_iter::ArchetypeSimpleIter`, `frag_iter::ArchetypeFragIter`, `heavy_compute::ArchetypeHeavyCompute` (spec §4.8). Each step query is one private function used by `step` and by a `#[cfg(test)] pub(crate) fn step_archetypes(&mut self) -> Vec<ArchetypeId>` hook (lesson ecs-1). `frag_iter` spawns `(L, Data)` through a local macro over the 26 `Letter` variants; its checksum runs one `query::<(&L, &Data)>()` per letter.
- **Test first** (`tests.rs`): `simple_iter_archetype_matches_the_reference`, `frag_iter_archetype_matches_the_reference`, `heavy_compute_archetype_matches_the_reference`; `<workload>_archetype_layout` for each (after `setup`, `archetype_count` is 2, 27 and 2; the step query matches 1, 26 and 1 archetypes); `zero_count_is_valid` for the three (focus 9). Fails first: does not compile.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the iteration workloads to the archetype backend (M3.4)`

### Step 6: `add_remove` and `entity_churn` (R9 in part, R10 in part)

- [ ] **Who:** builder
- **Files:** `src/add_remove.rs`, `src/entity_churn.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:** `add_remove::ArchetypeAddRemove` (reused `Vec<(Entity, f32)>` and `Vec<Entity>`; read query `(Entity, &A)`, removal query `query::<Entity>().with::<B>()`, each its own private function with a test hook) and `entity_churn::ArchetypeEntityChurn` (a `Commands`, the `Prng`, `next_id` and the target; respawns with `spawn_with` after `apply` until `world.len()` is the target).
- **Test first** (`tests.rs`): `add_remove_archetype_matches_the_reference`, `entity_churn_archetype_matches_the_reference`; `add_remove_archetype_layout` (after `setup`, 2 archetypes; after one `step`, 3, the read query matches `(A)` and `(A, B)`, the removal query matches `(A, B)` only); `entity_churn_archetype_layout` (2 archetypes after `setup` and after 120 steps; the step query matches 1); `add_remove_keeps_invariants_every_tick` and `entity_churn_keeps_invariants_every_tick` (1,000 entities, 120 ticks); `zero_count_is_valid` for the two. Fails first: does not compile.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes. Then the red checks, each a temporary edit, reverted, files byte-identical afterwards, recorded in `log.md` with the failing test's first lines: (1) skip the `remove::<B>` pass: both the `add_remove` conformance test and its invariant test fail; (2) skip `commands.despawn`: the `entity_churn` conformance test fails; (3) respawn one too few: the `entity_churn` conformance test fails.
- **Commit:** `feat(spikes): add the structural workloads to the archetype backend (M3.4)`

### Step 7: `sprites` and `agents` (R9, R10)

- [ ] **Who:** builder
- **Files:** `src/sprites.rs`, `src/agents.rs` (new), `src/lib.rs`, `src/tests.rs`, `Cargo.toml` (`agents_2d`), `Cargo.lock`.
- **Interfaces produced:** `sprites::ArchetypeSprites` (owns the draw list, reserved to `count` in `setup`; spawns one of two bundles by `Option<Hidden>`; move and draw queries each a private function with a hook) and `agents::ArchetypeAgents` (`pub struct AgentId(pub u64)`, `Velocity(pub Vec2)`, `Ttl(pub u32)`, as M3.3's; spawns the 4- or 5-element bundle by `Option<Seeker>`; seeker change recorded before the `despawn`, as M3.3's step does).
- **Test first** (`tests.rs`): `sprites_archetype_matches_the_reference`, `agents_archetype_matches_the_reference` (1,000 agents, 1,300 ticks); `sprites_archetype_layout` (3 archetypes after `setup`; the move query matches 2, the draw query 1); `agents_archetype_layout` (3 archetypes after `setup`; the step query matches 2); `agents_keeps_invariants_every_tick` (1,000 agents, 1,300 ticks); `zero_count_is_valid` for the two. Fails first: does not compile.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the sprite and agent workloads to the archetype backend (M3.4)`

### Step 8: bench registrations (R11)

- [ ] **Who:** builder
- **Files:** `spikes/ecs_storage/bench_compare/Cargo.toml`, `benches/compare.rs`, `spikes/ecs_storage/bench_cache/Cargo.toml`, `benches/cache.rs`, `Cargo.lock`.
- **Test first:** n/a for registrations. The check before the change: `cargo bench -p spike_ecs_bench_compare -- --test 2>&1 | grep -c /archetype/` prints 0.
- **Change:** dev-dependency `spike_ecs_archetype = { path = "../archetype" }` in both bench crates. `compare.rs`: one `bench::<…>(c);` per workload (7). `cache.rs`: one `cache_bench!(<workload>_archetype, <Type>, [<sizes>]);` per workload with each workload's `Spec::SIZES` (`simple_iter`, `add_remove`, `entity_churn`: `[1_000, 10_000, 100_000]`; `frag_iter`: `[20]`; `heavy_compute`: `[1_000]`; `sprites`: `[100_000]`; `agents`: `[10_000, 100_000]`), each named in its workload's group.
- **Verify:** the same command prints 13 (the Criterion ids, one per workload and size); `cargo build --benches -p spike_ecs_bench_cache` builds; the main session checks each new `cache_bench!` line's sizes against the workload's `vec_aos` line and its group (the cache bench's own size assertion does not run without Valgrind, #74); `cargo xtask ci --quick` passes; `cargo xtask deny` passes.
- **Commit:** `feat(spikes): benchmark the archetype backend (M3.4)`

### Step 9: documents (R13)

- [ ] **Who:** main session
- **Files:** `docs/roadmap.md`, `spikes/ecs_storage/README.md`, `CHANGELOG.md`.
- **Test first:** n/a for prose. The checks before the change: `grep -n "cached transition edges" docs/roadmap.md` and `grep -n "archetype/" spikes/ecs_storage/README.md` find nothing.
- **Change:** spec §4.10: M3.4's new title; the package row in the spike README; a CHANGELOG entry. Predictions keep their "expected, unmeasured" wording (docs-3).
- **Verify:** both greps find their text; `cargo xtask ci` passes in full, with nothing else running `cargo` in the tree.
- **Commit:** `docs: record the archetype prototype (M3.4)`

## 3. Final verification

- `cargo xtask ci` passes locally on the final code commit. `cargo xtask bench-regress` is not needed: nothing under `libs/` or `bench/regress/` changes.
- One informal `cargo xtask spike-bench ecs` run under the benchmark run protocol (`/bench-run`: AC power, `caffeinate`, no builds during the run, the power log checked afterwards); its output goes in the pull request, labelled informal. No cache-bench run unless a Valgrind host is available; the pull request says which.
- CI is green on Ubuntu, macOS and Windows with no workflow change (R12).
- The reviews of stage 5 are recorded in `log.md` and the pull request, and the spec's §9 list is ticked with evidence, each naming the commit it ran on (specs-4).

## 4. Lessons

From `lessons-researcher` (2026-11-20).

- **ecs-1** followed: every step and draw query is one private function shared by `step` and its `matched_archetypes` hook (steps 5 to 7).
- **ecs-2** followed: step 3 counts a query run's heap allocations as built and logs the count; close-out carries it into the pull request as a caveat for M3.8.
- **ecs-3** followed: `type Item<'w>` lives on `seal::Param` (§1, step 3).
- **docs-3** followed: spec §4.8's predictions keep "expected, unmeasured" in the CHANGELOG and README (step 9).
- **specs-1** followed: step 8's and step 9's before-checks name the command and files they search.
- **specs-2** followed: the only `log.md` evidence a step asks for (step 3's allocation count, step 6's red checks) is written into that step's own Change or Verify.
- **specs-3** followed: every panic of spec §4.2, §4.5 and §4.6 and every invariant has its own named test (steps 1 to 4), and the model test's operations are copied from spec §8 into steps 2 and 4.
- **specs-4** followed: §3's evidence names the commit it ran on.
- **specs-5** (not returned for these paths; followed anyway): steps 1 and 3 cite the bevy_ecs and hecs designs in the shipped module docs.

## 5. Deviations

- **Step 1, `ErasedColumn::new_empty`:** omitted. Spec §4.3 lists it, but `ComponentInfo::new_column` already creates a new archetype's columns, and an unused method fails clippy. `Table::columns_mut` waits for step 3, which uses it; `Table::column` is `#[cfg(test)]`.
- **Step 1, `Table::check(&[TypeId])`:** T1 and T2 are checked inside `table.rs`, so its tests can corrupt the private fields; `World::check_invariants` checks A3 first, then T1 and T2, A1 and A2, A4.
- **Step 1, extra tests:** `replace_and_get_mut_write_in_place`, `push_empty_on_a_table_with_columns_panics`, `world_debug_shows_the_live_count_and_archetypes`.
- **Step 2, `BundleInfo::types`:** a third field holds the element `TypeId`s in tuple order, because B1 cannot be checked from `archetype` and `columns` alone.
- **Step 2, row writes:** the seal's `write` takes a `RowWriter` (table.rs, private constructor) instead of the table, so `Table::push_bundle` is the only way to push a bundle row and it `assert!`s T1 afterwards. The builder's first version had separate `push_value`/`finish_row` methods, each breaking T1 alone; sent back under R3. `ComponentId` and `RowWriter` are `pub` in private modules for `private_interfaces`; `Table` stays `pub(crate)`.
