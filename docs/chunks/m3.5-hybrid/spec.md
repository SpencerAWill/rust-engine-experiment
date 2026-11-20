# M3.5: Per-component hybrid prototype

- **Status:** Draft (2026-11-20)
- **Milestone:** M3 (ECS storage decision) · **Depends on:** M3.3, M3.4 · **Decides:** no ADR
- **Issue:** #82 and #83, both answered here (tracking issue #78)
- **Estimated size:** about 2,400 changed lines, excluding tests, lockfiles and generated files (`component` about 100, `table` and `archetype` about 350, `sparse` about 250, `world` and `bundle` about 550, `query` about 650, `commands` about 150, the workload modules about 400, documents about 150)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds `spike_ecs_hybrid`, a minimal safe ECS in which each component type chooses its storage at compile time: archetype tables by default, as in M3.4, or a sparse set of its own, as in M3.3, for types that come and go often. One query layer runs over both: it walks the archetype tables that match the table types, reads their columns as slices, and probes a sparse set per row only for the sparse types the query names. It comes now because it is the third storage model M3.8 decides between, and because the informal runs of M3.3, M3.4 and M3.19 show archetypes leading every workload except `add_remove`, where every toggle moves a row. At the end, the `hybrid` backend passes conformance against `vec_aos` for all seven workloads, and `cargo xtask spike-bench ecs` and the cache bench list it beside the other backends.

## 2. Goals and non-goals

**Goals**

- A `Component` trait whose associated type picks a type's storage, so that each query parameter's fetch is chosen at compile time and nothing per entity branches on a storage kind (§4.2).
- Table storage that behaves as M3.4's: one table per set of table types, cached transition edges, bundle spawn straight into the final table (§4.4, §4.6).
- Sparse storage that behaves as M3.3's: inserting or removing a sparse type touches only its set and moves no row (§4.5).
- Typed tuple queries with the same parameters, filters and names as M3.3's and M3.4's, over both kinds (§4.7), and `Commands` as theirs (§4.8).
- Invariant checks and a model test that cover both kinds, transitions, bundles and commands (§8).
- Answers to #82 (enable bits) and #83 (one query and command API over several storage kinds), recorded in §4.11 and §7.

**Non-goals**

