# ECS storage spike

Status: Living (2026-11-27). Added by M3.2. The rules of the first seven workloads are in the [M3.2 spec](../../docs/chunks/m3.2-spike-harness/spec.md) (§4.4; `frag_iter`'s and `heavy_compute`'s steps amended by [M3.26](../../docs/chunks/m3.26-workload-checks-that-fail/spec.md) §4.2), those of `random_access` in the [M3.21 spec](../../docs/chunks/m3.21-random-access/spec.md) (§4.2), those of `many_queries` in the [M3.22 spec](../../docs/chunks/m3.22-many-queries/spec.md) (§4.2), those of `wide_entities` in the [M3.23 spec](../../docs/chunks/m3.23-wide-entities/spec.md) (§4.2); this page is how to use them.

## When this applies

- You add a backend or workload to the ECS storage spike, or run its benchmarks.
- You quote or check a number from M3's storage decision.

## Contents

| Directory | Package | Role |
|---|---|---|
| `harness/` | `spike_ecs_harness` | The `Workload` trait, the ten workloads' components, spawn, update and hash functions, `Fnv1a`, `check_conformance` |
| `vec/` | `spike_ecs_vec` | The reference backends: `vec_aos` for every workload, `vec_soa` for `agents` |
| `sparse_set/` | `spike_ecs_sparse_set` | The sparse-set prototype (M3.3): a minimal safe `World` with typed tuple queries and `Commands`; `sparse_set` (views only) for every workload, `sparse_set_grouped` (owning groups) for all but `frag_iter` |
| `archetype/` | `spike_ecs_archetype` | The archetype prototype (M3.4): a minimal safe `World` with one table per set of component types, cached add and remove edges, `spawn_with` bundles of up to 24 types, the same typed tuple queries and `Commands` as `sparse_set`; `archetype` for every workload. Also runtime-registered components (M3.6, [spec](../../docs/chunks/m3.6-runtime-components/spec.md)): registered from a size and alignment, stored in byte columns beside the typed ones, read by a raw query by id; `archetype_bytes_copy` (values copied through a codec) and `archetype_bytes_cast` (columns cast to `Pod` slices) for `simple_iter`, `add_remove` and `frag_iter` |
| `dense/` | `spike_ecs_dense` | The dense-array prototype (M3.19): a minimal safe `World` with one `Vec<Option<T>>` per component type indexed by entity index and a presence `HierarchicalBitSet` beside it, queries driven by `Join` over the presence sets, the same typed tuple queries and `Commands` as `sparse_set`; `dense` for every workload |
| `hybrid/` | `spike_ecs_hybrid` | The per-component hybrid prototype (M3.5): a minimal safe `World` in which each type's `Component` impl names its storage, archetype tables (as `archetype`) or a sparse set of its own (as `sparse_set`); tables key on table types only, so inserting or removing a sparse type moves no row; one typed tuple query layer over both, which probes a sparse set per row only for the sparse types a query names; `spawn_with` bundles of up to 24 types of either kind and `Commands`; `hybrid` for every workload, with `B` (`add_remove`), `Seeker` (`agents`) and `Extra` (`wide_entities`) sparse |
| `bevy/` | `spike_ecs_bevy` | A measurement baseline, not a prototype (M3.10, [spec](../../docs/chunks/m3.10-ecs-baselines/spec.md)): the workloads on `bevy_ecs` 0.20, driven directly on its `World` with no schedule, each step query a cached `QueryState`; `bevy_ecs` (table storage) for every workload, `bevy_ecs_sparse` (`B` and `Seeker` in `SparseSet` storage, which unlike `hybrid` still moves the entity to another archetype) for `add_remove` and `agents` |
| `hecs/` | `spike_ecs_hecs` | A measurement baseline, not a prototype (M3.10): the workloads on `hecs` 0.11, with `query_mut` per step and changes decided inside a query applied through `World` afterwards (not hecs's `CommandBuffer`, whose inserts skip its cached archetype edges); `hecs` for every workload |
| `bench_compare/` | `spike_ecs_bench_compare` | Criterion: wall-clock time |
| `bench_cache/` | `spike_ecs_bench_cache` | Gungraun: instructions and simulated cache misses |

The workloads are `simple_iter`, `frag_iter`, `heavy_compute`, `add_remove`, `entity_churn`, `sprites`, `agents`, `random_access`, `many_queries` and `wide_entities`. Every backend above implements `random_access`, `many_queries` and `wide_entities` except the variants (`vec_soa`, `sparse_set_grouped`, the byte-column backends, `bevy_ecs_sparse`). For `random_access`, each prototype has `World::lookup::<T>()`, a read-only `Lookup` that reads one component type by handle with the type resolved once (M3.21). `many_queries` runs its 56 queries through the harness's `for_each_query!` macro, so no backend can drop one (M3.22). `wide_entities` gives every entity 24 components (544 bytes), updates four of them and adds and removes a 25th each step, so on `archetype`, `bevy_ecs` and `hecs` each insert and remove copies the whole row, while `sparse_set`, `dense` and `hybrid` (whose 25th type is sparse) move none, nor does the reference `vec_aos`; the difference from `add_remove`'s row also holds the update pass, which is not measured separately (M3.23).

## Adding a backend

A backend is a crate in this directory (`spikes/ecs_storage/<name>/`, package `spike_ecs_<name>`, `publish = false` through the workspace, `#![forbid(unsafe_code)]`).

1. Depend on `spike_ecs_harness` and `engine`.
2. For each workload, implement `Workload` once, in the backend's own idiom. `setup` builds entities `0..n` with the harness's `spawn` function, `step` applies the rules, and `checksum` is the wrapping sum of the harness's hash function over every entity. The harness's `update` functions hold the floating-point order, so call them from your loops. Example, from `spike_ecs_vec`:

   ```rust
   impl Workload for VecAddRemove {
       type Spec = AddRemove;
       const BACKEND: &'static str = "vec_aos";
       // setup, step and checksum
   }
   ```

3. Test each against the reference. A backend may not quote a number before this passes:

   ```rust
   #[test]
   fn add_remove_matches_the_reference() {
       spike_ecs_harness::check_conformance::<spike_ecs_vec::add_remove::VecAddRemove, MyAddRemove>(
           Params { count: 1_000, seed: 42 },
           120,
       )
       .unwrap();
   }
   ```

   Use 1,300 ticks for `agents`, which covers respawns and a period without a cursor, and 700 for `random_access`, which outlasts every initial lifetime. A failure names the workload, both backends and the first tick (0 is the state after `setup`).
4. Add one line per workload to each bench crate: `bench::<MyAddRemove>(c);` in `bench_compare/benches/compare.rs`, and `cache_bench!(add_remove_my_backend, MyAddRemove, [1_000, 10_000, 100_000]);` plus the function's name in the workload's group in `bench_cache/benches/cache.rs`. The sizes are the workload's `Spec::SIZES`; the cache bench asserts that they match.

## Running the benchmarks

```sh
cargo xtask spike-bench ecs                # wall clock, every workload, size and backend
cargo xtask spike-bench ecs --save m3-run1 # the same, saved as a Criterion baseline
cargo bench -p spike_ecs_bench_cache       # Linux with Valgrind: instructions, L1/LL/RAM hits, estimated cycles
```

Criterion's ids are `spike/ecs/<workload>/<backend>/<count>`. Since M3.10 the spike benches build `hashbrown` with `inline-more` and `default-hasher` (turned on by `hecs`) and `indexmap` and `foldhash` with `std` (turned on by `bevy_ecs`; these are the changed features of crates the spike code calls), so compare rows within one run, not with a run or saved baseline from before M3.10. Both commands are manual: neither is a CI gate, and `cargo xtask bench` and `bench-doc` leave them out. `spike-bench` refuses battery power on macOS, as `bench` does. Quote results under the rules for [wall-clock benchmarks](../../bench/README.md#wall-clock-benchmarks).

The cache bench needs Valgrind 3.20 or later and `gungraun-runner` (see [the instruction-count gate](../../bench/README.md#instruction-count-gate)). Counts are only comparable between runs on the same architecture and the same simulated cache sizes. Callgrind takes the simulated cache sizes from the host CPU when it can and from defaults when it cannot, so M3.8 should pin them with `--I1`, `--D1` and `--LL` before comparing runs from different machines. A plain Docker container may block `setarch` (`failed to set personality`); `--security-opt seccomp=unconfined` lets it through.
