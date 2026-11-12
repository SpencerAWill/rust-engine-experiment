# Benchmarks

Status: M1 results, recorded 2026-11-12. Wall-clock numbers from one machine; see the
caveats before drawing conclusions.

## Summary

- **Against `slotmap::DenseSlotMap`** (the same dense design, and the fair comparison), `HandlePool`
  has medians within about 2% on iteration, insert, sequential `get`, and the per-frame update loop,
  and is about 5-20% slower on random-access paths (random `get`, stale-handle lookups, removal, resolving a target
  handle). Of 90 comparable cases, 46 are within 5%,
  41 are more than 5% slower, and 3 are more than 5% faster.
  The cause of the remaining gap has not been investigated.
- **Against the sparse containers** (`slotmap::SlotMap`, `thunderdome::Arena`), point operations
  (`insert`, `get`, `remove`) are typically 1.3-2x slower, which is the expected price of dense
  storage: a second array access per lookup, plus a parallel array of handles. In exchange, the
  loops a simulation actually runs are faster or equal: the mutating per-frame update is about 1.2-2x
  faster than `slotmap`, and iterating after half the entries were removed is about 8x faster
  (median ratio 0.12; 0.06-0.35 across sizes), because a sparse container must step over the holes.
  In the mixed spawn/despawn/update frame the two are at parity.
- **`HandleAllocator` is slower than `slotmap` with `()` values** by 1.3x (`is_alive`) to 3.1x
  (`allocate`). It is the least favorable result here: it wraps a `HandlePool<()>`, so it pays for
  the dense handle list it keeps in order to iterate live handles in O(len). Whether that matters
  depends on how hot entity allocation is in M2.
- **`Prng`** ties with `rand_xoshiro` and `rand`'s `SmallRng` on raw draws (same algorithm), is about
  3x faster than both on integer ranges, and is about 8% slower on `f32` ranges. `fastrand` (a
  different, lighter generator) is faster on every measure.
- Per spec section 8, losses are recorded and analyzed here and are not fixed in M1: optimization
  waits for real workloads in M2.

## Methodology

| | |
|---|---|
| Date | 2026-11-12T07:30:42Z |
| Machine | Apple M4 Pro (Mac16,8), 24 GB |
| Cores | 8 performance + 4 efficiency |
| Caches | L1D 128 KiB and L2 16 MiB (performance cores); L2 4 MiB (efficiency cores); 128-byte cache lines |
| OS | macOS 26.6.2 (25G83) |
| Toolchain | rustc 1.97.0 (2d8144b78 2026-07-07) |
| Power | Now drawing from 'AC Power' |
| System load at start | 2.38, 3.75, 3.41 (1, 5, 15-minute averages) |
| Source | commit `c00e123` plus this document |
| Command | `cargo xtask bench --save m1` (that is `cargo bench --workspace --bench '*' -- --save-baseline m1`) |

- **Profile:** `[profile.bench]` is `lto = "thin"`, `codegen-units = 1`, `debug = true` (symbols for
  profiling). Dependencies are optimized.
- **Harness:** Criterion 0.8 with its defaults (3 s warm-up, 5 s measurement) for the handle, PRNG,
  and time benchmarks. The realistic-scenario groups use a shorter 1 s warm-up and 3 s measurement.
  The 1M-element handle cases use 10 samples. All numbers are **medians**.
- **Subjects:** `engine_handle` is compared with `slotmap` 1.1 (`SlotMap` and `DenseSlotMap`) and
  `thunderdome` 0.6. `engine_random` is compared with `rand_xoshiro` 0.8 (the same algorithm),
  `rand` 0.10's `SmallRng`, and `fastrand` 2.5. Competitor crates are dev-dependencies
  only.
- **Values:** the classic handle groups use `u64` and a 64-byte struct (`64B`) at 1k, 100k, and 1M
  entries. The scenario groups use 16-byte and 64-byte values at 100 to 100k entries, because most
  simulations live in the cache-resident range. Random orders come from a fixed-seed `Prng`, so
  every library sees identical key orders.
- **Run conditions:** the laptop was plugged in, and the run was started only after heavy
  background system activity (Spotlight and security-scan indexing) had settled. The machine was
  not idle, though: the 1-minute load average was about 2.4 at the start, and background
  applications, at least a VPN client, were still running. Treat these as careful but not
  laboratory-grade numbers.
