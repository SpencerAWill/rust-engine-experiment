# M3.24: hierarchy workload for the ECS storage harness

- **Status:** Draft (2026-11-27)
- **Milestone:** M3 (ECS storage decision) · **Depends on:** M3.2, M3.10, M3.21, M3.26 · **Decides:** no ADR
- **Issue:** #110
- **Estimated size:** about 1,050 changed lines, excluding tests, lockfiles and generated files (harness module about 220, `vec_aos` about 80, six ECS backends about 110 each, bench registration about 30, `hybrid`'s component list about 20, documents about 50)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds an eleventh workload, `hierarchy`, to the ECS storage spike: a forest of trees one to eight levels deep, in which every tick rotates each entity's local transform and then propagates world transforms from the roots down, one level at a time, each child composing its parent's world transform of the same tick with its own local one. M4's `WorldTransform2d` needs this propagation in every frame, and the storage decision (M3.8, #102) has no workload whose cost depends on how a model stores the parent link and reaches a parent from its child. At the end the workload passes conformance against `vec_aos` on `archetype`, `sparse_set`, `dense`, `hybrid`, `bevy_ecs` and `hecs`, both spike benches list it, and an informal Criterion run is in the pull request and on #102.

## 2. Goals and non-goals

Goals:

- Rules for `hierarchy` (§4.2) precise enough that any backend that follows them reproduces the reference checksum bit for bit, in the M3.2 style, all in `spike_ecs_harness`.
- Implementations on `vec_aos` (the reference) and the six backends above, each passing `check_conformance` against `vec_aos`, all six ECS backends running the same level-by-level algorithm (§4.3).
- Registration in `bench_compare` and `bench_cache` with `Spec::SIZES`.

Non-goals:

- No structural change in the step: no entity is spawned, despawned or reparented, and no component is added or removed. Reparenting moves an entity between depth levels (and, on table storage, between tables); `add_remove` and `wide_entities` already measure such moves.
- No children list and no walk from the roots through children (§7). The parent link is the only stored relation.
- No use of `bevy_ecs`'s or `hecs`'s own hierarchy or transform propagation: both baselines run the harness's components and algorithm, as in every other workload.
- No variant backends (`vec_soa`, `sparse_set_grouped`, `bevy_ecs_sparse`, the byte-column backends): the tracking issue (#105) keeps each workload to the seven listed backends.
- No formal results. The informal run is the Criterion bench on the Mac under the benchmark run protocol; the cache bench is registered and built, not run (as M3.21 to M3.23). M3.8 measures both.

## 3. Guide-level explanation

A backend's step is one update pass, one root pass and seven level passes. On a prototype `World`, for depth `k` from 2 to 8:

```rust,ignore
use spike_ecs_harness::hierarchy::{self as rules, Depth3, GlobalTransform, LocalTransform, Parent};

// read: the parents (depth 2) were written by the previous level's pass
let globals = world.lookup::<GlobalTransform>();
world
    .query::<(&LocalTransform, &Parent<Entity>)>()
    .with::<Depth3>()
    .for_each(|(local, parent)| {
        scratch.push(rules::place(globals.get(parent.0).expect("parent is live"), local))
    });
// write, in the same order
let mut next = scratch.iter();
world
    .query_mut::<&mut GlobalTransform>()
    .with::<Depth3>()
    .for_each(|global| *global = *next.next().expect("one entry per entity"));
```

## 4. Reference-level design

### 4.1 Layout

- `spikes/ecs_storage/harness/src/hierarchy.rs`: the module of §4.2, `pub mod hierarchy` in `lib.rs`.
- One `hierarchy.rs` per backend crate (`vec`, `archetype`, `sparse_set`, `dense`, `hybrid`, `bevy`, `hecs`), with the type `<Prefix>Hierarchy` (`VecHierarchy`, `ArchetypeHierarchy`, …) and `BACKEND` the crate's usual name.
- Conformance and other tests in each backend's existing tests file; one line per backend in each bench crate.

### 4.2 The workload

Sizes 1,000, 10,000 and 100,000 *(maintainer, from the brainstorm: the sizes of `add_remove` and `wide_entities`; about 70 bytes per entity puts 100,000 near 7 MB, inside the 16 MB L2 of the maintainer's M4 Pro)*.

The common rules of M3.2 §4.4 apply (initial values from `Prng::from_seed(params.seed).stream(i)` in the listed order, component types defined in the harness, hashes as `Fnv1a` call sequences), with one exception: the propagation calls `WorldTransform2d::mul_transform`, which calls `engine::math::scalar::sin_cos` *(maintainer, from the brainstorm: the engine's types, which the M3.8 transform-layout ADR is about)*. Conformance compares backends within one build, the determinism ADR 0003 promises, so no golden checksum is added.

- **Components.** Every type derives `Clone, Copy, Debug, PartialEq`.
  - `SpawnId(pub u64)`: the spawn index.
  - `LocalTransform(pub Transform2d)` and `GlobalTransform(pub WorldTransform2d)`, the engine's types (20 and 32 bytes) *(my call: `World…` names would clash with every backend's `World`)*.
  - `Parent<E>(pub E)`: the parent's handle, generic over the backend's entity handle as `random_access`'s `Target<E>` is. Roots have none.
  - `Depth1` to `Depth8`, unit structs: an entity at level `k` (roots are level 1) has `Depth<k>` and no other depth type.
- **Forest.** `forest(prng: &Prng, count: usize) -> Forest`, an iterator of `count` `Node { id: u64, level: u32, parent: Option<u64>, local: LocalTransform }` in spawn order, ids `0..count`, every parent's id lower than its children's. It keeps a stack of pending nodes `(level, parent, depth)`, empty at the start, and for each id `i` in `0..count`:
  1. If the stack is empty, push `(1, None, 0)`: a new root. Pop `(level, parent, depth)`.
  2. Draw from `prng.stream(i)`, in this order: if `level` is 1, the tree's depth `depth = range(1_u32..=8)`; then translation x and y, each `range(0.0_f32..320.0)` and `range(0.0_f32..180.0)` for a root and `range(-16.0_f32..16.0)` for any other node; rotation `range(-PI..PI)` (`core::f32::consts::PI`); scale x then y, each `range(0.5_f32..2.0)`; then, if `level < depth`, the child count `range(1_u32..=2)`.
  3. Emit node `i`, and push the child count copies of `(level + 1, Some(i), depth)`.

  Children are pushed and popped from the same stack, so each subtree is spawned in full before its next sibling (pre-order, as a prefab spawns) *(maintainer, from the brainstorm)*. The last tree is cut off at `count`: some of its nodes have fewer children than they drew. In the grill's stub (the real `Prng`, seed 42), every level holds 5 to 20% of the entities at `count` 1,000 and 6 to 18% at 100,000, and a tree holds 17 to 20 entities on average, 152 at most at 100,000 (`log.md`) *(the shape is the maintainer's; the ranges are my call: roots in the `random_access` world, children within 16 units of their parent, and scales whose product over eight levels stays in `2^-8..2^8`)*.
- **Setup.** Spawn every node in `forest` order with `SpawnId(id)`, its `LocalTransform`, `GlobalTransform(WorldTransform2d::IDENTITY)`, its depth type, and, unless it is a root, `Parent(handle)` with the handle of node `parent`, which was spawned earlier. Then run the root pass and the level passes of the step once, so that `GlobalTransform` is consistent with the locals after `setup` (tick 0).
- **Step.** In this order:
  1. **Update.** For every entity, `spin(&mut LocalTransform)`: `r = rotation + SPIN` with `SPIN = 0.05`, then `r - TAU` if `r > PI`, else `r`. Rotations stay in `(-PI, PI]` with one add, one compare and at most one subtract, and no trig *(my call: the local rotation changes every tick, so every world transform below a root changes every tick, and the next tick's propagation cannot reuse a value. The rotation's period is about 126 ticks, but in the grill's stub at `count` 1,000, seed 42, over 1,000 ticks no checksum equals an earlier tick's, tick 0 included, and every entity's `GlobalTransform` changes at every tick (`log.md`))*.
  2. **Roots.** For every entity with `Depth1`: `GlobalTransform(local.0.to_world(DVec2::ZERO))`.
  3. **Levels.** For `k` from 2 to 8 in ascending order, for every entity with `Depth<k>`: `place(&parent_global, &local)`, which is `GlobalTransform(parent_global.0.mul_transform(local.0))`, where `parent_global` is the parent's `GlobalTransform` as level `k - 1` wrote it in this step. Every write in level `k` reads only level `k - 1`, so the order within a level does not change the result.
- **Hash.** `hash(id: &SpawnId, level: u32, parent: Option<u64>, local: &LocalTransform, global: &GlobalTransform)`: `u64(id)`, `u32(level)`, `u64(parent's spawn id)` or `u64(u64::MAX)` for a root, then the local's `vec2(translation)`, `f32(rotation)`, `vec2(scale)`, then the global's `u64(translation.x.to_bits())`, `u64(translation.y.to_bits())`, `f32(rotation)`, `vec2(scale)` *(my call: `Fnv1a` has no `f64` method; adding one is not needed for one workload)*. The parent term catches a wrong link at setup; the global terms catch a stale parent (read before its level was written), a skipped level, or a value written to the wrong entity.

The harness module exports `Hierarchy` (the `Spec` marker, `NAME = "hierarchy"`), the component types, `LEVELS = 8`, `SPIN`, `Node`, `Forest`, `forest`, `spin`, `root`, `place` and `hash`; `root(&LocalTransform) -> GlobalTransform` is step 2's rule, and backends call these and do not write their own arithmetic.

### 4.3 The level passes

*(maintainer, from the brainstorm)* Every ECS backend, the baselines included, runs the step as:

- **Update:** one mutable query over `LocalTransform`, calling `spin`.
- **Roots:** one mutable query over `(&LocalTransform, &mut GlobalTransform)` filtered by `Depth1`.
- **Level `k`, read:** one read-only query over `(&LocalTransform, &Parent<E>)` filtered by `Depth<k>`, with one lookup of the parent's `GlobalTransform` per entity, pushing `place(..)` to a scratch `Vec<GlobalTransform>` the workload owns, cleared before each level and reserved for `count` entries in `setup`.
- **Level `k`, write:** one mutable query over `GlobalTransform` filtered by `Depth<k>`, taking the `i`-th scratch entry for the `i`-th entity. Every entity of a level has the same component types and both queries match the same storage, so each backend's iteration order is the same for both; conformance catches a backend where it is not.
- **The lookup** resolves the type once per level, as M3.21 §4.3 does: `World::lookup::<GlobalTransform>()` on the four prototypes; `World::view::<&GlobalTransform>()` then `ViewBorrow::get` on `hecs`; a `QueryState<&Table<GlobalTransform>>` built in `setup` with `get_manual` on `bevy_ecs`. No backend uses `World::get` in the step.
- **`bevy_ecs`:** every type in table storage under the crate's `Table<T>` wrapper; the 17 queries are `QueryState`s built in `setup`; `world.clear_trackers()` at the end of the step, as its other workloads.
- **`hecs`:** `query_mut` for the update, root and write passes, and `query` beside the view for the read passes.
- **`hybrid`:** every type a table type *(my call, as `random_access`: nothing is added or removed, which is what `hybrid`'s sparse storage is for)*. The level passes then match one table per level, as on `archetype`.
- A backend MAY write the seven levels as one generic function over the depth type, called once per level; it MUST NOT fold several levels into one query.
- **`checksum`** (not measured) reads each entity's `SpawnId`, depth, `LocalTransform`, `GlobalTransform` and `Option<&Parent<E>>`, and the parent's `SpawnId` by any read path; the depth comes from eight filtered queries or from `Option<&Depth<k>>` terms.

On table storage each level is its own table, so a level's queries walk one table and each lookup reaches into the previous level's; on `sparse_set` and `dense` the filters walk the depth type's set and probe the others. Pre-order spawning puts a parent and its children close in spawn order, which keeps lookups near each other on every backend.

### 4.4 The reference

`vec_aos` keeps one `Vec` of `(SpawnId, u32 level, Option<u32> parent index, LocalTransform, GlobalTransform)` in spawn order, so the parent's index is its spawn id. Its step runs the update over every row, then one pass in spawn order that sets each row's global with `root` or with `place` from the parent's row: pre-order puts every parent before its children, so the parent's global is already this tick's. The result is the same as the level passes, since `place` reads only the parent's global and the entity's local. It is the reference, not a storage model.

### 4.5 Tests beyond conformance

- **Conformance** at `count` 1,000, seed 42, for 120 ticks, against `vec_aos`, on each of the six ECS backends.
- **Stale parent** (ecs-11), a red check, not a kept test. On each of the six ECS backends, with all seven read passes run before any write pass (so a level reads its parents' globals of the previous tick), the conformance test MUST fail (at tick 0 in the grill's stub, since `setup` runs the same propagation); recorded in `log.md` with `cargo nextest run -p <package> --no-fail-fast` output (ecs-6).
- **Changing state** (ecs-13). A `vec_aos` test runs 120 ticks at the conformance parameters and asserts each tick's checksum differs from every earlier tick's, tick 0 included, and that every entity's `GlobalTransform` changes at every tick.
- **Harness tests.** A forest test at `count` 1,000 and 100,000, seed 42: ids are `0..count` in order; every parent id is lower than its child's and its level is one less; every level from 1 to 8 holds at least one entity; no level exceeds 8. A spawn test: node 0's draws equal direct draws from `prng.stream(0)` in §4.2's order. A `spin` test covering the wrapping and the non-wrapping branch. A `place` test against a hand composition.
- **Table placement** on `archetype` and `hybrid`: after `setup` the world holds the empty archetype and exactly eight others, one per level, and the same after a step.

### 4.6 Benchmarks and documents

- `bench_compare/benches/compare.rs`: one `bench::<…>` line per backend. `bench_cache/benches/cache.rs`: one `cache_bench!` per backend with `[1_000, 10_000, 100_000]` and a `hierarchy` group.
- `spikes/ecs_storage/README.md` (eleven workloads; a link to §4.2 here as the rules of `hierarchy`; one sentence on what the workload measures), `spikes/README.md` (eleven workloads), the harness crate docs, `hybrid`'s `harness_components.rs` (every `hierarchy` type a table type), and `CHANGELOG.md` (one entry).
- The informal run: `cargo xtask spike-bench ecs` under the benchmark run protocol (`/bench-run`) on the final code commit, labelled informal. Its `hierarchy` rows go in `log.md`, the pull request, and a comment on #102, with the caveats for M3.8 said as measured or unmeasured: at least that one run gives no noise floor; that every row holds one `sin_cos` per non-root entity (in `mul_transform`'s rotation; `spin`, `to_world` and the in-range `wrap_angle` call none), the same work on every backend, which narrows the ratios between backends (unmeasured separately); that `vec_aos` runs one pass and no lookup; and that pre-order spawning keeps parent lookups close (a shuffled order is not measured).

## 5. Requirements

- **M3.24-R1** `spike_ecs_harness::hierarchy` MUST define `Hierarchy` with `NAME = "hierarchy"` and `SIZES = [1_000, 10_000, 100_000]`, and the items of §4.2 as §4.2 gives them.
- **M3.24-R2** `vec_aos` MUST implement `hierarchy` as §4.4 gives it.
- **M3.24-R3** `archetype`, `sparse_set`, `dense`, `hybrid`, `bevy_ecs` and `hecs` MUST each implement `hierarchy` as §4.3 gives it and MUST pass `check_conformance` against `vec_aos` at `count` 1,000, seed 42, for 120 ticks.
- **M3.24-R4** The tests of §4.5 MUST exist and pass, and their red checks MUST be recorded in `log.md`.
- **M3.24-R5** The workload's own code MUST NOT allocate in `step` beyond the reused scratch buffer's growth; the backends' own allocations are allowed and not changed.
- **M3.24-R6** Both spike benches MUST register `hierarchy` for all seven backends with `Spec::SIZES`; `cargo xtask spike-bench ecs` lists the rows.
- **M3.24-R7** The documents of §4.6 MUST be updated.
- **M3.24-R8** The pull request MUST show one informal `cargo xtask spike-bench ecs` run under the benchmark run protocol, labelled informal, with the `hierarchy` rows; the same rows MUST be in `log.md` and in a comment on #102, with the caveats of §4.6.
- **M3.24-R9** `cargo xtask ci` MUST pass.

## 6. Cross-cutting concerns

- **Determinism:** `mul_transform` calls `scalar::sin_cos`, the engine's funnel, so results are fixed within one build (ADR 0003), which is what conformance checks; no golden checksum is added, as for `heavy_compute`. `spin`, `root` and `place` hold the floating-point order; within a level every write touches one entity and reads only the level above, so iteration order does not change the result.
- **`unsafe`:** none; every crate keeps `#![forbid(unsafe_code)]`.
- **Concurrency:** n/a, because the step is single-threaded.
- **Heap allocation:** none from the workload's code after the scratch buffer reaches its largest level (R5). The prototypes' per-query-run allocations (M3.3, M3.4, M3.19) apply to each of the step's 16 queries.
- **`no_std` and wasm:** n/a, because spike crates are outside the `no_std` and wasm builds.
- **Frozen formats:** none touched: the workload draws from `Prng::stream` and `range`, whose streams it does not change.
- **Public API changes:** new public items in the spike crates only (the harness module and the backends' workload types); nothing in `engine`.
- **FFI exposure:** n/a, because spikes have no FFI.
- **Performance:** one informal Criterion run (R8); M3.8 records results under the protocol.
- **Documentation:** §4.6; every new public item documented.
- **Policy changes:** none.

## 7. Alternatives considered

- **A walk from the roots through a `Children` list** (as `bevy_transform` propagates). Rejected by the maintainer: the writes need a mutable by-handle path the four prototypes do not have (`World::get_mut` resolves the type on every call), so the chunk would add one to each, and the parent link would be stored twice.
- **An upward walk per entity**, each composing its ancestors' locals. Rejected by the maintainer: not parent first, and its lookups grow with depth.
- **Trig-free harness transforms**, with rotation stored as a cosine and sine pair. Rejected by the maintainer: they would not be M4's layout, which M3.8's transform ADR decides.
- **One to three children per node, or a shuffled spawn order.** Rejected by the maintainer: the first puts half the entities on the last two levels; the second is a different question (a hierarchy built over time) and can be a variant later.
- **Sizes 10,000 to 1,000,000.** Rejected by the maintainer: the sizes of `add_remove` and `wide_entities` were preferred.
- **A depth component holding the level, with one pass per level over every entity.** My call against: each level would visit every entity, eight times the work, and no storage model could use the level to skip entities.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | review against §4.2; the harness tests of §4.5 |
| R2 | review against §4.4; R3's tests use it as the reference; the changing-state test |
| R3 | `check_conformance` tests in each backend crate (CI); review against §4.3 |
| R4 | the tests in CI; the red checks in `log.md` |
| R5 | review of each `step`; an allocation count per step (ecs-2): after 10 warm-up steps the count is the same at every one of 100 steps, 0 on `vec_aos`, `bevy_ecs` and `hecs`, and on the prototypes the sum of their per-query-run allocations over the step's 16 query runs (counted the same way in the same binary) *(main session's call, from the cold read)* |
| R6 | the bench crates build in `ci`; `spike-bench` output in the pull request |
| R7 | `review-docs` |
| R8 | the pull request, `log.md` and the comment on #102; `/bench-run`'s power log |
| R9 | `cargo xtask ci` output |

## 9. Done when

- [ ] The harness module and the seven implementations exist, and every conformance test passes.
- [ ] The tests of §4.5 pass and their red checks are in `log.md`.
- [ ] Both spike benches register `hierarchy` for the seven backends.
- [ ] The documents of §4.6 are updated.
- [ ] One informal run is in the pull request, `log.md` and on #102.
- [ ] `cargo xtask ci` passes.

## 10. Open questions

None.

## 11. Prior art and sources

- The M3.21 spec §4.2 and §4.3 (`random_access`'s two passes and the per-pass `Lookup`, whose shape the level passes reuse), the M3.23 spec (the latest workload, whose layout this follows), and M3.26 §4.3 (red checks that make a workload's checks fail).
- `engine::math::Transform2d` and `WorldTransform2d` (`libs/engine/src/math/transform2d.rs`).
- Flecs's cascade queries (iteration in depth order) and `bevy_transform`'s propagation through `Children`, read as designs only ([ADR 0002](../../adr/0002-originality-and-dependency-licensing.md)).
