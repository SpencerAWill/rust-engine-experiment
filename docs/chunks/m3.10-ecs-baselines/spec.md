# M3.10: bevy_ecs and hecs baselines as harness backends

- **Status:** Accepted (2026-11-23)
- **Milestone:** M3 (ECS storage decision) · **Depends on:** M3.2 · **Decides:** no ADR
- **Issue:** #100
- **Estimated size:** about 1,300 changed lines, excluding tests, lockfiles and generated files (`spike_ecs_bevy` about 650 with its sparse variant, `spike_ecs_hecs` about 450, bench registration about 60, `deny.toml`, xtask and documents about 120)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds two backend crates to the ECS storage spike, `spike_ecs_bevy` (backends `bevy_ecs` and `bevy_ecs_sparse`) and `spike_ecs_hecs` (backend `hecs`), which implement the harness workloads on `bevy_ecs` 0.20 and `hecs` 0.11. These crates are used only for measurement: no code is copied from either project ([ADR 0002](../../adr/0002-originality-and-dependency-licensing.md)). It comes before the storage decision (M3.8), so that a prototype's numbers can be read against two mature engines measured in the same harness, not only against each other: M3.6's first informal run put a byte-column backend at 34 times `archetype` until a diagnosis found a code artifact. At the end, the three baseline backends pass conformance against `vec_aos`, `cargo xtask spike-bench ecs` and the cache bench list them beside the prototypes, and an informal Criterion run in the pull request places them beside `archetype`, `sparse_set` and `hybrid`.

## 2. Goals and non-goals

**Goals**

- `bevy_ecs` and `hecs` backends for all seven workloads, each written in that library's own idiom and checked by conformance (§4.4, §4.5).
- A `bevy_ecs_sparse` backend for `add_remove` and `agents`, with `B` and `Seeker` in bevy's `SparseSet` storage. It is the like-for-like baseline for `hybrid`, which keeps the same two types sparse *(maintainer, from the brainstorm)*.
- A CI guard so that only these two crates can depend on `bevy_ecs` and `hecs` (§4.2) *(maintainer, from the brainstorm)*.

**Non-goals**

- No schedule, systems, `App` or parallel executor. Each backend drives its `World` directly, single-threaded, as the prototypes do, so the numbers measure storage and query cost, not a scheduler *(maintainer, from the brainstorm)*.
- No change to ADR 0002 *(maintainer, from the brainstorm)*. The crates are unpublished spike crates that only the bench crates and tests use (§6, Policy changes).
- No batch APIs a prototype lacks (`insert_batch`, `spawn_batch`, hecs `spawn_batch`): structural changes go one entity at a time, as in the prototypes, so a row compares per-operation storage cost *(my call)*.
- No formal results. The informal run is the Criterion bench on the Mac. The cache bench is registered and built, not run *(maintainer, from the brainstorm)*. M3.8 measures both under the run protocol.
- No `bevy_reflect`, `multi_threaded`, `async_executor` or other default features of `bevy_ecs`. No other engine (flecs, specs, legion, shipyard).
- No change to `libs/`, the harness, or any existing backend.

## 3. Guide-level explanation

A baseline backend is added and checked like any other ([`spikes/ecs_storage/README.md`](../../../spikes/ecs_storage/README.md), "Adding a backend"):

```rust
#[test]
fn add_remove_bevy_sparse_matches_the_reference() {
    spike_ecs_harness::check_conformance::<
        spike_ecs_vec::add_remove::VecAddRemove,
        spike_ecs_bevy::add_remove::BevySparseAddRemove,
    >(Params { count: 1_000, seed: 42 }, 120)
    .unwrap();
}
```

`cargo xtask spike-bench ecs` then prints ids such as `spike/ecs/add_remove/bevy_ecs_sparse/100000` beside `spike/ecs/add_remove/hybrid/100000`.

## 4. Reference-level design

### 4.1 Packages

```
spikes/ecs_storage/bevy/   package spike_ecs_bevy (library), backends bevy_ecs, bevy_ecs_sparse
spikes/ecs_storage/hecs/   package spike_ecs_hecs (library), backend hecs
```

Each crate:

- MUST follow "Adding a backend" in the spike README: a workspace member through the `spikes/*/*` glob, `#![forbid(unsafe_code)]` in its crate root, `engine` and `spike_ecs_harness` as dependencies, `spike_ecs_vec` as a dev-dependency, conformance tests in `src/tests.rs`.
- MUST take its baseline as a normal dependency pinned in its own manifest, not in `[workspace.dependencies]` (only one crate uses each):
  - `bevy_ecs = { version = "0.20.1", default-features = false, features = ["std"] }`
  - `hecs = { version = "0.11.2", default-features = false, features = ["std"] }`