- Two storage kinds only, table and sparse set *(maintainer, from the brainstorm)*. Dense per-entity arrays (M3.19) are not a third kind: on the informal runs they tie sparse sets on `add_remove` and lose every iteration workload.
- One backend only, `hybrid`, with the sparse types fixed per workload (§4.9) *(maintainer)*. No variant with other choices and no enable-bit variant (§7).
- Sparse types are not part of an archetype's identity *(maintainer)*: archetypes and tables key on table types only (§4.4, §7).
- Only the shared core of #83 is built *(maintainer)*: no capability traits, no storage hints, no escape hatch to a concrete storage (§4.11).
- No query state cached between runs, no archetype removal, no batch moves, no external iterator for mutable queries, no owning groups, no queries driven from a sparse set (§4.10).
- No runtime-registered components (M3.6), no `unsafe` (M3.7), no parallel iteration (M7), no change detection (M6), no memory measurement (#41).
- No code shared with the other spike crates: `spike_ecs_hybrid` depends on none of them. Its table, sparse-set, query and command modules MAY be adapted from M3.3's and M3.4's, which are this repository's own code.
- No results in `docs/benchmarks.md`: that is M3.8's. No change to `libs/`, the harness, the other backends, or any frozen format.

## 3. Guide-level explanation

```rust
use spike_ecs_hybrid::{Commands, Component, Entity, SparseStorage, TableStorage, World};

#[derive(Clone, Copy)] struct Position(f32);
#[derive(Clone, Copy)] struct Velocity(f32);
struct Stunned;                                              // comes and goes often

impl Component for Position { type Storage = TableStorage; }
impl Component for Velocity { type Storage = TableStorage; }
impl Component for Stunned { type Storage = SparseStorage; }

let mut world = World::new();

let e = world.spawn_with((Position(0.0), Velocity(1.0)));   // straight into the (Position, Velocity) table
world.insert(e, Stunned);                                   // into Stunned's set; e's row does not move
world.remove::<Stunned>(e);                                 // out of the set; still no move

world
    .query_mut::<(&mut Position, &Velocity)>()              // table types only: no probe per row
    .without::<Stunned>()                                   // a sparse filter: one probe per row
    .for_each(|(p, v)| p.0 += v.0);

assert_eq!(world.get::<Position>(e).map(|p| p.0), Some(1.0));
assert_eq!(world.archetype_count(), 2);                     // the empty table and (Position, Velocity)
```

Structural changes inside a query go through `Commands`, as in M3.3 and M3.4; a recorded `insert` or `remove` applies to whichever storage its type uses:

```rust
let mut commands = Commands::new();
world
    .query::<(Entity, &Position)>()
    .for_each(|(e, p)| if p.0 > 10.0 { commands.insert(e, Stunned) });
world.apply(&mut commands);
```

The harness backend is `HybridX` for each workload `X`:

```rust
spike_ecs_harness::check_conformance::<spike_ecs_vec::add_remove::VecAddRemove,
                                       spike_ecs_hybrid::add_remove::HybridAddRemove>(
    Params { count: 1_000, seed: 42 },
    120,
)
.unwrap();
```

## 4. Reference-level design

### 4.1 Layout

```
spikes/ecs_storage/hybrid/         # package spike_ecs_hybrid (library)
  src/lib.rs                       # re-exports World, Entity, Component, TableStorage, SparseStorage, Commands, Bundle, InvariantViolation, With, Without, query types
  src/component.rs                 # Component, Storage (sealed), TableStorage, SparseStorage, StorageKind, the crate-private impl macro
  src/table.rs                     # Column<T>, ErasedColumn, Table
  src/archetype.rs                 # Archetype, ArchetypeId, Edges
  src/sparse.rs                    # SparseSet<T>, SparseMut, ErasedSparseSet
  src/world.rs                     # World, Location, ComponentId, ComponentInfo, InvariantViolation
  src/bundle.rs                    # Bundle (sealed), BundleInfo, the tuple macro
  src/query.rs                     # QueryParam, ReadOnly, Filter (sealed), Query, QueryMut, QueryIter, the tuple macro
  src/commands.rs                  # Commands, World::apply
  src/{simple_iter, frag_iter, heavy_compute, add_remove, entity_churn, sprites, agents}.rs
  src/tests.rs                     # conformance, invariants under churn, the model test, query tests
```

The package follows the spike-crate rules in [spikes/README.md](../../../spikes/README.md): `publish = false` through the workspace, the workspace lints, `#![forbid(unsafe_code)]`, documented public items. It depends on `engine` and `spike_ecs_harness`, and has `spike_ecs_vec` and `proptest` as dev-dependencies. The workload modules are `pub mod`, so the bench crates can name their types.

### 4.2 Components and storage kinds

```rust
pub trait Component: 'static { type Storage: Storage; }
pub trait Storage: seal::Storage {}             // sealed; implemented by the two markers only
pub enum TableStorage {}
pub enum SparseStorage {}
pub enum StorageKind { Table, Sparse }          // Debug, Clone, Copy, PartialEq, Eq
```

- **A type is a component only if it implements `Component`.** Unlike the other prototypes, where any `'static` type is one, every method that names a component type (`insert`, `remove`, `get`, `get_mut`, the query parameters and filters, `Bundle`'s elements, `Commands::insert` and `remove`) bounds it by `Component`. A tuple is therefore not a component unless someone implements the trait for it.
- **The associated type chooses the fetch.** The seal trait `seal::Storage` carries, for each kind, the per-query fetch types (`&'w [T]` for a table column, a reference to the set for a sparse one), the per-row fetch, and the insert, remove and get paths, so a `&T` parameter's fetch is `<T::Storage as seal::Storage>::Fetch<'w, T>` and nothing at run time chooses between kinds. `seal::Storage` also has `const KIND: StorageKind`, which the world records per registered type for `Debug`, the invariants and the test hooks. *(Main session's call: the maintainer chose a static choice with "`const STORAGE: StorageKind`"; a constant cannot pick a fetch type in stable Rust without a run-time branch on a two-variant fetch, so the constant lives on the storage marker and the trait names the marker.)*
- **Harness types.** The harness's component types are foreign; `Component` is local, so the crate implements it for them (the orphan rule allows a local trait on a foreign type). A crate-private `macro_rules!` macro writes one `impl Component` per type for a list of types and one storage marker; the workload modules and tests use it. The sparse types are those of §4.9; every other type is a table type.

### 4.3 `World`

```rust
pub struct EntityMarker;
pub type Entity = GenId<EntityMarker>;

impl World {
    pub fn new() -> Self;                       // also `Default`
    pub fn spawn(&mut self) -> Entity;
    pub fn spawn_with<B: Bundle>(&mut self, bundle: B) -> Entity;
    pub fn despawn(&mut self, entity: Entity) -> bool;
    pub fn is_alive(&self, entity: Entity) -> bool;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn archetype_count(&self) -> usize;
    pub fn insert<T: Component>(&mut self, entity: Entity, value: T) -> Option<T>;
    pub fn remove<T: Component>(&mut self, entity: Entity) -> Option<T>;
    pub fn get<T: Component>(&self, entity: Entity) -> Option<&T>;
    pub fn get_mut<T: Component>(&mut self, entity: Entity) -> Option<&mut T>;
    pub fn query<Q: ReadOnly>(&self) -> Query<'_, Q>;
    pub fn query_mut<Q: QueryParam>(&mut self) -> QueryMut<'_, Q>;
    pub fn apply(&mut self, commands: &mut Commands);
    pub fn check_invariants(&self) -> Result<(), InvariantViolation>;
}

pub struct InvariantViolation { /* message */ }     // Display, Error
```

- **Entities, live count and locations** are as M3.4's (its spec §4.2): `engine::genid`'s lean `GenIdAllocator<EntityMarker>`, `len` as a counter, `locations: Vec<Location>` with `Location { archetype, row }`. A location names the entity's table row only; sparse values are found through their sets. `spawn` and `spawn_with` MUST panic as M3.4's do when the allocator is full (`#[track_caller]`, a `# Panics` section, through `try_allocate`, a message that starts with the method's own name), with the world unchanged on the first call of its kind (lesson ecs-4).
- **Component identity.** A `BTreeMap<TypeId, ComponentId>` maps each type to a dense `ComponentId(u32)`, the index of its `ComponentInfo { type_id, name, kind: StorageKind, … }` in a `Vec`. Table and sparse types share one id space. A table type's info carries `new_column` (M3.4's); a sparse type's info carries the index of its set in `sparse_sets: Vec<Box<dyn ErasedSparseSet>>`, so a typed call reaches a set with the one `TypeId` lookup and one index. A type is registered on its first `insert`, or on the first `spawn_with` of a bundle containing it. Determinism is as M3.3's (§6).
- **Table types** behave exactly as in M3.4: `insert` replaces in place or moves the entity to the archetype with `T` added; `remove` moves it to the archetype with `T` removed; both return as M3.4's do.
- **Sparse types.** `insert` stores the value in `T`'s set and returns the replaced value, or `None`; `remove` takes it out of the set (swap-remove, as M3.3's) and returns it. Neither touches the entity's location, archetype or row. A warm sparse `insert` or `remove` MUST cost no more lookups and downcasts than M3.3's `World::insert` and `remove` (one `TypeId` lookup, one downcast), checked against M3.3's code before the benchmarks run (lesson ecs-7).
- Both kinds: `insert` MUST panic on a dead entity, as M3.4's does (`#[track_caller]`, a `# Panics` section, a message naming the entity and saying it was despawned); `remove`, `get` and `get_mut` return `None` for a dead entity or a missing component.
- **`despawn`** returns `false` for a dead entity. Otherwise it removes the entity's table row (dropping its values) and fixes the swapped entity's location as M3.4's does, removes the entity from every registered sparse set (one `dyn` call per sparse type, dropping any value), frees the ID and decrements `len`. Its sparse cost is one call per registered sparse type, not per sparse component the entity has. *(Main session's call, as M3.19's despawn over its columns: a per-entity mask would be a second record of membership to keep in step, and no workload registers more than one sparse type.)*
- **`archetype_count`** is as M3.4's: archetypes created so far, the empty one included, never removed. It counts tables, so a sparse insert never changes it.
- **`check_invariants`** checks §4.4's to §4.6's invariants and returns the first violation; `InvariantViolation` has the shape of M3.3's (a private message, `Debug`, `Clone`, `PartialEq`, `Eq`, `Display` as "invariant violated: …", `std::error::Error`).
- **No `WorldBuilder`**, as in M3.4: the storage of each type is in its `Component` impl, so nothing is declared before the first spawn.
- **Common traits.** `World` and `Commands` implement `Default` and `Debug`. `World`'s `Debug` shows the live count, each archetype's table type names and row count, and each sparse set's type name and length, not every value. Every other public type implements `Debug`.

