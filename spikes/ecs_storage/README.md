# ECS storage spike

Status: Accepted (2026-11-14), added by M3.2. The rules of every workload are in the [M3.2 spec](../../docs/chunks/m3.2-spike-harness/spec.md) (§4.4); this page is how to use them.

## When this applies

- You add a backend or workload to the ECS storage spike, or run its benchmarks.
- You quote or check a number from M3's storage decision.

## Contents

| Directory | Package | Role |
|---|---|---|
| `harness/` | `spike_ecs_harness` | The `Workload` trait, the seven workloads' components, spawn, update and hash functions, `Fnv1a`, `check_conformance` |
| `vec/` | `spike_ecs_vec` | The reference backends: `vec_aos` for every workload, `vec_soa` for `agents` |
| `bench_compare/` | `spike_ecs_bench_compare` | Criterion: wall-clock time |
| `bench_cache/` | `spike_ecs_bench_cache` | Gungraun: instructions and simulated cache misses |

The workloads are `simple_iter`, `frag_iter`, `heavy_compute`, `add_remove`, `entity_churn`, `sprites` and `agents`.

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

   Use 1,300 ticks for `agents`, which covers respawns and a period without a cursor. A failure names the workload, both backends and the first tick (0 is the state after `setup`).
4. Add one line per workload to each bench crate: `bench::<MyAddRemove>(c);` in `bench_compare/benches/compare.rs`, and `cache_bench!(add_remove_my_backend, MyAddRemove, [1_000, 10_000, 100_000]);` plus the function's name in the workload's group in `bench_cache/benches/cache.rs`. The sizes are the workload's `Spec::SIZES`; the cache bench asserts that they match.

## Running the benchmarks

```sh
cargo xtask spike-bench ecs                # wall clock, every workload, size and backend
cargo xtask spike-bench ecs --save m3-run1 # the same, saved as a Criterion baseline
cargo bench -p spike_ecs_bench_cache       # Linux with Valgrind: instructions, L1/LL/RAM hits, estimated cycles
```

Criterion's ids are `spike/ecs/<workload>/<backend>/<count>`. Both commands are manual: neither is a CI gate, and `cargo xtask bench` and `bench-doc` leave them out. `spike-bench` refuses battery power on macOS, as `bench` does. Quote results under the rules for [wall-clock benchmarks](../../bench/README.md#wall-clock-benchmarks).

The cache bench needs Valgrind 3.20 or later and `gungraun-runner` (see [the instruction-count gate](../../bench/README.md#instruction-count-gate)). Counts are only comparable between runs on the same architecture and the same simulated cache sizes. Callgrind takes the simulated cache sizes from the host CPU when it can and from defaults when it cannot, so M3.8 should pin them with `--I1`, `--D1` and `--LL` before comparing runs from different machines. A plain Docker container may block `setarch` (`failed to set personality`); `--security-opt seccomp=unconfined` lets it through.