- **Noise:** repeated measurements of the same large benchmarks in this session agreed to within
  about 2-3%. Cases in the tens or low hundreds of nanoseconds are noisier. **Treat differences
  under about 5% as noise.**
- **Scope:** wall-clock numbers come from aarch64 macOS and are not comparable with x86_64 or with
  instruction counts. Instruction-count regression gating starts in M2 (spec section 4.12).

## How to read the ratios

The table gives `engine_handle` time divided by the competitor's time, as a median with the range
across sizes and value types in parentheses. **Below 1.00 means `engine_handle` is faster; above
1.00 means it is slower.**

| Operation | vs `slotmap` | vs `slotmap` dense | vs `thunderdome` |
|---|---|---|---|
| `insert` | 1.71 (1.20–2.26) | 0.99 (0.97–1.07) | 1.74 (1.18–2.38) |
| `get_sequential` | 1.34 (1.06–1.46) | 1.02 (0.89–1.18) | 1.34 (1.05–1.46) |
| `get_random` | 1.41 (0.89–1.81) | 1.17 (1.01–1.19) | 1.41 (0.89–1.81) |
| `remove_all` | 2.00 (1.00–2.43) | 1.16 (1.01–1.31) | 1.59 (0.94–2.40) |
| `iterate` | 1.02 (0.88–1.08) | 1.00 (0.98–1.00) | 0.81 (0.76–0.87) |
| `churn_then_get` | 1.32 (0.89–1.74) | 1.13 (0.99–1.19) | 1.33 (0.89–1.79) |
| `iterate_after_churn` | 0.12 (0.06–0.35) | 1.00 (0.99–1.01) | 0.12 (0.06–0.32) |
| `scenario/frame_update` | 0.68 (0.48–0.87) | 1.00 (0.97–1.00) | 0.56 (0.33–0.85) |
| `scenario/steady_churn_frame` | 0.97 (0.69–1.09) | 1.06 (0.91–1.12) | 0.83 (0.62–0.98) |
| `scenario/despawn_random_order` | 1.99 (1.19–2.71) | 1.05 (1.02–1.16) | 1.49 (1.04–2.32) |
| `scenario/resolve_target` | 1.39 (1.15–1.62) | 1.12 (1.02–1.21) | 1.39 (1.15–1.63) |
| `scenario/get_stale_10pct` | 1.38 (0.79–1.48) | 1.18 (1.02–1.32) | 1.35 (0.78–1.48) |
| `scenario/get_stale_50pct` | 1.10 (0.94–1.20) | 1.12 (1.06–1.26) | 1.12 (0.92–1.20) |
| `allocator/allocate` | 2.51 (2.29–3.10) | — | — |
| `allocator/free_realloc_churn` | 1.72 (1.56–2.33) | — | — |
| `allocator/is_alive` | 1.32 (1.31–1.33) | — | — |

Across all comparable cases, `engine_handle` is more than 5% slower than `slotmap` in
63 of 99, than `slotmap` dense in 41 of
90, and than `thunderdome` in 51 of 90.

## Interpretation

**The dense design does what it was chosen to do.** Iterating the pool is a straight walk over
contiguous memory, so the per-frame update and iteration after churn are where it wins, and the
`values()` slice is the fastest way to process every value (for example 36 µs versus 2.2 ms to
iterate 1M `u64`s after removing half of them, against `slotmap`). The mutating `frame_update`
benchmark is the closest proxy here for how a simulation uses the container, and `engine_handle`
leads the sparse containers on it at every size.

**The price is on point operations.** A lookup reads the slot, then the dense position, so
`get`, `remove`, and resolving a handle to another entity cost more than in a sparse container
that stores the value in the slot. At 1M `u64` values, random `get` is 1.8x `slotmap`'s time.
In realistic mixes this largely washes out: the steady-state churn frame (2% despawned and
respawned, then everything updated) is at parity with `slotmap` (median ratio 0.97).

