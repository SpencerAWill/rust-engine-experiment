# M3.19: Dense per-entity component arrays

- **Status:** Accepted (2026-11-20)
- **Milestone:** M3 (ECS storage decision) · **Depends on:** M3.2, M3.17 · **Decides:** no ADR
- **Issue:** #79 (tracking issue #78)
- **Estimated size:** about 1,800 changed lines, excluding tests, lockfiles and generated files (`column` about 200, `world` about 300, `query` about 550, `commands` about 150, the workload modules about 450, documents about 150)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds `spike_ecs_dense`, a minimal safe ECS on dense per-entity arrays: each component type keeps one slot per entity index and a presence `HierarchicalBitSet`, insert and remove write one slot and one bit, and typed tuple queries run `engine::collections::Join` over the presence sets of their types. It comes before the hybrid (M3.5) so that M3.8 does not decide between sparse sets, archetypes and the hybrid without the simplest model, and so that M3.5 can treat dense arrays as one of its per-component choices. At the end, the `dense` backend passes conformance against `vec_aos` for all seven workloads, and `cargo xtask spike-bench ecs` and the cache bench list it beside the other backends.

## 2. Goals and non-goals

**Goals**

- A column whose values and presence set cannot fall out of step, because one private type owns both and offers only operations that change them together (§4.3).
- Insert and remove that move nothing: one slot write and one bit, plus the type lookup (§4.2).
- Typed tuple queries with the same parameters, filters and names as M3.3's and M3.4's (§4.5), driven by `Join` over the required and excluded presence sets.
- `Commands` as in M3.3 (§4.6).
- Invariant checks and a model test that cover columns, despawn and commands (§8).

**Non-goals**