### 4.4 Tables and archetypes

As M3.4's (its spec §4.3 and §4.4), with one change: an archetype's `components` list and its table hold **table types only**. A `ComponentId` of a sparse type never appears in an archetype's list, in an edge vector's index that is set, or in `by_components`.

- `Column<T>`, `ErasedColumn`, `Table` (entity list and columns owned together, whole-row operations only), the slices handed to queries (through a type that cannot change a length; lesson ecs-5), the add and remove edges, the row moves, and `ArchetypeId(0)` as the empty archetype are M3.4's.
- **Invariants** T1, T2, A1, A2, A3 and A4 are M3.4's, plus:
  - A5 (kinds): every id in every archetype's `components`, and every id `c` whose `add[c]` or `remove[c]` is `Some`, is a table type.

### 4.5 Sparse sets

```rust
pub(crate) struct SparseSet<T> { /* sparse index, packed entities and values; private */ }
pub(crate) struct SparseMut<'a, T> { /* private */ }
pub(crate) trait ErasedSparseSet { /* object-safe */ }
```

- **`SparseSet<T>`** is M3.3's storage (its spec §4.3): a sparse index from entity index to position, and packed entity and value arrays of equal length, changed only together. `index_of(entity)` compares the full ID, so a stale ID whose index was reused is absent. It offers `insert`, `remove` (swap-remove, fixing the moved entity's index), `get(entity)`, `get_mut(entity)`, `len`, and `split_mut() -> SparseMut<'_, T>` for mutable queries. No group support and no `swap` are needed.
- **`SparseMut`** offers only `get(entity) -> Option<&mut T>` (reborrowing). It cannot insert, remove, or change a length, so a mutable query cannot break S1 or S2 (lesson ecs-5).
- **`ErasedSparseSet`** is what code that does not know `T` uses: `remove_entity(entity)` (drops the value, if any), `len`, `type_id`, `type_name`, `check(is_alive)`, `as_any`, `as_any_mut`. Typed code downcasts once per call or per query.
- **Invariants**, M3.3's (its spec §4.3), checked per set:
  - S1: for every position `d`, the sparse slot of `entities[d]` holds `d`.
  - S2: every non-empty sparse slot `i` holds a position whose entity has index `i`.
  - S3: every entity in the set is alive.
  - S4 (kinds): set `k` is the set of exactly one registered sparse type, whose info names `k`, and its `type_id` is that type's.

### 4.6 Bundles

```rust
pub trait Bundle: seal::Bundle + 'static {}      // sealed; tuples of 1 to 8 Component types, of either kind
struct BundleInfo { archetype: ArchetypeId, columns: Box<[usize]>, sparse: Box<[usize]> }
```

