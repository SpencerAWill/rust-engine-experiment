# Benchmarks

Status: Accepted (2026-11-12). M1 results, recorded 2026-11-12 on one machine. The data sections
below (marked) are generated from saved criterion baselines by `cargo xtask bench-doc`; the analysis is
written by hand and every figure in it was checked against that data. Read the caveats before
drawing conclusions.

## Summary

- **Against `slotmap::DenseSlotMap`** (the same dense design, and the fair comparison), `HandlePool`
  is level or close on most operations: medians are within 5% or better on iteration, insert (6%
  faster), sequential and random `get`, resolving a target handle, stale-handle lookups (7% faster at
  50% stale), and the per-frame update loop. It is slower on the removal and churn paths: 13% on removing everything, 10% on
  despawning in random order, 9% on remove-then-get. Of 90 comparable cases, 48 are within 5%, 31
  are more than 5% slower, and 11 are more than 5% faster. The cause of the remaining gap has not
  been investigated.
- **Against the sparse containers** (`slotmap::SlotMap`, `thunderdome::Arena`), point operations
  (`insert`, `get`, `remove`) are typically 1.3-2x slower, with removal up to 5x in one case. That is
  the expected price of dense storage: a second array access per lookup, plus a parallel array of
  handles. In exchange, the loops a simulation actually runs are faster or equal: the mutating
  per-frame update is 1.15-2.1x faster than `slotmap` (median ratio 0.67), and iterating after half
  the entries were removed is about 8x faster (median ratio 0.13; 0.06-0.37 across sizes), because
  a sparse container must step over the holes. In the mixed spawn/despawn/update frame the two are
  at parity (median ratio 0.97).
- **Handle allocators.** The lean `HandleAllocator` allocates as fast as `slotmap` with `()` values
  (0.89-0.95x), but trails it on free/realloc churn (1.2-1.9x) and `is_alive` (1.2-1.3x).
  `DenseHandleAllocator` keeps a dense list of live handles so it can iterate them, and pays for it:
  2.1-2.7x on `allocate`. `allocate_many` is about 2x faster than a loop of `allocate` when it creates
  new slots; when it reuses freed slots the gain is 1.7-2x up to 100k handles and disappears at 1M.
- **`Prng`** ties with `rand_xoshiro` and `rand`'s `SmallRng` on raw draws (same algorithm), is about
  3x faster than both on integer ranges, 19% faster on shuffling, and 8% slower on `f32` ranges.
  `fastrand` (a different, lighter generator) is faster on every measure.
- Per spec section 8, losses are recorded and analyzed here and are not fixed in M1: optimization
  waits for real workloads in M2.

## Methodology

<!-- bench:env -->
| | run 1 | run 2 |
|---|---|---|
| Date | 2026-11-12T15:52:04Z | 2026-11-12T17:00:01Z |
| Machine | Apple M4 Pro (Mac16,8) | Apple M4 Pro (Mac16,8) |
| Memory (GB) | 24 | 24 |
| Cores | 8 performance + 4 efficiency | 8 performance + 4 efficiency |
| OS | macOS 26.6.2 (25G83) | macOS 26.6.2 (25G83) |
| Toolchain | rustc 1.97.0 (2d8144b78 2026-07-07) | rustc 1.97.0 (2d8144b78 2026-07-07) |
| Power | AC Power until 11:24 local, then battery with two idle sleeps (those results were re-run) | Now drawing from 'AC Power' |
| System load at start | { 2.19 2.76 2.79 } (waited 70s for load < 2.5) | { 2.48 2.49 2.22 } (waited 0s for AC, 50s for load < 2.5) |
| Source commit | 6780faa | 381cedf |
<!-- /bench:env -->

Two runs make up the data: run 1 is the full suite, and run 2 re-ran the groups that ran on battery
power or across sleeps in run 1 (see Run conditions).

- **Commands:** `cargo xtask bench --save final` (that is `cargo bench --workspace --bench '*' --
  --save-baseline final`) for run 1. Run 2 repeated the affected groups with a benchmark-name filter and
  the same baseline name, which replaces just those results. This document is then produced by
  `cargo xtask bench-doc --baseline final --env docs/data/bench-env-run1.txt --env
  docs/data/bench-env-run2.txt`.
- **Caches:** L1D 128 KiB and L2 16 MiB (performance cores), L2 4 MiB (efficiency cores), 128-byte cache
  lines, from `sysctl`.
