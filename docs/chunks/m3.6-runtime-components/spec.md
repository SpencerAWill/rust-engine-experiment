# M3.6: Runtime-registered components in safe byte columns

- **Status:** Accepted (2026-11-22)
- **Milestone:** M3 (ECS storage decision) · **Depends on:** M3.3, M3.4, M3.5 · **Decides:** no ADR
- **Followed by:** M3.10, the bevy_ecs and hecs baselines (maintainer, 2026-11-22)
- **Issue:** none (tracking issue #66)
- **Estimated size:** about 1,500 changed lines, excluding tests, lockfiles and generated files (`raw` layout and column about 350, `World` raw operations about 280, the raw query about 300, codecs and mirror types about 150, the workload modules about 300, documents about 100)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk lets the archetype prototype (`spike_ecs_archetype`, M3.4) store components that have no Rust type: a component registered at run time from a size and an alignment, kept in a byte column beside the typed `Column<T>`s in the same tables, and read by a query that names it by id. It comes now because two of M3.8's ADRs need it: component identity must work without a `TypeId`, and the ADR on foreign and mod-defined component storage needs the measured cost of safe byte columns, which M3.7's `unsafe` columns are then compared against. At the end, two new backends, `archetype_bytes_copy` and `archetype_bytes_cast`, pass conformance against `vec_aos` for `simple_iter`, `add_remove` and `frag_iter`, and both bench crates list them beside `archetype`.

## 2. Goals and non-goals

**Goals**

- Component identity that covers both kinds: a Rust type keyed by its `TypeId`, and a runtime component known only by the id its registration returned (§4.2).
- A byte column that holds plain-data values of a registered layout and moves rows between tables like `Column<T>` does, through the same cached edges (§4.3, §4.5).
- Two safe access paths over the same column, measured side by side (maintainer): copying each value in and out through a codec, and casting the column to a slice of a `Pod` type with `bytemuck` (§4.4).
- A query over runtime ids that hands out one table's columns at a time, so both access paths loop over a whole column (§4.6).
- An entity can hold typed and runtime components together; each query kind matches on its own ids (§4.2, §4.6).

**Non-goals**

- No drop or clone hooks: a runtime component is plain data, copied bitwise (maintainer). Values that own resources wait for M3.7 or M8.
- No zero-sized runtime components (tags), no runtime components in `Commands`, no optional runtime fetch: none of the three workloads needs them (my call; M4 takes or rejects each).
- No typed query over runtime ids and no raw query over Rust types (§4.6).
- No lookup of a runtime component by name: the name is for `Debug` and error messages (my call; a name registry is M4's).
- No change to the sparse-set, dense or hybrid prototypes, and no runtime backend for `heavy_compute`, `entity_churn`, `sprites` or `agents` (maintainer: the three chosen workloads isolate row access, row moves and matching; the others repeat those costs).
- No `unsafe` (M3.7), no memory measurement (#41), no parallel iteration (M7), no change detection (M6).

## 3. Guide-level explanation

A mod or a C# host has a component the engine was not compiled against. It registers it from its layout and gets an id:

```rust,ignore
let mut world = World::new();
let position = world.register_raw("Position", RawLayout::new(12, 4)?);
let velocity = world.register_raw("Velocity", RawLayout::new(12, 4)?);

let e = world.spawn_raw(&[(position, &bytes_of_position[..]), (velocity, &bytes_of_velocity[..])])?;
```

A system that knows the layout runs a raw query and gets one table at a time. With the copy path it decodes each row, works on the value and encodes it back:

```rust,ignore
let query = RawQuery::new().write(position).read(velocity);
world.query_raw(&query)?.for_each_table(|mut table| {
    let (mut pos, vel) = (table.column_mut(0), table.column(1));
    for (p, v) in pos.rows_mut().zip(vel.rows()) {
        let mut value = Position::decode(p);
        update(&mut value, &Velocity::decode(v));
        value.encode(p);
    }
});
```

With the cast path it views the column as a slice of a `Pod` mirror type of the same layout and works in place:

```rust,ignore
    let pos: &mut [RawVec3] = table.column_mut(0).cast_mut()?;
    let vel: &[RawVec3] = table.column(1).cast()?;
```

Typed components keep their API: an entity with a typed `Matrix` and a runtime `position` lives in one table, `world.query::<&Matrix>()` sees it, and so does a raw query on `position`.

## 4. Reference-level design

### 4.1 Where it goes

The code goes in `spike_ecs_archetype`: a module for the layout and the byte column, the raw operations on `World`, a module for the raw query, and one workload module per workload holding both backends. The typed paths of M3.4 MUST keep their behaviour and their cost, since the typed `archetype` backend is the baseline both new backends are read against: typed `insert`, `remove`, `spawn_with`, `despawn` and query runs MUST NOT gain a lookup, a branch on the component kind or an allocation per call (the kind is needed only when a column is created and by the raw operations), and the informal run (R14) MUST show `archetype` within noise of the same filter run on the parent commit *(main session's call)*.

`bytemuck` (exact version `=1.25.2`, already in `Cargo.lock` through the example crates, licence `Zlib OR Apache-2.0 OR MIT`) with its `derive` feature becomes a dependency of `spike_ecs_archetype` only, and MUST pass `cargo xtask deny`. Its derives compile under `#![forbid(unsafe_code)]` (checked in a scratch crate, 2026-11-22: the lint skips code an external macro generates). Library code (`libs/`) gets no new dependency, and glam's `bytemuck` feature stays off, so the `Pod` types are mirror types the backend defines (§4.4) (my call: turning the glam feature on would change `engine`'s dependency graph in every workspace build that includes the spike).

### 4.2 Component identity

`ComponentId` stays the dense index of a component's info in the world. The info MUST record which kind it is:

- **Rust:** keyed by `TypeId`, registered on first use as today; the `TypeId` map finds it.
- **Raw:** registered by `World::register_raw(name: impl Into<Box<str>>, layout: RawLayout) -> ComponentId`, which MUST return a new id on every call, even for a name or layout already registered. The `TypeId` map never contains it.

A raw component and a Rust type MUST NOT share an id, whatever their layouts. Typed operations name Rust types, so they cannot reach a raw id; every raw operation given a Rust type's id MUST return `RawError::NotRaw` and change nothing. An archetype's component set MAY mix both kinds; its columns stay in ascending `ComponentId` order (M3.4's T2).

`check_invariants` MUST cover raw columns: each one's layout is the layout its id was registered with, and its row count is the table's.

### 4.3 Layout and byte column

`RawLayout::new(size: usize, align: usize) -> Result<RawLayout, LayoutError>` MUST reject a size of 0, an alignment that is not a power of two or is above 16, and a size that is not a multiple of the alignment, each with its own `LayoutError` variant. The cap of 16 is the alignment of the column's storage (my call: it covers every `f32`, `f64` and SIMD-width vector type; a larger alignment is M4's).

A byte column holds `len` values of one layout back to back, value `i` at bytes `i * size .. (i + 1) * size`. Its storage MUST be a `Vec` of a 16-byte, 16-aligned `Pod` block type, so the start of the column is 16-aligned and every value is aligned to its layout. Bytes past `len * size` MUST be zero, so no stale value survives a removal. The column implements M3.4's `ErasedColumn`:

- `swap_remove_drop(row)` copies the last value's bytes over `row` and shortens the column by one value.
- `swap_remove_into(row, dst)` appends the value's bytes to `dst`, then does the same. `dst` MUST be a byte column of the same layout; anything else is a bug in the prototype and panics with a message that says so.

The column MUST grow by amortized doubling as `Vec` does, never by one block per push.

### 4.4 The two access paths

Both paths read and write the same column; they differ only in how workload code turns bytes into values.

- **Copy.** A codec trait implemented by the workload modules for the harness types they use: a `LAYOUT` constant, `encode(&self, out: &mut [u8])` and `decode(bytes: &[u8]) -> Self`. Fields are written as little-endian bytes in declaration order (my call: free on every target we build for, and the layout a save file would use). A codec MUST round-trip every value the workload produces bit for bit, `NaN` payloads included.
- **Cast.** `cast::<T: Pod>()` on a shared column view and `cast_mut::<T: Pod>()` on an exclusive one return the column as `&[T]` or `&mut [T]`, of exactly the table's row count (lesson ecs-9). They MUST return `CastError` if `size_of::<T>()` differs from the layout's size or `align_of::<T>()` exceeds its alignment. The check runs once per column per call, never per row. The workload modules define `#[repr(C)]` mirror types (for example three `f32`s for a `Vec3`) and convert them to and from the harness types around each harness `update` call.

The workload code for each path MUST call the harness's `update` and `hash` functions, as every backend does, so conformance holds bit for bit.

### 4.5 Raw operations on `World`

| Operation | Behaviour |
|---|---|
| `spawn_raw(components: &[(ComponentId, &[u8])]) -> Result<Entity, RawError>` | Spawns an entity straight into the archetype of exactly these ids, found through `by_components` or created once, writing each value into its column in one operation (T1), as `spawn_with` does for a bundle. An empty list spawns into the empty archetype. No intermediate archetype is created. |
| `insert_raw(entity, id, bytes: &[u8]) -> Result<bool, RawError>` | Writes the value; moves the entity through the cached add edge if it lacks `id`, replaces the value in place if it has it. `Ok(true)` if it replaced a value. |
| `remove_raw(entity, id) -> Result<bool, RawError>` | Moves the entity through the cached remove edge and drops the bytes. `Ok(false)` if the entity lacks `id`. |
| `get_raw(entity, id) -> Option<&[u8]>` | The value's bytes, `None` for a dead entity, an unknown or non-raw id, or a missing component. |

`RawError` MUST have a variant each for a dead entity, an unknown id, a Rust type's id (`NotRaw`), a byte slice whose length is not the layout's size (`WrongSize { expected, got }`) and an id listed twice in one `spawn_raw` (`Duplicate`). `spawn_raw` MUST validate every entry before it looks up or creates an archetype or allocates an entity. On any error the world MUST be unchanged, including on the first call of its kind, before any edge is cached (lesson ecs-4). `despawn` MUST drop an entity's raw values with its row, as it does typed ones.

`insert_raw` and `remove_raw` MUST use the same edge cache as `insert::<T>` and `remove::<T>` and MUST NOT do more lookups per call than they do, apart from the kind check on the id (lesson ecs-7).

### 4.6 The raw query

`RawQuery::new()` is built with `.read(id)`, `.write(id)`, `.with(id)` and `.without(id)`; fetch `k` is the `k`-th `read` or `write` call. `World::query_raw(&self, &RawQuery)` for queries that only read, and `query_raw_mut(&mut self, &RawQuery)` for queries that may write, return `Result<_, RawQueryError>`. Before visiting any table they MUST reject an id that is unknown or not raw, an id named twice in any combination of fetches and filters, and, in `query_raw`, a `write` fetch (`RawQueryError::WriteInReadQuery`).

A run matches the archetypes that contain every fetched and every `with` id and no `without` id, in archetype creation order, and calls `for_each_table` once per matching archetype with at least one row. The table view offers:

- `entities() -> &[Entity]`;
- `column(k) -> RawColumn<'t>`, the `k`-th fetch, only for a `read` fetch;
- `column_mut(k) -> RawColumnMut<'t>`, the `k`-th fetch, only for a `write` fetch, at most once per table.

Both return views that live as long as the table borrow, not the call, so a workload holds several at once (`let (pos, vel) = (table.column_mut(0), table.column(1));`). `column` on a write fetch, `column_mut` on a read fetch, and a second `column_mut(k)` on one table each panic with a message naming the fetch and the rule, since each is a bug in the workload; each panic has its own test *(main session's call: a shared view of a write fetch cannot outlive the call in safe code, as a stub of this API showed, 2026-11-22)*. The slots the views are taken from MUST live in one buffer per run, cleared for each table, so a run allocates nothing per table.

A column view offers `len()`, `get(row) -> &[u8]`, `rows()` (each value's bytes) and `cast`; the exclusive view adds `get_mut`, `rows_mut` and `cast_mut`. Neither view, nor anything it returns, can push, remove or resize: the views are newtypes built in the module that owns the column, never `&mut dyn ErasedColumn` or `&mut` to the column itself (lesson ecs-5). The view types implement `Debug`; `RawError`, `RawQueryError`, `LayoutError` and `CastError` implement `Display` and `Error`.

Fetches are by table, not by row, so each access path loops over a whole column with no per-row call into the query (my call: a per-row callback would add the same overhead to both paths and hide the difference between them). A raw query ignores a table's typed columns, and a typed query ignores its raw columns, beyond matching on the ids it names.

The heap allocations and lookups per query run, as built, MUST be counted before close-out and recorded in `log.md` with a note on whether a run's setup grows with the number of registered types (lesson ecs-2).

### 4.7 Workload backends

For each of `simple_iter`, `add_remove` and `frag_iter`, the chunk adds two `Workload` impls with `BACKEND` names `archetype_bytes_copy` and `archetype_bytes_cast`. Setup registers every component the workload uses as a raw component (four in `simple_iter`, two in `add_remove`, 27 in `frag_iter`) and spawns entities `0..n` from the harness's `spawn` function with one `spawn_raw` each, so the world has the same archetypes as the typed `archetype` backend's after setup and after every step (maintainer, 2026-11-22: one insert per component would leave empty intermediate archetypes, 53 against 27 in `frag_iter`, which every query run scans and M3.8 would read as a cost of byte columns). `spawn_raw` is not on any measured path, so a scratch allocation per call to sort the ids is allowed and recorded with R13's counts. The steps mirror the typed `archetype` backend's:

- `simple_iter`: one `query_raw_mut` writing `Position` and reading `Velocity`.
- `add_remove`: the same two passes as `archetype` (spec M3.4 §4.7): collect the entities with `A`, `insert_raw` a `B` into each; collect the entities with `B`, `remove_raw` it from each. The checksum reads `A` without `B`, then `A` and `B` with `B`.
- `frag_iter`: one `query_raw_mut` writing `Data`; the checksum runs one read query per letter.

Each backend MUST pass `check_conformance` against `vec_aos` with the ticks the typed `archetype` tests use, and MUST be listed in `bench_compare` and in `bench_cache` with the workload's `Spec::SIZES`.

**Expected, unmeasured:** in `simple_iter` the cast path SHOULD be within noise of `archetype`, since its loop is the same over slices of the same layout; the copy path's gap is the cost the ADR on foreign components weighs. In `add_remove` both new backends SHOULD be close to `archetype`, since a row move copies bytes either way. These are predictions for the informal run to check, not requirements.

## 5. Requirements

- **M3.6-R1** `RawLayout::new` MUST reject the three invalid layouts of §4.3, each with its own error.
- **M3.6-R2** `register_raw` MUST return a new id on every call; a raw id and a Rust type's id MUST NOT coincide; every raw operation and raw query given a Rust type's id MUST fail with `NotRaw` and change nothing (§4.2).
- **M3.6-R3** The byte column MUST keep values aligned to their layout, zero the bytes past its values, and move and remove rows as §4.3 says; `check_invariants` MUST cover raw columns.
- **M3.6-R4** `spawn_raw`, `insert_raw`, `remove_raw` and `get_raw` MUST behave as the table in §4.5 says, and MUST leave the world unchanged on each `RawError` variant, tested on the cold path.
- **M3.6-R5** `insert_raw` and `remove_raw` MUST use the typed operations' edge cache, with no extra lookups beyond the kind check (§4.5).
- **M3.6-R6** `cast` and `cast_mut` MUST return slices of exactly the row count and MUST reject a size or alignment mismatch with `CastError` (§4.4).
- **M3.6-R7** Each codec MUST round-trip its type bit for bit, `NaN` payloads included (§4.4).
- **M3.6-R8** `query_raw` and `query_raw_mut` MUST reject unknown, non-raw and repeated ids, and `query_raw` a write fetch, before visiting a table; they MUST visit exactly the matching non-empty archetypes in creation order; the table view's three misuse panics (§4.6) MUST each fire with their own message.
- **M3.6-R9** The column views MUST NOT offer any way to change a column's length (§4.6).
- **M3.6-R10** An entity holding both kinds MUST be seen by a typed query on its Rust types and by a raw query on its raw ids, and a mixed world MUST pass `check_invariants` after inserts, removes and despawns of both kinds.
- **M3.6-R11** The six new backends MUST pass `check_conformance` against `vec_aos`, and each MUST have the same archetype count as the typed `archetype` backend for its workload after setup and after a step (§4.7).
- **M3.6-R12** The six new backends MUST appear in `bench_compare` and `bench_cache`, the latter with the workload's `Spec::SIZES`.
- **M3.6-R13** The per-run allocations and lookups of the raw query, as built, MUST be recorded in `log.md` (§4.6).
- **M3.6-R14** One informal `cargo xtask spike-bench ecs` run under the benchmark run protocol MUST be recorded in `log.md` and quoted, labelled informal, in the pull request, with the cast and copy paths read against `archetype`.
- **M3.6-R15** The typed paths MUST keep their cost (§4.1).

## 6. Cross-cutting concerns

- **Determinism:** no new hash collections, clocks or global state. Archetypes are visited in creation order. Checksums come from decoded values through the harness's `hash`, never from raw bytes, so padding cannot reach them; the zeroed tail (§4.3) keeps the bytes deterministic anyway.
- **`unsafe`:** none. `bytemuck`'s derives compile under `forbid(unsafe_code)`; `bytemuck` itself contains `unsafe`, as `std` does, and is a dependency like any other under ADR 0001.
- **Concurrency:** n/a, because the prototype is single-threaded (M7).
- **Heap allocation:** byte columns grow by amortized doubling; per-query-run allocations are counted and recorded (R13). No allocation per row.
- **`no_std` and wasm:** n/a, because spike crates are outside the `no_std` and wasm builds and no library code changes.
- **Frozen formats:** none. The little-endian codec layout is the spike's, not a save format.
- **Public API changes:** none in `engine`. `spike_ecs_archetype` gains the raw types and operations; the spike crates have no consumers outside the workspace.
- **FFI exposure:** none yet. The raw API is the shape M8's C ABI will need (ids, layouts, byte slices); M8 decides it.
- **Performance:** wall clock for the six backends against `archetype`, `sparse_set` and `vec_aos` in one informal `spike-bench` run (R14); cache-bench numbers only if a Valgrind host is available, and the pull request says which.
- **Documentation:** the `spikes/ecs_storage/README.md` table row for `archetype/` names the raw components and the two backends; the M3.8 roadmap row gets this chunk's caveats at close-out (the R13 counts, any surprise from R14).
- **Policy changes:** none. A new dependency in a spike crate is covered by `cargo xtask deny`.

## 7. Alternatives considered

- **The hybrid prototype, or a new crate.** Rejected (maintainer): the hybrid's open caveats in `agents` and `sprites` would be mixed into the byte-column numbers, and archetype and hybrid share the table design, so the result carries over. A copy of the archetype crate would duplicate about 4,000 lines.
- **Widening the field first (more variants and tunings of each model) or shipping several models.** Rejected (maintainer): the measured gaps are structural; the variants (#80, #81, #88) are queued on M3.8's choice; M3.10's baselines check how well tuned the prototypes are; and every later milestone would have to support each model shipped.
- **Copy path only, on `Vec<u8>`.** Rejected (maintainer): the cast path may close most of the gap safely, which changes what M3.7 has to win.
- **glam's `bytemuck` feature, so harness types are `Pod`.** Rejected (my call): it changes `engine`'s dependency graph in workspace builds; mirror types are also what a mod-defined type looks like.
- **Drop and clone hooks in the layout.** Rejected (maintainer): nothing in M3 needs them; M3.7 or M8.
- **A per-row raw query callback.** Rejected (my call): same overhead on both paths, hiding their difference (§4.6).

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | One unit test per `LayoutError` variant, plus a valid layout at alignment 16 |
| R2 | Tests: two registrations with one name and layout get different ids; each raw operation and both raw queries given a typed id return `NotRaw`, and the world is unchanged (`check_invariants` and archetype count) |
| R3 | Byte-column unit tests: a `cast` at the layout's alignment succeeds for each alignment 1 to 16; after `swap_remove_drop` and `swap_remove_into` the tail bytes are zero and the moved values are intact; `check_invariants` fails on a corrupted raw column (a red check in the storage's own tests, lesson ecs-6) |
| R4 | One test per row of §4.5 and per `RawError` variant, each on a fresh world (cold path) asserting the error value and an unchanged world (entity count, archetype count, `check_invariants`); a test that `spawn_raw` creates exactly one archetype for a new id set and none for a known one |
| R5 | Code review against `insert::<T>` and `remove::<T>` (lesson ecs-7); a test that a toggle cycle creates no archetype after the first |
| R6 | Tests: `cast` and `cast_mut` lengths equal the row count; a size mismatch and an alignment mismatch return `CastError` |
| R7 | Proptest per codec over arbitrary bit patterns, comparing `to_bits` |
| R8 | Tests: each rejection before any table is visited (a counting closure stays at 0); a world with matching, non-matching and empty archetypes visits the matching non-empty ones in creation order; one `should_panic` test per table-view misuse, asserting its message |
| R9 | Code review of the view types' methods (lesson ecs-5) |
| R10 | A mixed-world test, and a proptest model over `insert`, `remove`, `spawn_raw`, `insert_raw`, `remove_raw` and `despawn` checking `check_invariants` and `get`/`get_raw` against a model after each operation |
| R11 | Six conformance tests in `spike_ecs_archetype`'s tests; six tests comparing `archetype_count()` with the typed backend's after setup and after one step |
| R12 | The bench crates' source; `cargo xtask bench-smoke`; the cache bench's size assertion (on a Valgrind host, since #74 is open) |
| R13 | `log.md` entry with the counts and the commit they ran on, measured from a scratch crate (lesson ecs-2) |
| R14 | `log.md` entry and the pull request (`/bench-run` protocol) |
| R15 | Code review of the typed paths against the parent commit (lesson ecs-7); an informal `spike-bench` run of `archetype` on the parent commit, saved as a baseline and compared with R14's run, in `log.md` |

## 9. Done when

- [ ] R1 to R15 have their evidence.
- [ ] `cargo xtask ci` passes on the final commit.
- [ ] `spikes/ecs_storage/README.md` updated; the M3.8 roadmap row carries this chunk's caveats; the roadmap row reads `Done (PR #n)`.

## 10. Open questions

None. Deferred: drop and clone hooks (M3.7 or M8); zero-sized runtime components, raw `Commands`, optional raw fetches and lookup by name (M4).

## 11. Prior art and sources

- Bevy's `ComponentDescriptor` and its `Layout`-based registration of components without a Rust type, and flecs's runtime components registered by size and alignment: studied as designs only (ADR 0002).
- `bytemuck` documentation for `Pod`, `cast_slice` and `cast_slice_mut`.