- One backend only, `dense`. No variant on a flat `BitSet`, no paged columns.
- No bundle spawn: `spawn` then one `insert` per component, as in M3.3 *(maintainer, from the brainstorm)*. An insert moves nothing here, so a bundle would save only the per-type lookups.
- No `T`-sized value array: absent slots are `None` in a `Vec<Option<T>>` *(maintainer, from the brainstorm)*. `MaybeUninit` needs `unsafe` (M3.7); a filler value needs a `Default` bound the harness's types lack.
- No query state cached between runs, no shrinking of columns, no runtime-registered components (M3.6), no `unsafe` (M3.7), no parallel iteration (M7), no change detection (M6), no memory measurement (#41).
- No query layer or code shared with the other spike crates; names and syntax match by convention (M3.3 spec §4.5, §7). The query and command modules MAY be adapted from M3.3's or M3.4's, which are this repository's own code.
- No results in `docs/benchmarks.md`: that is M3.8's. No change to `libs/`, the harness, the other backends, or any frozen format.

## 3. Guide-level explanation

```rust
use spike_ecs_dense::{Commands, Entity, World};

#[derive(Clone, Copy)] struct Position(f32);
#[derive(Clone, Copy)] struct Velocity(f32);
struct Frozen;

let mut world = World::new();

let e = world.spawn();
world.insert(e, Position(0.0));                             // slot e.index() of Position's column, one bit
world.insert(e, Velocity(1.0));
let f = world.spawn();
world.insert(f, Position(5.0));

world
    .query_mut::<(&mut Position, &Velocity)>()
    .without::<Frozen>()
    .for_each(|(p, v)| p.0 += v.0);                         // Join over {Position, Velocity}, none excluded

assert_eq!(world.get::<Position>(e).map(|p| p.0), Some(1.0));
assert_eq!(world.query::<Entity>().iter().count(), 2);      // every live entity
```

Structural changes inside a query go through `Commands`, as in M3.3:

```rust
let mut commands = Commands::new();
world
    .query::<(Entity, &Position)>()
    .for_each(|(e, p)| if p.0 > 10.0 { commands.despawn(e) });
world.apply(&mut commands);
```

The harness backend is `DenseX` for each workload `X`:

```rust
spike_ecs_harness::check_conformance::<spike_ecs_vec::simple_iter::VecSimpleIter,
                                       spike_ecs_dense::simple_iter::DenseSimpleIter>(
    Params { count: 1_000, seed: 42 },
    120,
)
.unwrap();
```

## 4. Reference-level design

### 4.1 Layout

```
spikes/ecs_storage/dense/          # package spike_ecs_dense (library)
  src/lib.rs                       # re-exports World, Entity, Commands, InvariantViolation, With, Without, query types
  src/column.rs                    # DenseColumn<T>, ValuesMut, ErasedColumn
  src/world.rs                     # World, ComponentId, InvariantViolation
  src/query.rs                     # QueryParam, ReadOnly, Filter (sealed), Query, QueryMut, QueryIter, the tuple macro
  src/commands.rs                  # Commands, World::apply
  src/{simple_iter, frag_iter, heavy_compute, add_remove, entity_churn, sprites, agents}.rs
  src/tests.rs                     # conformance, invariants under churn, the model test, query tests
```

The package follows the spike-crate rules in [spikes/README.md](../../../spikes/README.md): `publish = false` through the workspace, the workspace lints, `#![forbid(unsafe_code)]`, documented public items. It depends on `engine` and `spike_ecs_harness`, and has `spike_ecs_vec` and `proptest` as dev-dependencies. The workload modules are `pub mod`, as in M3.4, so the bench crates can name their types.

### 4.2 `World`

```rust
pub struct EntityMarker;
pub type Entity = GenId<EntityMarker>;

impl World {
    pub fn new() -> Self;                       // also `Default`
    pub fn spawn(&mut self) -> Entity;
    pub fn despawn(&mut self, entity: Entity) -> bool;
    pub fn is_alive(&self, entity: Entity) -> bool;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn insert<T: 'static>(&mut self, entity: Entity, value: T) -> Option<T>;
    pub fn remove<T: 'static>(&mut self, entity: Entity) -> Option<T>;
    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T>;
    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T>;
    pub fn query<Q: ReadOnly>(&self) -> Query<'_, Q>;
    pub fn query_mut<Q: QueryParam>(&mut self) -> QueryMut<'_, Q>;
    pub fn apply(&mut self, commands: &mut Commands);
    pub fn check_invariants(&self) -> Result<(), InvariantViolation>;
}

pub struct InvariantViolation { /* message */ }     // Display, Error
```

- **Entities** come from `engine::genid`'s lean `GenIdAllocator<EntityMarker>`. `spawn` MUST panic as M3.3's does when the allocator is full: `#[track_caller]`, a `# Panics` section, through `try_allocate`, with a message that starts with `World::spawn`, and with the world unchanged.
- **Per-index tables.** The world keeps `entities: Vec<Entity>`, indexed by `entity.index()` and written by `spawn`, because a query yields indices and the lean allocator cannot turn an index back into an ID; and `alive: HierarchicalBitSet`, the indices of the live entities, which drives a query with no required type (§4.5). A slot of `entities` whose bit in `alive` is clear holds a stale ID and is never read. *(Main session's call.)*
- **Live count.** `len` is a counter kept by `spawn` and `despawn`, as in M3.3 (§4.2 there), incremented only after allocation succeeds.
- **Component identity.** A `BTreeMap<TypeId, ComponentId>` maps each type to a dense `ComponentId(u32)`, the index of its column in `Vec<Box<dyn ErasedColumn>>`. A column is created on the first `insert` of its type. Determinism is as M3.3's (§6).
- **No `WorldBuilder`**, as in M3.4: nothing is declared before the first spawn.
- `insert` writes the value into slot `entity.index()` of `T`'s column and sets the bit, returning the replaced value or `None`. It MUST panic on a dead entity, as M3.3's does: `#[track_caller]`, a `# Panics` section, a message naming the entity and saying it was despawned.
- `remove` takes the slot's value, clears the bit, and returns the value; `remove`, `get` and `get_mut` return `None` for a dead entity or a missing component.
- `despawn` returns `false` for a dead entity. Otherwise it removes the entity's index from every column (one `dyn` call per column, dropping any value), clears its bit in `alive`, frees the ID and decrements `len`. Its cost is one call per registered component type, not per component the entity has. *(Main session's call: a per-entity component mask would make despawn proportional to the entity's own components, but adds a second record of presence to keep in step; no workload despawns in a world with many types.)*
- `check_invariants` checks §4.3's and §4.4's invariants and returns the first violation; `InvariantViolation` has the shape of M3.3's (a private message, `Debug`, `Clone`, `PartialEq`, `Eq`, `Display` as "invariant violated: …", `std::error::Error`).
- **Common traits.** `World` and `Commands` implement `Default` and `Debug`. `World`'s `Debug` shows the live count and each column's type name and number of present values, not every value. Every other public type implements `Debug`.

### 4.3 Columns

```rust
pub(crate) struct DenseColumn<T> { present: HierarchicalBitSet, slots: Vec<Option<T>> }   // fields private to the column module
pub(crate) struct ValuesMut<'a, T>(&'a mut [Option<T>]);                                     // field private
pub(crate) trait ErasedColumn { /* object-safe */ }
```

- **`DenseColumn<T>`** owns a type's values and presence set. Its operations each change both or neither:
  - `insert(index, value) -> Option<T>` grows `slots` with `None` to `index + 1` if it is shorter, stores the value and sets the bit;
  - `remove(index) -> Option<T>` takes the value, if any, and clears the bit; an index past the end is absent;
  - `get(index) -> Option<&T>` and `get_mut(index) -> Option<&mut T>`;
  - `present() -> &HierarchicalBitSet`, and `split_mut() -> (&HierarchicalBitSet, ValuesMut<'_, T>)` for mutable queries.
  
  No code outside the column module can reach the fields.
- **`ValuesMut`** offers only `get(index) -> Option<&mut T>` (reborrowing). It cannot write `None` into a slot or change its length, so a mutable query cannot break D1 (lesson ecs-5). `Option<&mut T>` parameters use the same method.
- **Growth.** `slots` grows on insert and never shrinks; its length is one more than the largest index ever inserted into that column, at most the allocator's slot count. The presence set grows by its own `insert`.
- **`ErasedColumn`** is what code that does not know `T` uses: `remove_index(index)` (drops the value, if any), `present`, `type_id`, `type_name`, `present_count`, `as_any`, `as_any_mut`. Typed code downcasts once per call or per query (one `TypeId` comparison).
- Zero-sized tags such as `Hidden` get an ordinary `DenseColumn<Hidden>`: `Option<Hidden>` is one byte per slot.

**Column invariants**, checked by `check_invariants`:

- D1 (presence): for every column and every index `i`, bit `i` of `present` is set exactly when `slots[i]` is `Some`; no bit is set at or past `slots.len()`.
- D2 (type): column `c`'s `type_id` is the `TypeId` the map assigns to `ComponentId(c)`, and the map's ids are exactly `0..columns.len()`.

### 4.4 World invariants

- W1 (liveness): every index set in any column's `present` is set in `alive`.
- W2 (live set): `alive` has exactly `len()` members, and for each member `i`, `entities[i]` has index `i` and is alive in the allocator.

With D1, W1 means a despawned entity leaves no value behind. With W2, the `Entity` a query yields for index `i` is the live entity at `i`.

### 4.5 Queries

**Same surface as M3.3 and M3.4** (M3.3 spec §4.5, M3.4 spec §4.6): `QueryParam` for `&T`, `&mut T`, `Option<&T>`, `Option<&mut T>`, `Entity` and tuples of 1 to 4 `QueryParam`s, nested; `ReadOnly` for those without `&mut`; `Query<'w, Q, F = ()>` and `QueryMut<'w, Q, F = ()>` with chained `.with::<T>()` and `.without::<T>()`; `Filter` for `()`, `With<T>`, `Without<T>` and pairs. `QueryParam`, `ReadOnly` and `Filter` are sealed, each by its own private supertrait, with the item GAT on the seal trait (lesson ecs-3). `&T`, `&mut T` and `With<T>` are required; `Option<…>` and `Without<T>` are not.

**Running a query.** `for_each` on both types, and `iter` and `IntoIterator for &Query`, are `#[track_caller]` and MUST:

1. Look up each type's `ComponentId`. If a required type has no column, the query matches nothing. An optional or excluded type with no column is ignored.
2. Panic if a type appears twice among the parameters and filters, with a message naming the type, as M3.3 does.
3. Collect the required types' presence sets and the excluded types' presence sets. If no type is required, the required list is `[&alive]`. *(Main session's call: as in M3.4, the correct behaviour costs nothing here, and `Join` needs at least one required set.)*
4. Iterate `Join::new(&required, &excluded)`. For each index `i` it yields, fetch every parameter at `i` and call the closure (or yield the item). `&T` and `&mut T` read their column's slot `i`, which D1 guarantees is `Some`: the fetch MUST panic with a message naming D1 if it is not, rather than skip the entity. `Option<…>` reads slot `i` if it is within the column, else `None`. `Entity` reads `entities[i]`.
5. For `QueryMut`, borrow the columns in one `iter_mut().enumerate()` pass over the column list, as M3.3 does over its storages, calling `split_mut` on each column the query names, so the presence sets are shared while the values are borrowed through `ValuesMut`.

**Iteration.**

- `QueryMut::for_each(self, f: impl FnMut(Q::Item<'_>))`. Each call gets fresh reborrows, so several `&mut` from different columns are sound without `unsafe`. A safe external iterator cannot prove the `&mut` items it yields disjoint, so mutable queries have no `iter`, as in M3.3.
- `Query::for_each(self, f)`, `Query::iter(&self) -> QueryIter<'_, 'w, Q, F>` and `IntoIterator for &'q Query<'w, Q, F>`. `QueryIter<'q, 'w, Q, F>` yields `Q::Item<'w>` and borrows the `Query` for `'q`, because `Join` borrows the slices of sets and an iterator that owned both the slices and the join would refer to itself. So `Query` resolves its sets (steps 1 and 3) when it is built and as each filter is added, and there is no `IntoIterator for Query` by value; no workload uses it. *(Main session's call; M3.3 and M3.4 have it.)* `QueryIter` implements `FusedIterator`; its `size_hint` is `(0, Some(n))`, `n` the smallest required set's length, counted when `size_hint` is called (`HierarchicalBitSet::len` scans layer 0), so a run that never asks pays nothing. The repeated-type check of step 2 runs in the methods that run the query, not when it is built, so its message names the method (`Query::iter: …`), as in M3.4. *(Main session's calls, from the grill.)*
- `Query`, `QueryMut` and `QueryIter` are `#[must_use]`.
- The closure and every fetch are monomorphized; nothing per entity goes through `dyn`. Per-query setup (steps 1 to 3 and 5) is the only erased work. Per-run allocations (the two lists of sets, at least) are counted as built, with a scratchpad binary that depends on the crate and installs a counting allocator (no `dhat` dependency, so R11's `deny.toml` stays unchanged), and recorded in the plan (lesson ecs-2).
- `pub(crate)`, `#[cfg(test)]` `Query::joined()` and `QueryMut::joined()` return the `ComponentId`s of the required and excluded sets the query would join (with a marker for the `alive` set, its form the builder's choice), so the crate's tests can check how each step query is driven; they are not public API.

### 4.6 Commands

As M3.3's (its spec §4.6): `Commands::{new, despawn, insert, remove, len, is_empty}`, `Default`, `Debug`; `Despawn(Entity)` unboxed, `insert` and `remove` one boxed closure each. `World::apply` MUST run the commands in recorded order, skip a command whose entity is dead when it runs, and leave the buffer empty with its capacity kept. No `spawn` command: every workload spawns outside its queries.

### 4.7 The backend

`BACKEND` is `"dense"` for every `DenseX` type. In every workload, `setup` spawns entities `0..n` in order, inserting each entity's components in the order the harness's `spawn` returns them; respawns in `entity_churn` and `agents` do the same; `checksum` folds a `query` with the harness's hash; the harness's `update` functions MUST be the ones called. The step queries are M3.3's `sparse_set` column (its spec §4.7), with the sets they join:

| Workload | Step | Joined after setup (required / excluded) |
|---|---|---|
| `simple_iter` | `(&mut Position, &Velocity)` | Position, Velocity / none |
| `frag_iter` | `&mut Data`; the checksum runs one `query::<(&L, &Data)>()` per letter type `L` | Data / none |
| `heavy_compute` | `(&mut Matrix, &mut Position, &Rotation)` | Matrix, Position, Rotation / none |
| `add_remove` | reads `(Entity, &A)` into a reused buffer and inserts `B` with `world.insert`; then reads `query::<Entity>().with::<B>()` the same way and removes `B` | A / none; then B / none |
| `entity_churn` | `(Entity, (&mut Position, &Velocity, &mut Lifetime))`, recording `commands.despawn(e)` for expired entities; `apply`; then spawn until `world.len()` is `count` | Position, Velocity, Lifetime / none |
| `sprites` | move: `(&mut Transform2d, &Velocity2d)`; draw: `query::<(&Transform2d, &Sprite)>().without::<Hidden>()` pushing to the owned draw list | Transform2d, Velocity2d / none; Transform2d, Sprite / Hidden |
| `agents` | `(Entity, (&mut Transform2d, &mut Velocity, &mut Ttl, Option<&mut Seeker>))`, as M3.3's: write back a kept seeker, record `insert`/`remove` of `Seeker` when it appeared or went away, `despawn` when the ttl reaches 0; `apply`; then spawn until `world.len()` reaches the target | Transform2d, Velocity, Ttl / none |

- `agents` uses its own `AgentId(u64)`, `Velocity(Vec2)` and `Ttl(u32)` newtypes, as M3.3's does, and builds the harness's agent per entity for `agent_hash` in its checksum.
- `sprites` stores `Hidden` as a zero-sized component on the hidden entities. `add_remove` uses no `Commands`, for M3.3's reason (its spec §4.7).
- Each step query is one private function used by `step` and by the test that checks its joined sets (lesson ecs-1).

**Expected, unmeasured**, for M3.8 (lesson docs-3):

- `add_remove` and the respawns in `entity_churn` and `agents` are expected to be this model's best case: no swap, no row move, one slot and one bit per component.
- The iteration workloads are expected to be slower than the archetype tables and the sparse-set group path: `Join` yields one index at a time, every required fetch checks its `Option`, and the loop does not vectorize. Every entity has every component, so the joined words are all ones and the hierarchy skips nothing.
- `frag_iter`'s `&mut Data` joins one set over all 520 entities, so the 26 letter types cost nothing in the step; they cost one column each on every `despawn` (none in this workload) and in memory.
- Memory is this model's weakness: a slot per entity index per type, and `Option<T>` is wider than `T` when `T` has no niche (`Position(f32)` takes 8 bytes). It shows only in space numbers (#41); M3.8 reads time with that caveat.

### 4.8 Deferred

| Idea | Why not now | Where |
|---|---|---|
| A `T`-sized value array (`MaybeUninit`) behind the presence set | Needs `unsafe` | M3.7, if dense leads |
| Bundle spawn | Maintainer, from the brainstorm: an insert moves nothing | Not planned |
| A per-entity component mask, so despawn visits only the entity's own columns | No workload despawns in a world with many types | M4, if dense is chosen |
| Paged columns (one page per 2^k indices, allocated when first written) | Lowers memory for sparse types; this chunk measures the plain model first | M4, or a spike after #41 |
| Cached query state | The other backends also rebuild per run; ecs-2's count records the cost | M4 |
| `IntoIterator for Query` by value | Would be self-referential without `unsafe` (§4.5) | M4's API |
| Per-component choice between dense, sparse set and table | That is the hybrid | M3.5 |
| Change-detection ticks next to each value | Adds memory traffic to every storage model alike | Noted for M3.8; built in M6 |

### 4.9 Documents

- **`docs/roadmap.md`:** M3.19's row is `Done (PR #n)` in the close-out commit, and the costs found while building it are added to M3.8's row as M3.4's were.
- **`spikes/ecs_storage/README.md`:** a row for `spike_ecs_dense` in the package table.
- **`CHANGELOG.md`:** an entry.
- **`xtask/src/coverage.rs`:** `spike_ecs_dense` joins `SPIKE_PACKAGES` and the expected `--exclude` list of `test_args_match_the_spec`.
- **`src/lib.rs`'s module docs** cite the designs followed (§11), with no code taken (ADR 0002, lesson specs-5).

## 5. Requirements

- **M3.19-R1** `spikes/ecs_storage/dense` MUST be a workspace member named `spike_ecs_dense`, and its crate root MUST carry `#![forbid(unsafe_code)]`.
- **M3.19-R2** `World` MUST provide the API of §4.2, the query types that of §4.5 and `Commands` that of §4.6, including `Default`, `Debug`, the sealed traits and `#[must_use]`, and the panics of `spawn`, `insert` and the methods that run a query, each with `#[track_caller]`, a `# Panics` section and a message that names the method and the type or entity.
- **M3.19-R3** A column's values and presence set MUST be owned by one type whose operations change both together, and no other code MUST be able to reach them; the mutable access it hands to queries MUST NOT be able to clear a slot or change a length.
- **M3.19-R4** `check_invariants` MUST report a violation of each of D1, D2, W1 and W2, and MUST return `Ok` after any sequence of public operations.
- **M3.19-R5** `insert` and `remove` MUST change only the given entity's slot and bit in the given type's column, and `despawn` MUST leave no value of the entity in any column.
- **M3.19-R6** Queries MUST match exactly the entities that have every required type and none of the `Without` types, in ascending index order, yield `Some` for an `Option` parameter exactly when the entity has the type, support nested tuples, and, with no required type, visit every live entity that passes the filters.
- **M3.19-R7** `apply` MUST run commands in recorded order, skip commands on dead entities, and leave the buffer empty.
- **M3.19-R8** The `dense` backend MUST implement all seven workloads as §4.7 says and MUST pass `check_conformance` against `vec_aos` for each: 1,000 entities (20 per letter for `frag_iter`) over 120 ticks, and 1,000 agents over 1,300 ticks for `agents`. Each step query MUST join the sets §4.7 says (`joined`, §4.5).
- **M3.19-R9** Stepping `entity_churn`, `add_remove` and `agents` MUST keep `check_invariants` at `Ok` after every tick, for as many ticks as R8.
- **M3.19-R10** Both bench crates MUST register every implementation of R8, as `spikes/ecs_storage/README.md` describes.
- **M3.19-R11** `cargo xtask ci` MUST pass, CI MUST be green on Ubuntu, macOS and Windows without workflow changes, and `cargo xtask deny` MUST pass without changes to `deny.toml`.
- **M3.19-R12** Every public item MUST be documented, and the documents of §4.9 MUST be updated as described.

## 6. Cross-cutting concerns

- **Determinism:** component ids follow first-insert order, entity indices come from the allocator, and every query visits indices in ascending order, so iteration depends only on the sequence of operations and a run is reproducible within a build ([ADR 0003](../../adr/0003-same-build-determinism.md)). Checksums are order-independent wrapping sums. The `TypeId` map is a `BTreeMap`; nothing observable iterates it. No `HashMap`, ambient state or transcendental function; the workspace lints enforce this.
- **`unsafe`:** none; the crate forbids it. The safe-Rust costs it accepts (`Option<T>` slots, a checked fetch per required parameter per entity, a downcast per column per query, the `enumerate` pass to split column borrows, `for_each` for mutable queries) are part of what M3.7 measures.
- **Concurrency:** n/a, because every workload is single-threaded. Parallel iteration is M7's.
- **Heap allocation:** `setup` allocates the columns and sets. In the steady state, entity indices are reused, so columns and sets do not grow and inserts and removes do not allocate; `agents` allocates one box per seeker command. Per-query-run allocations are counted as built (lesson ecs-2). Not measured here; #41 adds space measurement.
- **`no_std` and wasm:** n/a, because spike crates are not in those builds.
- **Frozen formats:** none.
- **Public API changes:** none in `libs/`. The crate's API is used only by its own workload modules, tests and the bench crates.
- **FFI exposure:** n/a.
- **Performance:** the pull request shows one informal `cargo xtask spike-bench ecs` run, and a cache-bench run if a Valgrind host is available, labelled as informal. M3.8 records results under the benchmark run protocol.
- **Documentation:** item docs, the roadmap, the spike README and CHANGELOG (§4.9), and this spec and its plan.
- **Policy changes:** none.

## 7. Alternatives considered

- **`Vec<T>` with a filler value in absent slots.** Rejected by the maintainer: needs a `Default` (or similar) bound that the harness's component types do not all have, so it would change the harness.
- **`Vec<Option<T>>` alone, with queries scanning the shortest required column.** Rejected by the maintainer: it does not measure `Join`, which #79 asks for, and a scan visits absent slots one by one where the hierarchy skips empty ranges.
- **Bundle spawn, as M3.4.** Rejected by the maintainer (§2).
- **A flat `BitSet` per type, intersected word by word.** Not built: on these workloads, where nearly every entity has every component, a flat AND might match the hierarchy, but #79 names `HierarchicalBitSet` and `Join`, which M3.17 built for this. A flat variant would be a second backend.
- **A per-entity component mask for despawn.** Rejected for now (§4.2, §4.8).
- **`IntoIterator for Query` by value, with the `Join` rebuilt inside the iterator.** Not possible without `unsafe` or a `Join` that owns its sets, which would change `libs/` (§2); `iter(&self)` covers the tests.
- **A query trait or crate shared with the other prototypes.** Rejected for M3.3's reasons (its spec §7); the hybrid (M3.5) is where one query over several storage kinds is designed (#83).

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | the workspace `Cargo.toml`; `cargo xtask check-unsafe` |
| R2 | unit tests: `insert` on a despawned entity panics with the expected message; `spawn` on a full allocator (`GenIdAllocator::with_fixed_capacity`) panics with its name and leaves `len` unchanged, on the first call (lesson ecs-4); a query with a repeated type panics with its message; review of the `# Panics` sections |
| R3 | review: `DenseColumn`'s and `ValuesMut`'s fields are private to the column module, every method changes values and bits together or neither, and `ValuesMut` offers only `get` |
| R4 | unit tests in `column.rs` and `world.rs` that corrupt one private field (test code in the same module) and check that `check_invariants` names the broken invariant, one per invariant; the model test |
| R5 | unit tests: insert and remove of one type leave every other column's slots and bits unchanged; despawn of an entity with values in several columns clears them all, and a value's `Drop` runs; the model test |
| R6 | query unit tests: required, `Option`, `With`, `Without`, `Entity`, a nested tuple, a never-inserted required type, an `Option` over a column shorter than the entity's index, `query::<Entity>()` including entities with no components, ascending order after despawns and respawns; the model test compares a fixed set of queries with the model |
| R7 | unit tests: order (insert then remove of the same type leaves it absent), a command after a despawn of its entity is skipped, `is_empty` after `apply` and capacity kept |
| R5 to R7 together | proptest model test: random `spawn`, `despawn`, `insert`, `remove` (of four component types), recorded `Commands` (`insert`, `remove`, `despawn`) and `apply`, compared with a `BTreeMap<Entity, components>` model (including `len`), `check_invariants` after every operation |
| R8 | conformance tests in `tests.rs`, one per workload; each step query's `joined` sets (through the step's own query function) asserted per workload |
| R9 | `tests.rs`: invariants checked every tick for `entity_churn`, `add_remove` and `agents` |
| R10 | `cargo xtask bench-smoke` (Criterion ids); `cargo build --benches -p spike_ecs_bench_cache` and a check that each `cache_bench!` line uses its workload's sizes; `cargo xtask spike-bench ecs` output in the pull request |
| R11 | `cargo xtask ci`; `cargo xtask deny`; the pull request's CI run |
| R12 | the `review-docs` review; `cargo xtask doc` |

## 9. Done when

- [ ] `cargo xtask ci` passes locally on the final code commit, and CI is green on all three platforms.
- [ ] The backend passes conformance for all seven workloads, and the invariant-under-churn and model tests pass.
- [ ] `cargo xtask spike-bench ecs` lists the `dense` backend; its output, and a cache-bench run if a Valgrind host is available, are in the pull request, labelled informal.
- [ ] The spike README, CHANGELOG, coverage list and M3.8's roadmap row are updated.
- [ ] The reviews of CONTRIBUTING.md "Reviews before a pull request" are recorded in `log.md` and the pull request.
- [ ] The spec is `Implemented`, the plan and log `Historical`, with Deviations filled in.

## 10. Open questions

None. The items of §4.8 are deferred to the chunks named there.

## 11. Prior art and sources

Designs only; no code is taken from any of them ([ADR 0002](../../adr/0002-originality-and-dependency-licensing.md)).

- [specs](https://docs.rs/specs/): `VecStorage` (one slot per entity index) and joins over hierarchical bitsets of component presence.
- [hibitset](https://docs.rs/hibitset/): the layered bitset that specs' joins iterate; M3.17's `HierarchicalBitSet` is our own code on the same idea.
- [M3.3 spec](../m3.3-sparse-set/spec.md) and [M3.4 spec](../m3.4-archetype/spec.md): the shared query surface, `Commands`, `check_invariants` and the backend conventions this chunk follows.
- [M3.2 spec](../m3.2-spike-harness/spec.md): the harness, the workloads and the conformance check this chunk implements against.
- [M3.17 spec](../m3.17-hierarchical-bitset/spec.md): `HierarchicalBitSet` and `Join`.