- **Profile:** `[profile.bench]` is `lto = "thin"`, `codegen-units = 1`, `debug = true` (symbols for
  profiling). Dependencies are optimized.
- **Harness:** Criterion 0.8 with its defaults (3 s warm-up, 5 s measurement) for the handle, PRNG,
  and time benchmarks. The realistic-scenario groups use a shorter 1 s warm-up and 3 s measurement.
  The 1M-element handle cases use 10 samples. All numbers are **medians**.
- **Subjects:** `engine_handle` is compared with `slotmap` 1.1 (`SlotMap` and `DenseSlotMap`) and
  `thunderdome` 0.6. `engine_random` is compared with `rand_xoshiro` 0.8 (the same algorithm),
  `rand` 0.10's `SmallRng`, and `fastrand` 2.5. Competitor crates are dev-dependencies only.
- **Values:** the classic handle groups use `u64` and a 64-byte struct (`64B`) at 1k, 100k, and 1M
  entries. The scenario groups use 16-byte and 64-byte values at 100 to 100k entries, because most
  simulations live in the cache-resident range. Random orders come from a fixed-seed `Prng`, so
  every library sees identical key orders.
- **Run conditions.** Run 1 started on AC power with other applications closed and the 1-minute load at
  about 2.2. About 32 minutes in (11:24 local) the laptop was unplugged, and the machine then went to
  idle sleep twice (39 seconds, then 6.8 minutes); I found out afterwards from the system power log.
  Every result from that point on was discarded: `allocator/is_alive`, both `allocator/batch_*` groups,
  all `scenario/*` and `prng/*` groups, `fixed_timestep` and `fixed_step_sim` (239 benchmarks). They were
  re-run in run 2 on AC power, under `caffeinate` so the machine could not sleep, and the power log for
  that window shows no power or sleep events. The other 198 results are from run 1, which was on AC power
  for all of them. Treat these as careful but not laboratory-grade numbers: the machine was not idle,
  and background applications, at least a VPN client, were running.
- **Noise.** Between two full runs, 291 benchmarks of libraries we did not change (`slotmap`,
  `thunderdome`, `rand`, `fastrand`) differed by a median of 0.6%, but about 13% of them moved by more
  than 5%, and a few moved by 30-70% (the `remove_all` and stale-lookup scenario groups are the
  most volatile). **Treat differences under about 5% as noise, and treat a single run-to-run change in
  those groups with suspicion.** The conclusions here therefore rest on ratios measured within the same
  run, not on comparisons between runs.
- **Scope:** wall-clock numbers come from aarch64 macOS and are not comparable with x86_64 or with
  instruction counts. Instruction-count regression gating starts in M2 (spec section 4.12).

## How to read the ratios

The table gives `engine_handle`'s (or `engine_random`'s) time divided by the competitor's, as a median
with the range across sizes and value types in parentheses, and the number of cases. **Below 1.00
means ours is faster; above 1.00 means it is slower.** The tallies under it count cases more than 5%
slower, within 5%, and more than 5% faster.

