# M3.22: many_queries workload for the ECS storage harness

- **Status:** Draft (2026-11-26); cold read applied 2026-11-26
- **Milestone:** M3 (ECS storage decision) · **Depends on:** M3.2, M3.10, M3.21 · **Decides:** no ADR
- **Issue:** #107
- **Estimated size:** about 650 changed lines, excluding tests, lockfiles and generated files (harness module about 130, `vec_aos` about 80, six ECS backends about 60 each, `hybrid`'s component list about 10, bench registration about 30, documents about 40)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds a ninth workload, `many_queries`, to the ECS storage spike: 56 distinct two-component queries per tick over a world of eight archetypes, each with a short update, so that the fixed cost of starting a query is a large share of the step. A real frame runs many systems, each touching a few components, and every existing workload runs one or two queries per tick, so the storage decision (M3.8, #102) has no measure of it; in M3.10's run a query start on `archetype` (`frag_iter`, 187 ns) cost four times `hecs`'s (43 ns). At the end the workload passes conformance against `vec_aos` on `archetype`, `sparse_set`, `dense`, `hybrid`, `bevy_ecs` and `hecs`, both spike benches list it, and an informal Criterion run is in the pull request and on #102.

## 2. Goals and non-goals

Goals:

- Rules for `many_queries` (§4.2) precise enough that any backend that follows them reproduces the reference checksum bit for bit, in the M3.2 style, all in `spike_ecs_harness`, with the query list defined once there (§4.3).
- Implementations on `vec_aos` (the reference) and the six backends above, each passing `check_conformance` against `vec_aos`.
- Registration in `bench_compare` and `bench_cache` with `Spec::SIZES`.

Non-goals:

- No cached query state on the prototypes *(maintainer, from the brainstorm)*. Each backend starts its queries by its own repeated-query path as it is today (§4.4), so the prototypes rebuild their match on every run while `bevy_ecs` and `hecs` reuse theirs. Cached query state is an M4 item (#84); this run's `hecs` against `archetype` rows estimate what it saves on archetype storage (§4.5).
- No structural change in the step: the archetypes are fixed after `setup`, so the step measures query start and iteration only (churn is `entity_churn`'s and `add_remove`'s).
- No variant backends (`vec_soa`, `sparse_set_grouped`, `bevy_ecs_sparse`, the byte-column backends), as the tracking issue (#105) keeps each workload to the seven listed backends.
- No formal results. The informal run is the Criterion bench on the Mac under the benchmark run protocol; the cache bench is registered and built, not run (as M3.10 and M3.21). M3.8 measures both.

## 3. Guide-level explanation

The harness defines the eight component types and one macro that lists the 56 queries in order. A backend writes how one query runs and hands that to the macro; for a prototype `World`:

```rust,ignore
macro_rules! run_query {
    ($write:ty, $read:ty) => {
        world.query_mut::<(&mut $write, &$read)>().for_each(|(w, r)| rules::update(&mut w.0, r.0));
    };
}
spike_ecs_harness::for_each_query!(run_query); // C0 from C1, C0 from C2, …, C7 from C6
```

## 4. Reference-level design

### 4.1 Layout

- `spikes/ecs_storage/harness/src/many_queries.rs`: the module of §4.2 and §4.3, `pub mod many_queries` in `lib.rs`.
- One `many_queries.rs` per backend crate (`vec`, `archetype`, `sparse_set`, `dense`, `hybrid`, `bevy`, `hecs`), with the type `<Prefix>ManyQueries` (`VecManyQueries`, `ArchetypeManyQueries`, …) and `BACKEND` the crate's usual name.
- Conformance tests in each backend's existing tests file; one line per backend in each bench crate.

### 4.2 The workload

Sizes 100, 1,000 and 10,000 *(my call: the issue names 1,000 and 10,000; at 100 the step's 1,200 entity updates take well under the 56 query starts on every backend, so query start dominates on every row, and the three sizes show where iteration takes over)*.

The common rules of M3.2 §4.4 apply (initial values from `Prng::from_seed(params.seed).stream(i)` in the listed order, component types defined in the harness, no transcendental functions, hashes as `Fnv1a` call sequences).

- **Components.** Eight types `C0` to `C7`, each `pub struct Cn(pub f32)` *(my call: named types, not one const-generic type, so `hybrid`'s component list and bevy's `Table<T>` wrapper take them as they take every other harness type)*.
- **Groups.** Entity `i` is in group `g = i % 8` and has exactly the four types `C(g)`, `C(g+1)`, `C(g+2)` and `C(g+3)`, indices modulo 8. So the world has eight archetypes of `count / 8` entities each (rounded as `i % 8` gives), and every type is in four of them.
- **Spawn.** `spawn(prng: &Prng, i: u64) -> Spawn` returns entity `i`'s group and its four values, one draw each in `0.0_f32..1.0` from `prng.stream(i)`, in the order `C(g)`, `C(g+1)`, `C(g+2)`, `C(g+3)`. `Spawn { group: u8, values: [f32; 4] }`, where `values[k]` is the value of `C((g + k) % 8)`.
- **Step.** The 56 queries of §4.3, in that order. Query `(w, r)` visits every entity that has both `Cw` and `Cr` and applies `update(&mut cw.0, cr.0)`: `x = cw + cr * 0.5 + 0.25` (the multiplication, then the additions left to right, not fused), then `cw = x - 1.0` if `x >= 1.0`, else `cw = x`. Values stay in `0.0..1.0` and never settle *(main session's call, from the cold read: the first draft's contracting rule, `cw * 0.5 + cr * 0.25 + 0.125`, froze every value at 0.5 by about tick 18, so most conformance ticks compared checksums that could not differ; the wrapping rule, simulated in f32 on 64 entities for 5,000 ticks, changed state every tick and never repeated one)*. Within one query each entity's update reads and writes only that entity, so iteration order inside a query does not change the result; the order of the queries does, and is fixed.
- **Hash.** `hash(group: u8, values: [f32; 4])`: `u8(group)`, then `f32` of each of the four values in the spawn order. No component holds the group: an ECS backend's `checksum` runs eight read-only queries, `(&C(g), &C(g+1), &C(g+2), &C(g+3))` for `g` in `0..8`, each of which matches exactly group `g` *(main session's call, from the cold read)*.

The harness module exports `ManyQueries` (the `Spec` marker), `C0` to `C7`, `Spawn`, `spawn`, `update`, `hash`, `QUERIES` and `for_each_query!`.

### 4.3 The queries

The queries are every ordered pair `(w, r)` of distinct type indices, `w` major and `r` minor: `(0, 1)`, `(0, 2)`, …, `(0, 7)`, `(1, 0)`, `(1, 2)`, …, `(7, 6)`. That is 56 queries. With `d = (r - w) mod 8`, a query matches three groups when `d` is 1 or 7, two when 2 or 6, one when 3 or 5, and none when 4: 16 queries match three eighths of the entities, 16 two eighths, 16 one eighth, and 8 match no entity, which a real frame has too (a system with nothing to do this frame still starts). Each entity is updated 12 times per step.

- `QUERIES: [(u8, u8); 56]` lists them in order, for `vec_aos` and for tests.
- `for_each_query!($m:ident)`, a `#[macro_export]` macro, expands to `$m!($crate::many_queries::C0, $crate::many_queries::C1); …; $m!($crate::many_queries::C7, $crate::many_queries::C6);` in the same order, one line per pair written out (no recursion); the callback takes `($write:ty, $read:ty)`, so a backend need not import the eight types and can wrap them (`Table<$write>`) *(main session's call, from the stub: a `macro_rules!` callback defined in `step` after the `World` borrow is in scope can use it)*. Every ECS backend MUST run its step's queries through it (`bevy_ecs` builds its 56 closures through it in `setup` and runs them in that order, §4.4), so no backend can drop or reorder a query. A dropped query that matches no entity changes no checksum, so conformance cannot catch it; the macro and a harness test that its expansion order equals `QUERIES` do *(main session's call)*.

### 4.4 Per backend

*(maintainer, from the brainstorm)* Each backend runs each query through its own repeated-query path, as it does in the other workloads:

- **The four prototypes:** `world.query_mut::<(&mut Cw, &Cr)>().for_each(…)` per query, each run building its match from scratch, as today.
- **`hybrid`:** all eight types are table types *(my call, as M3.21's `random_access`: nothing is added or removed, which is what `hybrid`'s sparse storage is for)*.
- **`bevy_ecs`:** the workload holds 56 `QueryState<(&mut Table<Cw>, &Table<Cr>)>`, built in `setup` (`world.query()`), and runs each with `iter_mut` in order. The 56 states have distinct types, so each is owned by a boxed `FnMut(&mut World)` closure built in `setup`, in a `Vec` run in order: one indirect call per query, as bevy's own schedule runs each system through `dyn System` *(main session's call, from the stub)*; `world.clear_trackers()` at the end of the step, as its other workloads. No archetype is created after `setup`.
- **`hecs`:** `world.query_mut::<(&mut Cw, &Cr)>()` per query, using `hecs`'s per-type cache in the `World`; no `PreparedQuery`, as its other workloads.
- **`vec_aos`:** one `Vec` of `{ group: u8, values: [f32; 8] }` (absent types unused), and for each query of `QUERIES` one pass over every entity that updates those in a matching group. It is the reference, not a storage model: its row pays a full pass per query.

### 4.5 Benchmarks and documents

- `bench_compare/benches/compare.rs`: one `bench::<…>` line per backend. `bench_cache/benches/cache.rs`: one `cache_bench!` per backend with `[100, 1_000, 10_000]` and a `many_queries` group.
- `spikes/ecs_storage/README.md`, `spikes/README.md`, the harness crate docs and `hybrid`'s `harness_components.rs`: nine workloads, and a link to §4.2 here as the rules of `many_queries`.
- The informal run: `cargo xtask spike-bench ecs` under the benchmark run protocol (`/bench-run`) on the final code commit, labelled informal. Its `many_queries` rows go in `log.md`, the pull request, and a comment on #102, with the caveats for M3.8 said as measured or unmeasured: at least the cache gap (`hecs` against `archetype`, and `bevy_ecs`, as estimates of what cached query state saves on archetype storage; no cached sparse-set baseline exists) and the prototypes' allocations per step. The same comment links #84, where cached query state is tracked, and a short comment there points back to it.

## 5. Requirements

- **M3.22-R1** `spike_ecs_harness::many_queries` MUST define `ManyQueries` with `NAME = "many_queries"` and `SIZES = [100, 1_000, 10_000]`, `C0` to `C7`, `Spawn`, `spawn`, `update` and `hash` as §4.2 gives them, and `QUERIES` and `for_each_query!` as §4.3 gives them.
- **M3.22-R2** `vec_aos` MUST implement `many_queries` as §4.4 gives it.
- **M3.22-R3** `archetype`, `sparse_set`, `dense`, `hybrid`, `bevy_ecs` and `hecs` MUST each implement `many_queries` as §4.4 gives it, running its queries through `for_each_query!` as §4.3 gives it, and MUST pass `check_conformance` against `vec_aos` at `count` 1,000, seed 42, for 120 ticks.
- **M3.22-R4** The workload's own code MUST NOT allocate in `step`. The backends' own allocations (the prototypes' per-query-run allocations) are allowed and not changed.
- **M3.22-R5** Both spike benches MUST register `many_queries` for all seven backends with `Spec::SIZES`; `cargo xtask spike-bench ecs` lists the rows.
- **M3.22-R6** The documents of §4.5 MUST be updated.
- **M3.22-R7** The pull request MUST show one informal `cargo xtask spike-bench ecs` run under the benchmark run protocol, labelled informal, with the `many_queries` rows; the same rows MUST be in `log.md` and in a comment on #102, with the caveats of §4.5.
- **M3.22-R8** `cargo xtask ci` MUST pass.

## 6. Cross-cutting concerns

- **Determinism:** no transcendental function; the floating-point order is fixed in `update`; the query order is fixed by `for_each_query!`; within a query each update touches one entity. Conformance proves it per backend.
- **`unsafe`:** none; every crate keeps `#![forbid(unsafe_code)]`.
- **Concurrency:** n/a, because the step is single-threaded.
- **Heap allocation:** none from the workload's code (R4); the prototypes allocate on each of the 56 query runs (about 2 to 5 per run, recorded in M3.3, M3.4, M3.19 and M3.21), so their rows carry about 100 to 300 allocations per step that the baselines do not. The informal run's caveats count them.
- **`no_std` and wasm:** n/a, because spike crates are outside the `no_std` and wasm builds.
- **Frozen formats:** none touched: the workload draws from `Prng::stream` and `range`, whose streams it does not change.
- **Public API changes:** new public items in the spike crates only (the harness module and macro, the backends' workload types); nothing in `engine`.
- **FFI exposure:** n/a, because spikes have no FFI.
- **Performance:** one informal Criterion run (R7); M3.8 records results under the protocol.
- **Documentation:** §4.5; every new public item documented.
- **Policy changes:** none.

## 7. Alternatives considered

- **Cached query state on the four prototypes**, built once and reused, so the rows compare storage against storage. Rejected by the maintainer: a new API on four `World`s, beyond the issue's size, for models M3.8 will mostly drop; tracked for M4 in #84.
- **No cache anywhere** (`bevy_ecs` calling `world.query()` each tick). Rejected: not how bevy is used, so its row would mislead.
- **One archetype** (every entity has all eight types). Rejected by the maintainer: every query visits every entity, so iteration, not query start, dominates even at 1,000.
- **Random component subsets** (each type present with probability one half, up to 256 archetypes). Rejected by the maintainer: query start then includes matching hundreds of archetypes, which mixes in `frag_iter`'s per-archetype cost.
- **Exactly fifty queries**, as the issue says. Rejected by the maintainer: 56 is every ordered pair of the shape, with no arbitrary subset to choose.
- **Each backend listing its own 56 queries.** Rejected *(main session's call)*: a dropped query that matches no entity passes conformance.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | review against §4.2 and §4.3; a spawn test (values equal direct draws from `prng.stream(i)` in order); a test that `for_each_query!`'s expansion order equals `QUERIES` and that `QUERIES` holds 56 distinct pairs with `w != r`; a test of the per-query match counts on `vec_aos` (3, 2, 1 or 0 groups by `d`); a `vec_aos` test that the checksum after tick 120 differs from the one after tick 119, so the conformance run's last tick still compares changing state |
| R2 | review against §4.4; R3's tests use it as the reference |
| R3 | `check_conformance` tests in each backend crate (CI); review that each step uses `for_each_query!` |
| R4 | review of each `step` |
| R5 | the bench crates build in `ci`; `spike-bench` output in the pull request |
| R6 | `review-docs` |
| R7 | the pull request, `log.md` and the comment on #102 |
| R8 | `cargo xtask ci` output |

## 9. Done when

- [ ] The harness module and the seven implementations exist, and every conformance test passes.
- [ ] Both spike benches register `many_queries` for the seven backends.
- [ ] The documents of §4.5 are updated.
- [ ] One informal run is in the pull request, `log.md` and on #102.
- [ ] `cargo xtask ci` passes.

## 10. Open questions

None.

## 11. Prior art and sources

- The M3.2 spec §4.4 (workload style), the M3.10 spec §4.4 (baseline idioms) and the M3.21 spec (the latest workload, whose layout this follows).
- bevy's `QueryState` held by a system and hecs's per-type query cache in `World`, read as designs only ([ADR 0002](../../adr/0002-originality-and-dependency-licensing.md)).
