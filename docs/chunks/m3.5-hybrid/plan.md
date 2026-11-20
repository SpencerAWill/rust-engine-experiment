# M3.5 plan: Per-component hybrid prototype

- **Status:** Accepted (2026-11-20)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. Progress and surprises go in [log.md](log.md).

## 1. Approach

Nine steps, one commit each, built bottom up as M3.4 was: the component trait, tables, archetypes, sparse sets and a `World` with single-component `insert`/`remove` of both kinds (together, because an unused `pub(crate)` item fails `clippy -D warnings`); bundles; queries; `Commands`; three workload commits; the bench registrations; the documents. M3.4's crate (`spikes/ecs_storage/archetype/`) is the closest base and M3.3's `storage.rs` (`spikes/ecs_storage/sparse_set/`) the sparse set's; both are this repository's own code, and the builder MAY adapt them. The builder SHOULD read M3.4's and M3.19's plan §5 Deviations and logs first, which record the compiler and lint traps they met.

Choices the spec leaves to implementation:

- **One type per workload**, `pub struct HybridX { world: World, … }` with private fields, `BACKEND = "hybrid"`. Workload modules are `pub mod`.
- **`Entity` and `EntityMarker`** live in `lib.rs`, as in M3.4.
- **Module homes.** `Component`, `Storage`, `TableStorage`, `SparseStorage`, `StorageKind` and the impl macro in `component.rs`; `ComponentId(u32)` and `ArchetypeId(u32)` in `archetype.rs`; `Column<T>`, `ErasedColumn`, `Table`, `ColumnMut` in `table.rs`; `SparseSet<T>`, `SparseMut<'a, T>`, `ErasedSparseSet` in `sparse.rs`. Items the seal traits mention are `pub` inside private modules (unnameable outside the crate), because `pub(crate)` trips `private_interfaces`, and `ComponentId` stays `pub(crate)` if no sealed signature names it (M3.4 and M3.19 plan §5).
- **The storage seal** (`component::seal::Storage`, spec §4.2) is the stub the grill built (log.md, 2026-11-20): `const KIND`, the fetch GATs `Ref<'w, T>` and `Mut<'w, T>` (`&'w [T]` and `&'w mut [T]` for tables, `&'w SparseSet<T>` and `SparseMut<'w, T>` for sparse sets), per-row `get_ref`, `get_mut` and `probe`, and the per-kind `World` paths `insert`, `remove`, `get`, `get_mut` and `register`, so `World::insert::<T>` is one call to `<T::Storage as seal::Storage>::insert(self, entity, value)`.
- **The impl macro**, `pub(crate) use` from `component.rs`: `components!(TableStorage: Position, Velocity);` writes one `impl Component` per type. Every harness type gets its impl in one module, `src/harness_components.rs` (step 5), because two workloads share `Transform2d`; `B` and `Seeker` are `SparseStorage`, the rest `TableStorage` (spec §4.9).
- **`ErasedColumn` and `ErasedSparseSet` method names** must not collide with `Any::type_id`: `value_type_id`, `value_type_name`. `ErasedSparseSet` has `remove_entity`, `len`, `as_any`, `as_any_mut` and `check(&self, is_alive) -> Result<(), String>` for S1 to S3 (so its tests can corrupt the private fields inside `sparse.rs`).
- **World fields.** M3.4's (`allocator`, `len`, `locations`, `archetypes`, `by_components`, `ids`, `infos`, `bundles`, the `#[cfg(test)] lookups` counter) plus `sparse_sets: Vec<Box<dyn ErasedSparseSet>>`. `ComponentInfo` gains `kind: StorageKind` and `sparse: Option<usize>` (the set's index); `new_column` becomes `Option<…>`, `Some` for table types only.
- **Bundles.** M3.4's seal (`types`, `register`, `write` through a `RowWriter`), with `register` returning each element's `ComponentId` and kind, and `write` taking the table row writer and the sparse sets with `BundleInfo::sparse`. `BundleInfo` keeps M3.4's third field, the element `TypeId`s in tuple order, for B1.
- **Query fetches are built once per run.** Each `seal::Param` has `init` (binds the sparse parts from the run's sparse slots, the table parts to empty slices) and `set_table` (rebinds the table parts to a matched archetype's columns); the row fetch is `get<'a>(fetch: &'a mut Fetch<'_>, row: usize, entities: &'a [Entity]) -> Item<'a>`, and a table fetch ignores `entities`, so a table-only query reads no entity it does not yield (spec §4.7). Sparse parts cannot be rebound per archetype, because a `SparseMut` taken from the run's slots can be taken only once. `for_each` destructures the `World` into its fields, so the archetype borrows and the sparse-set borrows are disjoint. The builder confirms this shape in a scratchpad stub before writing the module, as the grill's stub did for the seal.
- **Row probes are const-folded.** `seal::Param` and `seal::Filter` each carry `const PROBES: bool` (true if any sparse type is required or excluded) and `passes(fetch, row, entities) -> bool`; the row loop runs `if Q::PROBES || F::PROBES { … continue }` before the fetches, so a table-only query has no probe in its loop (R3). Table `With` and `Without` stay archetype-level, as M3.4's.
- **The mutable sparse split** hands each parameter a crate-private `SparseSlotMut<'w>` (private field around `&'w mut dyn ErasedSparseSet`) whose only method is `into_typed::<T>() -> SparseMut<'w, T>`, never the `&mut dyn` or `&mut Box` (ecs-5).
- **Test hooks.** `matched_archetypes()` as M3.4's; `probed() -> Vec<(ComponentId, Role)>` from the same type list the run uses (ecs-1). Each workload has `#[cfg(test)] step_archetypes()`, `step_probed()` and `archetype_count()` hooks built from its step query's own function.
- **Panic messages** name the operation first: "World::insert called on a despawned entity …", "World::spawn: …", "World::spawn_with: …", "`<T>` appears twice in the bundle", "`<T>` appears twice in the query" prefixed by the method (`Query::for_each:`, `Query::iter:`, `Query::into_iter:`, `QueryMut::for_each:`). The required-sparse-fetch panic (spec §4.7 step 5) reads "hybrid query: `<T>` has no value for <entity> although its probe passed (invariant S1)"; it is an internal-bug panic, not a documented one.
- **Invariant messages** contain the invariant's name (`"T1"`, `"T2"`, `"A1"` to `"A5"`, `"S1"` to `"S4"`, `"B1"`). `check_invariants` checks A3, then T1 and T2, A1 and A2, A4, A5, then S4 and each set's S1 to S3, then B1.