<!-- bench:ratios -->
| group | ours | competitor | median ratio (range) | cases |
|---|---|---|---|---|
| `allocator/allocate` | `engine_handle_dense` | `slotmap` | 2.31 (2.11–2.74) | 3 |
| `allocator/allocate` | `engine_handle_lean` | `slotmap` | 0.91 (0.89–0.95) | 3 |
| `allocator/free_realloc_churn` | `engine_handle_dense` | `slotmap` | 1.85 (1.56–2.31) | 3 |
| `allocator/free_realloc_churn` | `engine_handle_lean` | `slotmap` | 1.28 (1.18–1.89) | 3 |
| `allocator/is_alive` | `engine_handle_dense` | `slotmap` | 1.22 (1.20–1.27) | 3 |
| `allocator/is_alive` | `engine_handle_lean` | `slotmap` | 1.29 (1.18–1.31) | 3 |
| `churn_then_get` | `engine_handle` | `slotmap` | 1.33 (0.87–1.60) | 6 |
| `churn_then_get` | `engine_handle` | `slotmap_dense` | 1.09 (1.01–1.18) | 6 |
| `churn_then_get` | `engine_handle` | `thunderdome` | 1.33 (0.87–1.57) | 6 |
| `get_random` | `engine_handle` | `slotmap` | 1.49 (0.80–1.61) | 6 |
| `get_random` | `engine_handle` | `slotmap_dense` | 1.02 (0.98–1.29) | 6 |
| `get_random` | `engine_handle` | `thunderdome` | 1.34 (0.80–1.64) | 6 |
| `get_sequential` | `engine_handle` | `slotmap` | 1.32 (1.14–1.44) | 6 |
| `get_sequential` | `engine_handle` | `slotmap_dense` | 1.02 (0.94–1.19) | 6 |
| `get_sequential` | `engine_handle` | `thunderdome` | 1.22 (1.11–1.45) | 6 |
| `insert` | `engine_handle` | `slotmap` | 1.65 (1.21–2.02) | 6 |
| `insert` | `engine_handle` | `slotmap_dense` | 0.94 (0.91–1.16) | 6 |
| `insert` | `engine_handle` | `thunderdome` | 1.60 (1.19–2.12) | 6 |
| `iterate` | `engine_handle` | `slotmap` | 1.02 (0.90–1.07) | 6 |
| `iterate` | `engine_handle` | `slotmap_dense` | 1.00 (0.99–1.04) | 6 |
| `iterate` | `engine_handle` | `thunderdome` | 0.81 (0.76–0.89) | 6 |
| `iterate` | `engine_handle_values_slice` | `slotmap` | 0.38 (0.20–0.88) | 6 |
| `iterate` | `engine_handle_values_slice` | `slotmap_dense` | 0.37 (0.20–0.99) | 6 |
| `iterate` | `engine_handle_values_slice` | `thunderdome` | 0.29 (0.15–0.86) | 6 |
| `iterate_after_churn` | `engine_handle` | `slotmap` | 0.13 (0.06–0.37) | 6 |
| `iterate_after_churn` | `engine_handle` | `slotmap_dense` | 1.01 (0.99–1.02) | 6 |
| `iterate_after_churn` | `engine_handle` | `thunderdome` | 0.12 (0.06–0.32) | 6 |
| `iterate_after_churn` | `engine_handle_values_slice` | `slotmap` | 0.09 (0.02–0.18) | 6 |
| `iterate_after_churn` | `engine_handle_values_slice` | `slotmap_dense` | 0.38 (0.19–0.98) | 6 |
| `iterate_after_churn` | `engine_handle_values_slice` | `thunderdome` | 0.08 (0.02–0.15) | 6 |
| `prng/next_u64` | `engine_random` | `fastrand` | 2.96 (2.96–2.96) | 1 |
| `prng/next_u64` | `engine_random` | `rand_small_rng` | 1.01 (1.01–1.01) | 1 |
| `prng/next_u64` | `engine_random` | `rand_xoshiro` | 1.00 (1.00–1.00) | 1 |
| `prng/range_f32_0_1` | `engine_random` | `fastrand` | 1.16 (1.16–1.16) | 1 |
| `prng/range_f32_0_1` | `engine_random` | `rand_small_rng` | 1.08 (1.08–1.08) | 1 |
| `prng/range_f32_0_1` | `engine_random` | `rand_xoshiro` | 1.08 (1.08–1.08) | 1 |
| `prng/range_u32_0_1000` | `engine_random` | `fastrand` | 1.61 (1.61–1.61) | 1 |
| `prng/range_u32_0_1000` | `engine_random` | `rand_small_rng` | 0.34 (0.34–0.34) | 1 |
| `prng/range_u32_0_1000` | `engine_random` | `rand_xoshiro` | 0.34 (0.34–0.34) | 1 |
| `prng/shuffle_10k` | `engine_random` | `fastrand` | 1.57 (1.57–1.57) | 1 |
| `prng/shuffle_10k` | `engine_random` | `rand_small_rng` | 0.82 (0.82–0.82) | 1 |
| `prng/shuffle_10k` | `engine_random` | `rand_xoshiro` | 0.81 (0.81–0.81) | 1 |
| `remove_all` | `engine_handle` | `slotmap` | 2.13 (1.44–5.10) | 6 |
| `remove_all` | `engine_handle` | `slotmap_dense` | 1.13 (0.56–1.32) | 6 |
| `remove_all` | `engine_handle` | `thunderdome` | 2.08 (0.53–2.43) | 6 |
| `scenario/despawn_random_order` | `engine_handle` | `slotmap` | 2.07 (1.38–2.85) | 8 |
| `scenario/despawn_random_order` | `engine_handle` | `slotmap_dense` | 1.10 (1.03–1.74) | 8 |
| `scenario/despawn_random_order` | `engine_handle` | `thunderdome` | 1.52 (1.20–2.45) | 8 |
| `scenario/frame_update` | `engine_handle` | `slotmap` | 0.67 (0.48–0.87) | 8 |
| `scenario/frame_update` | `engine_handle` | `slotmap_dense` | 1.00 (0.99–1.02) | 8 |
| `scenario/frame_update` | `engine_handle` | `thunderdome` | 0.56 (0.34–0.84) | 8 |
| `scenario/frame_update` | `engine_handle_values_slice` | `slotmap` | 0.67 (0.48–0.87) | 8 |
| `scenario/frame_update` | `engine_handle_values_slice` | `slotmap_dense` | 1.00 (0.99–1.00) | 8 |
| `scenario/frame_update` | `engine_handle_values_slice` | `thunderdome` | 0.56 (0.34–0.85) | 8 |
| `scenario/get_stale_10pct` | `engine_handle` | `slotmap` | 1.34 (0.80–1.46) | 8 |
| `scenario/get_stale_10pct` | `engine_handle` | `slotmap_dense` | 1.03 (0.99–1.30) | 8 |
| `scenario/get_stale_10pct` | `engine_handle` | `thunderdome` | 1.34 (0.79–1.46) | 8 |
| `scenario/get_stale_50pct` | `engine_handle` | `slotmap` | 1.09 (0.92–1.23) | 8 |
| `scenario/get_stale_50pct` | `engine_handle` | `slotmap_dense` | 0.93 (0.84–1.20) | 8 |
| `scenario/get_stale_50pct` | `engine_handle` | `thunderdome` | 1.08 (0.93–1.17) | 8 |
| `scenario/resolve_target` | `engine_handle` | `slotmap` | 1.27 (1.08–1.46) | 8 |
| `scenario/resolve_target` | `engine_handle` | `slotmap_dense` | 1.02 (1.00–1.08) | 8 |
| `scenario/resolve_target` | `engine_handle` | `thunderdome` | 1.25 (1.09–1.46) | 8 |
| `scenario/steady_churn_frame` | `engine_handle` | `slotmap` | 0.97 (0.75–1.09) | 8 |
| `scenario/steady_churn_frame` | `engine_handle` | `slotmap_dense` | 1.05 (0.93–1.13) | 8 |
| `scenario/steady_churn_frame` | `engine_handle` | `thunderdome` | 0.80 (0.62–0.99) | 8 |

