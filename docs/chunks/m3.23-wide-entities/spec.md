# M3.23: wide_entities workload for the ECS storage harness

- **Status:** Accepted (2026-11-26)
- **Milestone:** M3 (ECS storage decision) · **Depends on:** M3.2, M3.10, M3.26 · **Decides:** no ADR
- **Issue:** #108
- **Estimated size:** about 800 changed lines, excluding tests, lockfiles and generated files (harness module about 160, `vec_aos` about 70, six ECS backends about 70 each, bundle impls about 40, `hybrid`'s component list about 30, bench registration about 30, documents about 50)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds a tenth workload, `wide_entities`, to the ECS storage spike. Every entity carries 24 components of six sizes (544 bytes), and each tick runs one four-component update pass, then adds a 25th component to every entity and removes it again, as `add_remove` does. In `add_remove` each archetype move copies one `f32`. Here a move on table storage copies 24 columns each way, while sparse storage and `hybrid` (with the 25th type sparse) copy nothing, and the storage decision (M3.8, #102) has no measure of that difference. At the end the workload passes conformance against `vec_aos` on `archetype`, `sparse_set`, `dense`, `hybrid`, `bevy_ecs` and `hecs`, both spike benches list it, and an informal Criterion run is in the pull request and on #102.

## 2. Goals and non-goals

Goals:

- Rules for `wide_entities` (§4.2) precise enough that any backend that follows them reproduces the reference checksum bit for bit, in the M3.2 style, all in `spike_ecs_harness`.
- Implementations on `vec_aos` (the reference) and the six backends above, each passing `check_conformance` against `vec_aos`, each with the per-step insert test of M3.26 §4.3.
- Every 24-type entity placed straight into its final archetype on the archetype-storage backends, so no row pays for empty intermediate archetypes (§4.4).
- Registration in `bench_compare` and `bench_cache` with `Spec::SIZES`.

Non-goals:

- No variant backends (`vec_soa`, `sparse_set_grouped`, `bevy_ecs_sparse`, the byte-column backends): the tracking issue (#105) keeps each workload to the seven listed backends.
- No change to how the prototypes move a row; the measured insert and remove are the ones `add_remove` uses.
- No formal results. The informal run is the Criterion bench on the Mac under the benchmark run protocol; the cache bench is registered and built, not run (as M3.10, M3.21 and M3.22). M3.8 measures both.

## 3. Guide-level explanation

The harness defines the 24 wide types and the 25th, `Extra`. A backend's step is one update query and the two passes of `add_remove`. On a prototype `World`:

```rust,ignore
use spike_ecs_harness::wide_entities::{self as rules, Extra, W0, W7, W14, W21};

world
    .query_mut::<(&mut W0, &mut W7, &mut W14, &mut W21)>()
    .for_each(|(a, b, c, d)| rules::update(a, b, c, d));
// then, as add_remove: collect (Entity, W0), insert Extra into each, collect With<Extra>, remove it
```

## 4. Reference-level design

### 4.1 Layout

- `spikes/ecs_storage/harness/src/wide_entities.rs`: the module of §4.2, `pub mod wide_entities` in `lib.rs`.
- One `wide_entities.rs` per backend crate (`vec`, `archetype`, `sparse_set`, `dense`, `hybrid`, `bevy`, `hecs`), with the type `<Prefix>WideEntities` (`VecWideEntities`, `ArchetypeWideEntities`, …) and `BACKEND` the crate's usual name.
- Conformance and per-step tests in each backend's existing tests file; one line per backend in each bench crate.

### 4.2 The workload

Sizes 1,000, 10,000 and 100,000 *(maintainer, from the brainstorm: the sizes of `add_remove`, so each row pairs with one `add_remove` row)*.

The common rules of M3.2 §4.4 apply (initial values from `Prng::from_seed(params.seed).stream(i)` in the listed order, component types defined in the harness, no transcendental functions, hashes as `Fnv1a` call sequences).

- **Components.** 24 wide types `W0` to `W23`, each `pub struct Wk(pub [f32; N(k)])`, where `N(k)` is the `k % 6`-th element of `[1, 2, 3, 4, 8, 16]`: four types each of 4, 8, 12, 16, 32 and 64 bytes, 136 `f32`s or 544 bytes per entity *(maintainer, from the brainstorm)*. And `pub struct Extra(pub f32)`, the type the step adds and removes *(maintainer)*. All derive `Clone, Copy, Debug, PartialEq`. No entity has `Extra` after `setup` or after a step.
- **Spawn.** `spawn(prng: &Prng, i: u64) -> Wide` returns entity `i`'s 24 values: one draw each in `0.0_f32..1.0` from `prng.stream(i)`, `W0`'s elements in index order first, then `W1`'s, through `W23`'s (136 draws). `Wide` is a struct with one public field per type, `w0: W0` to `w23: W23`, deriving `Clone, Copy, Debug, PartialEq`.
- **Step.** Three passes, in this order, each over every entity:
  1. **Update.** `update(&mut W0, &mut W7, &mut W14, &mut W21)`: each element `x` of the four (1 + 2 + 3 + 4 elements; one of each of the four smallest sizes, spread through the type list) becomes `y = x + 0.618_034`, then `y - 1.0` if `y >= 1.0`, else `y`. Values stay in `0.0..1.0`. It is the amended `frag_iter` rule (M3.26 §4.2), whose state M3.26 showed changes at every tick *(my call: a pass over 4 of 24 types, the shape of a movement system on a large entity, where table and sparse storage read only the four columns and `vec_aos` strides over 544-byte rows; simulated in f32 on the pinned toolchain at `count` 1,000, seed 42, with this section's spawn and hash: over 1,000 ticks every entity changes at every tick and no checksum equals an earlier tick's, tick 0 included; `log.md`)*.
  2. **Insert.** For every entity, insert `make_extra(&w0)`, which is `Extra(w0.0[0])`, reading `W0` after the update pass.
  3. **Remove.** For every entity, remove `Extra`. The insert and remove passes MUST be separate, as `add_remove`'s (M3.2 §4.4.4): every insert happens before the first removal.
- **Hash.** `hash(wide: &Wide, extra: Option<&Extra>)`: `f32` of each of the 136 values in spawn order, then `u8(0)` without `Extra`, or `u8(1)` and `f32(extra.0)` with it. Each of the 24 types enters the hash of its own entity, so a move that drops a column, leaves a stale value, or pairs one entity's value with another's changes the checksum. `Extra` is never present when the checksum is taken (it is removed in the same step), so its terms only catch a backend that leaves one behind; the insert pass is checked by §4.5's test.

The harness module exports `WideEntities` (the `Spec` marker, `NAME = "wide_entities"`), `W0` to `W23`, `Extra`, `Wide`, `spawn`, `update`, `make_extra` and `hash`. The 24 type definitions MAY come from one private `macro_rules!` in the module; each type still gets its own doc comment naming its size.

### 4.3 Per backend

Each backend runs the three passes in its own idiom, as its `add_remove` does today: the update pass is one four-type mutable query; the insert pass collects `(entity, W0's first element)` pairs from a read query into a `Vec` reused across steps and inserts `Extra` outside the query; the remove pass collects the entities with `Extra` into a second reused `Vec` and removes it from each (ecs-7: `for_each`, not a `for` loop over `next()`; buffers sized in `setup`).

- **`archetype`:** one table of the 24 types after `setup` (§4.4); the insert and remove use `World::insert` and `World::remove` and their cached edges, so each moves the row between the 24-type and the 25-type table, copying every column.
- **`hybrid`:** `W0` to `W23` are table types and `Extra` is sparse *(maintainer, as `add_remove`'s `B`)*: the insert and remove touch `Extra`'s sparse set only, and no row moves. Setup as `archetype` (§4.4).
- **`sparse_set`, `dense`:** `spawn` and 24 `insert`s per entity in `setup`, in type order; the insert and remove touch `Extra`'s storage only.
- **`bevy_ecs`:** every type in table storage under the crate's `Table<T>` wrapper; entities spawned from one nested tuple bundle of the 24 (bevy bundles nest; a flat tuple is limited to 15); the three queries are `QueryState`s built in `setup`; `world.clear_trackers()` at the end of the step, as its other workloads. Each insert and remove moves the row between two archetypes' tables.
- **`hecs`:** entities spawned through an `EntityBuilder` holding the 24 values (a fresh or a reused builder, the builder's choice: `setup` is not measured) (hecs tuple bundles stop at 15 elements, and a nested tuple would be stored as a single component); `query_mut` per pass; `insert_one` and `remove_one`, as its `add_remove`.
- **`checksum`** (not measured) on the ECS backends reads each entity's 24 values and `Option<&Extra>` by whatever read path the backend has: a query of nested tuples (the prototypes' 4-parameter tuples nest), per-entity `get`, or bevy's and hecs's own queries *(main session's call, from the cold read)*.
- **`vec_aos`:** one `Vec<(Wide, Option<Extra>)>`; the update pass over every row, then the insert pass sets each `Option` to `Some(make_extra(&wide.w0))`, then the remove pass sets it to `None`. It is the reference, not a storage model: no value moves.

### 4.4 Bundles of 24 on `archetype` and `hybrid`

The `Bundle` traits of `spike_ecs_archetype` and `spike_ecs_hybrid` are implemented for flat tuples of 1 to 8 types today. Spawning a 24-type entity as `spawn_with` of 8 types plus 16 `insert`s leaves 16 empty intermediate archetypes, which every later query run matches and walks, a cost that `bevy_ecs` and `hecs` (placed in one step, §4.3) would not pay *(found in the brainstorm)*.

- Both crates' `impl_bundle!` invocations MUST be extended to tuples of 9 to 24 types, with the same macro and no other change; the doc comment that says "1 to 8" says "1 to 24" *(maintainer, from the brainstorm)*.
- Both backends' `setup` MUST spawn each entity with one `spawn_with` of the 24-tuple.
- A test on each MUST assert that after `setup` the world holds exactly the empty archetype and the 24-type archetype, and after one step exactly those and the 25-type archetype (on `archetype`) or the same two (on `hybrid`, where `Extra` is sparse).

### 4.5 Tests beyond conformance

- **Per-step insert test** (M3.26 §4.3, ecs-11). Each of the six ECS backends gets a `#[cfg(test)]` accessor for the remove pass's list of the last step and a test that, at the conformance parameters, after each of 120 steps asserts the list holds each of the `count` live entities exactly once. The accessor MUST read the list the measured step filled. Red check: with the insert pass removed, each fails; with the insert at tick 1 only, each fails at tick 2; recorded in `log.md` with `cargo nextest run -p <package> --no-fail-fast` output (ecs-6).
- **Storage of `Extra` on `bevy_ecs`** (ecs-11). A test asserts after a step that the world holds the 25-type archetype and never registered `Sparse<Extra>`, so a flip of the wrapper fails a test; `hybrid`'s storage is pinned by §4.4's archetype-count test *(main session's call, from the cold read)*.
- **`vec_aos` between-passes hook** (M3.26 §4.4). The step's loop is generic over a closure called between the insert and remove passes; the measured `step` passes a no-op, and a `#[cfg(test)]` method asserts every row holds `Some(make_extra(&wide.w0))` there, after each of 120 steps. Red check: removing the insert pass fails it.
- **Changing state** (ecs-13). A `vec_aos` test runs 120 ticks at the conformance parameters and asserts each tick's checksum differs from every earlier tick's, tick 0 included.
- **Harness tests.** A spawn test (the 136 values equal direct draws from `prng.stream(i)` in order); an `update` test covering the wrapping and the non-wrapping branch; a test that `size_of::<Wk>()` is `4 * N(k)` for every `k` and the sum is 544.

### 4.6 Benchmarks and documents

- `bench_compare/benches/compare.rs`: one `bench::<…>` line per backend. `bench_cache/benches/cache.rs`: one `cache_bench!` per backend with `[1_000, 10_000, 100_000]` and a `wide_entities` group.
- `spikes/ecs_storage/README.md` (ten workloads; a link to §4.2 here as the rules of `wide_entities`; the archetype and hybrid rows' bundle arity), `spikes/README.md` (ten workloads), the harness crate docs, `hybrid`'s `harness_components.rs` (`Extra` sparse, `W0` to `W23` table), and `CHANGELOG.md` (one entry).
- The informal run: `cargo xtask spike-bench ecs` under the benchmark run protocol (`/bench-run`) on the final code commit, labelled informal. Its `wide_entities` rows, next to the same run's `add_remove` rows, go in `log.md`, the pull request, and a comment on #102, with the caveats for M3.8 said as measured or unmeasured: at least that one run gives no noise floor, that `vec_aos` moves nothing, which backends move a row per insert and remove (§4.3), and that the difference from `add_remove`'s rows includes the update pass, which `add_remove` lacks and which is not measured separately: expected small on the table backends, where it touches 40 bytes per entity while the step's two moves copy 1,088, and not small on `vec_aos`, which strides over 544-byte rows (expected, unmeasured; specs-6) *(maintainer, from the grill: a caveat, no separate row or timing)*.

## 5. Requirements

- **M3.23-R1** `spike_ecs_harness::wide_entities` MUST define `WideEntities` with `NAME = "wide_entities"` and `SIZES = [1_000, 10_000, 100_000]`, `W0` to `W23`, `Extra`, `Wide`, `spawn`, `update`, `make_extra` and `hash` as §4.2 gives them.
- **M3.23-R2** `vec_aos` MUST implement `wide_entities` as §4.3 gives it.
- **M3.23-R3** `archetype`, `sparse_set`, `dense`, `hybrid`, `bevy_ecs` and `hecs` MUST each implement `wide_entities` as §4.3 gives it and MUST pass `check_conformance` against `vec_aos` at `count` 1,000, seed 42, for 120 ticks.
- **M3.23-R4** The `Bundle` traits of `archetype` and `hybrid` MUST be implemented for 1 to 24 types, and both backends' `setup` MUST place every entity directly in the 24-type archetype, as §4.4 gives it.
- **M3.23-R5** The tests of §4.5 MUST exist and pass, and their red checks MUST be recorded in `log.md`.
- **M3.23-R6** The workload's own code MUST NOT allocate in `step` beyond the reused buffers' growth; the backends' own allocations are allowed and not changed.
- **M3.23-R7** Both spike benches MUST register `wide_entities` for all seven backends with `Spec::SIZES`; `cargo xtask spike-bench ecs` lists the rows.
- **M3.23-R8** The documents of §4.6 MUST be updated.
- **M3.23-R9** The pull request MUST show one informal `cargo xtask spike-bench ecs` run under the benchmark run protocol, labelled informal, with the `wide_entities` and `add_remove` rows; the same rows MUST be in `log.md` and in a comment on #102, with the caveats of §4.6.
- **M3.23-R10** `cargo xtask ci` MUST pass.

## 6. Cross-cutting concerns

- **Determinism:** no transcendental function; `update` holds the floating-point order (one add, one compare, one subtract per element); within each pass every write touches one entity, so iteration order does not change the result. Conformance proves it per backend.
- **`unsafe`:** none; every crate keeps `#![forbid(unsafe_code)]`.
- **Concurrency:** n/a, because the step is single-threaded.
- **Heap allocation:** none from the workload's code after the reused buffers reach `count` (R6). The prototypes' per-query-run allocations (M3.3, M3.4, M3.19) apply to the step's three queries, as in `add_remove`.
- **`no_std` and wasm:** n/a, because spike crates are outside the `no_std` and wasm builds.
- **Frozen formats:** none touched: the workload draws from `Prng::stream` and `range`, whose streams it does not change.
- **Public API changes:** new public items in the spike crates only (the harness module, the backends' workload types, and `Bundle` for tuples of 9 to 24 on two prototypes); nothing in `engine`.
- **FFI exposure:** n/a, because spikes have no FFI.
- **Performance:** one informal Criterion run (R9), read against the same run's `add_remove` rows; M3.8 records results under the protocol. Up to 24-element tuple impls add compile time to two spike crates only.
- **Documentation:** §4.6; every new public item documented.
- **Policy changes:** none.

## 7. Alternatives considered

- **Remove one of the 24 and add it back** (24 → 23 → 24). Rejected by the maintainer: the same copy volume, but the value must be carried across the passes and the row no longer pairs with `add_remove`'s.
- **Add and remove on a fraction of the entities.** Rejected by the maintainer: closer to a real frame, but not comparable with `add_remove`.
- **`Extra` in table storage on `hybrid`.** Rejected by the maintainer: `hybrid` would repeat `archetype`'s row, and the workload would not show what per-type storage saves.
- **Mostly small types (about 200 bytes), or 24 types of 16 bytes.** Rejected by the maintainer: the first narrows the gap to `add_remove`, the second is not the mixed sizes the issue names.
- **Sizes 1,000 and 10,000, or 100 to 10,000.** Rejected by the maintainer: both lose the pairing with `add_remove` at 100,000, the size where the copy is bound by memory.
- **`spawn_with` of 8 plus 16 inserts on `archetype` and `hybrid`.** Rejected by the maintainer: 16 empty intermediate archetypes stay in the world and every query run walks them, a cost `bevy_ecs` and `hecs` do not pay.
- **Nested bundles on the prototypes.** Rejected by the maintainer: a change to the sealed trait and its invariant checks, beyond the issue.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | review against §4.2; the harness tests of §4.5 |
| R2 | review against §4.3; R3's tests use it as the reference; the changing-state and hook tests of §4.5 |
| R3 | `check_conformance` tests in each backend crate (CI); review against §4.3 |
| R4 | the archetype-count tests of §4.4; review of both `impl_bundle!` lists |
| R5 | the tests in CI; the red checks in `log.md` |
| R6 | review of each `step` |
| R7 | the bench crates build in `ci`; `spike-bench` output in the pull request |
| R8 | `review-docs` |
| R9 | the pull request, `log.md` and the comment on #102; `/bench-run`'s power log |
| R10 | `cargo xtask ci` output |

## 9. Done when

- [ ] The harness module and the seven implementations exist, and every conformance test passes.
- [ ] `archetype` and `hybrid` take bundles of 24 and place every entity in one archetype.
- [ ] The tests of §4.5 pass and their red checks are in `log.md`.
- [ ] Both spike benches register `wide_entities` for the seven backends.
- [ ] The documents of §4.6 are updated.
- [ ] One informal run is in the pull request, `log.md` and on #102.
- [ ] `cargo xtask ci` passes.

## 10. Open questions

None.

## 11. Prior art and sources

- The M3.2 spec §4.4.4 (`add_remove`), the M3.26 spec §4.2 to §4.4 (the update rule and the per-step tests this reuses), and the M3.22 spec (the latest workload, whose layout this follows).
- bevy's nested bundles and hecs's `EntityBuilder`, read as APIs only ([ADR 0002](../../adr/0002-originality-and-dependency-licensing.md)).