**Against the design it most resembles, we are close but not level.** `DenseSlotMap` is the fair
comparison. On sequential `get`, insert, iteration, and the update loop the medians are within about 2% of
equal (individual sequential-`get` cases range from 0.89 to 1.18); on random access and removal we
are roughly 5-20% slower. I have not profiled the difference, so I do
not have a reason to offer, only the measurement.

**`HandleAllocator` is the weak spot.** Allocating is 2.3-3.1x `slotmap<()>`'s time and
free-then-reallocate churn is 1.6-2.3x. Both containers store no values, so this is pure
bookkeeping cost: our allocator keeps a dense list of live handles (so that `iter()` is O(len)
over contiguous memory), and `slotmap` does not. If M2 allocates entities in a hot path and does
not need dense iteration of the allocator itself, a variant without the dense handle list is the
obvious thing to try. That decision belongs to M2, when the real access pattern is known.

**PRNG.** On `next_u64`, `engine_random`, `rand_xoshiro`, and `SmallRng` are level, as expected
for one algorithm (xoshiro256++). Bounded integers are about 3x faster than `rand`'s, and `f32` ranges are
about 8% slower than `rand`'s (a candidate for tuning); I did not investigate either difference.
`fastrand` is faster on every measure (about 2.9x on `next_u64`). It uses a different generator
with a smaller state, and this comparison says nothing about statistical quality, only speed.

**Time and the sample.** One `FixedTimestep::accumulate` call with a 16 ms frame at 60 Hz costs
about 11 ns; I did not profile it (integer division is the likely cost), and it is not a
bottleneck today. The `fixed_step_sim` default run (10,000 particles for 600 ticks, about 6 million
particle-steps, including `retain` and respawning) takes about
6.9 ms, roughly 1.1 ns per particle-step, with zero
heap allocations in steady state.

## Limitations

- One machine, one run, medians only; no confidence intervals are shown in the tables.
- The classic groups use a freshly built container; the scenario groups use more realistic mixes
  but are still synthetic. The real test is the M2 ECS.
- The `HandlePool` and competitors were each driven through a thin common trait in the benchmark
  harness; differences in what each library's iteration API allows (for example `slotmap`
  exposing no values slice) are part of the comparison, not hidden.
- The reasons for the remaining gaps (random access versus `DenseSlotMap`, allocator cost) are
  hypotheses at best; none were profiled.

## Results

All times are Criterion medians. Columns are `<value type>/<number of entries>` where applicable.

### Handle pool: classic operations

#### `insert`

| subject | 64B/1000 | 64B/100000 | 64B/1000000 | u64/1000 | u64/100000 | u64/1000000 |
|---|---|---|---|---|---|---|
| `engine_handle` | 3.9 µs | 684.9 µs | 8.08 ms | 2.4 µs | 182.0 µs | 2.86 ms |
| `slotmap` | 2.4 µs | 566.0 µs | 6.72 ms | 1.2 µs | 80.4 µs | 1.60 ms |
| `slotmap_dense` | 4.0 µs | 702.6 µs | 7.57 ms | 2.4 µs | 178.8 µs | 2.96 ms |
| `thunderdome` | 2.3 µs | 579.7 µs | 6.07 ms | 1.2 µs | 76.5 µs | 1.63 ms |

#### `get_sequential`

| subject | 64B/1000 | 64B/100000 | 64B/1000000 | u64/1000 | u64/100000 | u64/1000000 |
|---|---|---|---|---|---|---|
| `engine_handle` | 505.7 ns | 70.5 µs | 795.8 µs | 505.1 ns | 50.4 µs | 523.0 µs |
| `slotmap` | 372.1 ns | 65.3 µs | 748.5 µs | 346.0 ns | 37.3 µs | 396.1 µs |
| `slotmap_dense` | 432.9 ns | 79.1 µs | 854.9 µs | 427.1 ns | 49.1 µs | 514.9 µs |
| `thunderdome` | 372.0 ns | 64.8 µs | 756.4 µs | 345.8 ns | 37.2 µs | 395.6 µs |

#### `get_random`

