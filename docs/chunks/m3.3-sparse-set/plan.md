# M3.3 plan: Sparse-set prototype

- **Status:** Accepted (2026-11-19). Rewritten from scratch after the spec was revised and accepted; the 2026-11-14 plan is in the history at 8e093c9.
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. Progress and surprises go in [log.md](log.md).

## 1. Approach

Ten steps, one commit each. The crate is built bottom up so each layer is tested before anything depends on it: storage with an ungrouped `World` (together, because an unused `pub(crate)` storage fails `clippy -D warnings`), then the builder and groups with the model test, the view-path queries, the group path, `Commands`, three workload commits, the bench registrations, and the documents.

Choices the spec leaves to implementation:

- **One implementation per workload, two backends.** Each workload module defines `pub struct XOn<const GROUPED: bool> { world: World, … }` (private fields) with `pub type SparseSetX = XOn<false>;` and `pub type GroupedX = XOn<true>;`. `BACKEND` is `if GROUPED { "sparse_set_grouped" } else { "sparse_set" }`. `setup` builds `World::builder().group::<…>().build()` when `GROUPED` and `World::new()` otherwise, so the backends differ only in the group. `frag_iter` has only `SparseSetFragIter`, a plain struct.
- **`Entity` and `EntityMarker`** live in `lib.rs`, so `storage.rs`, `world.rs` and `query.rs` use them without a cycle.
- **Fetches borrow a storage's fields separately.** `SparseSet<T>` offers `pub(crate) fn split(&self) -> (&SparseIndex, &[Entity], &[T])` and `split_mut(&mut self) -> (&SparseIndex, &[Entity], &mut [T])`. The driver's entity slice is shared, so the loop copies each entity out, then borrows the fetches mutably once per entity. This keeps spec §4.5's view path safe.
- **Borrow-splitting slots** are a `Vec<Option<&'w mut Box<dyn ErasedStorage>>>`, one slot per component type in declaration order (parameters, then filters), filled in one `iter_mut().enumerate()` pass: one allocation per query run, not per entity.
- **Where the query machinery lives.** The public traits (`QueryParam` with `type Item<'w>`, `ReadOnly`, `Filter`, `GroupSpec`) each have their own seal trait in a private module (`seal::Param`, `seal::Filter`, `seal::Group`; spec §4.5). The fetch methods live on the seal traits, and the helper types they mention (`TypeEntry`, `Slots<'w>`, `RefSlots<'w>`) are `pub` items of that private module: unnameable outside the crate, out of the docs, and accepted by the `private_interfaces` lint. If that lint objects anyway, mark the helpers `#[doc(hidden)]` and record it in §5.
- **Running a query.** `Query<'w, Q, F = ()>` holds `&'w World` and `QueryMut<'w, Q, F = ()>` holds `&'w mut World`, plus `PhantomData<fn() -> (Q, F)>`. `.with::<T>()` and `.without::<T>()` rebuild the struct with `F = (F, With<T>)` or `(F, Without<T>)`. Type lookup, the duplicate and no-required-type panics, and the borrow split all happen in the running methods (`for_each`, `iter`, `into_iter`), which are `#[track_caller]`. `uses_group` is `pub(crate)` and runs the same lookup without iterating.
- **The group-path flag.** On the group path each member's fetch is marked `direct` when the query runs and reads index `i` without a probe. The flag is the same for every entity, so the branch predicts perfectly; no second monomorphized loop.
- **Panic messages** name the type with `core::any::type_name::<T>()` and the entity with its `Debug` form.

**Global constraints**