- Has one module per workload, named as in the harness (`simple_iter.rs`, …), and one `Workload` type per backend per workload: `Bevy<Workload>` (for example `BevySimpleIter`), `BevySparse<Workload>` (`BevySparseAddRemove`, `BevySparseAgents`) and `Hecs<Workload>` (`HecsSimpleIter`, …).

The backend names (`Workload::BACKEND`) are `"bevy_ecs"`, `"bevy_ecs_sparse"` and `"hecs"`.

### 4.2 The dependency guard

`deny.toml`'s `[bans]` table MUST gain:

```toml
deny = [
    { crate = "bevy_ecs", wrappers = ["spike_ecs_bevy"] },
    { crate = "hecs", wrappers = ["spike_ecs_hecs"] },
]
```

With cargo-deny 0.20, a ban with `wrappers` fails `cargo deny check bans` when any direct parent of the banned crate other than a wrapper depends on it (checked in the brainstorm: a non-matching wrapper gave `error[banned]: crate 'hecs = 0.11.2' is explicitly banned`). The new dependency trees MUST pass the existing licence, source and advisory rules with no other change to `deny.toml`. The brainstorm probe passed licences and sources, with duplicate-version warnings for `hashbrown` and `syn`. Duplicates stay warnings.

### 4.3 Components

- **bevy_ecs.** The orphan rule forbids implementing bevy's `Component` for the harness's types, so each harness type is stored in a local wrapper that derives `Component`. A generic `Table<T>(pub T)` with default storage, plus `Sparse<T>(pub T)` with `#[component(storage = "SparseSet")]`, is the suggested form, so each harness type needs no wrapper of its own *(my call; a builder MAY use one named newtype per type instead if the generic derive fails)*. The generic derive, stored `QueryState`s with `With` and `Without` filters, and `Option<&mut Sparse<T>>` compiled and ran under `forbid(unsafe_code)` on 1.100.0 in the grill's stub; the non-generic derive also passed clippy pedantic in the brainstorm probe. No `#[derive(Bundle)]`: spawn takes tuples.
- **hecs.** Every `Send + Sync + 'static` type is a component, so the harness types are stored as they are.
- **Both.** `agents` uses its own `AgentId(u64)`, `Velocity(Vec2)` and `Ttl(u32)` newtypes, as the prototypes do. `sprites` stores `Hidden` as a zero-sized component on hidden entities only. `frag_iter` stores each letter as its own type (`A` to `Z`, wrapped for bevy), not the `Letter` enum, so the world has 26 archetypes.

### 4.4 Driving the world

- Each `Workload` owns its `World` and any buffers it reuses between steps (entity lists, a command buffer), allocated in `setup` and cleared, not reallocated, in `step`.
- **bevy_ecs:** every query a step runs MUST be a `QueryState` built in `setup` and kept in the workload, not a `world.query()` call per step. That is bevy's documented way to run a query repeatedly outside a system. Structural changes go through `World` (`entity_mut(e).insert(..)`, `.remove::<..>()`, `despawn(e)`, `spawn(..)`) outside any query iteration. Changes decided inside a query (the `agents` seeker changes and despawns, the `entity_churn` despawns) are collected into a reused `Vec` and applied after the iteration, in the order they were recorded *(my call: bevy's `Commands` needs a `CommandQueue` and a world borrow that conflicts with the running query)*.
- **bevy_ecs, end of step:** `step` MUST end with `World::clear_trackers()`. Without it, the removal buffers that `remove` and `despawn` write grow across steps, because a schedule is what normally clears them: in the grill's stub, `World::removed::<B>()` held 10, 20, then 30 entities after three `add_remove`-like steps on 10 entities without the call, and none after each step with it. `check_change_ticks` is not called, because nothing reads change detection.
- **hecs:** queries are `world.query_mut::<Q>()` (or `query::<Q>()` for reads) per step. With `std`, hecs 0.11.2 caches each query type's archetype matches in the `World` (`query_cache`, an `RwLock` over a type map) until the archetypes change, and also offers a workload-held `PreparedQuery<Q>`. The backend MUST NOT use `PreparedQuery`: plain queries are hecs's documented idiom, so each baseline takes its library's recommended path for a repeated query *(maintainer, from the grill)*. A steady-state query run then pays a lock, a type lookup, a downcast and an `Arc` clone, a caveat M3.8 reads the `hecs` rows with. Changes decided inside a query use a `hecs::CommandBuffer` kept in the workload and run with `run_on` after the iteration; `run_on` applies commands in recorded order. Changes outside a query use `World` directly.
- **Alive count:** where a step respawns up to a target (`entity_churn`, `agents`), `hecs` MAY use `hecs::World::len`. The bevy backends MUST keep their own counter, because a bevy `World` holds an entity of its own: `world.entities().count_spawned()` is 1 for a new world in 0.20.1. For the same reason a bevy query MUST NOT consist of `Entity` alone without a component term or filter. Either count MUST equal the number of workload entities alive after the step.