| subject | 64B/1000 | 64B/100000 | 64B/1000000 | u64/1000 | u64/100000 | u64/1000000 |
|---|---|---|---|---|---|---|
| `engine_handle` | 506.6 ns | 127.1 µs | 4.50 ms | 504.2 ns | 114.2 µs | 1.76 ms |
| `slotmap` | 374.4 ns | 125.7 µs | 5.04 ms | 345.3 ns | 71.7 µs | 971.1 µs |
| `slotmap_dense` | 431.9 ns | 118.4 µs | 3.86 ms | 423.1 ns | 113.0 µs | 1.49 ms |
| `thunderdome` | 371.1 ns | 126.8 µs | 5.08 ms | 348.7 ns | 71.6 µs | 972.5 µs |

#### `remove_all`

| subject | 64B/1000 | 64B/100000 | 64B/1000000 | u64/1000 | u64/100000 | u64/1000000 |
|---|---|---|---|---|---|---|
| `engine_handle` | 4.9 µs | 882.2 µs | 11.65 ms | 1.4 µs | 133.6 µs | 3.34 ms |
| `slotmap` | 2.6 µs | 880.2 µs | 8.36 ms | 568.6 ns | 55.8 µs | 1.57 ms |
| `slotmap_dense` | 4.7 µs | 875.5 µs | 11.26 ms | 1.1 µs | 103.8 µs | 2.62 ms |
| `thunderdome` | 4.0 µs | 938.2 µs | 9.62 ms | 574.9 ns | 56.6 µs | 1.70 ms |

#### `iterate`

| subject | 64B/1000 | 64B/100000 | 64B/1000000 | u64/1000 | u64/100000 | u64/1000000 |
|---|---|---|---|---|---|---|
| `engine_handle` | 271.9 ns | 48.5 µs | 593.0 µs | 267.5 ns | 27.0 µs | 272.2 µs |
| `engine_handle_values_slice` | 129.9 ns | 48.0 µs | 564.4 µs | 53.1 ns | 7.1 µs | 73.0 µs |
| `slotmap` | 265.8 ns | 54.8 µs | 677.3 µs | 262.7 ns | 25.1 µs | 256.1 µs |
| `slotmap_dense` | 272.3 ns | 49.4 µs | 606.6 µs | 267.2 ns | 27.0 µs | 271.1 µs |
| `thunderdome` | 353.8 ns | 55.7 µs | 712.2 µs | 353.9 ns | 33.0 µs | 335.8 µs |

#### `churn_then_get`

| subject | 64B/1000 | 64B/100000 | 64B/1000000 | u64/1000 | u64/100000 | u64/1000000 |
|---|---|---|---|---|---|---|
| `engine_handle` | 507.4 ns | 116.4 µs | 4.44 ms | 504.5 ns | 109.2 µs | 1.68 ms |
| `slotmap` | 402.0 ns | 125.0 µs | 5.00 ms | 365.2 ns | 69.7 µs | 967.5 µs |
| `slotmap_dense` | 448.3 ns | 117.4 µs | 3.83 ms | 425.7 ns | 108.6 µs | 1.49 ms |
| `thunderdome` | 394.7 ns | 125.1 µs | 5.00 ms | 365.0 ns | 70.2 µs | 941.0 µs |

#### `iterate_after_churn`

| subject | 64B/1000 | 64B/100000 | 64B/1000000 | u64/1000 | u64/100000 | u64/1000000 |
|---|---|---|---|---|---|---|
| `engine_handle` | 138.2 ns | 23.9 µs | 249.2 µs | 139.0 ns | 13.5 µs | 135.6 µs |
| `engine_handle_values_slice` | 67.8 ns | 23.8 µs | 249.5 µs | 25.1 ns | 3.6 µs | 35.9 µs |
| `slotmap` | 424.5 ns | 171.9 µs | 2.28 ms | 397.5 ns | 150.9 µs | 2.21 ms |
| `slotmap_dense` | 139.3 ns | 24.1 µs | 252.8 µs | 137.4 ns | 13.5 µs | 135.5 µs |
| `thunderdome` | 456.8 ns | 178.2 µs | 2.33 ms | 432.5 ns | 158.0 µs | 2.25 ms |

### Handle pool: realistic scenarios

Frame update mutates every element; steady churn despawns and respawns 2% per frame then updates;
despawn removes everything in shuffled order; resolve-target looks up each entity and one random
target; stale lookups resolve handles of which 10% or 50% were already removed.

#### `scenario/frame_update`