Across all comparable cases, `engine_handle` against `slotmap`: 55 of 90 more than 5% slower, 9 within 5%, 26 more than 5% faster; `slotmap_dense`: 31 of 90 more than 5% slower, 48 within 5%, 11 more than 5% faster; `thunderdome`: 52 of 90 more than 5% slower, 5 within 5%, 33 more than 5% faster.
<!-- /bench:ratios -->

## Interpretation

**The dense design does what it was chosen to do.** Iterating the pool is a straight walk over
contiguous memory, so the per-frame update and iteration after churn are where it wins, and the
`values()` slice is the fastest way to process every value (36 µs versus 2.25 ms to iterate 1M `u64`s
after removing half of them, against `slotmap`). The mutating `frame_update` benchmark is the closest
proxy here for how a simulation uses the container, and `engine_handle` leads the sparse containers on
it at every size.

**The price is on point operations.** A lookup reads the slot, then the dense position, so `get`,
`remove`, and resolving a handle to another entity cost more than in a sparse container that stores the
value in the slot. At 1M `u64` values, random `get` is 1.6x `slotmap`'s time. In realistic mixes this
largely washes out: the steady-state churn frame (2% despawned and respawned, then everything updated)
is at parity with `slotmap` (median ratio 0.97).

**Against the design it most resembles, we are close.** `DenseSlotMap` is the fair comparison. On
iteration, insert, sequential and random `get`, resolving a target and stale lookups the medians are
within about 5% of equal or better (individual sequential-`get` cases range from 0.94 to 1.19); removal and churn
paths are 9-13% slower. One change moved the random-access numbers between runs: resolving a handle now
does one range check against the dense length instead of a separate vacant-bit test, which a same-session
A/B measured as 4-19% faster for `get` on `u64` and 19-27% faster for `is_alive`, and about 10-14% slower
for sequential `get` on 64-byte values. I have not profiled the remaining gap, so I do not have a reason
to offer, only the measurement.