### 4.5 Workloads

The rules, spawn order and checksums are the harness's ([M3.2 spec](../m3.2-spike-harness/spec.md) §4.4). The harness's `update` functions are called from the loops. The queries and structural changes are as follows, the same for all three backends unless a column says otherwise (`T` means the stored form of `T`: `Table<T>` or `Sparse<T>` for bevy, `T` for hecs):

| Workload | Backends | Step |
|---|---|---|
| `simple_iter` | `bevy_ecs`, `hecs` | `(&mut Position, &Velocity)`, `update` per entity |
| `frag_iter` | `bevy_ecs`, `hecs` | `&mut Data` over all 26 archetypes, `update` per entity |
| `heavy_compute` | `bevy_ecs`, `hecs` | `(&mut Matrix, &mut Position, &Rotation)`, `update` per entity |
| `add_remove` | all three | Two passes, as the prototypes: collect `(Entity, A)` for every entity with `A`, insert `make_b` on each; then collect every entity with `B`, remove `B` from each. `bevy_ecs_sparse` stores `B` as `Sparse<B>`, `A` as `Table<A>` |
| `entity_churn` | `bevy_ecs`, `hecs` | `(Entity, &mut Position, &Velocity, &mut Lifetime)`, `update`, record a despawn when `is_expired`; apply; spawn `spawn(prng, next)` until `count` entities are alive |
| `sprites` | `bevy_ecs`, `hecs` | Move: `(&mut Transform2d, &Velocity2d)` over every sprite; draw: `(&Transform2d, &Sprite)` without `Hidden`, pushing `draw_item` to the reused draw list |
| `agents` | all three | `(Entity, &mut Transform2d, &mut Velocity, &mut Ttl, Option<&mut Seeker>)`, `update`; write a kept seeker back, record `insert` or `remove` of `Seeker` when it appeared or went away, and `despawn` when the ttl reaches 0, in that order; apply; respawn to the target. `bevy_ecs_sparse` stores `Seeker` as `Sparse<Seeker>`, the rest as `Table<…>` |

- Checksums read with whatever queries the backend needs. For `frag_iter`, which needs the letter's index, that MAY be one query per letter type.
- `bevy_ecs`'s `Option<&mut T>` item is `Option<Mut<T>>`. Writing through it marks the value changed, as any bevy write does. That cost is part of what the baseline measures.

**Expected, unmeasured**, for M3.8:

- `bevy_ecs` and `hecs` within a small factor of `archetype` on the iteration workloads (`simple_iter`, `heavy_compute`, `sprites`). All three iterate archetype tables. bevy also writes a change tick for every value it hands out mutably.
- `bevy_ecs_sparse` near `hybrid` on `add_remove`: neither moves a row when `B` is inserted or removed. `bevy_ecs` near `archetype`: both move a row on every insert and remove.
- `bevy_ecs_sparse` differs from `hybrid` in one way that M3.8 reads its rows against: bevy makes sparse-set components part of an archetype's identity, so inserting or removing `Sparse<B>` moves no table row but does move the entity to another archetype (in the grill's stub, one insert took the archetype count from 3 to 4 and left the table count at 3). `agents` is split across two archetypes by `Seeker`. `hybrid`'s sparse types change neither (M3.5 spec §2).

### 4.6 Benchmarks

Both bench crates gain `spike_ecs_bevy` and `spike_ecs_hecs` as dev-dependencies and one line per implementation (16 in all: 7 `bevy_ecs`, 2 `bevy_ecs_sparse`, 7 `hecs`), as the spike README's step 4 says. `cache_bench!` lines use each workload's `Spec::SIZES`.

### 4.7 Documents and tooling