**Global constraints**

- Every commit passes `cargo xtask ci --quick`. Steps 1 and 8 also pass `cargo xtask deny`, with `deny.toml` unchanged (R13).
- The crate root has `#![forbid(unsafe_code)]`; the package has `[lints] workspace = true`. Never lift, relax or `allow` a CI-enforced attribute, even temporarily in the working tree (M3.4's log). Every public item is documented (R14). Doctests are not required ([spikes/README.md](../../../spikes/README.md)).
- Package `spike_ecs_hybrid`: dependencies `engine = { workspace = true }` from step 1, `spike_ecs_harness = { path = "../harness" }` from step 5; dev-dependencies `proptest = { workspace = true }` from step 1 and `spike_ecs_vec = { path = "../vec" }` from step 5. No `agents_2d` and no other spike backend: the harness re-exports what `agents` needs.
- Items a later step uses from library code and nothing uses yet are `#[cfg(test)]` until then, never `#[allow(dead_code)]`.
- Conformance: `Params { count: 1_000, seed: 42 }` (`count: 20` for `frag_iter`) and 120 ticks; `agents` 1,000 agents and 1,300 ticks (R10).
- Run `cargo fmt --all` before `ci --quick`. Commits are conventional, end with ` (M3.5)` and the session's attribution line, and stage files by explicit path.
- While a full `cargo xtask ci` runs, nothing else runs `cargo` in the tree.

**Review focus** (inputs the spec implies that no single requirement's test pins; each has a test in the step that owns the code)

1. **A stale ID whose index was reused**, for both kinds. After `despawn(a)` and a `spawn()` that returns `b` at the same index: `get::<T>(a)`, `remove::<T>(a)` and `despawn(a)` return `None`, `None` and `false` and leave `b` alone; `insert(a, …)` panics; `b` has none of `a`'s sparse values. Step 1: `stale_id_does_not_reach_the_reused_slot`.
2. **Sparse swap-remove of the last entry and of a middle one**: the moved entity's sparse slot is fixed, and S1 holds. Step 1: `sparse_remove_fixes_the_moved_entry`.
3. **Replacing a present value**, for both kinds: the old value is returned; location and `archetype_count` unchanged. Step 1: `replacing_a_value_does_not_move_the_entity`.
4. **A sparse toggle never moves a row**: after `insert::<S>` and `remove::<S>`, the entity's `Location` is identical and no archetype exists that names `S` (A5). Step 1: `sparse_toggle_keeps_the_location`.
5. **Bundle element order and mixed kinds.** `spawn_with((S, C, A))` with `S` sparse stores `C` and `A` in their own columns of the `(A, C)` archetype and `S` in its set. Step 2: `mixed_bundle_values_land_in_their_own_storage`; the model test's bundles include out-of-order and mixed ones.
6. **A type both queried and excluded**, for each kind: `query::<&A>().without::<A>()` panics as a repeated type. Step 3: `a_type_in_a_parameter_and_a_filter_panics`.
7. **Only sparse types required.** `query::<Entity>().with::<S>()` visits every archetype, the empty one included, and yields exactly the entities with `S`. Step 3: `sparse_with_alone_drives_a_query`.
8. **`Option` of a sparse type on an entity without it**, and of a sparse type never registered. Step 3: `sparse_option_is_none_when_absent`.
9. **`count: 0`.** Every workload sets up empty, has checksum 0 and steps without panicking. Steps 5 to 7: `zero_count_is_valid` for each.
10. **`len` across failed operations**: `despawn` of a dead entity, a failed `spawn` or `spawn_with`, a skipped command. Steps 1, 2 and 4.

## 2. Steps

### Step 1: components, tables, sparse sets and the `World` (R1, R2 in part, R4, R5 in part, R6)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/hybrid/Cargo.toml`, `src/lib.rs`, `src/component.rs`, `src/table.rs`, `src/archetype.rs`, `src/sparse.rs`, `src/world.rs`, `src/tests.rs` (new); workspace `Cargo.toml` (member, if not covered by the `spikes/*/*` glob); `xtask/src/coverage.rs` (`spike_ecs_hybrid` in `SPIKE_PACKAGES` and in `test_args_match_the_spec`'s expected `--exclude` list, spec §4.12); `Cargo.lock`.
- **Interfaces produced:**
  - `lib.rs`: `EntityMarker`, `Entity`, `pub use component::{Component, Storage, TableStorage, SparseStorage};`, `pub use world::{InvariantViolation, World};`. Crate docs say what the crate is, point to the spec, and cite the designs it follows (bevy_ecs's per-component storage choice, archetype graph and bundles; hecs's tables; spec §11), with no code taken (specs-5).
  - `component.rs`: spec §4.2 and §1's seal and macro.
  - `table.rs`, `archetype.rs`: M3.4's (its plan §1 and §5), with A5 checked in `World`.
  - `sparse.rs`: `SparseSet<T>` with `new`, `index_of`, `insert`, `remove`, `get`, `get_mut`, `len`, `split_mut` (`#[cfg(test)]` until step 3); `SparseMut<'a, T>` with `get(&mut self, entity) -> Option<&mut T>` only; `ErasedSparseSet` (§1).
  - `world.rs`: `ComponentId`, `ComponentInfo`, `World` with `new`, `spawn`, `despawn`, `is_alive`, `len`, `is_empty`, `archetype_count`, `insert`, `remove`, `get`, `get_mut`, `check_invariants` (spec §4.3). `Default`; `Debug` per spec §4.3. `InvariantViolation` as M3.4's.
- **Test first** (inline test modules):
  - `table.rs`, `archetype.rs`: M3.4's T1, T2 and A-invariant tests as they apply, and `moving_the_last_row_reports_no_swap`.
  - `sparse.rs`: `insert_then_get`; `sparse_remove_fixes_the_moved_entry` (focus 2); `stale_id_is_absent`; `sparse_mut_writes_in_place`; `remove_entity_drops_the_value` (a `Drop` type counting into an `Rc<Cell<u32>>`); `s1_violation_is_reported`, `s2_violation_is_reported`, `s3_violation_is_reported` (each corrupts one private field; the message names it).
  - `world.rs`: `table_insert_then_remove_returns_to_the_first_archetype` and `a_second_transition_hits_the_edge` (the `lookups` counter; R6); `sparse_toggle_keeps_the_location` (focus 4; R6); `replacing_a_value_does_not_move_the_entity` (focus 3, both kinds); `despawn_clears_every_sparse_set` (an entity with two sparse values and table values; each `Drop` runs once; R7 in part, ecs-6); `stale_id_does_not_reach_the_reused_slot` (focus 1); `len_counts_live_entities` (focus 10); `insert_on_a_despawned_entity_panics`, one per kind (`#[should_panic(expected = "World::insert called on a despawned entity")]`); `a_failed_spawn_leaves_the_world_unchanged` (the allocator replaced in test code by `GenIdAllocator::with_fixed_capacity`, the first `spawn` after that fails, `catch_unwind`, the message starts with `"World::spawn:"`, `len` and `archetype_count` unchanged; ecs-4); `a5_violation_is_reported` (a sparse id pushed into an archetype's list in test code); `s4_violation_is_reported`; the A1 to A4 tests from M3.4; `invariant_violation_displays_its_message`; `world_debug_shows_tables_and_sets`.
  - `tests.rs`: `world_matches_the_model` (proptest). Component types `T0(u32)`, `T1(u32)` (table) and `S0(u32)`, `S1(u32)` (sparse); up to 64 operations of `Spawn`, `Despawn(k)`, `Insert(k, type, value)`, `Remove(k, type)`, with `k` choosing among entities spawned so far, live or dead. Model: `BTreeMap<u32, (Entity, [Option<u32>; 4])>` keyed by index. After every operation: `check_invariants` is `Ok`; `get` of every type for every entity ever spawned agrees with the model; `len` equals the model's size. `Insert` on a dead entity is skipped by the generator.
  - Fails first: `cargo xtask test` does not compile (the package does not exist).
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes; `cargo xtask deny` passes; `grep -n spike_ecs_hybrid xtask/src/coverage.rs` finds both entries. The main session then compares the sparse `insert` and `remove` paths with M3.3's `World::insert` and `remove` (`spikes/ecs_storage/sparse_set/src/world.rs`): one `TypeId` lookup and one downcast each (spec §4.3, ecs-7), and logs the result.
- **Commit:** `feat(spikes): add the hybrid components, storages and World (M3.5)`

### Step 2: bundles (R2 in part, R5 in part, R7)

- [x] **Who:** builder
- **Files:** `src/bundle.rs` (new), `src/table.rs`, `src/world.rs`, `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:** `pub trait Bundle` (sealed; tuples of 1 to 8 `Component`s), `BundleInfo { archetype, columns, sparse, types }`, `World::spawn_with` (spec §4.6, §1). `pub use bundle::Bundle;`.
- **Test first:** `spawn_with_creates_one_archetype_of_the_table_types` (`(A, S, C)` with `S` sparse on a fresh world: `archetype_count() == 2`, the archetype has `A` and `C`, `S` is in its set; R7); `mixed_bundle_values_land_in_their_own_storage` (focus 5); `sparse_only_bundle_spawns_into_the_empty_archetype`; `a_warm_spawn_with_creates_nothing` (the `lookups` counter unchanged, `archetype_count` unchanged); `repeated_type_in_a_bundle_panics` (one table, one sparse repeat); `a_failed_spawn_with_leaves_the_world_unchanged` (cold, a mixed bundle; `len`, `archetype_count`, the registered ids, the sparse set count and the bundle cache unchanged; the message starts with `"World::spawn_with:"`; ecs-4); `b1_violation_is_reported`. In `tests.rs`, `world_matches_the_model` gains `SpawnWith(bundle shape, values)` with one to three of the four types, in shapes that include out-of-order and mixed ones (specs-3). Fails first: does not compile.
- **Change:** as above. `spawn_with` allocates the ID first, then builds or fetches the `BundleInfo` (spec §4.3).
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add bundle spawn to the hybrid World (M3.5)`

### Step 3: queries (R2 in part, R3, R8)

- [ ] **Who:** builder
- **Files:** `src/query.rs` (new), `src/component.rs`, `src/sparse.rs`, `src/table.rs`, `src/world.rs`, `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:** `QueryParam`, `ReadOnly`, `Filter` (sealed, one seal each, `Item` on the seal, ecs-3), `With<T>`, `Without<T>` (`PhantomData<fn() -> T>`, `Debug`), `Query<'w, Q, F = ()>`, `QueryMut<'w, Q, F = ()>`, `QueryIter<'w, Q, F>` (spec §4.7; `#[must_use]`, `Debug`; `FusedIterator`; `size_hint` exact or `(0, Some(n))` per spec), `IntoIterator for Query` and `for &Query`, `World::query`, `World::query_mut`; `#[cfg(test)]` `matched_archetypes()` and `probed()` on both query types (§1). Module docs cite the tuple-query design of bevy_ecs and hecs, with no code taken (specs-5). `pub use query::{Filter, Query, QueryIter, QueryMut, QueryParam, ReadOnly, With, Without};`.
- **Test first:**
  - `query.rs`, each with table and sparse types (`A`, `B` table; `S`, `R` sparse): `required_types_select_entities` (a table and a sparse required type together); `option_yields_some_exactly_when_present` (both kinds); `sparse_option_is_none_when_absent` (focus 8); `with_and_without_filters_chain` (table `With`, sparse `Without`, and the reverse); `sparse_with_alone_drives_a_query` (focus 7); `entity_parameter_yields_the_entity`; `nested_tuple_works` (`(Entity, (&mut A, &S, &B, Option<&mut R>))` through `query_mut`, writing through `&mut A` and `Option<&mut R>`); `never_inserted_required_type_matches_nothing`, one per kind; `mutable_query_writes_are_visible` (a sparse `&mut S`); `iter_and_for_each_agree` (also `for _ in &query` and by value); `size_hint_is_exact_without_sparse_filters` and `size_hint_bounds_the_count_with_them`; `repeated_type_panics` (once each for `Query::for_each`, `Query::iter`, `Query::into_iter`, `QueryMut::for_each`, asserting the prefix; one table and one sparse case); `a_type_in_a_parameter_and_a_filter_panics` (focus 6); `query_without_a_required_type_visits_every_live_entity` (`query::<Entity>()`, `query::<Option<&S>>()`, `query::<Entity>().without::<S>()`, `query::<Entity>().without::<A>()`, including entities in the empty archetype); `matched_and_probed_report_the_run` (a table-only query has empty `probed()`; a query with `Option<&S>` and `Without<R>` reports `S` optional and `R` excluded; `matched_archetypes` for a filter that excludes some).
  - `tests.rs`: `world_matches_the_model` also compares, after every operation, the entity sets of `query::<(Entity, &T0, &S0)>()`, `query::<(Entity, &T0)>().without::<S1>()`, `query::<(Entity, Option<&S1>)>().with::<T1>()`, `query::<Entity>().with::<S0>()`, `query::<Entity>()` and `query::<Entity>().without::<T0>()` (through both `iter` and `for_each`) with the model's, and the `Option<&S1>` values; and one `query_mut::<(&mut T0, Option<&mut S0>)>()` that adds 1 to each, mirrored in the model.
  - Fails first: does not compile (`query` and `query_mut` are not defined).
- **Change:** as above, after a scratchpad stub of §1's `init`/`set_table` fetch shape builds on 1.100. Then count the heap allocations of one query run as built, from a scratchpad crate outside the repository (its own `[workspace]` table and a `rust-toolchain.toml` pin, depending on `spike_ecs_hybrid` by path, with a counting `#[global_allocator]`; never in the crate; ecs-2), for `Query::for_each`, `Query::iter` and `QueryMut::for_each` with a table-only query, one with a sparse `Option`, one with a sparse `With`, and a world of 2 and of 28 registered types (whether setup grows with the registered types; ecs-2). Write the counts in `log.md` with the commit they ran on (specs-2, specs-4).
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes; `log.md` has the allocation counts; `git status` shows no file outside `spikes/ecs_storage/hybrid/` and `log.md` changed.
- **Commit:** `feat(spikes): add typed tuple queries over tables and sparse sets (M3.5)`

### Step 4: `Commands` (R9)

- [ ] **Who:** builder
- **Files:** `src/commands.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:** `pub struct Commands` and `World::apply` per spec §4.8 (`impl World { apply }` in `commands.rs`), `T: Component` bounds; `apply` drains so capacity is kept; `#[cfg(test)] fn capacity`. `Default`, `Debug` (the number of commands). `pub use commands::Commands;`.
- **Test first:** `commands_run_in_recorded_order` (insert then remove of the same type leaves it absent, once per kind); `command_on_a_dead_entity_is_skipped` (no panic, `check_invariants` `Ok`, `len` unchanged; focus 10); `apply_empties_the_buffer_and_keeps_its_capacity`; `commands_reach_each_storage` (a recorded sparse `insert` leaves the location unchanged; a recorded table `insert` moves the entity). In `tests.rs`, `world_matches_the_model` gains `DeferInsert(k, type, value)`, `DeferRemove(k, type)`, `DeferDespawn(k)` and `Apply`, with one buffer and a model of the recorded commands that applies them in order and skips dead entities (specs-3). Fails first: does not compile.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add a command buffer to the hybrid World (M3.5)`

### Step 5: harness components and the iteration workloads (R10 in part)

- [ ] **Who:** builder
- **Files:** `src/harness_components.rs`, `src/simple_iter.rs`, `src/frag_iter.rs`, `src/heavy_compute.rs` (new), `src/lib.rs`, `src/tests.rs`, `Cargo.toml` (`spike_ecs_harness`, dev `spike_ecs_vec`), `Cargo.lock`.
- **Interfaces produced:** `harness_components.rs` (private module) implements `Component` for every harness type the seven workloads use: `B` and `Seeker` `SparseStorage`, the rest `TableStorage` (§1; the 26 letter types through the harness's letter list or a local macro over them). `simple_iter::HybridSimpleIter`, `frag_iter::HybridFragIter`, `heavy_compute::HybridHeavyCompute` (spec §4.9), with each step query one private function used by `step` and the hooks of §1 (ecs-1).
- **Test first** (`tests.rs`): `simple_iter_hybrid_matches_the_reference`, `frag_iter_hybrid_matches_the_reference`, `heavy_compute_hybrid_matches_the_reference`; `<workload>_hybrid_matches_its_archetypes` for each (after setup, `archetype_count` is §4.9's count plus one; after one step, `step_archetypes()` is the expected list and `step_probed()` is empty); `zero_count_is_valid` for the three (focus 9). Fails first: does not compile.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the iteration workloads to the hybrid backend (M3.5)`

### Step 6: `add_remove` and `entity_churn` (R10 in part, R11 in part)

- [ ] **Who:** builder
- **Files:** `src/add_remove.rs`, `src/entity_churn.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:** `add_remove::HybridAddRemove` (reused `Vec<(Entity, f32)>` and `Vec<Entity>`; read query `(Entity, &A)` and removal query `query::<Entity>().with::<B>()`, each its own private function with hooks) and `entity_churn::HybridEntityChurn` (a `Commands`, the `Prng`, `next_id` and the target; respawns with `spawn_with` after `apply` until `world.len()` is the target).
- **Test first** (`tests.rs`): `add_remove_hybrid_matches_the_reference`, `entity_churn_hybrid_matches_the_reference`; `add_remove_hybrid_matches_its_archetypes` (after one step: read query matches `(A)` with empty `probed()`; removal query matches every archetype with `B` required in `probed()`); `entity_churn_hybrid_matches_its_archetypes`; `add_remove_keeps_invariants_every_tick` (also asserts no entity has `B` after a tick, and `archetype_count` after every tick equals its value after setup; R11) and `entity_churn_keeps_invariants_every_tick`; `zero_count_is_valid` for the two. Fails first: does not compile.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes. Then the red checks, each a temporary edit, reverted, files byte-identical afterwards (`cmp` against a copy), recorded in `log.md` with the failing test's first lines: (1) make `B` a `TableStorage` type in `harness_components.rs`: `add_remove_keeps_invariants_every_tick` (archetype count) and `add_remove_hybrid_matches_its_archetypes` fail; (2) skip the `remove::<B>` pass: the conformance test and the invariant test fail; (3) in `World::despawn`, skip the sparse sets: `despawn_clears_every_sparse_set` and `world_matches_the_model` fail (ecs-6); (4) in the row loop, ignore excluded sparse probes: `with_and_without_filters_chain` and `world_matches_the_model` fail.
- **Commit:** `feat(spikes): add the structural workloads to the hybrid backend (M3.5)`

### Step 7: `sprites` and `agents` (R10, R11)

- [ ] **Who:** builder
- **Files:** `src/sprites.rs`, `src/agents.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:** `sprites::HybridSprites` (owns the draw list, reserved to `count` in `setup`; `Hidden` a table type in the bundle; move and draw queries each a private function with hooks) and `agents::HybridAgents` (`pub struct AgentId(pub u64)`, `Velocity(pub Vec2)`, `Ttl(pub u32)` as M3.4's, their `Component` impls in `agents.rs`; seeker change recorded before the `despawn`, as M3.4's step does; respawn with a 4- or 5-element bundle).
- **Test first** (`tests.rs`): `sprites_hybrid_matches_the_reference`, `agents_hybrid_matches_the_reference` (1,000 agents, 1,300 ticks); `sprites_hybrid_matches_its_archetypes` (2 archetypes plus the empty one; both queries' `probed()` empty; draw matches only the archetype without `Hidden`); `agents_hybrid_matches_its_archetypes` (1 plus the empty one after setup; after one step the step query matches it and probes `Seeker` as optional); `agents_keeps_invariants_every_tick` (1,000 agents, 1,300 ticks; `archetype_count` constant); `zero_count_is_valid` for the two. Fails first: does not compile.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the sprite and agent workloads to the hybrid backend (M3.5)`

### Step 8: bench registrations (R12)

- [ ] **Who:** builder
- **Files:** `spikes/ecs_storage/bench_compare/Cargo.toml`, `benches/compare.rs`, `spikes/ecs_storage/bench_cache/Cargo.toml`, `benches/cache.rs`, `Cargo.lock`.
- **Test first:** n/a for registrations. The check before the change: `cargo bench -p spike_ecs_bench_compare -- --test 2>&1 | grep -c /hybrid/` prints 0.
- **Change:** dev-dependency `spike_ecs_hybrid = { path = "../hybrid" }` in both bench crates. `compare.rs`: one `bench::<…>(c);` per workload (7). `cache.rs`: one `cache_bench!(<workload>_hybrid, <Type>, [<sizes>]);` per workload with each workload's `Spec::SIZES` (`simple_iter`, `add_remove`, `entity_churn`: `[1_000, 10_000, 100_000]`; `frag_iter`: `[20]`; `heavy_compute`: `[1_000]`; `sprites`: `[100_000]`; `agents`: `[10_000, 100_000]`), each named in its workload's group.
- **Verify:** the same command prints 14 (3 + 1 + 1 + 3 + 3 + 1 + 2); `cargo build --benches -p spike_ecs_bench_cache` builds; the main session checks each new `cache_bench!` line's sizes and group against the workload's `vec_aos` line (#74); `cargo xtask ci --quick` passes; `cargo xtask deny` passes.
- **Commit:** `feat(spikes): benchmark the hybrid backend (M3.5)`

### Step 9: documents (R14 in part)

- [ ] **Who:** main session
- **Files:** `spikes/ecs_storage/README.md`, `CHANGELOG.md`, `docs/roadmap.md` (M3.5's title, spec §4.12).
- **Test first:** n/a for prose. The checks before the change: `grep -n "hybrid/" spikes/ecs_storage/README.md` and `grep -n "spike_ecs_hybrid" CHANGELOG.md` find nothing; `grep -n "^| M3.5 " docs/roadmap.md` shows the old title.
- **Change:** spec §4.12: the package row in the spike README; a CHANGELOG entry; the roadmap title. Predictions keep their "expected, unmeasured" wording (docs-3). In the close-out commit: M3.5's row `Done`, M3.8's row gains the costs found while building (step 3's allocation count, step 1's lookup comparison and anything the reviews find); the follow-up issue for enable bits under #78 and the comment on #84 (spec §4.11, §4.12), linked from the pull request, which closes #82 and #83.
- **Verify:** the greps find their text; `cargo xtask ci` passes in full, with nothing else running `cargo` in the tree.
- **Commit:** `docs: record the hybrid prototype (M3.5)`

## 3. Final verification

- `cargo xtask ci` passes locally on the final code commit. `cargo xtask bench-regress` is not needed: nothing under `libs/` or `bench/regress/` changes.
- One informal `cargo xtask spike-bench ecs` run under the benchmark run protocol (`/bench-run`: AC power, `caffeinate`, no builds during the run, the power log checked afterwards); its output goes in the pull request, labelled informal. No cache-bench run unless a Valgrind host is available; the pull request says which.
- CI is green on Ubuntu, macOS and Windows with no workflow change (R13).
- The reviews of stage 5 are recorded in `log.md` and the pull request, and the spec's §9 list is ticked with evidence, each naming the commit it ran on (specs-4).

## 4. Lessons

From `lessons-researcher` (2026-11-20), plus three it skipped that match these paths and topics.

- **ecs-1** followed (not returned; it matches `spikes/ecs_storage/**`): every step query is one private function shared by `step` and its `step_archetypes` and `step_probed` hooks, and `probed()` uses the run's own type list (§1, steps 5 to 7).
- **ecs-2** followed: step 3 counts a query run's allocations from a scratchpad crate, at 2 and 28 registered types; close-out carries the count into M3.8's row.
- **ecs-3** followed: `type Item<'w>` lives on the seal trait (§1, step 3).
- **ecs-4** followed (not returned): `a_failed_spawn_leaves_the_world_unchanged` and `a_failed_spawn_with_leaves_the_world_unchanged` fail on the first call and assert the message (steps 1 and 2).
- **ecs-5** followed: queries get `ColumnMut` and `SparseMut` (only `get`), and the sparse split passes `SparseSlotMut`, never `&mut dyn ErasedSparseSet` or `&mut Box` (§1, steps 1 and 3).
- **ecs-6** followed: step 6's red check 3 (despawn skipping the sparse sets) is pointed at `despawn_clears_every_sparse_set` and the model test.
- **ecs-7** followed: step 1's verify compares the sparse `insert` and `remove` paths with M3.3's before any benchmark.
- **docs-2** followed: when a review fix changes a count (tests, bench ids), the plan is grepped for the old number.
- **docs-3** followed: spec §4.9's predictions keep "expected, unmeasured" in the CHANGELOG, README and M3.8's row (step 9, close-out).
- **specs-1** followed: step 8's and step 9's before-checks name the command and files they search.
- **specs-2** followed: the `log.md` evidence a step asks for (step 1's lookup comparison, step 3's allocation count, step 6's red checks) is in that step's own Change or Verify.
- **specs-3** followed: every panic of spec §4.3, §4.6 and §4.7 and every invariant has its own named test (steps 1 to 4), and the model test's operations are copied from spec §8 into steps 1 to 4, `Apply` included.
- **specs-4** followed: §3's evidence names the commit it ran on.
- **specs-5** followed (not returned): steps 1 and 3 cite the designs in the shipped module docs.

## 5. Deviations

- **Step 1, the storage seal:** carries `KIND` and the four `World` paths only; the fetch GATs and per-row functions arrive with step 3, `register` is `World::register::<T>()` matching on `KIND` (unused items fail clippy; log.md, 2026-11-20).
- **Step 1, `StorageKind`:** `pub` inside the private `component` module (the seal names it), so unnameable outside the crate, rather than `pub(crate)` as spec §4.2 says.
- **Step 1, `check_invariants` order:** A3, then A5, then T1 and T2, A1 and A2, A4, S4 and S1 to S3, so A5 is reachable (sent back once; log.md, 2026-11-20).
- **Step 2, cold `spawn_with` order:** the repeated-type check runs before the allocation (it registers nothing), so a repeated type uses no entity ID; then allocate, register and build the info.
- **Step 2, visibility:** `ComponentId` and `ErasedSparseSet` are `pub` inside private modules, since the bundle seal names them (`private_interfaces`), as in M3.4.
- **Step 2, warm `spawn_with`:** one `bundles` lookup, as M3.4's (sent back once from three; ecs-7; log.md, 2026-11-20).