**The two allocators trade iteration for speed.** Allocating a handle with `HandleAllocator` costs about
what `slotmap<()>` does; `DenseHandleAllocator` costs 2.1-2.7x because it also maintains the dense list of
live handles, which is what makes its `iter()` O(len) over contiguous memory. If M2 allocates entities in
a hot path and does not need dense iteration of the allocator itself, use the lean one. Free/realloc
churn and `is_alive` are 1.2-1.9x `slotmap`'s time for the lean allocator; I have not profiled why.
`allocate_many` reserves all the room it needs up front, which is where its roughly 2x gain on fresh
slots comes from; reusing freed slots means following the free list one slot at a time, so the gain
fades as the free list outgrows the cache (it is gone at 1M).

**PRNG.** On `next_u64`, `engine_random`, `rand_xoshiro`, and `SmallRng` are level, as expected for one
algorithm (xoshiro256++). Bounded integers are about 3x faster than `rand`'s, shuffling 10,000 elements
is 19% faster, and `f32` ranges are about 8% slower than `rand`'s (a candidate for tuning); I did not
investigate either difference. `fastrand` is faster on every measure (about 3x on `next_u64`). It uses
a different generator with a smaller state, and this comparison says nothing about statistical quality,
only speed.

**Time and the sample.** One `FixedTimestep::accumulate` call with a 16 ms frame at 60 Hz costs about
11 ns; I did not profile it (integer division is the likely cost), and it is not a bottleneck today. The
`fixed_step_sim` default run (10,000 particles for 600 ticks, about 6 million particle-steps, including
`retain` and respawning) takes about 6.9 ms, roughly 1.1 ns per particle-step, with zero heap allocations
in steady state.

## Limitations

- One machine and medians only; the tables show no confidence intervals (the HTML report from
  `cargo xtask bench-report` does).
- The classic groups use a freshly built container; the scenario groups use more realistic mixes
  but are still synthetic. The real test is the M2 ECS.
- The `HandlePool` and competitors were each driven through a thin common trait in the benchmark
  harness; differences in what each library's iteration API allows (for example `slotmap`
  exposing no values slice) are part of the comparison, not hidden.
- The reasons for the remaining gaps (removal paths versus `DenseSlotMap`, the lean allocator's churn and
  `is_alive` cost, the `get_sequential` slowdown on 64-byte values) are hypotheses at best; none were
  profiled.
- Run 1 was partly spoiled by a power-source change and two sleeps, and the affected groups were
  re-run (see Run conditions). Individual benchmarks in the volatile families can still differ a lot
  between runs even on clean power.

## Instruction counts

The `bench-regress` gate (see CONTRIBUTING.md, "Instruction-count gate") counts CPU instructions under Valgrind, which does not depend on the machine's load, but only on the code that ran. Those counts decide whether one commit changed the cost of an operation relative to another. They are never compared with the Criterion figures in this document, which are wall-clock times on one machine, and counts from different architectures (the aarch64 devcontainer, the x86_64 CI runner) are never compared with each other.

### Instruction counts across milestones

Filled in by the milestone check in `docs/maintainers.md` (`cargo xtask bench-regress --report-only --base m<N-1>`): for each run, the machine's architecture, both tags, and every benchmark whose count changed by more than 5%. None recorded yet.

## Results

All times are Criterion medians per iteration. Columns are the number of entries where applicable;
rows are `<implementation> · <value type>`. Groups are listed alphabetically.

<!-- bench:tables -->
#### `allocator/allocate`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine_handle_dense` | 2.34 µs | 201 µs | 2.75 ms |
| `engine_handle_lean` | 988 ns | 66.7 µs | 1.13 ms |
| `slotmap` | 1.11 µs | 73.4 µs | 1.19 ms |

#### `allocator/batch_fresh`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `batch` | 622 ns | 43.7 µs | 697 µs |
| `loop` | 1.27 µs | 93.7 µs | 1.40 ms |

#### `allocator/batch_recycled`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `batch` | 1.60 µs | 156 µs | 1.67 ms |
| `loop` | 2.64 µs | 306 µs | 1.73 ms |

#### `allocator/free_realloc_churn`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine_handle_dense` | 2.42 µs | 523 µs | 9.50 ms |
| `engine_handle_lean` | 1.98 µs | 429 µs | 6.06 ms |
| `slotmap` | 1.05 µs | 336 µs | 5.14 ms |