- **`spikes/ecs_storage/README.md`:** rows for both packages in the Contents table, saying that they are baselines on external crates, not prototypes.
- **`spikes/README.md`, Rules:** one line saying that a spike's baseline crate may depend on a competing library for measurement, and that `deny.toml` names each such crate.
- **`CONTRIBUTING.md`, "Licence and originality", the line on other crates:** add "or as the dependency of a spike's baseline crate that `deny.toml` names".
- **`xtask/src/coverage.rs`:** both packages join `SPIKE_PACKAGES` and the expected `--exclude` list of `test_args_match_the_spec`.
- **`CHANGELOG.md`:** an entry.
- **Each crate's `lib.rs` docs:** say that the crate is a measurement baseline on `bevy_ecs` (or `hecs`), with no code taken from it (ADR 0002), and give its version.

## 5. Requirements

- **M3.10-R1** `spikes/ecs_storage/bevy` and `spikes/ecs_storage/hecs` MUST be workspace members named `spike_ecs_bevy` and `spike_ecs_hecs`. Each crate root MUST carry `#![forbid(unsafe_code)]`, and each MUST depend on its baseline as §4.1 says.
- **M3.10-R2** `deny.toml` MUST ban `bevy_ecs` and `hecs` except through their wrapper crates as §4.2 says, and `cargo xtask deny` MUST pass with no other change to `deny.toml`.
- **M3.10-R3** `bevy_ecs` and `hecs` MUST each implement all seven workloads, and `bevy_ecs_sparse` MUST implement `add_remove` and `agents`, with the components of §4.3 and the steps of §4.5. Each MUST pass `check_conformance` against `vec_aos`: 1,000 entities (20 per letter for `frag_iter`) over 120 ticks, and 1,000 agents over 1,300 ticks for `agents`.
- **M3.10-R4** The bevy backends MUST run every step query through a `QueryState` built in `setup`, MUST apply changes decided inside a query after the iteration in recorded order, and MUST end every `step` with `World::clear_trackers()`.
- **M3.10-R5** `bevy_ecs_sparse` MUST store `B` and `Seeker` with `SparseSet` storage, and every other type with table storage. `bevy_ecs` MUST use table storage for every type. The `hecs` backend MUST NOT use `PreparedQuery`.
- **M3.10-R6** A backend MUST NOT use a schedule, a system, an `App` or a batch spawn or insert API.
- **M3.10-R7** Both bench crates MUST register every implementation of R3 as §4.6 says.
- **M3.10-R8** `cargo xtask ci` MUST pass, and CI MUST be green on Ubuntu, macOS and Windows without workflow changes.
- **M3.10-R9** Every public item MUST be documented, and the documents and tooling of §4.7 MUST be updated as described.
- **M3.10-R10** The pull request MUST show one informal `cargo xtask spike-bench ecs` run, under the benchmark run protocol and labelled informal, with the baseline rows beside `archetype`, `sparse_set` and `hybrid`.

## 6. Cross-cutting concerns

