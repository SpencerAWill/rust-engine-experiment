# M3.4: Archetype prototype

- **Status:** Accepted (2026-11-20)
- **Milestone:** M3 (ECS storage decision) · **Depends on:** M3.2 · **Decides:** no ADR
- **Issue:** none (milestone tracking issue #66)
- **Estimated size:** about 1,900 changed lines, excluding tests, lockfiles and generated files (`table` and `archetype` about 350, `world` and `bundle` about 450, `query` about 550, the workload modules about 400, documents about 150)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds `spike_ecs_archetype`, a minimal safe ECS on archetype storage: a `World` that keeps one table per distinct set of component types, moves an entity's row between tables when a component is inserted or removed (through cached transition edges), spawns whole bundles straight into their table, and runs typed tuple queries over the matching tables. It comes now because it is the second of the three storage models M3 compares, and the hybrid prototype (M3.5) builds on it and on M3.3. At the end, the `archetype` backend passes conformance against `vec_aos` for all seven workloads, and `cargo xtask spike-bench ecs` and the cache bench list it beside the `Vec` and sparse-set backends.

## 2. Goals and non-goals

**Goals**

- A table whose entity list and columns cannot fall out of step, because one private type owns them and offers only whole-row operations (§4.3).
- Archetype transitions through cached add and remove edges, so a warm `insert` or `remove` that changes the entity's set costs one array read plus the row move (§4.4).
- A bundle spawn, `spawn_with((A, B, C))`, that places an entity in its final table with no intermediate moves (§4.5).
- Typed tuple queries with the same parameters, filters and names as M3.3's (§4.6), walking matching tables and reading their columns as slices, with no probes.
- A command buffer for structural changes decided during a query (§4.7).
- Invariant checks and a model test that cover transitions, bundles and commands (§8).

**Non-goals**

- One backend only, `archetype`: no chunked tables and no cached-query variant *(maintainer, from the brainstorm)*. M3.10's bevy_ecs and hecs baselines are the outside archetype comparisons.
- No query state cached between runs, no archetype removal, no batch moves, no external iterator for mutable queries, no `Component` trait. §4.9 lists where each would go.
- No runtime-registered components (M3.6), no `unsafe` (M3.7), no parallel iteration (M7), no change detection (M6), no memory measurement (#41).
- No query layer or code shared with `spike_ecs_sparse_set`; the names and syntax match by convention (M3.3 spec §4.5, §7). The query module MAY be adapted from M3.3's, which is this repository's own code.
- No results in `docs/benchmarks.md`: that is M3.8's. No change to `libs/`, the harness, the other backends, or any frozen format.

## 3. Guide-level explanation

```rust
use spike_ecs_archetype::{Commands, Entity, World};

#[derive(Clone, Copy)] struct Position(f32);
#[derive(Clone, Copy)] struct Velocity(f32);
struct Frozen;

let mut world = World::new();

let e = world.spawn_with((Position(0.0), Velocity(1.0)));   // straight into the (Position, Velocity) table
let f = world.spawn();                                      // the empty table
world.insert(f, Position(5.0));                             // moves f to the (Position) table

world
    .query_mut::<(&mut Position, &Velocity)>()
    .without::<Frozen>()
    .for_each(|(p, v)| p.0 += v.0);

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

The harness backend is `ArchetypeX` for each workload `X`:

```rust
spike_ecs_harness::check_conformance::<spike_ecs_vec::simple_iter::VecSimpleIter,
                                       spike_ecs_archetype::simple_iter::ArchetypeSimpleIter>(
    Params { count: 1_000, seed: 42 },
    120,
)
.unwrap();
```

## 4. Reference-level design

### 4.1 Layout

```
spikes/ecs_storage/archetype/      # package spike_ecs_archetype (library)
  src/lib.rs                       # re-exports World, Entity, Commands, Bundle, InvariantViolation, With, Without, query types
  src/table.rs                     # Column<T>, ErasedColumn, Table
  src/archetype.rs                 # Archetype, ArchetypeId, Edges
  src/world.rs                     # World, Location, ComponentId, ComponentInfo, InvariantViolation
  src/bundle.rs                    # Bundle (sealed), BundleInfo, the tuple macro
  src/query.rs                     # QueryParam, ReadOnly, Filter (sealed), Query, QueryMut, QueryIter, the tuple macro
  src/commands.rs                  # Commands, World::apply
  src/{simple_iter, frag_iter, heavy_compute, add_remove, entity_churn, sprites, agents}.rs
  src/tests.rs                     # conformance, invariants under churn, the model test, query tests
```

The package follows the spike-crate rules in [spikes/README.md](../../../spikes/README.md): `publish = false` through the workspace, the workspace lints, `#![forbid(unsafe_code)]`, documented public items. It depends on `engine`, `spike_ecs_harness` and `agents_2d`, and has `spike_ecs_vec` and `proptest` as dev-dependencies.

### 4.2 `World`

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

- **Entities** come from `engine::genid`'s lean `GenIdAllocator<EntityMarker>`. `spawn` allocates and pushes the entity into the empty archetype; `spawn_with` allocates and pushes it into its bundle's archetype (§4.5). Both MUST panic as M3.3's `spawn` does when the allocator is full: `#[track_caller]`, a `# Panics` section, through `try_allocate`, with a message that starts with the method's own name. `despawn` removes the entity's row (dropping its values), then frees the ID, and returns `false` for a dead entity.
- **Live count.** `len` is a counter kept by `spawn`, `spawn_with` and `despawn`, as in M3.3 (§4.2 there), incremented only after allocation succeeds. `entity_churn` and `agents` spawn back up to their targets with it.
- **Locations.** `locations: Vec<Location>`, indexed by `entity.index()`, with `Location { archetype: ArchetypeId, row: u32 }`. `despawn` sets the slot to `Location::NONE` (`archetype` is `u32::MAX`). Public methods check `is_alive` before reading a location.
- **Component identity.** A `BTreeMap<TypeId, ComponentId>` maps each type to a dense `ComponentId(u32)`, the index of its `ComponentInfo { type_id, name, new_column: fn() -> Box<dyn ErasedColumn> }` in a `Vec`. A type is registered on its first `insert` or first `spawn_with` of a bundle containing it. `new_column` is how a new archetype creates empty columns for types it knows only by id. Determinism is as M3.3's (§6).
- **No `WorldBuilder`.** There is nothing to declare before the first spawn, so M3.3's `World::builder()` has no counterpart. *(Main session's call.)*
- `insert` on an entity that already has `T` replaces the value in place, without a move, and returns the old one. Otherwise it moves the entity to the archetype with `T` added (§4.4) and returns `None`. It MUST panic on a dead entity, as M3.3's does: `#[track_caller]`, a `# Panics` section, a message naming the entity and saying it was despawned.
- `remove` returns `None` for a dead entity or a missing component, without a move; otherwise it moves the entity to the archetype with `T` removed and returns the value. `get` and `get_mut` return `None` for a dead entity or a missing component.
- `archetype_count` is the number of archetypes created so far, the empty one included. Archetypes are never removed. Tests use it, and M3.8 can quote it next to `frag_iter`'s numbers.
- `check_invariants` checks §4.3's to §4.5's invariants and returns the first violation; `InvariantViolation` has the shape of M3.3's (a private message, `Debug`, `Clone`, `PartialEq`, `Eq`, `Display` as "invariant violated: …", `std::error::Error`).
- **Common traits.** `World` and `Commands` implement `Default` and `Debug`. `World`'s `Debug` shows the live count and, for each archetype, its component type names and row count, not every value. Every other public type implements `Debug`.

### 4.3 Tables

```rust
pub(crate) struct Column<T>(Vec<T>);
pub(crate) trait ErasedColumn { /* object-safe */ }
pub(crate) struct Table { entities: Vec<Entity>, columns: Box<[Box<dyn ErasedColumn>]> }   // fields private to Table
```

- **`Column<T>`** wraps a `Vec<T>`. **`ErasedColumn`** is what code that does not know `T` uses: `len`, `type_id`, `type_name`, `swap_remove_drop(row)`, `swap_remove_into(row, dst: &mut dyn ErasedColumn)` (downcasts `dst` to `Column<T>` and pushes the value there), `new_empty() -> Box<dyn ErasedColumn>`, `as_any`, `as_any_mut`. Typed code downcasts once per call (one `TypeId` comparison).
- **`Table`** owns an archetype's entity list and columns, column `i` holding the archetype's `i`-th component in ascending `ComponentId` order. Its operations each change every column and the entity list together:
  - `push_bundle<B: Bundle>(entity, bundle, columns: &[usize])` writes each bundle element into the column the bundle's cached mapping names (§4.5);
  - `swap_remove(row) -> Option<Entity>` drops the row and returns the entity moved into the hole, if any;
  - `move_row_adding<T>(src, row, dst, value: T) -> Option<Entity>` moves every value of `src`'s row into `dst`'s matching column and pushes `value` into `dst`'s column of `T`;
  - `move_row_removing<T>(src, row, dst) -> (T, Option<Entity>)` takes `T` out of `src`'s row and moves the rest into `dst`.
  
  The two `move_row_*` functions return the entity swapped into `src`'s hole, as `swap_remove` does, so the `World` can fix its location. The columns and entity list MUST have the same length after every operation; no code outside `Table` can reach them.
- **Slices.** `Table` offers `len`, `entities() -> &[Entity]`, `column(i) -> &dyn ErasedColumn` and `columns_mut() -> (&[Entity], &mut [Box<dyn ErasedColumn>])` for queries; none of them can change a length.
- **Two archetypes at once.** A move borrows the source and destination archetypes with `<[Archetype]>::get_disjoint_mut([src, dst])`. They are never equal: a move happens only when the set changes.
- Zero-sized tags such as `Hidden` get an ordinary `Column<Hidden>`; its `Vec` never allocates.

**Table invariants**, checked by `check_invariants`:

- T1: in every archetype, every column's length equals the entity list's length.
- T2: column `i`'s `type_id` is the `type_id` of the archetype's `i`-th `ComponentId`.

### 4.4 Archetypes and transitions

```
Archetype { components: Box<[ComponentId]>, table: Table, add: Vec<Option<ArchetypeId>>, remove: Vec<Option<ArchetypeId>> }
```

- `components` is strictly ascending. An archetype's column for a `ComponentId` is found by binary search on it.
- `World` keeps `archetypes: Vec<Archetype>` and `by_components: BTreeMap<Box<[ComponentId]>, ArchetypeId>`. `ArchetypeId(0)` is the empty archetype, created by `new`.
- **Edges.** `add[c]` and `remove[c]`, indexed by `ComponentId` and grown with `None` on demand, cache the archetype reached by adding or removing `c`. A transition reads the edge; on a miss it builds the target's sorted component list, looks it up in `by_components`, creates the archetype if it is absent (columns from each component's `new_column`), and records the edge in both directions (`src.add[c] = dst` and `dst.remove[c] = src`, or the reverse for a removal). *(Maintainer, from the brainstorm: a `by_components` lookup on every move would allocate and compare on each insert.)*
- **Row moves.** After a move, the entity's location is `(dst, dst.len() - 1)`, and the entity swapped into its old row, if any, gets the old row.
- `despawn` swap-removes the entity's row from its table and fixes the swapped entity's location in the same way.

