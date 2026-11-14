# M3.3 plan: Sparse-set prototype

- **Status:** Draft (2026-11-14)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both.

## 1. Approach

Ten commits on branch `m3.3-sparse-set`, after the spec commit already on it. The crate is built from the bottom up, so each layer is tested before anything depends on it. First come the storage and an ungrouped `World`, together, because an unused `pub(crate)` storage would fail `clippy -D warnings`. Then groups with the model test, then queries on the view path, then the group path, then `Commands`. The workloads follow in three commits, both backends of each workload in the same commit as its tests. The bench registrations and the documents come last.

Choices the spec leaves to implementation:

- **One implementation per workload, two backends.** Each workload module defines a private-field `pub struct XOn<const GROUPED: bool> { pub(crate) world: World, … }` and `pub type SparseSetX = XOn<false>; pub type GroupedX = XOn<true>;`. `BACKEND` is `if GROUPED { "sparse_set_grouped" } else { "sparse_set" }`. `setup` declares the group only when `GROUPED`. Otherwise the code is shared, so the backends differ only in the group declaration, which is the comparison the spec wants. `frag_iter` has only `SparseSetFragIter`, a plain struct.
- **`Entity` and `EntityMarker`** are defined in `lib.rs`, so `storage.rs` and `world.rs` both use them without a cycle.
- **Query fetches borrow the storage's fields separately.** `SparseSet<T>` offers `pub(crate) fn split_mut(&mut self) -> (&SparseIndex, &[Entity], &mut [T])` and `pub(crate) fn split(&self) -> (&SparseIndex, &[Entity], &[T])`. The fetch of `&mut T` holds the three pieces. The driver's `&'w [Entity]` is a shared reference, so it is copied out before the loop, and the loop then borrows the fetch mutably once per entity. This is how §4.5's view path stays safe.
- **The borrow-splitting slots** are a `Vec<Option<&'w mut Box<dyn ErasedStorage>>>` with one slot per component type in declaration order (parameters first, then filters), filled by one `iter_mut().enumerate()` pass. That is one allocation per `query` call, which is not per entity, and is accepted.
- **A query with no required type** (for example `query::<Entity>()` or `query::<Option<&A>>()`) is a case the spec does not cover. The lean `HandleAllocator` cannot iterate live entities, so there is nothing to drive such a query. `query` and `query_ref` panic with "a query needs at least one required type (`&T`, `&mut T` or `With<T>`); `<Q>` has none". This adds a fourth query panic to R2; it is recorded in §4 Deviations as a spec addition.
- **The group-path flag.** When the group path is chosen, each member's fetch is marked `direct` at build time and reads index `i` without a probe. The flag is the same for every entity, so the branch predicts perfectly. Two monomorphized loops are not needed.

**Global constraints**

- Every commit passes `cargo xtask ci --quick`. Steps 1 and 9 also pass `cargo xtask deny`, with `deny.toml` unchanged (R14).
- The crate root has `#![forbid(unsafe_code)]` and the package `[lints] workspace = true`. Every public item is documented (R15). Doctests are not required (CONTRIBUTING.md "Spike crates").
- Package `spike_ecs_sparse_set`: dependencies `engine = { workspace = true }`, `agents_2d = { path = "../../../examples/agents_2d" }` (added in step 8, when `agents` first uses it; an unused dependency fails clippy), `spike_ecs_harness = { path = "../harness" }`; dev-dependencies `spike_ecs_vec = { path = "../vec" }`, `proptest = { workspace = true }`. `publish = false` through the workspace. The workspace `members` glob `spikes/*/*` already covers the directory.
- Conformance uses `Params { count: 1_000, seed: 42 }` (`count: 20` for `frag_iter`) and 120 ticks; `agents` uses 1,000 agents and 1,300 ticks (R10).
- Panic messages name the type with `core::any::type_name::<T>()` and the entity with its `Debug` form.
- Commit messages are conventional, end with ` (M3.3)`, and end with the session's attribution line. Stage by explicit path.
- Stop and ask the maintainer before the first push and before opening the pull request.