| subject | 16B/100 | 16B/1000 | 16B/10000 | 16B/100000 | 64B/100 | 64B/1000 | 64B/10000 | 64B/100000 |
|---|---|---|---|---|---|---|---|---|
| `engine_handle` | 13.3 ns | 127.7 ns | 2.1 µs | 21.8 µs | 19.6 ns | 194.5 ns | 5.7 µs | 57.7 µs |
| `engine_handle_values_slice` | 13.2 ns | 127.2 ns | 2.1 µs | 21.8 µs | 19.6 ns | 194.6 ns | 5.7 µs | 57.4 µs |
| `slotmap` | 25.6 ns | 264.7 ns | 3.5 µs | 35.2 µs | 25.6 ns | 264.0 ns | 6.6 µs | 66.5 µs |
| `slotmap_dense` | 13.4 ns | 127.8 ns | 2.1 µs | 21.8 µs | 19.8 ns | 194.9 ns | 5.7 µs | 57.7 µs |
| `thunderdome` | 40.7 ns | 372.3 ns | 3.6 µs | 35.5 µs | 36.2 ns | 350.3 ns | 6.8 µs | 68.1 µs |

#### `scenario/steady_churn_frame`

| subject | 16B/100 | 16B/1000 | 16B/10000 | 16B/100000 | 64B/100 | 64B/1000 | 64B/10000 | 64B/100000 |
|---|---|---|---|---|---|---|---|---|
| `engine_handle` | 27.3 ns | 277.7 ns | 4.3 µs | 52.8 µs | 35.0 ns | 424.4 ns | 8.8 µs | 98.3 µs |
| `slotmap` | 39.5 ns | 365.5 ns | 4.9 µs | 55.7 µs | 35.3 ns | 388.8 ns | 8.4 µs | 99.1 µs |
| `slotmap_dense` | 27.8 ns | 249.4 ns | 3.9 µs | 48.9 µs | 38.6 ns | 400.2 ns | 8.4 µs | 94.6 µs |
| `thunderdome` | 42.1 ns | 445.4 ns | 5.4 µs | 58.2 µs | 44.6 ns | 501.4 ns | 8.9 µs | 104.2 µs |

#### `scenario/despawn_random_order`

| subject | 16B/100 | 16B/1000 | 16B/10000 | 16B/100000 | 64B/100 | 64B/1000 | 64B/10000 | 64B/100000 |
|---|---|---|---|---|---|---|---|---|
| `engine_handle` | 288.1 ns | 2.2 µs | 25.9 µs | 374.3 µs | 559.9 ns | 5.2 µs | 55.9 µs | 1.24 ms |
| `slotmap` | 112.9 ns | 997.8 ns | 12.2 µs | 138.3 µs | 300.0 ns | 2.8 µs | 30.0 µs | 1.04 ms |
| `slotmap_dense` | 281.2 ns | 2.0 µs | 23.9 µs | 322.3 µs | 538.9 ns | 5.0 µs | 53.3 µs | 1.20 ms |
| `thunderdome` | 147.9 ns | 1.3 µs | 15.0 µs | 161.2 µs | 426.4 ns | 4.2 µs | 42.8 µs | 1.19 ms |

#### `scenario/resolve_target`

| subject | 16B/100 | 16B/1000 | 16B/10000 | 16B/100000 | 64B/100 | 64B/1000 | 64B/10000 | 64B/100000 |
|---|---|---|---|---|---|---|---|---|
| `engine_handle` | 103.3 ns | 970.7 ns | 17.1 µs | 207.5 µs | 94.0 ns | 970.3 ns | 19.5 µs | 234.7 µs |
| `slotmap` | 69.7 ns | 698.2 ns | 10.6 µs | 143.0 µs | 69.9 ns | 698.2 ns | 16.6 µs | 203.9 µs |
| `slotmap_dense` | 88.7 ns | 867.6 ns | 14.2 µs | 203.6 µs | 87.2 ns | 870.4 ns | 17.0 µs | 230.2 µs |
| `thunderdome` | 73.4 ns | 698.4 ns | 10.5 µs | 142.5 µs | 73.7 ns | 697.8 ns | 16.7 µs | 203.7 µs |

#### `scenario/get_stale_10pct`