- **Determinism:** each backend's iteration order is its library's. Checksums are order-independent wrapping sums, and respawn draws come from a counter, not from iteration order, so conformance does not depend on it. Our own code uses no `HashMap`, ambient state or transcendental function, and the workspace lints enforce that. The baseline libraries' internals (bevy uses hash maps) are not library code of this repository and are outside the determinism rule, as `bench/compare`'s dependencies are.
- **`unsafe`:** none in our code: both crates forbid it. The baselines contain `unsafe` internally. That is their implementation, measured, not ours ([ADR 0001](../../adr/0001-unsafe-policy.md) governs this repository's code).
- **Concurrency:** n/a, because every backend is single-threaded and `multi_threaded` is off.
- **Heap allocation:** `setup` allocates the worlds and the reused buffers. In the steady state the buffers keep their capacity. bevy's removal buffers are cleared every step (R4). Neither library's internal allocation is counted. #41 adds space measurement.
- **`no_std` and wasm:** n/a, because spike crates are not in those builds.
- **Frozen formats:** none.
- **Public API changes:** none in `libs/`.
- **FFI exposure:** n/a.
- **Performance:** one informal Criterion run in the pull request (R10). M3.8 records results under the protocol, with saved baselines and the noise floor. The CI cost is the build time of about 60 more crates on three platforms. It is reported from the pull request's CI run.
- **Documentation:** item docs, the two READMEs, CONTRIBUTING.md, CHANGELOG (§4.7), and this spec and its plan.
- **Policy changes:** the `deny.toml` bans of §4.2, and the CONTRIBUTING.md and spikes README lines that say a spike's baseline crate may take a competing library as a normal dependency. ADR 0002's "benchmark dev-dependencies" is read as "dependencies used only for measurement". An unpublished crate that only bench crates and tests depend on fits that reading, and the bans keep it there *(maintainer, from the brainstorm)*.

## 7. Alternatives considered

- **Systems in a `Schedule`, with bevy's `Commands`.** Rejected by the maintainer: closer to real bevy use, but it adds scheduler and command-flush costs the prototypes do not have, so a row would not isolate storage.
- **Both a direct and a schedule variant of bevy.** Rejected by the maintainer: more code and rows for a question (scheduler cost) M3 does not ask.
- **Table storage only for bevy.** Rejected by the maintainer: `hybrid` then has no external sparse baseline. The grill found that bevy's sparse components are part of archetype identity, unlike `hybrid`'s. The maintainer kept the variant, with that difference as a caveat and a test (§4.5, R5), because it is the design difference M3.8 weighs.
- **`PreparedQuery` for hecs**, like-for-like with bevy's `QueryState`. Rejected by the maintainer in the grill: hecs's docs recommend plain queries, and its `World` already caches archetype matches.
- **The baseline `Workload` impls inside the bench crates, the libraries as dev-dependencies.** Rejected: the impls would be written twice (two bench crates), and the conformance tests would have to live in a bench crate.
- **Amending ADR 0002.** Rejected by the maintainer: the bans enforce the reading in §6, and the ADR's intent is unchanged.
- **Running the cache bench in the informal run.** Rejected by the maintainer: it needs the devcontainer, and M3.8 runs it formally.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | the workspace members (`cargo metadata`); `cargo xtask check-unsafe`; the manifests |
| R2 | `cargo xtask deny`; a check in the plan that temporarily adds `hecs` to a non-wrapper crate's manifest, sees `cargo deny check bans` fail with `banned`, and reverts (output in `log.md`) |
| R3 | conformance tests in each crate's `tests.rs`, one per implementation (16) |
| R4 | review of each bevy workload against §4.4; a unit test that `step` leaves the removed-components buffer of `B` (`add_remove`) empty of the previous step's removals after `clear_trackers` (through bevy's public `World::removed`, or `removed_components()`) |
| R5 | a unit test per bevy backend that reads the storage type of each registered component through bevy's public component info (`World::components()` and `ComponentInfo::storage_type`); a `bevy_ecs_sparse` test that inserting `Sparse<B>` leaves the table count (`World::storages().tables.len()`) unchanged and changes the entity's archetype, so the §4.5 caveat stays true of the code *(maintainer, from the grill)*; `grep` finds no `PreparedQuery` in `spike_ecs_hecs` |
| R6 | review; `grep` for `Schedule`, `IntoSystem`, `App`, `insert_batch` and `spawn_batch` in both crates finds nothing |
| R7 | `cargo xtask bench-smoke` (Criterion ids); `cargo build --benches -p spike_ecs_bench_cache`; the `prepared` assertion on sizes |
| R8 | `cargo xtask ci`; the pull request's CI run |
| R9 | the `review-docs` review; `cargo xtask doc` |
| R10 | the pull request body; the run's power log in `log.md` |

## 9. Done when

- [ ] `cargo xtask ci` passes locally on the final code commit, and CI is green on all three platforms.
- [ ] The 16 conformance tests and the R4 and R5 unit tests pass.
- [ ] `cargo xtask deny` passes and the R2 negative check is recorded in `log.md`.
- [ ] `cargo xtask spike-bench ecs` lists the three baseline backends, and its informal output is in the pull request.
- [ ] The documents and tooling of §4.7 are updated.
- [ ] The reviews of CONTRIBUTING.md "Reviews before a pull request" are recorded in `log.md` and the pull request.
- [ ] The spec is `Implemented`, the plan and log `Historical`, with Deviations filled in.

## 10. Open questions

None yet.

## 11. Prior art and sources

Used only as measurement baselines. No code is taken from either ([ADR 0002](../../adr/0002-originality-and-dependency-licensing.md)).

- [bevy_ecs 0.20 documentation](https://docs.rs/bevy_ecs/0.20.1/bevy_ecs/): `World`, `QueryState`, `World::clear_trackers`, `#[component(storage = "SparseSet")]`.
- [hecs 0.11 documentation](https://docs.rs/hecs/0.11.2/hecs/): `World::query_mut`, `CommandBuffer`.
- [M3.2 spec](../m3.2-spike-harness/spec.md): the harness, the workloads and the conformance check.
- [M3.5 spec](../m3.5-hybrid/spec.md): `hybrid`'s sparse choice of `B` and `Seeker`, which `bevy_ecs_sparse` mirrors.