#### `allocator/is_alive`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine_handle_dense` | 382 ns | 70.1 µs | 862 µs |
| `engine_handle_lean` | 376 ns | 74.2 µs | 891 µs |
| `slotmap` | 318 ns | 57.5 µs | 678 µs |

#### `churn_then_get`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine_handle · 64B` | 509 ns | 117 µs | 4.61 ms |
| `engine_handle · u64` | 506 ns | 109 µs | 1.54 ms |
| `slotmap · 64B` | 398 ns | 125 µs | 5.30 ms |
| `slotmap · u64` | 367 ns | 70.0 µs | 961 µs |
| `slotmap_dense · 64B` | 462 ns | 116 µs | 3.95 ms |
| `slotmap_dense · u64` | 428 ns | 109 µs | 1.43 ms |
| `thunderdome · 64B` | 396 ns | 124 µs | 5.28 ms |
| `thunderdome · u64` | 370 ns | 70.2 µs | 980 µs |

#### `fixed_step_sim`

| subject | — |
|---|---|
| `default_config` | 6.85 ms |

#### `fixed_timestep`

| subject | — |
|---|---|
| `accumulate_16ms_at_60hz` | 11.1 ms |

#### `get_random`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine_handle · 64B` | 560 ns | 119 µs | 4.00 ms |
| `engine_handle · u64` | 508 ns | 114 µs | 1.59 ms |
| `slotmap · 64B` | 369 ns | 126 µs | 5.02 ms |
| `slotmap · u64` | 350 ns | 71.9 µs | 992 µs |
| `slotmap_dense · 64B` | 435 ns | 120 µs | 3.85 ms |
| `slotmap_dense · u64` | 428 ns | 113 µs | 1.62 ms |
| `thunderdome · 64B` | 454 ns | 127 µs | 4.99 ms |
| `thunderdome · u64` | 350 ns | 72.0 µs | 972 µs |

#### `get_sequential`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine_handle · 64B` | 506 ns | 74.4 µs | 879 µs |
| `engine_handle · u64` | 504 ns | 49.8 µs | 515 µs |
| `slotmap · 64B` | 368 ns | 65.1 µs | 745 µs |
| `slotmap · u64` | 349 ns | 37.2 µs | 396 µs |
| `slotmap_dense · 64B` | 438 ns | 79.1 µs | 852 µs |
| `slotmap_dense · u64` | 424 ns | 49.1 µs | 507 µs |
| `thunderdome · 64B` | 458 ns | 66.3 µs | 762 µs |
| `thunderdome · u64` | 347 ns | 37.1 µs | 399 µs |

#### `insert`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine_handle · 64B` | 3.89 µs | 658 µs | 8.65 ms |
| `engine_handle · u64` | 2.22 µs | 160 µs | 2.83 ms |
| `slotmap · 64B` | 2.40 µs | 542 µs | 6.02 ms |
| `slotmap · u64` | 1.18 µs | 79.1 µs | 1.68 ms |
| `slotmap_dense · 64B` | 4.05 µs | 682 µs | 7.44 ms |
| `slotmap_dense · u64` | 2.39 µs | 176 µs | 3.10 ms |
| `thunderdome · 64B` | 2.27 µs | 551 µs | 5.82 ms |
| `thunderdome · u64` | 1.24 µs | 75.3 µs | 1.93 ms |

#### `iterate`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine_handle · 64B` | 271 ns | 48.7 µs | 623 µs |
| `engine_handle · u64` | 267 ns | 27.0 µs | 271 µs |
| `engine_handle_values_slice · 64B` | 130 ns | 48.1 µs | 564 µs |
| `engine_handle_values_slice · u64` | 52.9 ns | 7.04 µs | 71.3 µs |
| `slotmap · 64B` | 264 ns | 54.4 µs | 673 µs |
| `slotmap · u64` | 261 ns | 25.2 µs | 259 µs |
| `slotmap_dense · 64B` | 273 ns | 48.6 µs | 600 µs |
| `slotmap_dense · u64` | 271 ns | 27.1 µs | 267 µs |
| `thunderdome · 64B` | 353 ns | 55.7 µs | 699 µs |
| `thunderdome · u64` | 352 ns | 33.0 µs | 337 µs |