**Review focus** (inputs the spec implies but no requirement's test pins on its own; each has a test in the step that owns the code)

1. **A stale handle whose index was reused.** After `despawn(a)` and `spawn()` returns `b` with the same index, `get::<T>(a)`, `remove::<T>(a)` and `despawn(a)` must return `None`, `None` and `false` and leave `b`'s components alone, and `insert(a, …)` must panic. Step 1: `stale_handle_does_not_reach_the_reused_slot`.
2. **Removing the last or only entry.** Swap-remove of position `len - 1` swaps an entry with itself, and S1 must still hold. Step 1: `removing_the_last_and_the_only_entry_keeps_the_index_consistent`.
3. **Replacing an owned value.** `insert` of a member type that the entity already has must return the old value, leave the entity's position and the group length unchanged, and not swap anything. Step 2: `replacing_an_owned_value_keeps_the_group_length`.
4. **A type both queried and excluded.** `query::<&A, Without<A>>()` is a contradiction, and it must panic as a repeated type, not silently match nothing. Step 3: `a_type_in_a_parameter_and_a_filter_panics`.
5. **`count: 0`.** Every workload in both backends sets up with no entities, has checksum 0, and steps without panicking. The `query` of a never-created storage matches nothing, and `entity_churn` and `agents` spawn nothing. Steps 6 to 8: `zero_count_is_valid` for each workload.

## 2. Steps

### Step 1: storage and the ungrouped `World` (R1, R2 in part, R3, R4 for S1 to S3)

- **Files:** `spikes/ecs_storage/sparse_set/Cargo.toml`, `src/lib.rs`, `src/storage.rs`, `src/world.rs` (new), `Cargo.lock`.
- **Interfaces produced:**
  - `lib.rs`: `pub struct EntityMarker;`, `pub type Entity = Handle<EntityMarker>;`, `pub use world::World;`.
  - `storage.rs`:
    - `struct Packed<T>` with the methods of spec §4.3: `push`, `swap_remove(i) -> (Entity, T)`, `swap(i, j)`, `len`, `entities`, `data`, `data_mut`, `split_mut`.
    - `pub(crate) struct SparseIndex(Vec<u32>)` with `const EMPTY: u32 = u32::MAX`, `get(index: u32) -> Option<u32>`, `set(index: u32, position: u32)` and `clear(index: u32)`.
    - `pub(crate) struct SparseSet<T>` with `new`, `index_of(Entity) -> Option<usize>`, `insert(Entity, T) -> Option<T>`, `remove(Entity) -> Option<T>`, `swap(usize, usize)`, `get(Entity) -> Option<&T>`, `get_mut`, `split`, `split_mut` (§1) and `len`.
    - `pub(crate) trait ErasedStorage` (object-safe) with `index_of(&self, Entity) -> Option<usize>`, `swap(&mut self, usize, usize)`, `remove_entity(&mut self, Entity) -> bool`, `len(&self) -> usize`, `entities(&self) -> &[Entity]`, `check(&self, is_alive: &dyn Fn(Entity) -> bool) -> Result<(), String>` (S1 to S3, the message naming the invariant and the type), `as_any(&self) -> &dyn Any` and `as_any_mut(&mut self) -> &mut dyn Any`, implemented for `SparseSet<T: 'static>`.
  - `world.rs`: `World` with `new`, `spawn`, `despawn`, `is_alive`, `insert`, `remove`, `get`, `get_mut` and `check_invariants`, as in spec §4.2. Fields: `entities: HandleAllocator<EntityMarker>`, `ids: BTreeMap<TypeId, ComponentId>`, `storages: Vec<Box<dyn ErasedStorage>>`. `pub(crate) struct ComponentId(u32)`. `pub(crate) fn storage<T>(&self) -> Option<&SparseSet<T>>` and `storage_mut`, `pub(crate) fn id_or_create<T: 'static>(&mut self) -> ComponentId`. `Default` calls `new`.
- **Test first** (inline test modules):
  - `storage.rs`:
    - `packed_operations_keep_both_arrays_paired`: after push, push, push, `swap(0, 2)`, `swap_remove(0)`, `entities()` and `data()` have the same length and pair the same values.
    - `insert_then_remove_round_trips`; `insert_replaces_and_returns_the_old_value`.
    - `removing_the_last_and_the_only_entry_keeps_the_index_consistent` (review focus 2): `check` is `Ok` after each removal.
    - `index_past_the_sparse_end_is_absent`.
    - One corruption test per invariant (`s1_violation_is_reported`, `s2_violation_is_reported`, `s3_violation_is_reported`), each setting one private field directly; the message contains `"S1"`, `"S2"` or `"S3"`.
    - `zero_sized_values_do_not_allocate`: `SparseSet<Hidden>`'s data has capacity `usize::MAX` (the `Vec` capacity of a zero-sized type).
  - `world.rs`:
    - `components_are_independent_per_type`.
    - `despawn_removes_every_component`: after `despawn`, `get` of each type is `None`, and `check_invariants` is `Ok`.
    - `stale_handle_does_not_reach_the_reused_slot` (review focus 1).
    - `insert_on_a_despawned_entity_panics`: `#[should_panic(expected = "despawned")]`.
  - Run `cargo xtask test`: compile failure (the package does not exist).
- **Change:** implement the items above. The crate docs say what the crate is and point to the spec.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes; `cargo xtask deny` passes.
- **Commit:** `feat(spikes): add the sparse-set storage and World (M3.3)`

### Step 2: owning groups and the model test (R2 in part, R4 for G1 and G2, R5)

- **Files:** `src/world.rs`, `src/query.rs` (new: `GroupSpec` only), `src/lib.rs`, `src/tests.rs` (new).
- **Interfaces produced:**
  - `pub trait GroupSpec { fn members(world: &mut World) -> Vec<ComponentId>; fn names() -> Vec<&'static str>; }` for tuples of 2 to 4 `'static` types (a small macro; the members' storages are created by `id_or_create`).
  - `World::group<G: GroupSpec>(&mut self)`, with `#[track_caller]` and the three panics of spec §4.2. Messages: "`group` must be called before the first spawn"; "`<T>` already belongs to a group"; "`<T>` appears twice in the group".
  - `pub(crate) struct Group { members: Vec<ComponentId>, len: usize }`, `World::groups: Vec<Group>`, `World::owner: Vec<Option<usize>>` indexed by `ComponentId`.
  - `insert`, `remove` and `despawn` apply spec §4.4's steps. Mutable member access uses `<[_]>::get_disjoint_mut` on `storages` with the member indices, in a `match` over the 2, 3 and 4 member cases.
  - `check_invariants` also checks G1 and G2 (messages contain `"G1"` or `"G2"`).
  - `pub(crate) fn group_len(&self, group: usize) -> usize`, for tests.
- **Test first:**
  - `world.rs`:
    - `joining_and_leaving_a_group_moves_the_entity`: with group `(A, B)`, inserting `A` then `B` makes the group length 1; removing `A` makes it 0; `check_invariants` is `Ok` after each step.
    - `replacing_an_owned_value_keeps_the_group_length` (review focus 3).
    - `despawning_a_grouped_entity_shrinks_the_group`.
    - `group_after_spawn_panics`, `type_in_two_groups_panics` and `repeated_type_in_a_group_panics`, each `#[should_panic(expected = …)]` with the messages above.
    - `g1_violation_is_reported` and `g2_violation_is_reported` (corruption tests).
  - `tests.rs`: `world_matches_the_model` (proptest).
    - Four component types `C0(u32)` to `C3(u32)`. Up to 64 operations, each one of `Spawn`, `Despawn(k)`, `Insert(k, type, value)` or `Remove(k, type)`, where `k` picks an entity from those spawned so far (live or dead).
    - Three group setups: none, `(C0, C1)`, and `(C0, C1, C2)`.
    - The model is a `BTreeMap<u64, [Option<u32>; 4]>` keyed by `Entity::to_bits`.
    - After every operation, `check_invariants` is `Ok`, and `get` of every type for every entity ever spawned agrees with the model. `Insert` on a dead entity is skipped by the test, since it panics by design.
  - Run `cargo xtask test`: compile failure (`group` is not defined).
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add owning groups to the sparse-set World (M3.3)`

### Step 3: queries on the view path (R2 in part, R6, R8)

- **Files:** `src/query.rs`, `src/world.rs`, `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:**

  ```rust
  pub trait QueryParam {
      type Fetch<'w>;
      type Item<'a>;
      type Key: Copy;
      /// Appends this parameter's types: (TypeId, type name, required).
      fn types(out: &mut Vec<TypeEntry>);
      fn fetch<'w>(slots: &mut Slots<'w>) -> Self::Fetch<'w>;
      /// Offers this parameter's entities as the driver if it is required and smaller than `best`.
      fn offer_driver<'w>(fetch: &Self::Fetch<'w>, best: &mut Option<&'w [Entity]>);
      fn probe(fetch: &Self::Fetch<'_>, entity: Entity, index: usize) -> Option<Self::Key>;
      fn get<'a>(fetch: &'a mut Self::Fetch<'_>, key: Self::Key, entity: Entity) -> Self::Item<'a>;
  }
  pub trait ReadOnly: QueryParam {
      fn fetch_ref<'w>(slots: &mut RefSlots<'w>) -> Self::Fetch<'w>;
      fn get_ref<'w>(fetch: &Self::Fetch<'w>, key: Self::Key, entity: Entity) -> Self::Item<'w>;
  }
  pub trait Filter { /* types, fetch, fetch_ref, offer_driver (With only), matches(fetch, entity) -> bool */ }
  ```

  - `TypeEntry`, `Slots<'w>` and `RefSlots<'w>` are `pub` because they appear in the public traits, and their fields are private. `TypeEntry { id: TypeId, name: &'static str, required: bool }` is one component type of a query. `Slots<'w>` wraps the slot `Vec` of §1 with `pub(crate) fn take<T: 'static>(&mut self) -> Option<&'w mut SparseSet<T>>`, which takes the next slot in order and downcasts it; `None` means the storage does not exist. `RefSlots<'w>` is the same over `&'w dyn ErasedStorage`.
  - Impls: `&T`, `&mut T`, `Option<&T>`, `Option<&mut T>`, `Entity`; `With<T>`, `Without<T>` and `()`. One `macro_rules! tuples` generates the tuple impls of `QueryParam`, `ReadOnly` and `Filter` for `(P0,)` up to `(P0, P1, P2, P3)` from the list `P0 P1 P2 P3`.
  - `probe` for a required type is `sparse.get(entity.index())` and nothing else (R8). It returns the driver's `index` unprobed when the fetch is the driver, which the fetch knows by comparing its entity slice's pointer with the driver's at build time.
  - `pub struct With<T>(PhantomData<T>)` and `pub struct Without<T>(PhantomData<T>)`.
  - `World::query<Q: QueryParam, F: Filter = ()>(&mut self) -> Query<'_, Q, F>` and `World::query_ref<Q: ReadOnly, F: Filter = ()>(&self) -> QueryRef<'_, Q, F>`, both `#[track_caller]`.
  - `Query::for_each(self, f: impl FnMut(Q::Item<'_>))`; `QueryRef::for_each` and `QueryRef::iter(&self) -> impl Iterator<Item = Q::Item<'_>>`.
  - Both panic on a repeated type ("`<T>` appears twice in the query") and on a query with no required type (§1).
  - `pub use query::{Filter, Query, QueryParam, QueryRef, ReadOnly, With, Without};`.
- **Test first:**
  - `query.rs`:
    - `required_types_select_entities`: three entities with `{A, B}`, `{A}` and `{B}`; `(&A, &B)` visits only the first.
    - `option_yields_some_exactly_when_present`.
    - `with_and_without_filter`.
    - `entity_parameter_yields_the_entity`.
    - `nested_tuple_works`: `(Entity, (&mut A, &B, &C, Option<&D>))` visits the right entities and writes through `&mut A`.
    - `never_inserted_required_type_matches_nothing`.
    - `mutable_query_writes_are_visible`.
    - `iter_and_for_each_agree` (on `query_ref`).
    - `repeated_type_panics`, `a_type_in_a_parameter_and_a_filter_panics` (review focus 4) and `query_without_a_required_type_panics`, each `#[should_panic(expected = …)]`.
    - `driver_is_the_smallest_required_storage`: 100 entities with `A` and 2 with `B`. A query over `(&A, &B)` probes 2 entities, which a test-only probe counter (`#[cfg(test)]` `Cell<usize>` on the `Query`) confirms.
  - `tests.rs`: `world_matches_the_model` also compares, after every operation, the entity sets of `(Entity, &C0, &C1)`, `(Entity, &C0)` with `Without<C2>`, and `(Entity, Option<&C3>)` with `With<C1>` with the model's, as sorted `Vec<u64>` of `to_bits`.
  - Run `cargo xtask test`: compile failure.
- **Change:** as above. If closure inference over `Q::Item<'_>` fails for a nested tuple at the call site, the fix is a named helper in the test, not a change to the trait. Record it in §4 if it happens.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add typed tuple queries to the sparse-set World (M3.3)`

### Step 4: the group path (R7)

- **Files:** `src/query.rs`, `src/world.rs`.
- **Interfaces produced:** `Query::uses_group(&self) -> bool` and `QueryRef::uses_group(&self) -> bool`. Group selection follows spec §4.5: the required types include every member; if several groups qualify, the most members wins, then the first declared. Member fetches are re-sliced to `..group.len` and marked `direct`. The driver is the first member's entity slice, re-sliced to `..group.len`.
- **Test first** (`query.rs`):
  - `exact_match_takes_the_group_path`: group `(A, B)`, query `(&mut A, &B)`.
  - `superset_takes_the_group_path_and_probes_the_rest`: query `(&A, &B, &C)` visits only the entities with `C`.
  - `strict_subset_takes_the_view_path`: query `&A` with group `(A, B)` does not use the group and visits entities that have only `A`.
  - `the_largest_qualifying_group_wins`: groups `(A, B)` and `(C, D, E)`, query `(&A, &B, &C, &D)` with `With<E>`; the group path is taken over `(C, D, E)`, which a test-only accessor confirms.
  - `group_path_and_view_path_visit_the_same_entities`: a proptest over random populations of `A`, `B` and `C`. One world has group `(A, B)` and one has none. They are fed the same operations, and `(Entity, &A, &B)` yields the same sorted entity set in both.
  - Run `cargo xtask test`: compile failure (`uses_group` is not defined).
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): iterate owning groups by aligned prefix (M3.3)`

### Step 5: `Commands` (R9)

- **Files:** `src/world.rs` (or `src/commands.rs` if `world.rs` passes about 600 lines), `src/lib.rs`.
- **Interfaces produced:** `pub struct Commands` and `World::apply(&mut self, commands: &mut Commands)`, as in spec §4.6. The `Deferred` closures for `insert` and `remove` check `is_alive` before acting; `apply` drains with `Vec::drain` so the capacity is kept. `Default` calls `new`.
- **Test first:**
  - `commands_run_in_recorded_order`: insert `A`, then remove `A`, on one entity: `A` is absent after `apply`.
  - `command_on_a_dead_entity_is_skipped`: despawn `e`, then insert `A` into `e`: `apply` does not panic, and `check_invariants` is `Ok`.
  - `apply_empties_the_buffer_and_keeps_its_capacity`.
  - `commands_maintain_groups`: with group `(A, B)`, inserting `B` through `Commands` makes the entity join the group.
  - Run `cargo xtask test`: compile failure.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add a command buffer to the sparse-set World (M3.3)`

### Step 6: `simple_iter`, `frag_iter` and `heavy_compute` (R10, R11 in part)

- **Files:** `src/simple_iter.rs`, `src/frag_iter.rs`, `src/heavy_compute.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:** `simple_iter::{SimpleIterOn, SparseSetSimpleIter, GroupedSimpleIter}`, `frag_iter::SparseSetFragIter` and `heavy_compute::{HeavyComputeOn, SparseSetHeavyCompute, GroupedHeavyCompute}`, built as spec §4.7 and §1 say.
  - `frag_iter`'s `setup` inserts the letter as its own type, with a local macro over the 26 `Letter` variants. Its checksum runs one `query_ref::<(&L, &Data)>()` per letter type and hashes `Letter::from_index(index, l.0)`.
- **Test first** (`tests.rs`):
  - one `<workload>_<backend>_matches_the_reference` conformance test per implementation (five tests)
  - `grouped_simple_iter_uses_the_group` and `grouped_heavy_compute_uses_the_group`: the workload's step query on its `world` reports `uses_group()`
  - `zero_count_is_valid` for each of the five (review focus 5)
  - Run `cargo xtask test`: compile failure.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the iteration workloads to the sparse-set backends (M3.3)`

### Step 7: `add_remove` and `entity_churn` (R10, R11 in part, R12 in part)

- **Files:** `src/add_remove.rs`, `src/entity_churn.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:** `add_remove::{AddRemoveOn, SparseSetAddRemove, GroupedAddRemove}` and `entity_churn::{EntityChurnOn, SparseSetEntityChurn, GroupedEntityChurn}`, as spec §4.7 says.
  - `add_remove` keeps a reused `Vec<(Entity, f32)>` and `Vec<Entity>` in the struct.
  - `entity_churn` keeps a `Commands`, the `Prng`, `next_id` and `count`, and respawns with direct `spawn` and `insert` after `apply`.
- **Test first** (`tests.rs`):
  - the four conformance tests
  - `zero_count_is_valid` for the four
  - `grouped_entity_churn_uses_the_group`
  - `grouped_add_remove_takes_the_view_path`: both queries report `!uses_group()`, as spec R11 says
  - `entity_churn_keeps_invariants_every_tick`: both backends, 1,000 entities, 120 ticks, `check_invariants` after every step
  - Run `cargo xtask test`: compile failure.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the structural workloads to the sparse-set backends (M3.3)`

### Step 8: `sprites` and `agents` (R10, R11, R12)

- **Files:** `src/sprites.rs`, `src/agents.rs` (new), `src/lib.rs`, `src/tests.rs`, `Cargo.toml` (`agents_2d`), `Cargo.lock`.
- **Interfaces produced:** `sprites::{SpritesOn, SparseSetSprites, GroupedSprites}` and `agents::{AgentsOn, SparseSetAgents, GroupedAgents}`, as spec §4.7 says.
  - `sprites` owns the draw list, reserved to `count` in `setup`.
  - `agents` defines `pub struct AgentId(pub u64)`, `pub struct Velocity(pub Vec2)` and `pub struct Ttl(pub u32)`, and keeps a `Commands`, the `Prng`, `next_id`, `tick` and the target.
  - Within one agent's closure, the recording order is: seeker change first, then `despawn` if the ttl reached 0. A caught-and-expired agent's `remove` then runs before its `despawn`, and a bounced-and-expired agent's `insert` likewise.
- **Test first** (`tests.rs`):
  - the four conformance tests (`agents`: 1,000 agents, 1,300 ticks)
  - `zero_count_is_valid` for the four
  - `grouped_sprites_moves_on_the_group_path_and_draws_on_the_view_path`
  - `grouped_agents_uses_the_group`
  - `agents_keeps_invariants_every_tick`: both backends, 1,000 agents, 1,300 ticks
  - Run `cargo xtask test`: compile failure.
- **Change:** as above.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the sprite and agent workloads to the sparse-set backends (M3.3)`

### Step 9: bench registrations (R13)

- **Files:** `spikes/ecs_storage/bench_compare/Cargo.toml`, `benches/compare.rs`, `spikes/ecs_storage/bench_cache/Cargo.toml`, `benches/cache.rs`, `Cargo.lock`.
- **Test first:** n/a for registrations. The check before the change: `cargo xtask bench-smoke` output has no `sparse_set` id.
- **Change:**
  - Add the dev-dependency `spike_ecs_sparse_set = { path = "../sparse_set" }` to both bench crates.
  - `compare.rs` gets one `bench::<…>(c);` line per implementation (13). `cache.rs` gets one `cache_bench!(<workload>_<backend>, <Type>, [<sizes>]);` line per implementation, with each workload's `Spec::SIZES` written out (`simple_iter`, `add_remove` and `entity_churn`: `[1_000, 10_000, 100_000]`; `frag_iter`: `[20]`; `heavy_compute`: `[1_000]`; `sprites`: `[100_000]`; `agents`: `[10_000, 100_000]`), and each function is named in its workload's group.
- **Verify:** `cargo xtask bench-smoke` runs every new id once; `cargo xtask ci --quick` passes; `cargo xtask deny` passes.
- **Commit:** `feat(spikes): benchmark the sparse-set backends (M3.3)`

### Step 10: documents (R15)

- **Files:** `docs/roadmap.md`, `spikes/ecs_storage/README.md`, `CHANGELOG.md`.
- **Test first:** n/a for prose. The check before the change: `grep -n "M3.11" docs/roadmap.md` finds nothing.
- **Change:** the text of spec §4.9. The M3.3 row's status becomes `In progress`, then `Done (PR #n)` in the close-out commit.
- **Verify:** `cargo xtask ci` passes in full (`typos`, `doc` and the rest).
- **Commit:** `docs: record the sparse-set prototype and plan M3.11 (M3.3)`

## 3. Final verification

- `cargo xtask ci` passes locally on macOS. `cargo xtask bench-regress` is not needed, because no file under `libs/` or `bench/regress/` changes.
- One informal `cargo xtask spike-bench ecs` run, under the benchmark run protocol (AC power, `caffeinate`, no builds during the run, power log checked afterwards). Its output goes in the pull request, labelled informal. A cache-bench run is added if the devcontainer is available; otherwise the pull request says it was not run.
- The pull request's CI is green on Ubuntu, macOS and Windows with no workflow change (R14).
- The eight reviews of CONTRIBUTING.md are recorded in the pull request, and the spec's §9 checklist is ticked with evidence.
- Once the checks pass, in the same pull request: the spec becomes `Implemented (PR #n, date)`, this plan `Historical`, the roadmap row `Done (PR #n)`, and §4 is filled in.

## 4. Deviations

- **Spec addition, decided while planning:** a query with no required type panics (§1). Spec §4.5 does not cover the case, and the lean allocator cannot drive such a query.