- As M3.4's (its spec §4.5): flat tuples of 1 to 8 `Component`s by one `macro_rules!` macro, a `BundleInfo` cached per bundle type in a `BTreeMap<TypeId, BundleInfo>`, a repeated type MUST panic with a message naming it (`#[track_caller]`, a `# Panics` section), checked when the info is built.
- **Mixed kinds.** The bundle's archetype has exactly its **table** elements. `spawn_with` pushes the table elements into that archetype's table as one row and inserts each sparse element into its set. `columns` maps each table element, in tuple order, to its column; `sparse` maps each sparse element, in tuple order, to its set. A bundle of sparse types only spawns into the empty archetype. A warm `spawn_with` costs one `BTreeMap` lookup plus one push per element.
- **Invariant** B1: every cached `BundleInfo`'s archetype has exactly the bundle's table element types, `columns[k]` is the column of the `k`-th table element's type, and `sparse[k]` is the set of the `k`-th sparse element's type.

### 4.7 Queries

**Same surface as M3.3 and M3.4** (M3.3 spec §4.5, M3.4 spec §4.6): `QueryParam` for `&T`, `&mut T`, `Option<&T>`, `Option<&mut T>` (`T: Component`), `Entity` and tuples of 1 to 4 `QueryParam`s, nested; `ReadOnly` for those without `&mut`; `Query<'w, Q, F = ()>` and `QueryMut<'w, Q, F = ()>` with chained `.with::<T>()` and `.without::<T>()`; `Filter` for `()`, `With<T>`, `Without<T>` and pairs. `QueryParam`, `ReadOnly` and `Filter` are sealed, each by its own private supertrait, with the item GAT on the seal trait (lesson ecs-3). `&T`, `&mut T` and `With<T>` are required; `Option<…>` and `Without<T>` are not.

**Running a query.** `for_each` on both types, and `iter`, `IntoIterator for Query` and `IntoIterator for &Query`, are `#[track_caller]` and MUST:

1. Look up each type's `ComponentId`. If a required type, of either kind, has none, the query matches nothing. An optional or excluded type with none is ignored.
2. Panic if a type appears twice among the parameters and filters, with a message naming the type, as M3.4 does.
3. Visit the archetypes in id order. An archetype matches if its `components` contain every required **table** type and no excluded **table** type. Sparse types play no part in matching. A matched archetype with no rows is skipped.
4. For each matched archetype, bind each parameter once: a table `&T` or `&mut T` to its column's slice, a table `Option<…>` to `Some(slice)` or `None` for the whole table, and `Entity` to the entity slice, as M3.4 does; a sparse `&T` or `Option<&T>` to its set, a sparse `&mut T` or `Option<&mut T>` to its `SparseMut`, and a sparse `With` or `Without` to its set. For `QueryMut`, split the archetype's columns into disjoint `&mut` with one `iter_mut().enumerate()` pass over its column list, and the sparse sets the query names with one such pass over `sparse_sets`; the repeated-type check (step 2) is what makes the borrows disjoint.
5. For each row `0..len`: if the query names a required or excluded sparse type, read the row's entity and probe each such set; skip the row if a required sparse type is absent or an excluded one present. Then fetch every parameter: a table parameter by row index from slices re-sliced to `..len`, as M3.4 does; a sparse `&T` or `&mut T` from its set by the row's entity (present, since the row passed step 5's probe; the fetch MUST panic with a message naming the invariant if it is not, rather than skip the row); a sparse `Option<…>` by probing its set.

**Table-only queries pay nothing for sparse storage.** A query that names no sparse type MUST do no per-row probe and read no entity it does not yield, so its loop is M3.4's; it SHOULD vectorize where M3.4's does (not asserted). This is checked in review and by the test hook below, and its timing is what the five workloads without sparse types measure (§4.9).

**No required type.** As M3.4's: a query with no required parameter or filter matches every archetype that passes the table filters, the empty one included, so it visits every live entity that passes all the filters. A query whose only required types are sparse (`query::<Entity>().with::<B>()`) is the same walk with a probe per row.

**Iteration.** As M3.4's (its spec §4.6): `QueryMut::for_each(self, f)` only; `Query::for_each`, `Query::iter`, `IntoIterator for Query` and for `&Query`; `QueryIter<'w, Q, F>` yields `Q::Item<'w>` and implements `FusedIterator`. Its `size_hint` is exact (the matched archetypes' rows not yet yielded) for a query with no required or excluded sparse type, and `(0, Some(rows not yet visited))` otherwise. `Query`, `QueryMut` and `QueryIter` are `#[must_use]`.

- The closure and every fetch are monomorphized; nothing per entity goes through `dyn`, and nothing per entity branches on a storage kind. Per-archetype setup (step 4) and the per-run lookups are the only erased work. The query SHOULD NOT allocate per matched archetype; its per-run allocations are counted as built, from a scratchpad crate outside the repository with a counting allocator, and recorded in the plan, together with whether a run's setup grows with the number of registered types (lesson ecs-2).
- `pub(crate)`, `#[cfg(test)]` `matched_archetypes()` on `Query` and `QueryMut` return the matched archetype ids, as M3.4's do, and `probed()` returns the `ComponentId`s of the sparse sets the query probes per row, with each one's role (required, optional or excluded), so the crate's tests can check how each step query is driven (lesson ecs-1). They are not public API.

### 4.8 Commands