#### `iterate_after_churn`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine_handle · 64B` | 137 ns | 23.8 µs | 251 µs |
| `engine_handle · u64` | 138 ns | 13.5 µs | 136 µs |
| `engine_handle_values_slice · 64B` | 67.9 ns | 23.7 µs | 247 µs |
| `engine_handle_values_slice · u64` | 25.2 ns | 3.57 µs | 36.0 µs |
| `slotmap · 64B` | 375 ns | 154 µs | 2.26 ms |
| `slotmap · u64` | 388 ns | 154 µs | 2.25 ms |
| `slotmap_dense · 64B` | 138 ns | 24.0 µs | 252 µs |
| `slotmap_dense · u64` | 136 ns | 13.3 µs | 133 µs |
| `thunderdome · 64B` | 439 ns | 170 µs | 2.32 ms |
| `thunderdome · u64` | 436 ns | 153 µs | 2.24 ms |

#### `prng/next_u64`

| subject | — |
|---|---|
| `engine_random` | 687 µs |
| `fastrand` | 233 µs |
| `rand_small_rng` | 683 µs |
| `rand_xoshiro` | 684 µs |

#### `prng/range_f32_0_1`

| subject | — |
|---|---|
| `engine_random` | 739 µs |
| `fastrand` | 636 µs |
| `rand_small_rng` | 683 µs |
| `rand_xoshiro` | 685 µs |

#### `prng/range_u32_0_1000`

| subject | — |
|---|---|
| `engine_random` | 780 µs |
| `fastrand` | 484 µs |
| `rand_small_rng` | 2.31 ms |
| `rand_xoshiro` | 2.31 ms |

#### `prng/shuffle_10k`

| subject | — |
|---|---|
| `engine_random` | 8.03 µs |
| `fastrand` | 5.13 µs |
| `rand_small_rng` | 9.76 µs |
| `rand_xoshiro` | 9.86 µs |

#### `remove_all`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine_handle · 64B` | 4.88 µs | 477 µs | 11.1 ms |
| `engine_handle · u64` | 1.36 µs | 138 µs | 3.13 ms |
| `slotmap · 64B` | 2.59 µs | 254 µs | 7.73 ms |
| `slotmap · u64` | 571 ns | 55.6 µs | 613 µs |
| `slotmap_dense · 64B` | 4.71 µs | 848 µs | 10.8 ms |
| `slotmap_dense · u64` | 1.03 µs | 104 µs | 2.57 ms |
| `thunderdome · 64B` | 3.98 µs | 895 µs | 5.16 ms |
| `thunderdome · u64` | 576 ns | 56.8 µs | 1.57 ms |

#### `scenario/despawn_random_order`

| subject | 100 | 1K | 10K | 100K |
|---|---|---|---|---|
| `engine_handle · 16B` | 299 ns | 2.27 µs | 27.2 µs | 395 µs |
| `engine_handle · 64B` | 565 ns | 5.21 µs | 57.6 µs | 1.25 ms |
| `slotmap · 16B` | 113 ns | 1.01 µs | 12.2 µs | 139 µs |
| `slotmap · 64B` | 300 ns | 2.82 µs | 30.1 µs | 906 µs |
| `slotmap_dense · 16B` | 291 ns | 2.01 µs | 24.0 µs | 324 µs |
| `slotmap_dense · 64B` | 539 ns | 5.06 µs | 53.3 µs | 720 µs |
| `thunderdome · 16B` | 148 ns | 1.34 µs | 15.0 µs | 161 µs |
| `thunderdome · 64B` | 427 ns | 4.09 µs | 42.8 µs | 1.04 ms |

#### `scenario/frame_update`

| subject | 100 | 1K | 10K | 100K |
|---|---|---|---|---|
| `engine_handle · 16B` | 13.4 ns | 128 ns | 2.13 µs | 22.0 µs |
| `engine_handle · 64B` | 20.0 ns | 197 ns | 5.78 µs | 58.6 µs |
| `engine_handle_values_slice · 16B` | 13.3 ns | 128 ns | 2.09 µs | 22.0 µs |
| `engine_handle_values_slice · 64B` | 19.8 ns | 197 ns | 5.85 µs | 58.9 µs |
| `slotmap · 16B` | 25.8 ns | 266 ns | 3.57 µs | 36.0 µs |
| `slotmap · 64B` | 25.9 ns | 267 ns | 6.71 µs | 67.4 µs |
| `slotmap_dense · 16B` | 13.5 ns | 128 ns | 2.09 µs | 22.0 µs |
| `slotmap_dense · 64B` | 20.0 ns | 197 ns | 5.86 µs | 59.0 µs |
| `thunderdome · 16B` | 37.3 ns | 376 ns | 3.76 µs | 37.6 µs |
| `thunderdome · 64B` | 36.4 ns | 354 ns | 6.92 µs | 69.4 µs |

