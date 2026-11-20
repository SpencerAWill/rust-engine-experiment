# M3.3: Sparse-set prototype

- **Status:** Implemented (PR #75, 2026-11-19; first drafted 2026-11-14, before M3.11 to M3.18)
- **Milestone:** M3 (ECS storage decision) · **Depends on:** M3.2 · **Decides:** no ADR
- **Issue:** none (milestone tracking issue #66)
- **Estimated size:** about 1,650 changed lines, excluding tests, lockfiles and generated files (`storage` about 250, `world` about 350, `query` about 450, the workload modules about 450, documents about 150)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds `spike_ecs_sparse_set`, a minimal safe ECS on sparse-set storage: a `World` with entities, per-type component storages, owning groups, typed tuple queries and a deferred command buffer, plus two harness backends built on it, `sparse_set` (views only) and `sparse_set_grouped` (the same `World` with owning groups declared). It comes now because it is the first of the three storage models M3 compares, and the hybrid prototype (M3.5) reuses its storage. At the end, both backends pass conformance against `vec_aos` for every workload they implement, and `cargo xtask spike-bench ecs` and the cache bench list them beside the `Vec` backends.

## 2. Goals and non-goals

**Goals**

- A sparse-set storage whose packed entity and value arrays cannot fall out of step, because one private type owns both (§4.3).
- Owning groups: storages kept sorted so that the entities with every member component sit in an aligned prefix, maintained on every insert, remove and despawn (§4.4).
- Typed tuple queries with `&T`, `&mut T`, `Option<&T>`, `Option<&mut T>` and `Entity`, chained `.with::<T>()` and `.without::<T>()` filters, nested tuples, and two execution paths: probing (view) and aligned prefixes (group) (§4.5).
- A command buffer for structural changes decided during a query (§4.6).
- Two backends, so the gap between them is the measured cost and benefit of groups (§4.7).
- Invariant checks and a model test that cover group maintenance under churn (§8).

**Non-goals**

- No paged sparse array. The sparse array is flat, behind a private type, so paging later touches one type (§7). Whether paging is needed is judged from space numbers, which issue #41 adds.
- No memory measurement. Issue #41 adds space measurement; this chunk adds no roadmap row or dependency for it (§4.9).
- No query layer shared with other backends. M3.4 reuses the same names and syntax by convention (§4.5, §7).
- No runtime-registered components (M3.6), no `unsafe` (M3.7), no parallel iteration (M7), no change detection (M6).
- No bundle API (`spawn((A, B, C))`). It mostly helps archetypes, which can place a whole entity at once; M3.4 may add it.
- No nested or partial-owning groups, no `sort_as`, no per-entity component mask, no chunked (slice) iteration, no cached query state. §4.8 lists where each would go.
- No results in `docs/benchmarks.md`: that is M3.8's. No change to `libs/`, the harness, or any frozen format.

## 3. Guide-level explanation

A `World` holds entities and one storage per component type. Any `'static` type is a component.

```rust
use spike_ecs_sparse_set::{Commands, Entity, With, Without, World};

#[derive(Clone, Copy)] struct Position(f32);
#[derive(Clone, Copy)] struct Velocity(f32);
struct Frozen;

let mut world = World::builder()
    .group::<(Position, Velocity)>()            // optional; World::new() has no groups
    .build();

let e = world.spawn();
world.insert(e, Position(0.0));
world.insert(e, Velocity(1.0));

world
    .query_mut::<(&mut Position, &Velocity)>()
    .without::<Frozen>()
    .for_each(|(p, v)| p.0 += v.0);

assert_eq!(world.get::<Position>(e).map(|p| p.0), Some(1.0));
```

A query that changes structure records the change and applies it after the loop:

```rust
let mut commands = Commands::new();
world
    .query::<(Entity, &Position)>()
    .for_each(|(e, p)| if p.0 > 10.0 { commands.despawn(e) });
world.apply(&mut commands);
```

`query` borrows the world shared and allows only read-only parameters; `query_mut` borrows it mutably. Mutable queries are iterated with `for_each`. Read-only queries also offer `iter()` and `IntoIterator`:

```rust
for (p, v) in world.query::<(&Position, &Velocity)>().with::<Frozen>() {
    /* ... */
}
```

Filters chain: `.with::<T>()` and `.without::<T>()`, any number of each. Queries take up to four parameters; a tuple of tuples goes beyond that:

```rust
world
    .query_mut::<(Entity, (&mut A, &mut B, &mut C, Option<&mut D>))>()
    .for_each(|(e, (a, b, c, d))| { /* ... */ });
```

The harness backends are `SparseSetX` and `GroupedX` for each workload `X`, used like every other backend:

```rust
spike_ecs_harness::check_conformance::<spike_ecs_vec::simple_iter::VecSimpleIter,
                                       spike_ecs_sparse_set::simple_iter::GroupedSimpleIter>(
    Params { count: 1_000, seed: 42 },
    120,
)
.unwrap();
```

## 4. Reference-level design

### 4.1 Layout

```
spikes/ecs_storage/sparse_set/     # package spike_ecs_sparse_set (library)
  src/lib.rs                       # re-exports World, WorldBuilder, Entity, Commands, InvariantViolation, With, Without, query types
  src/storage.rs                   # SparseSet<T>, Packed<T>, SparseIndex, ErasedStorage
  src/world.rs                     # World, WorldBuilder, groups, Commands, InvariantViolation
  src/query.rs                     # QueryParam, Filter, ReadOnly, GroupSpec (sealed), Query, QueryMut, QueryIter, the tuple macro
  src/{simple_iter, frag_iter, heavy_compute, add_remove, entity_churn, sprites, agents}.rs
  src/tests.rs                     # conformance, invariants under churn, the model test, query tests
```

The package follows the spike-crate rules in [spikes/README.md](../../../spikes/README.md): `publish = false` through the workspace, the workspace lints, `#![forbid(unsafe_code)]`, documented public items. It depends on `engine`, `spike_ecs_harness` and `agents_2d`, and has `spike_ecs_vec` and `proptest` as dev-dependencies.

### 4.2 `World`

```rust
pub struct EntityMarker;
pub type Entity = GenId<EntityMarker>;

impl World {
    pub fn new() -> Self;                       // no groups; also `Default`
    pub fn builder() -> WorldBuilder;
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

#[must_use]
pub struct WorldBuilder { /* declared groups */ }   // also `Default`

impl WorldBuilder {
    pub fn new() -> Self;
    pub fn group<G: GroupSpec>(self) -> Self;
    pub fn build(self) -> World;
}

pub struct InvariantViolation { /* message */ }     // Display, Error
```

- Entities come from `engine::genid`'s lean `GenIdAllocator<EntityMarker>`. `spawn` allocates; `despawn` removes the entity from every storage, then frees the ID, and returns `false` for a dead entity.
- **Live count.** `len` is the number of live entities, kept in a counter that `spawn` increments and a successful `despawn` decrements, because the lean allocator does not count or list its IDs. `entity_churn` and `agents` use it to spawn back up to their targets (§4.7). The model test checks it against the model (§8, R5). *(Main session's call, from the grill: the alternative, each backend counting the despawns it records, puts bookkeeping in the measured `step` and breaks if a command is ever skipped.)*
- **Component identity.** A `BTreeMap<TypeId, ComponentId>` maps each type to a dense `ComponentId(u32)`, the index of its storage in `Vec<Box<dyn ErasedStorage>>`. A storage is created on the first `insert` of its type, or by `build` for a group's members. `TypeId` order is stable within one build, which is the determinism [ADR 0003](../../adr/0003-same-build-determinism.md) asks for; nothing observable iterates the map. The identity model the engine adopts is M3.8's decision.
- `insert` returns the replaced value, or `None`. It MUST panic on a dead entity, with `#[track_caller]`, a `# Panics` section and a message naming the entity and saying it was despawned. A silent drop of the value would hide a bug. Spike crates are exempt from the `try_` twin rule.
- `remove`, `get` and `get_mut` return `None` for a dead entity or a missing component.
- **Groups are declared on a `WorldBuilder`**, so a group cannot be declared once entities exist. `WorldBuilder::group::<G>()` MUST panic, as `insert` does, if any of `G`'s types already belongs to a declared group, or if a type appears twice in `G`. `GroupSpec` is implemented for tuples of 2 to 4 `'static` types. `World::new()` and `World::default()` are `WorldBuilder::new().build()`. *(Maintainer, from the API review: a `group` method on `World` read as a getter and needed an "after the first spawn" panic.)*
- `check_invariants` checks §4.3's and §4.4's invariants and returns the first violation as an `InvariantViolation`. It has the shape of `engine::genid::InvariantViolation` (a private message, `Debug`, `Clone`, `PartialEq`, `Eq`, `Display` as "invariant violated: …", `std::error::Error`), which the spike cannot construct because its constructor is `pub(crate)`.
- **Common traits.** `World`, `WorldBuilder` and `Commands` implement `Default` and `Debug`. `World`'s `Debug` shows the live count and each storage's type name and length, not every value. Every other public type implements `Debug`.

### 4.3 Storage

```rust
struct Packed<T> { entities: Vec<Entity>, data: Vec<T> }   // fields private to Packed
struct SparseIndex(Vec<u32>);                              // EMPTY = u32::MAX
pub(crate) struct SparseSet<T> { sparse: SparseIndex, packed: Packed<T> }
```

- **`Packed<T>`** owns the parallel arrays and offers only paired operations: `push(entity, value)`, `swap_remove(i) -> (Entity, T)`, `swap(i, j)`, `len`, `entities() -> &[Entity]`, `data() -> &[T]`, `data_mut() -> &mut [T]`, `split_mut() -> (&[Entity], &mut [T])`. The two vectors MUST have the same length after every operation; no code outside `Packed` can reach them. A `Vec<(Entity, T)>` was rejected (§7).
- **`SparseIndex`** maps `entity.index()` to a dense position. It offers `get(index) -> Option<u32>` (an index past the end is absent), `set(index, position)`, which grows the vector with `EMPTY` as needed, and `clear(index)`. Its field is private, so a paged layout replaces this type only.
- **`index_of(entity)`** is the position `d = sparse[entity.index()]` if `d` is present and `packed.entities()[d] == entity`. Comparing the full ID catches stale IDs, so the sparse array stores no generations.
- **`insert`** replaces the value of a present entity. Otherwise it pushes to `packed` and sets the sparse slot.
- **`remove`** swap-removes from `packed`, updates the sparse slot of the entity moved into the hole, if any, and clears the removed entity's slot.
- **`swap(i, j)`** swaps two positions in `packed` and updates both sparse slots. Only group maintenance calls it.
- **`ErasedStorage`** is the object-safe trait the `World` uses without knowing `T`: `index_of`, `swap`, `remove_entity`, `len`, `as_any`, `as_any_mut`. Queries downcast to `SparseSet<T>` once per query.
- Zero-sized tags such as `Hidden` use the same `SparseSet<T>`; their `Vec<T>` never allocates.

**Storage invariants**, checked by `check_invariants`:

- S1: for every position `d`, `sparse[packed.entities()[d].index()] == d`.
- S2: every present sparse slot `s` at index `i` satisfies `s < len` and `packed.entities()[s].index() == i`.
- S3: every entity in `packed` is alive in the allocator. Together with S2, a present sparse slot at index `i` belongs to the live entity at index `i`; §4.5's probes rely on this.

### 4.4 Groups

```
Group { members: [ComponentId; up to 4], len: usize }
```

Each component type belongs to at most one group; the `World` records the owner of each `ComponentId`.

- G1 (aligned): for every member storage, `packed.entities()[0..len]` is the same sequence.
- G2 (complete): an entity has every member component if and only if it is in that prefix.
- **After an insert** of an owned type into entity `e`: if `e` now has every member and its position in the member storages is `>= len`, swap it to position `len` in each member storage, then increment `len`.
- **Before a remove** of an owned type from `e`: if `e` is in the prefix, decrement `len`, swap `e` to position `len` in each member storage, then swap-remove it as §4.3 says.
- `despawn` removes `e` from each storage it has, and the first owned storage it reaches applies the step above.
- Mutable access to the member storages uses `<[T]>::get_disjoint_mut` over the storage list, with at most four indices.

Group maintenance MUST keep G1 and G2 after every `insert`, `remove`, `despawn` and `apply`.

### 4.5 Queries

**Parameters.** `QueryParam` is implemented for `&T`, `&mut T`, `Option<&T>`, `Option<&mut T>`, `Entity`, and tuples of 1 to 4 `QueryParam`s, recursively, so nested tuples are queries. `ReadOnly` marks the parameters without `&mut`. One `macro_rules!` macro generates the tuple impls of both traits from a single list of type names, so raising the limit is one edit.

**Filters.** `Query<'w, Q, F = ()>` and `QueryMut<'w, Q, F = ()>` carry their filters as a type parameter with a default, which a struct allows and a function does not. `.with::<T>()` and `.without::<T>()` consume the query and return it with `F` replaced by `(F, With<T>)` or `(F, Without<T>)`. `Filter` is implemented for `()`, `With<T>`, `Without<T>` and pairs of filters, so a chain of any length is one type and needs no tuple impls. *(Maintainer, from the API review: `query::<Q, F: Filter = ()>` does not compile, see §7.)*

**Sealed traits.** `QueryParam`, `ReadOnly`, `Filter` and `GroupSpec` are sealed, each by its own private supertrait (one seal shared by all four gives conflicting impls on pairs, which are parameter tuples, filters and groups at once), so only this crate implements them, and the helper types their methods mention stay out of the public API.

`&T`, `&mut T` and `With<T>` are **required**: an entity matches only if it has `T`. `Option<…>` and `Without<T>` are not.

**Running a query.** Building a query only records its types; the filters are known once the chain ends. The methods that run it (`for_each` on both types, and `iter` and `into_iter` on `Query`) are `#[track_caller]` and MUST:

1. Look up each type's `ComponentId`. If a required type has no storage, the query matches nothing.
2. Panic if a type appears twice among the parameters and filters (for example `(&mut Position, &Position)`), with a message naming the type. Panic in the same way if no parameter or filter is required (for example `query::<Entity>()`, `query::<Option<&A>>()` or `query::<Entity>().without::<A>()`), with a message saying that this prototype needs at least one `&T`, `&mut T` or `.with::<T>()`. The correct behavior is to match every live entity that passes the filters, but nothing can drive it: the lean allocator does not list its IDs. No workload runs such a query. *(Maintainer, from the grill; the correct behavior is M4's, §4.8.)*
3. For `QueryMut`, borrow the storages in one `iter_mut().enumerate()` pass over the storage list, handing each `&mut` to its slot. This is safe and costs O(storages) per query. (`get_disjoint_mut` needs a constant `N`, and `Q::COUNT + F::COUNT` is not a constant expression on stable Rust.)

**View path.** The driver is the smallest storage among the required parameters and filters. For each position `i` of the driver: copy the entity out, probe each other parameter, check the `Without` filters, and on a full match fetch the items and call the closure. The driver's own index is `i` and is not probed.

**Trusted probes.** Inside a query, the entity comes from a storage, so by S3 it is live and a present sparse slot is its own. A probe MUST therefore be `sparse.get(e.index())` alone, without reading the probed storage's entity array. The public `get`, `get_mut`, `insert` and `remove` keep `index_of`'s full check, because their IDs come from the caller.

**Group path.** A query takes the group path if its required types include every member of some group. If several groups qualify, it takes the one with the most members, and of those the first declared. It walks `0..group.len`. Members are read at index `i` from data slices re-sliced to `..group.len`, so the compiler can drop their bounds checks (SHOULD; not asserted). Other parameters and filters are probed as on the view path. A query whose required types are a strict subset of a group's members uses the view path. `pub(crate)` methods `Query::uses_group()` and `QueryMut::uses_group()` report which path would be taken, so the crate's tests can check it; they are not public API.

**Iteration.**

- `QueryMut::for_each(self, f: impl FnMut(Q::Item<'_>))`. Each call gets fresh reborrows, so several `&mut` from different storages are sound without `unsafe`. A safe external iterator cannot yield `&mut` items from probed positions (it cannot prove two yielded references disjoint), so mutable queries have no `iter`.
- `Query::for_each`, `Query::iter` and `IntoIterator for Query`, whose iterator `QueryIter<'w, Q, F>` yields `Q::Item<'w>`.
- `Query`, `QueryMut` and `QueryIter` are `#[must_use]`: a query that is built and never run does nothing.
- The closure and every fetch are monomorphized; nothing per entity goes through `dyn`. The probe and fetch functions are `#[inline]`.

**Shared surface by convention.** M3.4 and M3.5 SHOULD use the same names and syntax (`query`, `query_mut`, `.with::<T>()`, `.without::<T>()`, `Option`, `Entity`, `Commands`, `World::builder()`), so the workload modules read alike across backends. No trait is shared across backend crates (§7).

### 4.6 Commands

```rust
pub struct Commands { /* Vec<Command> */ }
enum Command { Despawn(Entity), Deferred(Box<dyn FnOnce(&mut World)>) }

impl Commands {
    pub fn new() -> Self;                       // also `Default`
    pub fn despawn(&mut self, entity: Entity);
    pub fn insert<T: 'static>(&mut self, entity: Entity, value: T);
    pub fn remove<T: 'static>(&mut self, entity: Entity);
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

- `World::apply` MUST run the commands in the order they were recorded, then leave the buffer empty with its capacity kept.
- A command whose entity is dead when it runs MUST be skipped. This is the one place an insert on a dead entity does not panic: a rule can despawn and change the same entity in one query (for example an agent caught and expired in the same tick).
- `Despawn` is not boxed. `insert` and `remove` box one closure each, which is one allocation per command. Only `agents` uses them, for seeker changes. If the boxing dominates its numbers, the plan MAY switch to per-type queues and record it as a Deviation.

### 4.7 Backends

`BACKEND` is `"sparse_set"` for the `SparseSetX` types and `"sparse_set_grouped"` for the `GroupedX` types. In every workload, `setup` spawns entities `0..n` in order, inserting each entity's components in the order the harness's `spawn` returns them; `checksum` folds a `query` with the harness's hash. The harness's `update` functions hold the floating-point order and MUST be the ones called.

| Workload | `sparse_set` | `sparse_set_grouped`: group declared on the builder |
|---|---|---|
| `simple_iter` | `(&mut Position, &Velocity)` | `(Position, Velocity)` |
| `frag_iter` | `&mut Data`; the checksum runs one `query::<(&L, &Data)>()` per letter type `L` | not implemented: a single-type query has nothing to group |
| `heavy_compute` | `(&mut Matrix, &mut Position, &Rotation)` | `(Matrix, Position, Rotation)` |
| `add_remove` | reads `(Entity, &A)` into a reused buffer and inserts `B` with `world.insert`; then reads `query::<Entity>().with::<B>()` the same way and removes `B` | `(A, B)`: each insert of `B` joins the group and each removal leaves it, so this is where group maintenance costs show. Both queries are strict subsets of the group and take the view path |
| `entity_churn` | `(Entity, (&mut Position, &Velocity, &mut Lifetime))`, recording `commands.despawn(e)` for expired entities; `apply`; then spawn with direct inserts until `world.len()` is `count` | `(Position, Velocity, Lifetime)` |
| `sprites` | move: `(&mut Transform2d, &Velocity2d)`; draw: `query::<(&Transform2d, &Sprite)>().without::<Hidden>()` pushing to the owned draw list | `(Transform2d, Velocity2d)`; the draw query uses the view path |
| `agents` | `(Entity, (&mut Transform2d, &mut Velocity, &mut Ttl, Option<&mut Seeker>))`: copies the seeker to a local, calls the harness's `update`, writes back, or records `insert`/`remove` of `Seeker` when it appeared or went away, and records `despawn` when the ttl reaches 0; `apply`; then spawn until `world.len()` reaches the target | `(Transform2d, Velocity, Ttl)` |

- `agents` stores `AgentId(u64)`, `Transform2d`, `Velocity(Vec2)`, `Ttl(u32)` and `Seeker` as separate components. `AgentId`, `Velocity` and `Ttl` are newtypes local to the backend, because the harness uses bare `u64`, `Vec2` and `u32`. Its checksum builds an `agents_2d::Agent` on the stack for each entity and calls `agent_hash`, as `vec_soa` does.
- `sprites` stores `Hidden` as a zero-sized component on the hidden entities.
- `add_remove` uses no `Commands`: its passes are not inside a query, so boxing 100,000 commands a tick would measure the command buffer, not the storage.

**Measurement caveat**, for M3.8. When every entity is spawned in order with all its components and none is removed, every storage's dense array has the same order. The view path's probes are then sequential, so `sparse_set` will be close to `sparse_set_grouped` in `simple_iter`, `heavy_compute` and `sprites`. The difference shows once swap-removes have scrambled the order, as in `entity_churn` and `agents`. A shuffled-setup workload could show it directly; it is not in this chunk.

### 4.8 Deferred optimizations

| Idea | Why not now | Where |
|---|---|---|
| Paged sparse array | Its lookup cost would show and its memory benefit would not: no workload has a rare component in a large world, and nothing measures memory yet | Judged from #41's space numbers; its own chunk if needed |
| Per-type command queues | Only `agents` uses boxed commands | A Deviation here if boxing dominates |
| Per-entity component mask (despawn visits only the entity's storages; faster `With`/`Without`) | Entities in the churn workloads have at most five components | M3.5 or M4 |
| Chunked slice iteration for SIMD | Archetypes have the same opportunity; it is an API decision | M4 |
| `sort_as` to realign storages | Groups already do this; one mechanism is enough to measure | Not planned |
| Cached query state | At most a few `BTreeMap` lookups per query per tick | M4 |
| Queries with no required type (`query::<Entity>()`): match every live entity that passes the filters | Needs a list of live entities, such as EnTT's sparse set of entities, which every spawn and despawn would pay for; no workload runs such a query, so this prototype panics (§4.5) | M4 |
| Change-detection ticks next to each value | They add memory traffic to every storage model alike | Noted for M3.8; built in M6 |

**API notes for M4**, from the API review; none changes what this chunk measures:

- Any `'static` type is a component. With no bundle API, `world.insert(e, (A, B))` silently stores the tuple as one component; a `Component` trait (as bevy has) would reject it.
- `Entity` is a type alias, so its `Debug` is `GenId`'s; M4 would make it a newtype.
- `World` is not `Send` or `Sync` (`Box<dyn ErasedStorage>`, boxed commands); M7 needs both.

### 4.9 Documents

- **`docs/roadmap.md`:**
  - M3.3's title becomes "Sparse-set prototype (safe): views-only and owning-group backends on one minimal `World`".
  - M3.8's title also asks it to read the iteration numbers with §4.7's measurement caveat, to note that change-detection ticks (M6) will add a column to every storage model, and to say the decision rests on time alone unless space numbers (#41) exist by then. Its dependencies do not change.
  - M3.3's row is `Done (PR #n)` in the close-out commit.
- **`spikes/ecs_storage/README.md`:** a row for `spike_ecs_sparse_set` in the package table.
- **`CHANGELOG.md`:** an entry.
- **`xtask/src/coverage.rs`:** `spike_ecs_sparse_set` joins `SPIKE_PACKAGES`, so coverage keeps excluding spikes. *(Main session's call, from the grill.)*

## 5. Requirements

- **M3.3-R1** `spikes/ecs_storage/sparse_set` MUST be a workspace member named `spike_ecs_sparse_set`, and its crate root MUST carry `#![forbid(unsafe_code)]`.
- **M3.3-R2** `World` MUST provide the API of §4.2, and `WorldBuilder`, the query types of §4.5 and `Commands` the API of §4.6, including `Default`, `Debug`, the sealed traits and `#[must_use]`, and the panics of `insert`, `WorldBuilder::group` and the methods that run a query, each with `#[track_caller]`, a `# Panics` section and a message that names the type or entity.
- **M3.3-R3** The packed entity and value arrays MUST be owned by one type whose operations keep them the same length; no other code MUST be able to reach them.
- **M3.3-R4** `check_invariants` MUST report a violation of each of S1, S2, S3, G1 and G2, and MUST return `Ok` after any sequence of public operations.
- **M3.3-R5** Group maintenance MUST keep G1 and G2 after every `insert`, `remove`, `despawn` and `apply`.
- **M3.3-R6** Queries MUST match exactly the entities that have every required type and none of the `Without` types, yield `Some` for an `Option` parameter exactly when the entity has the type, and support nested tuples.
- **M3.3-R7** A query whose required types include every member of a group MUST take the group path, and any other query MUST take the view path, as `uses_group` reports.
- **M3.3-R8** Probes inside a query MUST read only the probed storage's sparse array, never its entity array.
- **M3.3-R9** `apply` MUST run commands in recorded order, skip commands on dead entities, and leave the buffer empty.
- **M3.3-R10** The `sparse_set` backend MUST implement all seven workloads as §4.7 says and MUST pass `check_conformance` against `vec_aos` for each: 1,000 entities (20 per letter for `frag_iter`) over 120 ticks, and 1,000 agents over 1,300 ticks for `agents`.
- **M3.3-R11** The `sparse_set_grouped` backend MUST implement the six workloads of §4.7 with the groups listed there and MUST pass the same conformance checks. The step queries of `simple_iter`, `heavy_compute`, `entity_churn`, `agents` and the `sprites` move pass MUST take the group path; the `sprites` draw query and `add_remove`'s queries take the view path.
- **M3.3-R12** Stepping `entity_churn` and `agents` in both backends MUST keep `check_invariants` at `Ok` after every tick, for as many ticks as R10.
- **M3.3-R13** Both bench crates MUST register every implementation of R10 and R11, as `spikes/ecs_storage/README.md` describes.
- **M3.3-R14** `cargo xtask ci` MUST pass, CI MUST be green on Ubuntu, macOS and Windows without workflow changes, and `cargo xtask deny` MUST pass without changes to `deny.toml`.
- **M3.3-R15** Every public item MUST be documented, and the documents of §4.9 MUST be updated as described.

## 6. Cross-cutting concerns

- **Determinism:** iteration order follows storage order, which follows only the sequence of operations, so a run is reproducible within a build ([ADR 0003](../../adr/0003-same-build-determinism.md)). Checksums are order-independent wrapping sums, so conformance does not depend on that order. The `TypeId` map is a `BTreeMap` and nothing observable iterates it. No `HashMap`, ambient state or transcendental function is used; the workspace lints enforce this.
- **`unsafe`:** none; the crate forbids it. The safe-Rust costs this design accepts (bounds checks on probed accesses, `for_each` instead of external mutable iterators, one `enumerate` pass to split borrows) are part of what M3.7 measures.
- **Concurrency:** n/a, because every workload is single-threaded. Parallel iteration is M7's.
- **Heap allocation:** `setup` may allocate. In the steady state, entity indices are reused LIFO, so sparse arrays stop growing; packed arrays keep their capacity; the reused buffers in `add_remove` and `entity_churn` keep theirs. `agents` allocates one box per seeker change. Not measured here; #41 adds space measurement.
- **`no_std` and wasm:** n/a, because spike crates are not in those builds.
- **Frozen formats:** none.
- **Public API changes:** none in `libs/`. The crate's API is used only by its own workload modules and tests.
- **FFI exposure:** n/a.
- **Performance:** the pull request shows one informal `cargo xtask spike-bench ecs` run and, if the devcontainer is available, one cache-bench run, labelled as informal. M3.8 records results under the benchmark run protocol.
- **Documentation:** item docs, the roadmap, the spike README and CHANGELOG (§4.9), and this spec and its plan.
- **Policy changes:** none.

## 7. Alternatives considered

- **Storage only, no `World`:** typed fields per workload (`positions: SparseSet<Position>`). Rejected by the maintainer: the archetype prototype cannot be built from fixed fields, so it pays for type lookup and borrow splitting, and a sparse-set prototype that skipped those costs would be measured on different terms. M4 also starts from the winning prototype.
- **Fixed-arity query methods** (`view2::<A, B>(|a, b| …)`). Rejected by the maintainer: about half the trait code, but it tells M4 less and reads worse.
- **No query API** (workloads call storages directly). Rejected: it measures storage alone and nothing about query overhead.
- **Groups always on, or groups left out.** Rejected by the maintainer in favour of two backends: the gap between them is the information M3.8 needs.
- **A paged sparse array, alone or as a third backend.** Rejected by the maintainer for now: its cost would show and its benefit would not (§4.8). The flat array is behind `SparseIndex`, so paging later is local.
- **Paged packed arrays** (EnTT's, for reference stability). Rejected: the borrow checker already forbids holding a reference across an insert, so stability buys little in safe Rust.
- **`Vec<(Entity, T)>` instead of `Packed<T>`.** Rejected: a query that reads only `T` would also pull every entity ID through the cache.
- **Generations in the sparse array.** Rejected: comparing the ID in the packed array catches stale IDs at the same cost, and keeps the sparse array at 4 bytes per slot.
- **`RefCell` per storage for borrow splitting.** Rejected: a runtime flag per storage per query, and panics instead of build-time errors for overlapping access within one query. The `enumerate` pass has no runtime state.
- **A query trait shared across backends.** Rejected: the query is where storage models differ most (archetypes iterate tables, sparse sets probe), so a shared trait either forces per-entity `get` on everyone or is a shell around per-backend fetches. The orphan rule also forbids implementing a shared trait for `&T` in each backend crate. And the hybrid (M3.5) is the chunk that has to answer "one query over two storage kinds".
- **A call-site macro that flattens long tuples.** Rejected: rust-analyzer completes and type-checks a plain generic call better, and errors point into macro output.
- **Tuples of up to 5, for `agents`.** Rejected by the maintainer: the nested tuple handles it with no extra code, and puts the nested path in a real workload.
- **`Commands` boxing every command, including despawn.** Rejected: `entity_churn` despawns about 6.5% of its entities a tick, and `Despawn(Entity)` needs no box.
- **`Commands` for `add_remove`'s passes.** Rejected: they are not inside a query, so boxed commands would measure the buffer, not the storage.
- **An external `iter` for mutable queries.** Not possible in safe Rust for probed positions (§4.5).
- **Filters as a defaulted type parameter of the method (`query::<Q, F: Filter = ()>()`).** Rejected: it does not compile. On the pinned 1.100 toolchain a default on a function's type parameter is denied (`invalid_type_param_default`, a future hard error), and even with the lint allowed `query::<Q>()` cannot infer `F`. Found by the API review of 2026-11-19.
- **`query_filtered::<Q, F>()` beside `query::<Q>()`** (bevy). Rejected by the maintainer: with `query_mut` it makes four entry points, where chained filters need two.
- **`query` on `&mut World` and `query_ref` on `&World`.** Rejected by the maintainer: std names the shared borrow plainly and the mutable one `_mut` (`get`/`get_mut`, `iter`/`iter_mut`), as hecs does for queries.
- **A `group` method on `World` that panics after the first spawn.** Rejected by the maintainer for the builder (§4.2).
- **`check_invariants` returning `Result<(), String>`.** Rejected: the Rust API Guidelines ask for an error type that implements `Error` (C-GOOD-ERR), and `engine::genid` already has one of this shape.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | the workspace `Cargo.toml`; `cargo xtask check-unsafe` |
| R2 | unit tests: `insert` on a despawned entity panics with the expected message; `WorldBuilder::group` with a type in two groups and with a repeated type each panics; a query with a repeated type panics; a query with no required type panics; review of the `# Panics` sections |
| R3 | review: `Packed<T>`'s fields are private to its module, and every method changes both vectors or neither |
| R4 | unit tests in `storage.rs` and `world.rs` that corrupt one private field (test code in the same module) and check that `check_invariants` names the broken invariant, one per invariant; the model test |
| R5 | proptest model test: random `spawn`, `despawn`, `insert`, `remove` and `apply` over four component types, with and without a group of two or three of them, compared with a `BTreeMap<Entity, components>` model (including `len`), `check_invariants` after every operation |
| R6 | query unit tests: required, `Option`, `With`, `Without`, `Entity`, a nested tuple, a never-inserted required type gives no matches; the model test also compares a fixed set of queries with the model |
| R7 | query unit tests of `uses_group` for an exact match, a superset with an extra probed type, and a strict subset |
| R8 | review of the probe functions |
| R9 | unit tests: order (insert then remove of the same type leaves it absent), a command after a despawn of its entity is skipped, `is_empty` after `apply` |
| R10, R11 | conformance tests in `tests.rs`, one per implementation; `uses_group` asserted in each grouped workload's test |
| R12 | `tests.rs`: invariants checked every tick for `entity_churn` and `agents`, both backends |
| R13 | `cargo xtask bench-smoke`; `cargo xtask spike-bench ecs` output in the pull request |
| R14 | `cargo xtask ci`; `cargo xtask deny`; the pull request's CI run |
| R15 | the `review-docs` review; `cargo xtask doc` |

## 9. Done when

- [ ] `cargo xtask ci` passes locally, and CI is green on all three platforms. *Local: passes on 4d37876, the last code commit (log.md); later commits are documents only. CI on PR #75: pending.*
- [x] Both backends pass conformance for every workload they implement, and the invariant-under-churn and model tests pass. *13 conformance tests, `entity_churn_keeps_invariants_every_tick`, `agents_keeps_invariants_every_tick` and `world_matches_the_model` (with `apply`) pass on 4d37876.*
- [x] `cargo xtask spike-bench ecs` lists both backends; its output, and a cache-bench run if the devcontainer is available, are in the pull request, labelled informal. *43 ids on 17532d2 (code as of 4d37876), table in PR #75; no cache-bench run, since neither Valgrind nor Docker is available on this Mac (said in the PR; #74).*
- [x] The roadmap shows M3.3's new title and M3.8's notes; the spike README and CHANGELOG are updated. *0a74f54, a1e989b, 0716258.*
- [x] The reviews of CONTRIBUTING.md "Reviews before a pull request" are recorded in `log.md` and the pull request. *Three rounds, PR #75 Reviews section.*
- [x] The spec is `Implemented`, the plan and log `Historical`, with Deviations filled in. *This close-out commit.*

## 10. Open questions

None. Paging and space measurement are deferred to #41, and the items of §4.8 to the chunks named there.

## 11. Prior art and sources

Designs only; no code is taken from any of them ([ADR 0002](../../adr/0002-originality-and-dependency-licensing.md)).

- Michele Caini (skypjack), [ECS back and forth, part 2](https://skypjack.github.io/2019-03-07-ecs-baf-part-2/): sparse sets, and owning groups as aligned prefixes.
- Michele Caini, [ECS back and forth, part 9](https://skypjack.github.io/2020-08-02-ecs-baf-part-9/): sparse set internals: the sparse and packed arrays, paging, and swap-and-pop removal.
- [EnTT's wiki](https://github.com/skypjack/entt/wiki): views driven from the smallest pool; full-owning groups and their constraints.
- [bevy_ecs](https://docs.rs/bevy_ecs/): tuple queries over `&T`, `&mut T`, `Option`, `With` and `Without`; commands applied after a system; `QueryState`.
- [hecs](https://docs.rs/hecs/): a small ECS whose query API is a tuple of references.
- [M3.2 spec](../m3.2-spike-harness/spec.md): the harness, the workloads and the conformance check this chunk implements against.