As M3.4's (its spec §4.7, after M3.3's §4.6): `Commands::{new, despawn, insert, remove, len, is_empty}`, `Default`, `Debug`; `Despawn(Entity)` unboxed, `insert` and `remove` one boxed closure each, bounded by `T: Component`. A recorded command calls `World::insert`, `remove` or `despawn` when applied, so it reaches the storage its type uses with no command-side knowledge of kinds. `World::apply` MUST run the commands in recorded order, skip a command whose entity is dead when it runs, and leave the buffer empty with its capacity kept. No `spawn` command.

### 4.9 The backend

`BACKEND` is `"hybrid"` for every `HybridX` type. In every workload, `setup` spawns entities `0..n` in order with `spawn_with`, one bundle per entity holding the components the harness's `spawn` returns, in that order; `checksum` folds a `query` with the harness's hash; the harness's `update` functions MUST be the ones called. The sparse types are `B` (`add_remove`) and `Seeker` (`agents`) *(maintainer)*; every other type, `Hidden` included, is a table type.

| Workload | Bundles (archetypes after setup) | Step (sparse sets probed per row) |
|---|---|---|
| `simple_iter` | `(Matrix, Position, Rotation, Velocity)` (1) | `(&mut Position, &Velocity)` (none) |
| `frag_iter` | `(L, Data)` per letter type `L` (26) | `&mut Data`, visiting all 26 archetypes (none); the checksum runs one `query::<(&L, &Data)>()` per letter |
| `heavy_compute` | as `simple_iter` (1) | `(&mut Matrix, &mut Position, &Rotation)` (none) |
| `add_remove` | `(A,)` (1) | reads `(Entity, &A)` into a reused buffer (none) and inserts `B` with `world.insert`, into `B`'s set; then reads `query::<Entity>().with::<B>()` the same way (walks `(A)`, probing `B` as required) and removes `B`. No row moves and no archetype is created after setup |
| `entity_churn` | `(SpawnId, Position, Velocity, Lifetime)` (1) | `(Entity, (&mut Position, &Velocity, &mut Lifetime))` (none), recording `commands.despawn(e)` for expired entities; `apply`; then `spawn_with` until `world.len()` is `count` |
| `sprites` | `(Transform2d, Velocity2d, Sprite)` or with `Hidden` (2) | move: `(&mut Transform2d, &Velocity2d)` over both (none); draw: `query::<(&Transform2d, &Sprite)>().without::<Hidden>()` (none: `Hidden` is a table type) pushing to the owned draw list |
| `agents` | `(AgentId, Transform2d, Velocity, Ttl)`, or the same with `Seeker` as a fifth element, whose value goes to `Seeker`'s set (1) | `(Entity, (&mut Transform2d, &mut Velocity, &mut Ttl, Option<&mut Seeker>))` (`Seeker`, optional), as M3.4's `agents` (its spec §4.8): write back a kept seeker, record `insert`/`remove` of `Seeker` when it appeared or went away, `despawn` when the ttl reaches 0; `apply`; then `spawn_with` until `world.len()` reaches the target |

- `agents` uses its own `AgentId(u64)`, `Velocity(Vec2)` and `Ttl(u32)` newtypes, as M3.4's does, and builds the harness's agent per entity for `agent_hash` in its checksum.
- `add_remove` uses no `Commands`, for M3.3's reason (its spec §4.7).
- Each step query is one private function used by `step` and by the test that checks its matched archetypes and probed sets (lesson ecs-1).

**Expected, unmeasured**, for M3.8 (lesson docs-3):

- `add_remove` is expected near `sparse_set`'s time, not `archetype`'s: each toggle is one push or swap-remove on `B`'s set and no row move. The removal query walks the `(A)` table and probes `B` per row where `sparse_set` iterates `B`'s set; with every entity holding `B` at that point, the two are expected to be close.
- `agents` is expected at or below `archetype`'s time: a `Seeker` toggle no longer moves a row, at the price of one probe per row for `Option<&mut Seeker>`; a respawn is one table push plus one set insert when the agent has a seeker. Its Criterion row is not comparable across backends until M3.8 fixes the tick count.
- The other five workloads name no sparse type, so they are expected within noise of `archetype`; a gap there would be a cost of the hybrid's query layer, and M3.8 should read it as such.

### 4.10 Deferred