#### `scenario/get_stale_10pct`

| subject | 100 | 1K | 10K | 100K |
|---|---|---|---|---|
| `engine_handle · 16B` | 54.3 ns | 574 ns | 7.45 µs | 110 µs |
| `engine_handle · 64B` | 54.4 ns | 575 ns | 7.89 µs | 123 µs |
| `slotmap · 16B` | 38.9 ns | 394 ns | 6.29 µs | 85.7 µs |
| `slotmap · 64B` | 39.2 ns | 395 ns | 9.91 µs | 127 µs |
| `slotmap_dense · 16B` | 54.4 ns | 558 ns | 7.15 µs | 111 µs |
| `slotmap_dense · 64B` | 43.9 ns | 441 ns | 7.68 µs | 115 µs |
| `thunderdome · 16B` | 39.2 ns | 393 ns | 6.18 µs | 84.3 µs |
| `thunderdome · 64B` | 38.8 ns | 393 ns | 10.0 µs | 125 µs |

#### `scenario/get_stale_50pct`

| subject | 100 | 1K | 10K | 100K |
|---|---|---|---|---|
| `engine_handle · 16B` | 43.7 ns | 586 ns | 7.67 µs | 262 µs |
| `engine_handle · 64B` | 42.5 ns | 647 ns | 7.69 µs | 274 µs |
| `slotmap · 16B` | 38.3 ns | 526 ns | 7.13 µs | 279 µs |
| `slotmap · 64B` | 38.4 ns | 524 ns | 7.96 µs | 297 µs |
| `slotmap_dense · 16B` | 51.7 ns | 664 ns | 8.95 µs | 314 µs |
| `slotmap_dense · 64B` | 39.9 ns | 538 ns | 6.86 µs | 278 µs |
| `thunderdome · 16B` | 37.7 ns | 536 ns | 7.20 µs | 274 µs |
| `thunderdome · 64B` | 37.9 ns | 552 ns | 8.05 µs | 295 µs |

#### `scenario/resolve_target`

| subject | 100 | 1K | 10K | 100K |
|---|---|---|---|---|
| `engine_handle · 16B` | 88.4 ns | 882 ns | 15.4 µs | 204 µs |
| `engine_handle · 64B` | 91.8 ns | 888 ns | 18.0 µs | 234 µs |
| `slotmap · 16B` | 69.8 ns | 699 ns | 10.6 µs | 143 µs |
| `slotmap · 64B` | 69.8 ns | 701 ns | 16.6 µs | 203 µs |
| `slotmap_dense · 16B` | 88.6 ns | 870 ns | 14.2 µs | 203 µs |
| `slotmap_dense · 64B` | 86.2 ns | 871 ns | 16.9 µs | 227 µs |
| `thunderdome · 16B` | 73.6 ns | 700 ns | 10.6 µs | 142 µs |
| `thunderdome · 64B` | 73.9 ns | 699 ns | 16.6 µs | 203 µs |

#### `scenario/steady_churn_frame`

| subject | 100 | 1K | 10K | 100K |
|---|---|---|---|---|
| `engine_handle · 16B` | 27.4 ns | 277 ns | 4.37 µs | 53.7 µs |
| `engine_handle · 64B` | 34.9 ns | 425 ns | 8.84 µs | 99.8 µs |
| `slotmap · 16B` | 33.6 ns | 369 ns | 4.98 µs | 56.5 µs |
| `slotmap · 64B` | 35.5 ns | 391 ns | 8.46 µs | 100 µs |
| `slotmap_dense · 16B` | 29.4 ns | 250 ns | 3.88 µs | 49.3 µs |
| `slotmap_dense · 64B` | 37.5 ns | 401 ns | 8.42 µs | 96.0 µs |
| `thunderdome · 16B` | 44.3 ns | 450 ns | 5.38 µs | 58.9 µs |
| `thunderdome · 64B` | 47.3 ns | 536 ns | 8.96 µs | 105 µs |
<!-- /bench:tables -->