| subject | 16B/100 | 16B/1000 | 16B/10000 | 16B/100000 | 64B/100 | 64B/1000 | 64B/10000 | 64B/100000 |
|---|---|---|---|---|---|---|---|---|
| `engine_handle` | 56.4 ns | 581.1 ns | 7.5 µs | 112.7 µs | 56.4 ns | 583.6 ns | 7.8 µs | 119.1 µs |
| `slotmap` | 38.8 ns | 392.1 ns | 6.2 µs | 86.3 µs | 39.1 ns | 393.8 ns | 10.0 µs | 126.6 µs |
| `slotmap_dense` | 43.6 ns | 439.8 ns | 7.0 µs | 109.8 µs | 43.8 ns | 440.9 ns | 7.6 µs | 115.7 µs |
| `thunderdome` | 38.7 ns | 393.3 ns | 6.2 µs | 89.5 µs | 39.0 ns | 394.9 ns | 10.0 µs | 125.5 µs |

#### `scenario/get_stale_50pct`

| subject | 16B/100 | 16B/1000 | 16B/10000 | 16B/100000 | 64B/100 | 64B/1000 | 64B/10000 | 64B/100000 |
|---|---|---|---|---|---|---|---|---|
| `engine_handle` | 45.1 ns | 621.8 ns | 7.5 µs | 288.7 µs | 44.4 ns | 635.2 ns | 7.5 µs | 288.3 µs |
| `slotmap` | 37.8 ns | 519.3 ns | 7.2 µs | 294.9 µs | 37.7 ns | 553.7 ns | 7.9 µs | 308.2 µs |
| `slotmap_dense` | 39.9 ns | 491.9 ns | 6.9 µs | 271.0 µs | 39.8 ns | 521.2 ns | 6.7 µs | 273.1 µs |
| `thunderdome` | 37.7 ns | 516.7 ns | 7.0 µs | 289.0 µs | 37.8 ns | 546.6 ns | 8.0 µs | 313.7 µs |

### Handle allocator

#### `allocator/allocate`

| subject | 1000 | 100000 | 1000000 |
|---|---|---|---|
| `engine_handle` | 2.6 µs | 228.2 µs | 3.28 ms |
| `slotmap` | 1.1 µs | 73.7 µs | 1.30 ms |

#### `allocator/free_realloc_churn`

| subject | 1000 | 100000 | 1000000 |
|---|---|---|---|
| `engine_handle` | 2.4 µs | 530.9 µs | 8.88 ms |
| `slotmap` | 1.0 µs | 340.5 µs | 5.17 ms |

#### `allocator/is_alive`

| subject | 1000 | 100000 | 1000000 |
|---|---|---|---|
| `engine_handle` | 414.8 ns | 75.9 µs | 900.0 µs |
| `slotmap` | 315.9 ns | 57.4 µs | 676.4 µs |

### PRNG (1,000,000 draws per iteration; shuffle is 10,000 elements)

#### `prng/next_u64`

| subject | — |
|---|---|
| `engine_random` | 682.9 µs |
| `fastrand` | 232.2 µs |
| `rand_small_rng` | 678.3 µs |
| `rand_xoshiro` | 681.6 µs |

#### `prng/range_u32_0_1000`

| subject | — |
|---|---|
| `engine_random` | 780.2 µs |
| `fastrand` | 483.0 µs |
| `rand_small_rng` | 2.31 ms |
| `rand_xoshiro` | 2.30 ms |

#### `prng/range_f32_0_1`

| subject | — |
|---|---|
| `engine_random` | 743.2 µs |
| `fastrand` | 628.5 µs |
| `rand_small_rng` | 684.8 µs |
| `rand_xoshiro` | 684.7 µs |

#### `prng/shuffle_10k`

| subject | — |
|---|---|
| `engine_random` | 8.0 µs |
| `fastrand` | 5.1 µs |
| `rand_small_rng` | 9.8 µs |
| `rand_xoshiro` | 9.7 µs |

### Time and the sample (per iteration of 1,000,000 frames; full default simulation)

#### `fixed_timestep`

| subject | — |
|---|---|
| `accumulate_16ms_at_60hz` | 11.24 ms |

#### `fixed_step_sim`

| subject | — |
|---|---|
| `default_config` | 6.86 ms |
