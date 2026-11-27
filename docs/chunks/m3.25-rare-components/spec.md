# M3.25: rare_components workload for the ECS storage harness

- **Status:** Accepted (2026-11-27)
- **Milestone:** M3 (ECS storage decision) · **Depends on:** M3.2, M3.10, M3.26 · **Decides:** no ADR
- **Issue:** #109
- **Estimated size:** about 500 changed lines, excluding tests, lockfiles and generated files (harness module about 90, `vec_aos` about 50, six ECS backends about 45 each, bench registration about 30, documents about 60)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds a twelfth workload, `rare_components`, to the ECS storage spike. Every entity has a `Common` component and about 1% of them, chosen by a per-entity draw, also have `Rare`; each tick one query `(&mut Common, &Rare)` updates the rare entities. Finding few entities in a large world costs very different time across storage models (`vec_aos` and `dense` walk the whole index space, table storage walks one small table, `sparse_set` walks `Rare`'s set and probes `Common`'s sparse index at scattered indices), and the storage decision (M3.8, #102) has no workload that measures it. At the end the workload passes conformance against `vec_aos` on `archetype`, `sparse_set`, `dense`, `hybrid`, `bevy_ecs` and `hecs`, both spike benches list it, and an informal Criterion run is in the pull request and on #102.

## 2. Goals and non-goals

Goals:

- Rules for `rare_components` (§4.2) precise enough that any backend that follows them reproduces the reference checksum bit for bit, in the M3.2 style, all in `spike_ecs_harness`.
- Implementations on `vec_aos` (the reference) and the six backends above, each passing `check_conformance` against `vec_aos`.
- Registration in `bench_compare` and `bench_cache` with `Spec::SIZES`.

Non-goals:

- **Space.** Heap bytes per backend (a flat sparse index's 4 bytes per entity per type, #81; `dense`'s slot per entity) wait for M3.12 (#41), which is open with no spec. This chunk measures time and posts a comment on #41 naming `rare_components` as a space case for M3.8 (§4.6) *(maintainer, from the brainstorm)*.
- No variant backends (`vec_soa`, `sparse_set_grouped`, `bevy_ecs_sparse`, the byte-column backends, a `hybrid` with `Rare` sparse): the tracking issue (#105) keeps each workload to the seven listed backends.
- No second rare fraction: the fraction is fixed at 1%, and the sizes vary the world.
- No formal results. The informal run is the Criterion bench on the Mac under the benchmark run protocol; the cache bench is registered and built, not run (as M3.10, M3.21 to M3.23). M3.8 measures both.

## 3. Guide-level explanation

On a prototype `World`, setup spawns each entity with `Common` and, for the rare ones, `Rare`; the step is one query:

```rust,ignore
use spike_ecs_harness::rare_components::{self as rules, Common, Rare};

world
    .query_mut::<(&mut Common, &Rare)>()
    .for_each(|(common, rare)| rules::update(common, rare));
```

## 4. Reference-level design

### 4.1 Layout

- `spikes/ecs_storage/harness/src/rare_components.rs`: the module of §4.2, `pub mod rare_components` in `lib.rs`.
- One `rare_components.rs` per backend crate (`vec`, `archetype`, `sparse_set`, `dense`, `hybrid`, `bevy`, `hecs`), with the type `<Prefix>RareComponents` (`VecRareComponents`, `ArchetypeRareComponents`, …) and `BACKEND` the crate's usual name.
- Conformance and the tests of §4.5 in each backend's existing tests file; one line per backend in each bench crate.

### 4.2 The workload

Sizes 10,000, 100,000 and 1,000,000, the sizes of `random_access` *(maintainer, from the brainstorm)*. `Params::count` is the number of entities; the rare fraction is fixed.

The common rules of M3.2 §4.4 apply (initial values from `Prng::from_seed(params.seed).stream(i)` in the listed order, component types defined in the harness, no transcendental functions, hashes as `Fnv1a` call sequences).

- **Components.** `pub struct Common(pub f32)`, which every entity has, and `pub struct Rare(pub f32)`, which the rare entities have. Both derive `Clone, Copy, Debug, PartialEq`. No component is added or removed after `setup`.
- **Spawn.** `spawn(prng: &Prng, i: u64) -> (Common, Option<Rare>)`, three draws from `prng.stream(i)` in this order: `Common`'s value in `0.0_f32..1.0`; the rarity draw `d` in `0.0_f32..1.0`; then, only if `d < 0.01`, `Rare`'s value in `0.0_f32..1.0`, and the result is `Some(Rare(value))`; otherwise `None` and no third draw *(maintainer: a per-entity draw, so the rare entities are spaced irregularly and a stride prefetcher cannot predict them, as in a real world; my call: the draw order and the threshold's form)*. At seed 42 this gives 78 rare entities of 10,000, 981 of 100,000 and 10,009 of 1,000,000 (simulated, `log.md`).
- **Step.** One pass over every entity that has `Rare`: `update(&mut Common, &Rare)`: `x = common + rare`, then `common = x - 1.0` if `x >= 1.0`, else `x`. Values stay in `0.0..1.0`. Entities without `Rare` are not touched *(maintainer: the query writes the common component from the rare one, as a status effect changes shared data, so `sparse_set`'s probe into `Common` is part of the row; my call: the rule, a per-entity rotation in the style of `frag_iter`'s; simulated in f32 on the pinned toolchain at seed 42, over 1,000 ticks at `count` 1,000 and at 10,000: every rare entity changes at every tick and no checksum equals an earlier tick's, tick 0 included; `log.md`)*.
- **Hash.** `hash(common: &Common, rare: Option<&Rare>)`: `f32(common.0)`, then `u8(0)` without `Rare`, or `u8(1)` and `f32(rare.0)` with it. Every entity enters the checksum, so a backend that updates an entity without `Rare`, skips one with it, or loses or gains a `Rare` changes the checksum.

The harness module exports `RareComponents` (the `Spec` marker, `NAME = "rare_components"`), `Common`, `Rare`, `spawn`, `update` and `hash`.

### 4.3 Per backend

Each backend spawns entity `i` with `Common` alone or with `Common` and `Rare` in `setup` (one call where the backend has bundles; `spawn` and `insert` on `sparse_set` and `dense`, which have none), so every rare entity has both from the start, and runs the step as one mutable query `(&mut Common, &Rare)` with `for_each` (ecs-7). `checksum` (not measured) reads `(&Common, Option<&Rare>)` for every entity by the backend's usual read path.

- **`archetype`:** `spawn_with((common,))` or `spawn_with((common, rare))`; the world holds the `{Common}` and `{Common, Rare}` tables, and the query walks the second only.
- **`hybrid`:** as `archetype`; `Common` and `Rare` are both table types *(maintainer: `Rare` is never added or removed, only iterated, so a user of the hybrid model would make it a table type)*. With `Rare` sparse, the query would walk every `Common` row and probe `Rare`'s set for each; that configuration is not measured (§4.6 caveat).
- **`sparse_set`, `dense`:** `spawn` then `insert` of `Common`, then of `Rare` for the rare ones. On `sparse_set` the query is driven by the smaller storage, `Rare`'s, and probes `Common`'s sparse index; on `dense` it joins the two presence bitsets over the whole index space.
- **`bevy_ecs`:** both types in table storage under the crate's `Table<T>` wrapper; spawned with a one- or two-element tuple bundle; the step's query is a `QueryState` built in `setup`; `world.clear_trackers()` at the end of the step, as its other workloads.
- **`hecs`:** spawned from a one- or two-element tuple; `query_mut::<(&mut Common, &Rare)>()` per step.
- **`vec_aos`:** one `Vec<(Common, Option<Rare>)>`; the step walks every row and updates those with `Some`. It is the reference, not a storage model.

### 4.4 Conformance parameters

Conformance runs at `count` 10,000, seed 42, for 120 ticks, not the usual `count` 1,000: at seed 42 that is 78 rare entities instead of 11 *(my call)*.

### 4.5 Tests beyond conformance

- **Placement** (ecs-11: the checksum cannot see where a type is stored). On `archetype` and `hybrid`, a test asserts that after `setup` and after one step the world holds exactly the empty archetype, `{Common}` and `{Common, Rare}`; on `hybrid` this also pins `Rare` as a table type (with `Rare` sparse there would be one non-empty archetype), and a second test asserts `Common`'s and `Rare`'s `StorageKind` is the table kind directly, as M3.23's `wide_entities_hybrid_stores_extra_sparse` does for its types *(main session's call)*. On `bevy_ecs`, a test asserts after a step that `Table<Rare>` is registered and `Sparse<Rare>` never was.
- **Changing state** (ecs-13). A `vec_aos` test runs 120 ticks at the conformance parameters and asserts each tick's checksum differs from every earlier tick's, tick 0 included.
- **Harness tests** (ecs-14). A spawn test replays the draws of `prng.stream(i)` for a rare and a non-rare entity at seed 42 and asserts the values (each entity has its own stream, so whether a non-rare entity makes a third draw changes nothing outside it); a test counts the rare entities at seed 42 for `count` 10,000 and asserts 78; an `update` test covers the wrapping and the non-wrapping branch.
- **Allocations** (ecs-2). Before close-out, the heap allocations per step are counted for each backend as M3.23 did (a scratchpad crate outside the workspace with a counting allocator; `count` 10,000, seed 42), with each backend's `simple_iter` in the same binary as a one-query control, and recorded in `log.md`. The workload's own code allocates nothing in `step` (R5); a backend's per-query-run allocations are its own and are recorded, not changed. The count passes when each backend's per-step count is the same at every step after warm-up and equals its `simple_iter` control's, since both steps are one query run; a backend whose count differs is explained in `log.md` before close-out, not assumed (ecs-2) *(main session's call, from the cold read)*.

### 4.6 Benchmarks and documents

- `bench_compare/benches/compare.rs`: one `bench::<…>` line per backend. `bench_cache/benches/cache.rs`: one `cache_bench!` per backend with `[10_000, 100_000, 1_000_000]` and a `rare_components` group.
- `spikes/ecs_storage/README.md` (twelve workloads; a link to §4.2 here as the rules of `rare_components`; one sentence on what the row shows, with the caveats below that bear on reading it; `hybrid`'s row unchanged, since `Rare` is a table type), `spikes/README.md` (twelve workloads), the harness crate docs, `hybrid`'s `harness_components.rs` (`Common` and `Rare` table types), and `CHANGELOG.md` (one entry).
- The informal run: `cargo xtask spike-bench ecs` under the benchmark run protocol (`/bench-run`) on the final code commit, labelled informal. Its `rare_components` rows go in `log.md`, the pull request, and a comment on #102, with the caveats for M3.8 said as measured or unmeasured (specs-6): at least that one run gives no noise floor; that the rare count is per-seed (about 1%, §4.2); that `hybrid` has `Rare` as a table type, so its row is expected to match `archetype`'s, and the sparse configuration is not measured; that `vec_aos` walks every row; and that the row is time only, space waiting for M3.12.
- A comment on #41: `rare_components` at 1,000,000 entities is a space case for M3.8 (per type, a flat sparse index or a dense slot per entity, for 1% occupancy), so M3.12's space table should cover it. Posted when the pull request opens.

## 5. Requirements

- **M3.25-R1** `spike_ecs_harness::rare_components` MUST define `RareComponents` with `NAME = "rare_components"` and `SIZES = [10_000, 100_000, 1_000_000]`, `Common`, `Rare`, `spawn`, `update` and `hash` as §4.2 gives them.
- **M3.25-R2** `vec_aos` MUST implement `rare_components` as §4.3 gives it.
- **M3.25-R3** `archetype`, `sparse_set`, `dense`, `hybrid`, `bevy_ecs` and `hecs` MUST each implement `rare_components` as §4.3 gives it and MUST pass `check_conformance` against `vec_aos` with the parameters of §4.4.
- **M3.25-R4** The tests of §4.5 MUST exist and pass, and the allocation counts MUST be recorded in `log.md`.
- **M3.25-R5** The workload's own code MUST NOT allocate in `step`; the backends' own per-query-run allocations are allowed and not changed.
- **M3.25-R6** Both spike benches MUST register `rare_components` for all seven backends with `Spec::SIZES`; `cargo xtask spike-bench ecs` lists the rows.
- **M3.25-R7** The documents of §4.6 MUST be updated.
- **M3.25-R8** The pull request MUST show one informal `cargo xtask spike-bench ecs` run under the benchmark run protocol, labelled informal, with the `rare_components` rows; the same rows MUST be in `log.md` and in a comment on #102, with the caveats of §4.6; the comment on #41 MUST be posted.
- **M3.25-R9** `cargo xtask ci` MUST pass.

## 6. Cross-cutting concerns

- **Determinism:** no transcendental function; `update` holds the floating-point order (one add, one compare, one subtract); each write touches one entity, so iteration order does not change the result. Conformance proves it per backend.
- **`unsafe`:** none; every crate keeps `#![forbid(unsafe_code)]`.
- **Concurrency:** n/a, because the step is single-threaded.
- **Heap allocation:** none from the workload's code (R5); the prototypes' per-query-run allocations (M3.3, M3.4, M3.19) apply to the step's one query, counted per §4.5.
- **`no_std` and wasm:** n/a, because spike crates are outside the `no_std` and wasm builds.
- **Frozen formats:** none touched: the workload draws from `Prng::stream` and `range`, whose streams it does not change.
- **Public API changes:** new public items in the spike crates only (the harness module and the backends' workload types); nothing in `engine`.
- **FFI exposure:** n/a, because spikes have no FFI.
- **Performance:** one informal Criterion run (R8); M3.8 records results under the protocol. Setup at 1,000,000 entities is not measured, but it lengthens the bench run and, under Callgrind, the cache bench.
- **Documentation:** §4.6; every new public item documented.
- **Policy changes:** none.

## 7. Alternatives considered

- **`&mut Rare` alone, or two passes (`&mut Rare`, then `(&mut Common, &Rare)`).** Rejected by the maintainer: the first leaves out the probe into the common component, where a sparse index costs time; the second mixes two costs in one row.
- **Every 100th entity rare, or the first 1% of entities.** Rejected by the maintainer: a fixed stride is prefetchable and flatters the backends that walk the index space; a contiguous block is every backend's best case.
- **Sizes of 1,000,000 only, or 100,000 and 1,000,000.** Rejected by the maintainer: no curve, or a short one, to show when the world stops fitting in cache.
- **`Rare` sparse on `hybrid`, or a `hybrid_sparse` variant.** Rejected by the maintainer: a user would not make a static, iterated type sparse, and a variant goes beyond the seven backends.
- **Counting heap bytes ad hoc in this chunk.** Rejected by the maintainer: it overlaps M3.12.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | review against §4.2; the harness tests of §4.5 |
| R2 | review against §4.3; R3's tests use it as the reference; the changing-state test of §4.5 |
| R3 | `check_conformance` tests in each backend crate (CI); review against §4.3 |
| R4 | the tests in CI; the allocation counts in `log.md` |
| R5 | review of each `step`; the allocation counts |
| R6 | the bench crates build in `ci`; `spike-bench` output in the pull request |
| R7 | `review-docs` |
| R8 | the pull request, `log.md`, the comments on #102 and #41; `/bench-run`'s power log |
| R9 | `cargo xtask ci` output |

## 9. Done when

- [ ] The harness module and the seven implementations exist, and every conformance test passes.
- [ ] The tests of §4.5 pass and the allocation counts are in `log.md`.
- [ ] Both spike benches register `rare_components` for the seven backends.
- [ ] The documents of §4.6 are updated.
- [ ] One informal run is in the pull request, `log.md` and on #102, and the comment on #41 is posted.
- [ ] `cargo xtask ci` passes.

## 10. Open questions

None.

## 11. Prior art and sources

- The M3.2 spec §4.4 (the common rules and `add_remove`'s hash shape), the M3.26 spec §4.2 (the wrapping update rule), the M3.21 spec (the sizes) and the M3.23 spec (the latest workload, whose layout this follows).