- Every commit passes `cargo xtask ci --quick`. Steps 1 and 9 also pass `cargo xtask deny`, with `deny.toml` unchanged (R14).
- The crate root has `#![forbid(unsafe_code)]`; the package has `[lints] workspace = true`. Every public item is documented (R15). Doctests are not required ([spikes/README.md](../../../spikes/README.md)).
- Package `spike_ecs_sparse_set`: dependencies `engine = { workspace = true }`, `spike_ecs_harness = { path = "../harness" }`, and `agents_2d = { path = "../../../examples/agents_2d" }` from step 8 (an unused dependency fails clippy); dev-dependencies `spike_ecs_vec = { path = "../vec" }` and `proptest = { workspace = true }`. The workspace glob `spikes/*/*` already covers the directory.
- Conformance: `Params { count: 1_000, seed: 42 }` (`count: 20` for `frag_iter`) and 120 ticks; `agents` 1,000 agents and 1,300 ticks (R10).
- Commits are conventional, end with ` (M3.3)` and the session's attribution line, and stage files by explicit path.

**Review focus** (inputs the spec implies that no single requirement's test pins; each has a test in the step that owns the code)

1. **A stale ID whose index was reused.** After `despawn(a)` and a `spawn()` that returns `b` at the same index: `get::<T>(a)`, `remove::<T>(a)` and `despawn(a)` return `None`, `None` and `false`, leave `b` alone, and `insert(a, …)` panics. Step 1: `stale_id_does_not_reach_the_reused_slot`.
2. **Removing the last or only entry.** A swap-remove of position `len - 1` swaps with itself; S1 must still hold. Step 1: `removing_the_last_and_the_only_entry_keeps_the_index_consistent`.
3. **Replacing an owned value.** `insert` of a member type the entity already has returns the old value and changes neither its position nor the group length. Step 2: `replacing_an_owned_value_keeps_the_group_length`.
4. **A type both queried and excluded.** `query::<&A>().without::<A>()` panics as a repeated type instead of silently matching nothing. Step 3: `a_type_in_a_parameter_and_a_filter_panics`.
5. **`count: 0`.** Every workload in both backends sets up empty, has checksum 0 and steps without panicking. Steps 6 to 8: `zero_count_is_valid` for each.
6. **`len` across failed operations.** `despawn` of a dead entity returns `false` and leaves `len` unchanged; a skipped command leaves it unchanged. Step 1 (`len_counts_live_entities`) and step 5 (`command_on_a_dead_entity_is_skipped`).

## 2. Steps

### Step 1: storage and the ungrouped `World` (R1, R2 in part, R3, R4 for S1 to S3)

- [x] **Who:** builder
- **Files:** `spikes/ecs_storage/sparse_set/Cargo.toml`, `src/lib.rs`, `src/storage.rs`, `src/world.rs` (new); `xtask/src/coverage.rs` (`spike_ecs_sparse_set` in `SPIKE_PACKAGES`, spec §4.9); `Cargo.lock`.
- **Interfaces produced:**
  - `lib.rs`: `pub struct EntityMarker;`, `pub type Entity = GenId<EntityMarker>;`, `pub use world::{InvariantViolation, World};`.
  - `storage.rs`: `struct Packed<T>` with spec §4.3's paired methods; `pub(crate) struct SparseIndex(Vec<u32>)` with `EMPTY = u32::MAX`, `get(u32) -> Option<u32>`, `set(u32, u32)`, `clear(u32)`; `pub(crate) struct SparseSet<T>` with `new`, `index_of`, `insert`, `remove`, `swap`, `get`, `get_mut`, `split`, `split_mut`, `len`; `pub(crate) trait ErasedStorage` (object-safe) with `index_of`, `swap`, `remove_entity(&mut self, Entity) -> bool`, `len`, `entities(&self) -> &[Entity]`, `type_name(&self) -> &'static str`, `check(&self, is_alive: &dyn Fn(Entity) -> bool) -> Result<(), InvariantViolation>`, `as_any`, `as_any_mut`.
  - `world.rs`: `World` with `new`, `spawn`, `despawn`, `is_alive`, `len`, `is_empty`, `insert`, `remove`, `get`, `get_mut`, `check_invariants` (spec §4.2). Fields: `entities: GenIdAllocator<EntityMarker>`, `live: usize`, `ids: BTreeMap<TypeId, ComponentId>`, `storages: Vec<Box<dyn ErasedStorage>>`. `pub(crate) struct ComponentId(u32)`; `pub(crate) fn storage<T>`, `storage_mut<T>`, `id_or_create<T: 'static>`. `Default` calls `new`; `Debug` prints `live` and each storage's type name and length.
  - `pub struct InvariantViolation { message: String }`: `Debug`, `Clone`, `PartialEq`, `Eq`, `Display` ("invariant violated: …"), `std::error::Error`; `pub(crate) fn new(String)`.
- **Test first** (inline test modules):
  - `storage.rs`: `packed_operations_keep_both_arrays_paired`; `insert_then_remove_round_trips`; `insert_replaces_and_returns_the_old_value`; `removing_the_last_and_the_only_entry_keeps_the_index_consistent` (focus 2); `index_past_the_sparse_end_is_absent`; `s1_violation_is_reported`, `s2_violation_is_reported`, `s3_violation_is_reported` (each corrupts one private field; the `Display` contains `"S1"`, `"S2"` or `"S3"`); `zero_sized_values_do_not_allocate` (capacity is `usize::MAX`).
  - `world.rs`: `components_are_independent_per_type`; `despawn_removes_every_component`; `stale_id_does_not_reach_the_reused_slot` (focus 1); `len_counts_live_entities` (spawn three, despawn one, despawn it again: `len` is 3, 2, 2; `is_empty` on a new world; focus 6); `insert_on_a_despawned_entity_panics` (`#[should_panic(expected = "despawned")]`); `invariant_violation_displays_its_message`.
  - Fails first: `cargo xtask test` does not compile (the package does not exist).
- **Change:** implement the above. The crate docs say what the crate is, point to the spec, and cite the designs it follows (EnTT's sparse sets and owning groups, spec §11), per specs-5.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes; `cargo xtask deny` passes; `grep -n spike_ecs_sparse_set xtask/src/coverage.rs` finds the entry.
- **Commit:** `feat(spikes): add the sparse-set storage and World (M3.3)`

### Step 2: `WorldBuilder`, owning groups and the model test (R2 in part, R4 for G1 and G2, R5)

- [x] **Who:** builder
- **Files:** `src/world.rs`, `src/query.rs` (new: `GroupSpec` and `seal::Group` only), `src/lib.rs`, `src/tests.rs` (new).
- **Interfaces produced:**
  - `pub trait GroupSpec: seal::Group` for tuples of 2 to 4 `'static` types (a small macro); `seal::Group` gives the members' `TypeId`s and names.
  - `World::builder() -> WorldBuilder`; `#[must_use] pub struct WorldBuilder` with `new`, `group<G: GroupSpec>(self) -> Self` (`#[track_caller]`; panics "`<T>` already belongs to a group" and "`<T>` appears twice in the group"), `build(self) -> World` (creates the members' storages). `Default` and `Debug`. `World::new()` is `WorldBuilder::new().build()`.
  - `pub(crate) struct Group { members: Vec<ComponentId>, len: usize }`; `World::groups: Vec<Group>`; `World::owner: Vec<Option<usize>>` indexed by `ComponentId`.
  - `insert`, `remove` and `despawn` apply spec §4.4. Mutable member access uses `<[_]>::get_disjoint_mut` on `storages`, in a `match` over 2, 3 and 4 members.
  - `check_invariants` also checks G1 and G2 (messages contain `"G1"` or `"G2"`). `pub(crate) fn group_len(&self, group: usize) -> usize` for tests.
- **Test first:**
  - `world.rs`: `joining_and_leaving_a_group_moves_the_entity`; `replacing_an_owned_value_keeps_the_group_length` (focus 3); `despawning_a_grouped_entity_shrinks_the_group`; `type_in_two_groups_panics`; `repeated_type_in_a_group_panics`; `g1_violation_is_reported`; `g2_violation_is_reported`.
  - `tests.rs`: `world_matches_the_model` (proptest). Component types `C0(u32)` to `C3(u32)`; up to 64 operations of `Spawn`, `Despawn(k)`, `Insert(k, type, value)`, `Remove(k, type)`, with `k` choosing among entities spawned so far, live or dead; three setups: no group, `(C0, C1)`, `(C0, C1, C2)`. The model is a `BTreeMap<u64, [Option<u32>; 4]>` keyed by `Entity::to_bits`, plus the set of live keys. After every operation: `check_invariants` is `Ok`; `get` of every type for every entity ever spawned agrees with the model; `len` equals the live count. `Insert` on a dead entity is skipped by the generator, since it panics by design.
  - Fails first: does not compile (`builder` is not defined).
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add owning groups to the sparse-set World (M3.3)`

### Step 3: queries on the view path (R2 in part, R6, R8)

- [ ] **Who:** builder
- **Files:** `src/query.rs`, `src/world.rs`, `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:**
  - `pub trait QueryParam: seal::Param { type Item<'w>; }` for `&T`, `&mut T`, `Option<&T>`, `Option<&mut T>`, `Entity` and tuples of 1 to 4 (one `macro_rules!` over `P0 P1 P2 P3`, also generating `ReadOnly`); `pub trait ReadOnly: QueryParam`; `pub trait Filter: seal::Filter` for `()`, `With<T>`, `Without<T>` and `(F, G)`. `seal::Param` and `seal::Filter` carry the type lists, fetch, probe and match functions (§1).
  - `pub struct With<T>(PhantomData<fn() -> T>)`, `pub struct Without<T>(PhantomData<fn() -> T>)`, each with `Debug`.
  - `World::query<Q: ReadOnly>(&self) -> Query<'_, Q>` and `World::query_mut<Q: QueryParam>(&mut self) -> QueryMut<'_, Q>`.
  - `#[must_use] pub struct Query<'w, Q, F = ()>` with `with`, `without`, `for_each(self, impl FnMut(Q::Item<'w>))`, `iter(&self) -> QueryIter<'w, Q, F>`, and `IntoIterator`; `#[must_use] pub struct QueryIter<'w, Q, F>: Iterator<Item = Q::Item<'w>>`; `#[must_use] pub struct QueryMut<'w, Q, F = ()>` with `with`, `without`, `for_each(self, impl FnMut(Q::Item<'_>))`. All `Debug`. The running methods are `#[track_caller]` and panic "`<T>` appears twice in the query" and "this prototype needs at least one `&T`, `&mut T` or `.with::<T>()` in a query; `<Q>` has none".
  - A required probe is `sparse.get(entity.index())` and nothing else (R8); the driver's own fetch reads index `i` unprobed.
  - `pub use query::{Filter, GroupSpec, Query, QueryIter, QueryMut, QueryParam, ReadOnly, With, Without};`.
- **Test first:**
  - `query.rs`: `required_types_select_entities`; `option_yields_some_exactly_when_present`; `with_and_without_filters_chain` (`.with::<B>().without::<C>().with::<D>()`); `with_alone_drives_a_query` (`query::<Entity>().with::<B>()`, as `add_remove` uses); `entity_parameter_yields_the_entity`; `nested_tuple_works` (`(Entity, (&mut A, &B, &C, Option<&D>))` through `query_mut`, writing through `&mut A`); `never_inserted_required_type_matches_nothing`; `mutable_query_writes_are_visible`; `iter_for_each_and_into_iter_agree`; `repeated_type_panics`; `a_type_in_a_parameter_and_a_filter_panics` (focus 4); `query_without_a_required_type_panics`, once each for `query::<Entity>()`, `query::<Option<&A>>()` and `query::<Entity>().without::<A>()`; `driver_is_the_smallest_required_storage` (100 entities with `A`, 2 with `B`; `(&A, &B)` probes 2, counted by a `#[cfg(test)]` counter).
  - `tests.rs`: `world_matches_the_model` also compares, after every operation, the entity sets of `query::<(Entity, &C0, &C1)>()`, `query::<(Entity, &C0)>().without::<C2>()` and `query::<(Entity, Option<&C3>)>().with::<C1>()` with the model's, as sorted `Vec<u64>` of `to_bits`.
  - Fails first: does not compile.
- **Change:** as above. If closure inference over `Q::Item<'_>` fails for a nested tuple at a call site, use a named helper in the test, not a trait change, and record it in §5.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add typed tuple queries to the sparse-set World (M3.3)`

### Step 4: the group path (R7)

- [ ] **Who:** builder
- **Files:** `src/query.rs`, `src/world.rs`.
- **Interfaces produced:** `pub(crate) fn uses_group(&self) -> bool` on `Query` and `QueryMut`. Selection per spec §4.5: required types include every member; most members wins, then first declared. Member fetches are re-sliced to `..group.len` and marked `direct`; the driver is the first member's entity slice, re-sliced the same way.
- **Test first** (`query.rs`): `exact_match_takes_the_group_path`; `superset_takes_the_group_path_and_probes_the_rest` (`(&A, &B, &C)` visits only entities with `C`); `strict_subset_takes_the_view_path`; `the_largest_qualifying_group_wins` (groups `(A, B)` and `(C, D, E)`, `query::<(&A, &B, &C, &D)>().with::<E>()` picks `(C, D, E)`, via a test-only accessor); `group_path_and_view_path_visit_the_same_entities` (proptest: one world with group `(A, B)`, one without, the same operations, the same sorted entity set from `(Entity, &A, &B)`). Fails first: does not compile (`uses_group` is not defined).
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): iterate owning groups by aligned prefix (M3.3)`

### Step 5: `Commands` (R9)

- [ ] **Who:** builder
- **Files:** `src/world.rs` (or a new `src/commands.rs` if `world.rs` passes about 600 lines), `src/lib.rs`.
- **Interfaces produced:** `pub struct Commands` and `World::apply(&mut self, &mut Commands)` per spec §4.6. `Deferred` closures check `is_alive` first; `apply` drains with `Vec::drain` so capacity is kept. `Default` and `Debug` (the number of commands).
- **Test first:** `commands_run_in_recorded_order` (insert then remove `A`: absent after `apply`); `command_on_a_dead_entity_is_skipped` (no panic, `check_invariants` `Ok`, `len` unchanged; focus 6); `apply_empties_the_buffer_and_keeps_its_capacity`; `commands_maintain_groups` (inserting `B` through `Commands` joins group `(A, B)`). Fails first: does not compile.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add a command buffer to the sparse-set World (M3.3)`

### Step 6: `simple_iter`, `frag_iter` and `heavy_compute` (R10, R11 in part)

- [ ] **Who:** builder
- **Files:** `src/simple_iter.rs`, `src/frag_iter.rs`, `src/heavy_compute.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:** `simple_iter::{SimpleIterOn, SparseSetSimpleIter, GroupedSimpleIter}`, `frag_iter::SparseSetFragIter`, `heavy_compute::{HeavyComputeOn, SparseSetHeavyCompute, GroupedHeavyCompute}` (spec §4.7, §1). `frag_iter` inserts each letter as its own type via a local macro over the 26 `Letter` variants; its checksum runs one `query::<(&L, &Data)>()` per letter and hashes `Letter::from_index(index, l.0)`.
- **Test first** (`tests.rs`): one `<workload>_<backend>_matches_the_reference` per implementation (five); `grouped_simple_iter_uses_the_group`; `grouped_heavy_compute_uses_the_group`; `zero_count_is_valid` for each of the five (focus 5). Fails first: does not compile.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the iteration workloads to the sparse-set backends (M3.3)`

### Step 7: `add_remove` and `entity_churn` (R10, R11 in part, R12 in part)

- [ ] **Who:** builder
- **Files:** `src/add_remove.rs`, `src/entity_churn.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:** `add_remove::{AddRemoveOn, SparseSetAddRemove, GroupedAddRemove}` (reused `Vec<(Entity, f32)>` and `Vec<Entity>` in the struct; the second pass is `query::<Entity>().with::<B>()`) and `entity_churn::{EntityChurnOn, SparseSetEntityChurn, GroupedEntityChurn}` (keeps a `Commands`, the `Prng`, `next_id` and the target; respawns with `spawn` and `insert` after `apply` until `world.len()` is the target).
- **Test first** (`tests.rs`): the four conformance tests; `zero_count_is_valid` for the four; `grouped_entity_churn_uses_the_group`; `grouped_add_remove_takes_the_view_path` (both queries report `!uses_group()`, R11); `entity_churn_keeps_invariants_every_tick` (both backends, 1,000 entities, 120 ticks). Fails first: does not compile.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the structural workloads to the sparse-set backends (M3.3)`

### Step 8: `sprites` and `agents` (R10, R11, R12)

- [ ] **Who:** builder
- **Files:** `src/sprites.rs`, `src/agents.rs` (new), `src/lib.rs`, `src/tests.rs`, `Cargo.toml` (`agents_2d`), `Cargo.lock`.
- **Interfaces produced:** `sprites::{SpritesOn, SparseSetSprites, GroupedSprites}` (owns the draw list, reserved to `count` in `setup`; draw is `query::<(&Transform2d, &Sprite)>().without::<Hidden>()`) and `agents::{AgentsOn, SparseSetAgents, GroupedAgents}` (`pub struct AgentId(pub u64)`, `Velocity(pub Vec2)`, `Ttl(pub u32)`; keeps a `Commands`, the `Prng`, `next_id`, `tick` and the target; respawns until `world.len()` reaches it). Within one agent's closure the seeker change is recorded before the `despawn`, so a caught-and-expired agent's `remove` runs first.
- **Test first** (`tests.rs`): the four conformance tests (`agents`: 1,000 agents, 1,300 ticks); `zero_count_is_valid` for the four; `grouped_sprites_moves_on_the_group_path_and_draws_on_the_view_path`; `grouped_agents_uses_the_group`; `agents_keeps_invariants_every_tick` (both backends, 1,000 agents, 1,300 ticks). Fails first: does not compile.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the sprite and agent workloads to the sparse-set backends (M3.3)`

### Step 9: bench registrations (R13)

- [ ] **Who:** builder
- **Files:** `spikes/ecs_storage/bench_compare/Cargo.toml`, `benches/compare.rs`, `spikes/ecs_storage/bench_cache/Cargo.toml`, `benches/cache.rs`, `Cargo.lock`.
- **Test first:** n/a for registrations. The check before the change: `cargo xtask bench-smoke` lists no `sparse_set` id.
- **Change:** dev-dependency `spike_ecs_sparse_set = { path = "../sparse_set" }` in both bench crates. `compare.rs`: one `bench::<…>(c);` per implementation (13). `cache.rs`: one `cache_bench!(<workload>_<backend>, <Type>, [<sizes>]);` per implementation with each workload's `Spec::SIZES` (`simple_iter`, `add_remove`, `entity_churn`: `[1_000, 10_000, 100_000]`; `frag_iter`: `[20]`; `heavy_compute`: `[1_000]`; `sprites`: `[100_000]`; `agents`: `[10_000, 100_000]`), each named in its workload's group.
- **Verify:** `cargo xtask bench-smoke` runs every new id once (it needs `gungraun-runner`; if it is missing on the host, run it in the devcontainer and say so in `log.md`); `cargo xtask ci --quick` passes; `cargo xtask deny` passes.
- **Commit:** `feat(spikes): benchmark the sparse-set backends (M3.3)`

### Step 10: documents (R15)

- [ ] **Who:** main session
- **Files:** `docs/roadmap.md`, `spikes/ecs_storage/README.md`, `CHANGELOG.md`.
- **Test first:** n/a for prose. The checks before the change: `grep -n "views-only and owning-group" docs/roadmap.md` and `grep -n sparse_set spikes/ecs_storage/README.md` find nothing.
- **Change:** spec §4.9: M3.3's new title; M3.8's notes (measurement caveat, change-detection ticks, time alone unless #41); the package row in the spike README; a CHANGELOG entry.
- **Verify:** both greps find their text; `cargo xtask ci` passes in full.
- **Commit:** `docs: record the sparse-set prototype (M3.3)`

## 3. Final verification

- `cargo xtask ci` passes locally on macOS. `cargo xtask bench-regress` is not needed: nothing under `libs/` or `bench/regress/` changes.
- One informal `cargo xtask spike-bench ecs` run under the benchmark run protocol (AC power, `caffeinate`, no builds during the run, the power log checked afterwards); its output goes in the pull request, labelled informal. A cache-bench run is added if the devcontainer is available; otherwise the pull request says it was not run.
- CI is green on Ubuntu, macOS and Windows with no workflow change (R14).
- The reviews of stage 5 are recorded in `log.md` and the pull request, and the spec's §9 list is ticked with evidence, each naming the commit it ran on (specs-4).

## 4. Lessons

From `lessons-researcher` (2026-11-19). `spikes/ecs_storage/**` matches no lesson's `applies_to`.

- **docs-1** followed: step 10 links to the spike README and spec instead of copying their rules.
- **docs-2** followed: the implementation counts (13 registrations, five, four and four conformance tests) were checked against spec §4.7; a builder who changes a count greps this plan for the old one.
- **specs-1** followed: step 10's "finds nothing" checks name their files.
- **specs-2** n/a: §8 asks for no `log.md` evidence; the bench output goes in the pull request (§3).
- **specs-3** followed: every panic of spec §4.2 and §4.5 has its own named test (steps 1 to 3), and the three no-required-type forms have one each.
- **specs-4** followed: §3's evidence names the commit it ran on.
- **specs-5** followed: step 1's crate docs cite EnTT's sparse-set and owning-group design.
- **xtask-1** n/a: step 1 only adds a name to `SPIKE_PACKAGES`; no tree-reading code changes.
- **testing-2** (path does not match; followed anyway): panic tests assert a message that names the operation (`WorldBuilder::group`, the query run), not a fragment that any panic could contain.

## 5. Deviations

- **Step 1, coverage:** `xtask/src/coverage.rs` lists the spike packages twice; the expected `--exclude` list in `test_args_match_the_spec` also gets the name, or that test fails.
- **Step 1, dependencies:** only `engine`; the harness, `spike_ecs_vec`, `proptest` and `agents_2d` arrive with the steps that use them (an unused dependency fails clippy).
- **Step 1, dead code:** the storage methods that later steps use from library code (`SparseSet::{swap, split, split_mut}`, `Packed::{swap, split_mut}`, `ErasedStorage::{index_of, swap, entities}`) are `#[cfg(test)]` until then; steps 2 and 3 remove the gates. No `#[allow(dead_code)]`.
- **Step 1, tests:** three tests beyond the plan (`erased_storage_works_through_the_trait`, `world_debug_shows_the_live_count_and_storages`, `world_reports_a_corrupt_storage`); the insert panic is "World::insert called on a despawned entity …" (testing-2), and its test expects that prefix.
- **Step 2, member access:** group maintenance reaches member storages one call at a time (`storages[m].index_of`, `.swap`), not with `get_disjoint_mut` as spec §4.4 says: no two member borrows are ever live at once, so the disjoint helper has nothing to do.
- **Step 2, `ComponentId`:** `pub` inside the private `world` module (unnameable outside the crate), because `seal::Group::register` names it and `pub(crate)` trips `private_interfaces`.
- **Step 2, `spawn` (maintainer request):** `World::spawn` increments the live count after `allocate` returns, so a failed allocation leaves `len` unchanged; tested by `a_failed_spawn_leaves_len_unchanged` (`GenIdAllocator::with_fixed_capacity(1)`), which failed first.