| Idea | Why not now | Where |
|---|---|---|
| Driving a query from its smallest required sparse set (look up each member's location) instead of walking tables | No workload separates the two: in `add_remove` every entity of `(A)` holds `B` when the removal query runs | M4, if the hybrid is chosen |
| Sparse types in archetype identity (Bevy's archetype/table split) | Rejected by the maintainer (§7) | Not planned |
| Enable bits for toggled table components (#82) | Rejected here (§7) | A follow-up issue under #78, conditional on M3.8 choosing table storage |
| Capability traits, storage hints, an escape hatch to the concrete storage (#83) | One crate has nothing to dispatch them over (§4.11) | M4 (#84) |
| A per-entity record of sparse membership, so despawn visits only the entity's sets | No workload registers more than one sparse type | M4, if the hybrid is chosen |
| Cached query state, removing empty archetypes, batch moves, an external `iter` for `QueryMut` | As M3.4 (its spec §4.9) | M4 (#84) |
| Owning groups over sparse sets | They reorder packed arrays to make sparse iteration contiguous; here contiguous iteration is the tables' job | Not planned |
| Change-detection ticks next to each value | Adds memory traffic to every storage model alike | Noted for M3.8; built in M6 |

### 4.11 The query and command API over several storage kinds (#83)

#83 proposed a layered design: a shared core with one meaning on every storage, storage chosen statically per type, capability traits for what only some storages can do, hints a storage may ignore, and an escape hatch to the concrete storage. This chunk builds the first two layers and records the rest:

- **Built.** The shared core (§4.3, §4.6 to §4.8) has one meaning on both kinds: `spawn_with`, `insert`, `remove`, `get`, `get_mut`, `despawn`, tuple queries with `With` and `Without`, and `Commands` that record the same way for every type and apply through `World`'s per-kind paths. The storage is chosen statically by `Component::Storage` (§4.2), so each fetch is picked at compile time. The costs still differ by kind (a sparse insert moves nothing; a table insert moves a row; a sparse parameter costs a probe per row), which is what the benchmarks show.
- **Not built.** Capability traits (chunk or slice iteration, an external `iter_mut`, layout declarations such as groups, batch moves, enable bits), hints, and the escape hatch. With one prototype crate and two kinds there is no second implementation to check a capability boundary against, and no workload measures them. They are added to #84 at close-out as M4 design items.
- **What M4 inherits, if M3.8 chooses the hybrid:** the `Component` trait with a sealed storage marker; table-only queries that pay nothing for the existence of sparse storage (§4.7); sparse membership kept out of archetype identity, revisited if M4 needs sparse filters matched per archetype; and the per-kind costs above as the reason batch operations belong in M4's API.

### 4.12 Documents

- **`docs/roadmap.md`:** M3.5's title becomes "Per-component hybrid prototype (safe): archetype tables by default, sparse sets for the component types that choose them through a `Component` trait, one query layer over both"; its row is `Done (PR #n)` in the close-out commit, and the costs found while building it are added to M3.8's row as M3.4's and M3.19's were.
- **`spikes/ecs_storage/README.md`:** a row for `spike_ecs_hybrid` in the package table.
- **`CHANGELOG.md`:** an entry.
- **`xtask/src/coverage.rs`:** `spike_ecs_hybrid` joins `SPIKE_PACKAGES` and the expected `--exclude` list of `test_args_match_the_spec`.
- **`src/lib.rs`'s and `src/query.rs`'s module docs** cite the designs followed (§11), with no code taken (ADR 0002, lesson specs-5).
- **Issues**, at close-out: the pull request closes #82 and #83; a follow-up issue for enable bits under #78 (§4.10); a comment on #84 adding §4.11's unbuilt layers and the deferred items of §4.10 marked M4.

## 5. Requirements

- **M3.5-R1** `spikes/ecs_storage/hybrid` MUST be a workspace member named `spike_ecs_hybrid`, and its crate root MUST carry `#![forbid(unsafe_code)]`.
- **M3.5-R2** The crate MUST provide `Component` and the storage markers of §4.2, `World`'s API of §4.3, `Bundle` that of §4.6, the query types that of §4.7 and `Commands` that of §4.8, including `Default`, `Debug`, the sealed traits and `#[must_use]`, and the panics of `spawn`, `spawn_with`, `insert` and the methods that run a query, each with `#[track_caller]`, a `# Panics` section and a message that names the method and the type or entity.
- **M3.5-R3** A query parameter's fetch MUST be chosen by its type's `Component::Storage` at compile time, with no per-entity branch on a storage kind and no per-entity `dyn` call; a query that names no sparse type MUST do no per-row probe.
- **M3.5-R4** An archetype's entity list and columns MUST be owned by one type whose operations keep them the same length, a sparse set's index and packed arrays by one type whose operations keep them consistent, and the mutable access either hands to queries MUST NOT be able to change a length or membership.
- **M3.5-R5** `check_invariants` MUST report a violation of each of T1, T2, A1 to A5, S1 to S4 and B1, and MUST return `Ok` after any sequence of public operations.
- **M3.5-R6** For a table type, `insert` and `remove` MUST move the entity to the archetype with exactly its new set of table types, keeping every other value, through the edge cache when warm, as M3.4-R5. For a sparse type, they MUST change only that type's set and MUST NOT change the entity's location or any archetype.
- **M3.5-R7** `spawn_with` MUST place the entity in the archetype of exactly its bundle's table types and each sparse element in its set, with no intermediate archetype created or visited; `despawn` MUST leave no value of the entity in any table or set.
- **M3.5-R8** Queries MUST match exactly the entities that have every required type and none of the `Without` types, of either kind, yield `Some` for an `Option` parameter exactly when the entity has the type, support nested tuples, and, with no required type, visit every live entity that passes the filters.
- **M3.5-R9** `apply` MUST run commands in recorded order, reaching the storage of each command's type, skip commands on dead entities, and leave the buffer empty.
- **M3.5-R10** The `hybrid` backend MUST implement all seven workloads as §4.9 says and MUST pass `check_conformance` against `vec_aos` for each: 1,000 entities (20 per letter for `frag_iter`) over 120 ticks, and 1,000 agents over 1,300 ticks for `agents`. After `setup`, `archetype_count` MUST be one more than §4.9's count, and each step query MUST match the archetypes and probe the sets §4.9 says (`matched_archetypes`, `probed`, §4.7).
- **M3.5-R11** Stepping `entity_churn`, `add_remove` and `agents` MUST keep `check_invariants` at `Ok` after every tick, for as many ticks as R10, and `add_remove` MUST create no archetype after `setup`.
- **M3.5-R12** Both bench crates MUST register every implementation of R10, as `spikes/ecs_storage/README.md` describes.
- **M3.5-R13** `cargo xtask ci` MUST pass, CI MUST be green on Ubuntu, macOS and Windows without workflow changes, and `cargo xtask deny` MUST pass without changes to `deny.toml`.
- **M3.5-R14** Every public item MUST be documented, and the documents and issues of §4.12 MUST be updated as described.

## 6. Cross-cutting concerns

- **Determinism:** component ids follow registration order, archetype ids creation order, table rows push and swap-remove order, sparse positions insert and swap-remove order; queries visit archetypes in id order and rows in table order, so iteration depends only on the sequence of operations and a run is reproducible within a build ([ADR 0003](../../adr/0003-same-build-determinism.md)). Checksums are order-independent wrapping sums. The `TypeId`, component-list and bundle maps are `BTreeMap`s; `TypeId` order is stable within one build, and nothing observable iterates the `TypeId`-keyed maps. No `HashMap`, ambient state or transcendental function; the workspace lints enforce this.
- **`unsafe`:** none; the crate forbids it. The safe-Rust costs it accepts (M3.4's downcasts, bounds-checked row moves and `enumerate` splits, M3.3's full-ID comparison per probe, a checked fetch per sparse parameter per row, `for_each` for mutable queries) are part of what M3.7 measures.
- **Concurrency:** n/a, because every workload is single-threaded. Parallel iteration is M7's.
- **Heap allocation:** `setup` may allocate, and so does the first transition, bundle or sparse insert of each kind. In the steady state, tables and sets keep their capacity and no archetype is created, so moves, spawns and sparse inserts do not allocate unless a table or set grows; `agents` allocates one box per seeker command. Per-query-run allocations are counted as built (lesson ecs-2). Not measured here; #41 adds space measurement.
- **`no_std` and wasm:** n/a, because spike crates are not in those builds.
- **Frozen formats:** none.
- **Public API changes:** none in `libs/`. The crate's API is used only by its own workload modules, tests and the bench crates.
- **FFI exposure:** n/a.
- **Performance:** the pull request shows one informal `cargo xtask spike-bench ecs` run, and a cache-bench run if a Valgrind host is available, labelled as informal. M3.8 records results under the benchmark run protocol.
- **Documentation:** item docs, the roadmap, the spike README and CHANGELOG (§4.12), and this spec and its plan.
- **Policy changes:** none.

## 7. Alternatives considered

- **Dense per-entity arrays as a third kind, or instead of sparse sets.** Rejected by the maintainer: on the informal runs they tie sparse sets on `add_remove` and are the slowest on every iteration workload, so no workload would choose them, and a third kind adds a fetch path.
- **Storage declared at run time in a `WorldBuilder`** (`World::builder().sparse::<Seeker>()`), with no `Component` trait. Rejected by the maintainer: each fetch would hold a slice-or-set enum and branch on it per row, which may stop table-only loops from vectorizing, so the hybrid's table workloads would not measure M3.4's loop.
- **`const STORAGE: StorageKind` on `Component`**, as the maintainer first put it. Replaced by an associated storage marker carrying the constant (§4.2, main session's call): the constant alone cannot choose a fetch type in safe stable Rust.
- **Sparse types in archetype identity, as bevy_ecs does** (the archetype is the full set; the table holds only table types; a sparse toggle moves the entity between archetypes but not its columns). Rejected by the maintainer: a toggle would still change the entity's archetype and two entity lists, archetype row and table row would be different indices, and the code would grow for a filter cost no workload measures.
- **Enable bits for toggled components (#82)**: keep the component in the table row and flip a per-row bit, so queries skip disabled rows. Rejected for this chunk by the maintainer: it is a third storage option with its own query path, worth measuring only if M3.8 chooses table storage; the sparse set is this chunk's answer to the same cost. A follow-up issue under #78 tracks it (§4.10), as #80 and #81 track the chunked-table and paged-index variants.
- **Capability traits, hints and an escape hatch (#83's upper layers).** Not built (§4.11).
- **More sparse types** (`Hidden` in `sprites`, to measure a sparse `Without` probe). Rejected by the maintainer: `sprites` never toggles `Hidden`, so no user would store it sparse, and the five non-toggling workloads are more useful as a check that the hybrid's query layer costs nothing over archetypes.
- **Depending on `spike_ecs_sparse_set` and `spike_ecs_archetype`** for their storages. Rejected: their storages are `pub(crate)`, the other backends would gain a public surface only this crate uses, and M3.3's convention is one self-contained crate per model (its spec §7).
- **A query trait shared with the other prototypes.** Rejected for M3.3's reasons (its spec §7); this chunk's one query over two kinds lives in one crate.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | the workspace `Cargo.toml`; `cargo xtask check-unsafe` |
| R2 | unit tests: `insert` on a despawned entity panics with the expected message, for a table and a sparse type; `spawn` and `spawn_with` on a full allocator (`GenIdAllocator::with_fixed_capacity`) panic with their own names and leave `len` and `archetype_count` unchanged, on the first call of their kind (lesson ecs-4); `spawn_with` with a repeated type panics; a query with a repeated type panics, with one table and one sparse case; review of the `# Panics` sections |
| R3 | review: the fetch types come from `<T::Storage as seal::Storage>`, no fetch matches on a kind, and a query with an empty `probed()` runs a loop with no probe; the step-query tests of R10 (`probed()` empty for the five table-only workloads) |
| R4 | review: `Table`'s and `SparseSet`'s fields are private to their modules; every method changes all of a table's columns and its entity list or none; `SparseMut` offers only `get` |
| R5 | unit tests in `table.rs`, `archetype.rs`, `sparse.rs` and `world.rs` that corrupt one private field (test code in the same module) and check that `check_invariants` names the broken invariant, one per invariant; the model test |
| R6 | unit tests: a table insert then remove returns the entity to its first archetype with its other values intact, and a second identical transition creates no archetype and hits the edge (a `#[cfg(test)]` counter of `by_components` lookups on the `World`, not a static); a sparse insert and remove leave the entity's location and `archetype_count` unchanged; the model test |
| R7 | unit tests: `spawn_with((A, B, C))` with a sparse `B` on a fresh world creates exactly one archetype besides the empty one, of `A` and `C`, and puts `B` in its set; a bundle of sparse types only spawns into the empty archetype; `despawn` of an entity with sparse values leaves none in any set; the model test |
| R8 | query unit tests, each with table and sparse types: required, `Option`, `With`, `Without`, `Entity`, a nested tuple, a never-inserted required type of each kind, `query::<Entity>()` over entities in several archetypes including the empty one, a query whose only required type is sparse, `matched_archetypes` and `probed` for a filter that excludes some |
| R9 | unit tests: order (insert then remove of the same type leaves it absent, for each kind), a command after a despawn of its entity is skipped, `is_empty` after `apply` and capacity kept |
| R6 to R9 together | proptest model test: random `spawn`, `spawn_with` (bundles of one to three of four component types, two table and two sparse), `despawn`, `insert`, `remove`, recorded `Commands` (`insert`, `remove`, `despawn`) and `apply`, compared with a `BTreeMap<Entity, components>` model (including `len`) and a fixed set of queries mixing the kinds, `check_invariants` after every operation |
| R10 | conformance tests in `tests.rs`, one per workload; `archetype_count` after `setup` and each step query's `matched_archetypes` and `probed` (through the step's own query function) asserted per workload |
| R11 | `tests.rs`: invariants checked every tick for `entity_churn`, `add_remove` and `agents`; `archetype_count` after every `add_remove` tick equals its value after `setup` |
| R12 | `cargo xtask bench-smoke` (Criterion ids); `cargo build --benches -p spike_ecs_bench_cache` and a check that each `cache_bench!` line uses its workload's sizes (the cache bench's own size assertion does not run without Valgrind, #74); `cargo xtask spike-bench ecs` output in the pull request |
| R13 | `cargo xtask ci`; `cargo xtask deny`; the pull request's CI run |
| R14 | the `review-docs` review; `cargo xtask doc`; the follow-up issue and the #84 comment linked from the pull request |

## 9. Done when

- [ ] `cargo xtask ci` passes locally on the final code commit, and CI is green on all three platforms.
- [ ] The backend passes conformance for all seven workloads, and the invariant-under-churn and model tests pass.
- [ ] `cargo xtask spike-bench ecs` lists the `hybrid` backend; its output, and a cache-bench run if a Valgrind host is available, are in the pull request, labelled informal.
- [ ] The roadmap shows M3.5's new title and the M3.8 caveats; the spike README, CHANGELOG and coverage list are updated.
- [ ] #82 and #83 are closed by the pull request; the enable-bits follow-up issue exists under #78, and #84 has the comment of §4.12.
- [ ] The reviews of CONTRIBUTING.md "Reviews before a pull request" are recorded in `log.md` and the pull request.
- [ ] The spec is `Implemented`, the plan and log `Historical`, with Deviations filled in.

## 10. Open questions

None. The items of §4.10 are deferred to the chunks named there.

## 11. Prior art and sources

Designs only; no code is taken from any of them ([ADR 0002](../../adr/0002-originality-and-dependency-licensing.md)).

- [bevy_ecs](https://docs.rs/bevy_ecs/): per-component storage chosen on the `Component` trait (table by default, sparse set opt-in), archetype tables with cached add and remove edges, bundles spawned straight into their table, tuple queries with `With` and `Without`. Its archetype/table split, where sparse types are part of the archetype, is the alternative of §7.
- skypjack, [ECS back and forth, part 10: hybrid storage](https://skypjack.github.io/2021-02-27-ecs-baf-part-10): the argument that hybrid schemes "look better than they are", which the five non-toggling workloads test.
- Unity Entities, enableable components: the enable-bit design of #82 (§7).
- [M3.3 spec](../m3.3-sparse-set/spec.md) and [M3.4 spec](../m3.4-archetype/spec.md): the sparse-set storage, the tables, the shared query surface, `Commands`, `check_invariants` and the backend conventions this chunk combines.
- [M3.2 spec](../m3.2-spike-harness/spec.md): the harness, the workloads and the conformance check this chunk implements against.