**Archetype invariants:**

- A1 (locations): for every archetype `a` and row `r`, `locations[entities[r].index()] == (a, r)`.
- A2 (liveness): every entity in a table is alive, and the rows of all archetypes add up to `len()`. With A1, every live entity is in exactly one row.
- A3 (identity): every `components` list is strictly ascending; archetype 0's is empty; `by_components` maps each archetype's list to its own id and has one entry per archetype.
- A4 (edges): if `add[c]` of `a` is `Some(b)`, `c` is not in `a` and `b`'s list is `a`'s plus `c`; if `remove[c]` of `a` is `Some(b)`, `c` is in `a` and `b`'s list is `a`'s minus `c`.

### 4.5 Bundles

```rust
pub trait Bundle: seal::Bundle + 'static {}      // sealed; tuples of 1 to 8 'static types
struct BundleInfo { archetype: ArchetypeId, columns: Box<[usize]> }
```

- `Bundle` is implemented for flat tuples of 1 to 8 `'static` types by one `macro_rules!` macro. Bundles do not nest: because any `'static` type is a component, `(A, (B, C))` is a bundle of `A` and the tuple component `(B, C)`. *(Main session's call: a nesting impl would overlap the flat one.)*
- `World` caches a `BundleInfo` per bundle type in `bundles: BTreeMap<TypeId, BundleInfo>`, keyed by the tuple's `TypeId`. It is built on the first `spawn_with` of that type: register the element types, find or create the archetype with exactly those types (through `by_components`), and record, for each element in tuple order, its column index in that archetype. A warm `spawn_with` costs one `BTreeMap` lookup plus one push per element.
- `spawn_with` MUST panic if a type appears twice in the bundle (for example `(A, B, A)`), with a message naming the type: `#[track_caller]` and a `# Panics` section. The check runs when the `BundleInfo` is built.

**Bundle invariant:**

- B1: every cached `BundleInfo`'s archetype has exactly the bundle's element types, and `columns[k]` is the column of the `k`-th element's type.

### 4.6 Queries

**Same surface as M3.3** (its spec §4.5): `QueryParam` for `&T`, `&mut T`, `Option<&T>`, `Option<&mut T>`, `Entity` and tuples of 1 to 4 `QueryParam`s, nested; `ReadOnly` for those without `&mut`; `Query<'w, Q, F = ()>` and `QueryMut<'w, Q, F = ()>` with chained `.with::<T>()` and `.without::<T>()`; `Filter` for `()`, `With<T>`, `Without<T>` and pairs. `QueryParam`, `ReadOnly` and `Filter` are sealed, each by its own private supertrait, with the item GAT on the seal trait (lesson ecs-3). `&T`, `&mut T` and `With<T>` are required; `Option<…>` and `Without<T>` are not.

**Running a query.** `for_each` on both types, and `iter`, `IntoIterator for Query` and `IntoIterator for &Query`, are `#[track_caller]` and MUST:

1. Look up each type's `ComponentId`. If a required type has none, the query matches nothing. An optional or excluded type with none is ignored.
2. Panic if a type appears twice among the parameters and filters, with a message naming the type, as M3.3 does.
3. Visit the archetypes in id order. An archetype matches if its `components` contain every required id and no excluded id. A matched archetype with no rows is skipped.
4. For each matched archetype, bind each parameter once: `&T` and `&mut T` to its column's slice (`&[T]` or `&mut [T]`, downcast once per archetype), `Option<…>` to `Some(slice)` or `None` for the whole table, `Entity` to the entity slice. For `QueryMut`, split the archetype's columns into disjoint `&mut` with one `iter_mut().enumerate()` pass over its column list, as M3.3 does over its storages.
5. Call the closure for each row `0..len`, fetching every parameter by row index from slices re-sliced to `..len`, so the compiler can drop bounds checks and vectorize (SHOULD; not asserted).

**No required type.** Unlike M3.3 (§4.5 step 2 there), a query with no required parameter or filter (`query::<Entity>()`, `query::<Option<&A>>()`, `query::<Entity>().without::<A>()`) MUST be supported: it matches every archetype that passes the filters, the empty one included, so it visits every live entity that passes them. *(Main session's call: every live entity has a row here, so this costs nothing to support.)*

**Iteration.**

- `QueryMut::for_each(self, f: impl FnMut(Q::Item<'_>))`, as in M3.3. Mutable queries have no `iter`. Unlike in M3.3, a safe external mutable iterator is possible here, since each column's `IterMut` yields disjoint references, but it is an API question for M4 (§4.9), and `for_each` keeps the workload code the same across backends.
- `Query::for_each`, `Query::iter`, `IntoIterator for Query` and for `&Query`; `QueryIter<'w, Q, F>` yields `Q::Item<'w>`, implements `FusedIterator`, and has an exact `size_hint` (the matched archetypes' rows not yet yielded).
- `Query`, `QueryMut` and `QueryIter` are `#[must_use]`.
- The closure and every fetch are monomorphized; nothing per entity goes through `dyn`. Per-archetype setup (step 4) is the only erased work. The query SHOULD NOT allocate per matched archetype; its per-run allocations are counted as built and recorded in the plan (lesson ecs-2).
- `pub(crate)`, `#[cfg(test)]` `Query::matched_archetypes()` and `QueryMut::matched_archetypes()` return the ids of the archetypes that match (step 3), in id order and whether or not they have rows, so the crate's tests can check matching; they are not public API. *(Main session's call, from the grill: matched rather than visited ids, so an assertion does not depend on which tables happen to be empty on a given tick.)*

### 4.7 Commands

As M3.3's (its spec §4.6): `Commands::{new, despawn, insert, remove, len, is_empty}`, `Default`, `Debug`; `Despawn(Entity)` unboxed, `insert` and `remove` one boxed closure each. `World::apply` MUST run the commands in recorded order, skip a command whose entity is dead when it runs, and leave the buffer empty with its capacity kept. No `spawn` command: every workload spawns outside its queries.

### 4.8 The backend

`BACKEND` is `"archetype"` for every `ArchetypeX` type. In every workload, `setup` spawns entities `0..n` in order with `spawn_with`, one bundle per entity holding the components the harness's `spawn` returns, in that order; `checksum` folds a `query` with the harness's hash; the harness's `update` functions MUST be the ones called.

| Workload | Bundles (archetypes after setup) | Step |
|---|---|---|
| `simple_iter` | `(Matrix, Position, Rotation, Velocity)` (1) | `(&mut Position, &Velocity)` |
| `frag_iter` | `(L, Data)` per letter type `L` (26) | `&mut Data`, visiting all 26 archetypes; the checksum runs one `query::<(&L, &Data)>()` per letter |
| `heavy_compute` | as `simple_iter` (1) | `(&mut Matrix, &mut Position, &Rotation)` |
| `add_remove` | `(A,)` (1) | reads `(Entity, &A)` into a reused buffer and inserts `B` with `world.insert`; then reads `query::<Entity>().with::<B>()` the same way and removes `B`. Each insert moves a row from `(A)` to `(A, B)` and each removal moves it back. After the first step both archetypes exist: the read query matches both (`(A, B)` has no rows when it runs), the removal query matches `(A, B)` only |
| `entity_churn` | `(SpawnId, Position, Velocity, Lifetime)` (1) | `(Entity, (&mut Position, &Velocity, &mut Lifetime))`, recording `commands.despawn(e)` for expired entities; `apply`; then `spawn_with` until `world.len()` is `count` |
| `sprites` | `(Transform2d, Velocity2d, Sprite)` or `(Transform2d, Velocity2d, Sprite, Hidden)` (2) | move: `(&mut Transform2d, &Velocity2d)` over both; draw: `query::<(&Transform2d, &Sprite)>().without::<Hidden>()` pushing to the owned draw list |
| `agents` | `(AgentId, Transform2d, Velocity, Ttl)` or with `Seeker` as a fifth element (2) | `(Entity, (&mut Transform2d, &mut Velocity, &mut Ttl, Option<&mut Seeker>))`, as M3.3's `agents` (its spec §4.7): write back a kept seeker, record `insert`/`remove` of `Seeker` when it appeared or went away, `despawn` when the ttl reaches 0; `apply`; then `spawn_with` until `world.len()` reaches the target |

- `agents` uses its own `AgentId(u64)`, `Velocity(Vec2)` and `Ttl(u32)` newtypes, as M3.3's does, and builds an `agents_2d::Agent` per entity for `agent_hash` in its checksum.
- `add_remove` uses no `Commands`, for M3.3's reason (its spec §4.7).
- Each step query is one private function used by `step` and by the test hook that checks its matched archetypes (lesson ecs-1).

**Expected, unmeasured**, for M3.8 (lesson docs-3): `add_remove` is expected to be archetypes' worst case, since every insert and removal moves a row and copies `A`; `frag_iter` is expected to show the per-archetype setup cost, 26 times per query over 20 rows each; the iteration workloads read contiguous slices with no probes and are expected to vectorize as M3.3's group path did in `simple_iter`; the respawns in `entity_churn` and `agents` are one bundle push each.

### 4.9 Deferred

| Idea | Why not now | Where |
|---|---|---|
| Cached query state (the matched archetype list, kept between runs) | M3.3 also rebuilds per run, so the backends are compared on the same terms; ecs-2's count records the cost | M4 |
| Chunked tables (fixed-size chunks) | Maintainer, from the brainstorm: one backend | Not planned |
| Removing empty archetypes | No workload creates archetypes it later abandons | M4, if needed |
| Batch moves (insert `B` into every entity of a table at once) | An API decision; `add_remove` measures the per-entity path both models have | M4 |
| An external `iter` for `QueryMut` | Possible in safe Rust here (§4.6), not on the sparse set; an API question | M4 |
| A `Component` trait, which would let bundles nest and reject a tuple stored as one component | Changes what counts as a component for every backend | M4 (M3.3 spec §4.8's API notes) |
| Per-component storage choice (table or sparse set) | That is the hybrid | M3.5 |
| Change-detection ticks next to each value | Adds memory traffic to every storage model alike | Noted for M3.8; built in M6 |

### 4.10 Documents

- **`docs/roadmap.md`:** M3.4's title becomes "Archetype prototype (safe): one table per archetype, cached transition edges, bundle spawn"; its row is `Done (PR #n)` in the close-out commit.
- **`spikes/ecs_storage/README.md`:** a row for `spike_ecs_archetype` in the package table.
- **`CHANGELOG.md`:** an entry.
- **`xtask/src/coverage.rs`:** `spike_ecs_archetype` joins `SPIKE_PACKAGES` and the expected `--exclude` list of `test_args_match_the_spec`.
- **`src/query.rs`'s and `src/lib.rs`'s module docs** cite the designs followed (bevy_ecs's archetype graph and tuple queries, hecs's archetype tables), with no code taken (ADR 0002, lesson specs-5).

## 5. Requirements

- **M3.4-R1** `spikes/ecs_storage/archetype` MUST be a workspace member named `spike_ecs_archetype`, and its crate root MUST carry `#![forbid(unsafe_code)]`.
- **M3.4-R2** `World` MUST provide the API of §4.2, `Bundle` that of §4.5, the query types that of §4.6 and `Commands` that of §4.7, including `Default`, `Debug`, the sealed traits and `#[must_use]`, and the panics of `spawn`, `spawn_with`, `insert` and the methods that run a query, each with `#[track_caller]`, a `# Panics` section and a message that names the method and the type or entity.
- **M3.4-R3** An archetype's entity list and columns MUST be owned by one type whose operations keep them the same length; no other code MUST be able to reach them.
- **M3.4-R4** `check_invariants` MUST report a violation of each of T1, T2, A1, A2, A3, A4 and B1, and MUST return `Ok` after any sequence of public operations.
- **M3.4-R5** `insert` and `remove` MUST move the entity to the archetype with exactly its new set of types, keeping every other value, and a warm transition MUST be found through the edge cache without a `by_components` lookup.
- **M3.4-R6** `spawn_with` MUST place the entity in the archetype of exactly its bundle's types, with no intermediate archetype created or visited.
- **M3.4-R7** Queries MUST match exactly the entities that have every required type and none of the `Without` types, yield `Some` for an `Option` parameter exactly when the entity has the type, support nested tuples, and, with no required type, visit every live entity that passes the filters.
- **M3.4-R8** `apply` MUST run commands in recorded order, skip commands on dead entities, and leave the buffer empty.
- **M3.4-R9** The `archetype` backend MUST implement all seven workloads as §4.8 says and MUST pass `check_conformance` against `vec_aos` for each: 1,000 entities (20 per letter for `frag_iter`) over 120 ticks, and 1,000 agents over 1,300 ticks for `agents`. After `setup`, `archetype_count` MUST be one more than §4.8's count (the empty archetype), and each step query MUST match the archetypes §4.8 says (`matched_archetypes`, §4.6).
- **M3.4-R10** Stepping `entity_churn`, `add_remove` and `agents` MUST keep `check_invariants` at `Ok` after every tick, for as many ticks as R9.
- **M3.4-R11** Both bench crates MUST register every implementation of R9, as `spikes/ecs_storage/README.md` describes.
- **M3.4-R12** `cargo xtask ci` MUST pass, CI MUST be green on Ubuntu, macOS and Windows without workflow changes, and `cargo xtask deny` MUST pass without changes to `deny.toml`.
- **M3.4-R13** Every public item MUST be documented, and the documents of §4.10 MUST be updated as described.

## 6. Cross-cutting concerns

- **Determinism:** archetype ids follow creation order, rows follow push and swap-remove order, and queries visit archetypes in id order, so iteration depends only on the sequence of operations and a run is reproducible within a build ([ADR 0003](../../adr/0003-same-build-determinism.md)). Checksums are order-independent wrapping sums. The `TypeId`, component-list and bundle maps are `BTreeMap`s; `TypeId` order is stable within one build, and nothing observable iterates the `TypeId`-keyed maps. No `HashMap`, ambient state or transcendental function; the workspace lints enforce this.
- **`unsafe`:** none; the crate forbids it. The safe-Rust costs it accepts (a downcast per column per matched archetype and per typed move, bounds-checked row moves, the `enumerate` pass to split column borrows, `for_each` for mutable queries) are part of what M3.7 measures.
- **Concurrency:** n/a, because every workload is single-threaded. Parallel iteration is M7's.
- **Heap allocation:** `setup` may allocate, and so does the first transition or bundle of each kind (a new archetype, its columns, its edge vectors, a `BundleInfo`). In the steady state, tables keep their capacity and no archetype is created, so moves and spawns do not allocate unless a table grows; `agents` allocates one box per seeker change. Per-query-run allocations are counted at close-out (lesson ecs-2). Not measured here; #41 adds space measurement.
- **`no_std` and wasm:** n/a, because spike crates are not in those builds.
- **Frozen formats:** none.
- **Public API changes:** none in `libs/`. The crate's API is used only by its own workload modules and tests.
- **FFI exposure:** n/a.
- **Performance:** the pull request shows one informal `cargo xtask spike-bench ecs` run, and a cache-bench run if a Valgrind host is available, labelled as informal. M3.8 records results under the benchmark run protocol.
- **Documentation:** item docs, the roadmap, the spike README and CHANGELOG (§4.10), and this spec and its plan.
- **Policy changes:** none.

## 7. Alternatives considered

- **Insert one component at a time, as M3.3 does.** Rejected by the maintainer: each respawn in `entity_churn` and `agents` would pass through four or five archetypes inside the measured `step`, measuring archetypes without their native spawn. Each backend implements a workload in its own idiom ([spikes/ecs_storage/README.md](../../../spikes/ecs_storage/README.md)).
- **Two backends, with and without bundles.** Rejected by the maintainer: more code and bench lines for a predictable gap.
- **A `by_components` lookup on every transition.** Rejected by the maintainer for the edge cache (§4.4).
- **Edges in a `BTreeMap<ComponentId, ArchetypeId>` per archetype.** Rejected: a `Vec` indexed by the dense `ComponentId` is one read, and the few component types in these workloads keep it short.
- **A chunked variant (fixed-size chunks) or a cached-query variant as a second backend.** Rejected by the maintainer: M3.10's bevy_ecs and hecs baselines are the outside archetype comparisons, and caching stays M4's on both models.
- **A `WorldBuilder` with nothing to declare**, to match M3.3's surface. Rejected: an empty builder is noise; `World::new()` is the same call in both workload sets except where M3.3 declares groups.
- **Nested bundles.** Not possible while any `'static` type is a component (§4.5); flat tuples up to 8 cover the widest workload (five).
- **A column index map per archetype (`BTreeMap<ComponentId, usize>`).** Rejected: binary search over a sorted list of at most five ids is as fast and needs no second structure to keep in step.
- **`RefCell` per column for borrow splitting.** Rejected for M3.3's reason (its spec §7): a runtime flag per column, and panics instead of build-time errors.
- **A query trait or crate shared with `spike_ecs_sparse_set`.** Rejected for M3.3's reasons (its spec §7); the hybrid (M3.5) is where one query over two storage kinds is designed.
- **Panicking on a query with no required type, as M3.3 does.** Rejected: here every live entity has a row, so the correct behaviour costs nothing.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | the workspace `Cargo.toml`; `cargo xtask check-unsafe` |
| R2 | unit tests: `insert` on a despawned entity panics with the expected message; `spawn` and `spawn_with` on a full allocator (`GenIdAllocator::with_fixed_capacity`) panic with their own names and leave `len` unchanged; `spawn_with` with a repeated type panics; a query with a repeated type panics; review of the `# Panics` sections |
| R3 | review: `Table`'s fields are private to its module, and every method changes every column and the entity list or none |
| R4 | unit tests in `table.rs` and `world.rs` that corrupt one private field (test code in the same module) and check that `check_invariants` names the broken invariant, one per invariant; the model test |
| R5 | unit tests: insert then remove returns the entity to its first archetype with its other values intact; a second identical transition creates no archetype and hits the edge (a `#[cfg(test)]` counter of `by_components` lookups on the `World`, not a static); the model test |
| R6 | unit test: `spawn_with((A, B, C))` on a fresh world creates exactly one archetype besides the empty one; the model test |
| R7 | query unit tests: required, `Option`, `With`, `Without`, `Entity`, a nested tuple, a never-inserted required type, `query::<Entity>()` over entities in several archetypes including the empty one, `matched_archetypes` for a filter that excludes some; the model test compares a fixed set of queries with the model |
| R8 | unit tests: order (insert then remove of the same type leaves it absent), a command after a despawn of its entity is skipped, `is_empty` after `apply` and capacity kept |
| R5 to R8 together | proptest model test: random `spawn`, `spawn_with` (bundles of one to three of four component types), `despawn`, `insert`, `remove`, recorded `Commands` (`insert`, `remove`, `despawn`) and `apply`, compared with a `BTreeMap<Entity, components>` model (including `len`), `check_invariants` after every operation |
| R9 | conformance tests in `tests.rs`, one per workload; `archetype_count` after `setup` and each step query's `matched_archetypes` (through the step's own query function) asserted per workload |
| R10 | `tests.rs`: invariants checked every tick for `entity_churn`, `add_remove` and `agents` |
| R11 | `cargo xtask bench-smoke` (Criterion ids); `cargo build --benches -p spike_ecs_bench_cache` and a check that each `cache_bench!` line uses its workload's sizes (the cache bench's own size assertion does not run without Valgrind, #74); `cargo xtask spike-bench ecs` output in the pull request |
| R12 | `cargo xtask ci`; `cargo xtask deny`; the pull request's CI run |
| R13 | the `review-docs` review; `cargo xtask doc` |

## 9. Done when

- [ ] `cargo xtask ci` passes locally on the final code commit, and CI is green on all three platforms.
- [ ] The backend passes conformance for all seven workloads, and the invariant-under-churn and model tests pass.
- [ ] `cargo xtask spike-bench ecs` lists the `archetype` backend; its output, and a cache-bench run if a Valgrind host is available, are in the pull request, labelled informal.
- [ ] The roadmap shows M3.4's new title; the spike README, CHANGELOG and coverage list are updated.
- [ ] The reviews of CONTRIBUTING.md "Reviews before a pull request" are recorded in `log.md` and the pull request.
- [ ] The spec is `Implemented`, the plan and log `Historical`, with Deviations filled in.

## 10. Open questions

None. The items of §4.9 are deferred to the chunks named there.

## 11. Prior art and sources

Designs only; no code is taken from any of them ([ADR 0002](../../adr/0002-originality-and-dependency-licensing.md)).

- [bevy_ecs](https://docs.rs/bevy_ecs/): archetypes and tables, the archetype graph of cached add and remove edges, bundles spawned straight into their archetype, tuple queries with `With` and `Without`.
- [hecs](https://docs.rs/hecs/): a small archetype ECS whose tables hold one column per type and whose queries are tuples of references.
- Sander Mertens, [Building an ECS #2: Archetypes and Vectorization](https://ajmmertens.medium.com/building-an-ecs-2-archetypes-and-vectorization-fe21690805f9): archetype tables, the archetype graph, and why contiguous columns vectorize.
- [M3.3 spec](../m3.3-sparse-set/spec.md): the shared query surface, `Commands`, `check_invariants` and the backend conventions this chunk follows.
- [M3.2 spec](../m3.2-spike-harness/spec.md): the harness, the workloads and the conformance check this chunk implements against.
