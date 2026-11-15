# Benchmarks

Status: Accepted (2026-11-12). Results re-recorded 2026-11-14 (M3.13), on one machine. The data
sections below (marked) are generated from a saved criterion baseline by `cargo xtask bench-doc`; the
analysis is written by hand and every figure in it was checked against that data. Read the caveats
before drawing conclusions.

## Summary

- **Against `slotmap::DenseSlotMap`** (the same dense design, and the fair comparison), `GenIdMap`
  is level on iteration (1.00), insert (0.96), sequential `get` (1.03), resolving a target (1.01), the
  per-frame update loop (1.00) and the steady churn frame (1.05). It is 6-15% slower on random `get`
  (1.09), both stale-lookup scenarios (1.15 at 10% stale, 1.08 at 50%), remove-then-get (1.06),
  despawning in random order (1.07) and removing everything (1.10). Of 90 comparable cases, 52 are
  within 5%, 34 are more than 5% slower, and 4 are more than 5% faster. The cause of the remaining gap
  has not been investigated.
- **Against the sparse containers** (`slotmap::SlotMap`, `thunderdome::Arena`), point operations
  (`insert`, `get`, `remove`) are typically 1.3-1.9x slower by median ratio, with individual cases up
  to 2.9x (despawning in random order at one size). That is the expected price of dense storage: a
  second array access per lookup, plus a parallel array of IDs. In exchange, the loops a simulation
  actually runs are faster or equal: the mutating per-frame update is 1.15-2.1x faster than `slotmap`
  (median ratio 0.68), and iterating after half the entries were removed is about 8x faster (median
  ratio 0.13; 0.06-0.34 across sizes), because a sparse container must step over the holes. In the
  mixed spawn/despawn/update frame the two are at parity (median ratio 0.95).
- **`GenIdAllocator`** allocates 4-12% faster than `slotmap` with `()` values (0.88), but trails it on
  free/realloc churn (1.2-1.9x) and `is_alive` (1.2-1.3x). `allocate_many` is 1.3-2x faster than a loop
  of `allocate` when it creates new slots; when it reuses freed slots the gain is 1.9x at 1k, 1.3x at
  100k and disappears at 1M.
- **`Prng`** ties with `rand_xoshiro` and `rand`'s `SmallRng` on raw draws (same algorithm), is about
  3x faster than both on integer ranges, 26% faster on shuffling, and 6% slower on `f32` ranges.
  `fastrand` (a different, lighter generator) is faster on every measure.
- **`IndexMap`** is level with `indexmap` using the same hasher (`foldhash`) on `get`, insert and
  iteration (median ratios 0.98-1.00), 1.5-4x faster than `indexmap` with its default SipHash on
  lookups, and several times faster than `BTreeMap`.
- Per spec section 8 of the M0+M1 spec, losses are recorded and analyzed here and are not fixed:
  optimization waits for real workloads in M2.

## Methodology

<!-- bench:env -->
| | |
|---|---|
| Date | 2026-11-15T02:10:18Z |
| Machine | Apple M4 Pro (Mac16,8) |
| Memory (GB) | 24 |
| Cores | 8 performance + 4 efficiency |
| OS | macOS 26.6.2 (25G83) |
| Toolchain | rustc 1.100.0 (0ef1bc386 2026-11-09) |
| Power | AC Power for the whole run (21:10 to 22:10 local); the power log shows no source change and no sleep |
| System load at start | { 4.01 3.84 3.69 } (waited 300s; the load never fell below 2.5, and Spotify was closed about 16 minutes into the run) |
| Source commit | 8b856f2 |
<!-- /bench:env -->

One run makes up the data: a full run of the suite, saved as the baseline `m3.13`.

- **Commands:** `cargo xtask bench --save m3.13` (that is `cargo bench --workspace --bench '*' --
  --save-baseline m3.13`), then `cargo xtask bench-doc --baseline m3.13 --env docs/data/bench-env.txt`.
- **Caches:** L1D 128 KiB and L2 16 MiB (performance cores), L2 4 MiB (efficiency cores), 128-byte cache
  lines, from `sysctl`.
- **Profile:** `[profile.bench]` is `lto = "thin"`, `codegen-units = 1`, `debug = true` (symbols for
  profiling). Dependencies are optimized.
- **Harness:** Criterion 0.8 with its defaults (3 s warm-up, 5 s measurement) for the ID, PRNG,
  collection and time benchmarks. The realistic-scenario groups use a shorter 1 s warm-up and 3 s measurement.
  The 1M-element ID cases use 10 samples. All numbers are **medians**.
- **Subjects:** the subjects are named after the types they measure. `engine::GenIdMap` and
  `engine::GenIdAllocator` are compared with `slotmap` 1.1 (`SlotMap` and `DenseSlotMap`) and
  `thunderdome` 0.6. `engine::Prng` is compared with `rand_xoshiro` 0.8 (the same algorithm),
  `rand` 0.10's `SmallRng`, and `fastrand` 2.5. `engine::IndexMap` is compared with `indexmap` (with
  `foldhash` and with SipHash) and `BTreeMap`. Competitor crates are dev-dependencies only.
- **Values:** the classic ID groups use `u64` and a 64-byte struct (`64B`) at 1k, 100k, and 1M
  entries. The scenario groups use 16-byte and 64-byte values at 100 to 100k entries, because most
  simulations live in the cache-resident range. Random orders come from a fixed-seed `Prng`, so
  every library sees identical key orders.
- **Run conditions.** The run took one hour (21:10 to 22:10 local, 2026-11-14), on AC power, under
  `caffeinate`. The power log for that window shows no power-source change and no sleep (its one entry
  is a "Using AC" assertion at 21:59). The 1-minute load never fell below 2.5 while the script waited
  (300 s), and was 4.0 at the start: a Spotify renderer process was using a lot of CPU, and it was
  closed about 16 minutes into the run. The groups that ran before then (the first 105 or so of the
  476 benchmarks: `allocator/*`, `churn_then_get`, `fixed_step_sim`, `fixed_timestep` and the start of
  `get_random`) ran alongside it. Background applications, at least a VPN client, were running
  throughout. I also ran one `cargo bench --list` during the run, by mistake, to count benchmarks; it
  should have been a no-op build. Treat these as careful but not laboratory-grade numbers.
- **Noise.** This baseline is a single run, so it has no noise floor of its own, and I did not repeat it.
  Treat differences of a few percent as ties, and treat any single case that looks out of line with its
  neighbours with suspicion. The conclusions rest on ratios measured within this run, not on
  comparisons with the earlier M1 run, whose subjects had different names and which ran on an older
  toolchain (rustc 1.97).
- **Scope:** wall-clock numbers come from aarch64 macOS and are not comparable with x86_64 or with
  instruction counts. Instruction-count regression gating starts in M2 (spec section 4.12).

## How to read the ratios

The table gives our subject's time (`engine::GenIdMap`, `engine::Prng` and so on) divided by the competitor's, as a median
with the range across sizes and value types in parentheses, and the number of cases. **Below 1.00
means ours is faster; above 1.00 means it is slower.** The tallies under it count cases more than 5%
slower, within 5%, and more than 5% faster.

<!-- bench:ratios -->
| group | ours | competitor | median ratio (range) | cases |
|---|---|---|---|---|
| `allocator/allocate` | `engine::GenIdAllocator` | `slotmap` | 0.88 (0.88–0.96) | 3 |
| `allocator/free_realloc_churn` | `engine::GenIdAllocator` | `slotmap` | 1.27 (1.19–1.90) | 3 |
| `allocator/is_alive` | `engine::GenIdAllocator` | `slotmap` | 1.29 (1.19–1.33) | 3 |
| `churn_then_get` | `engine::GenIdMap` | `slotmap` | 1.33 (0.80–1.55) | 6 |
| `churn_then_get` | `engine::GenIdMap` | `slotmap_dense` | 1.06 (1.00–1.18) | 6 |
| `churn_then_get` | `engine::GenIdMap` | `thunderdome` | 1.33 (0.80–1.64) | 6 |
| `get_random` | `engine::GenIdMap` | `slotmap` | 1.41 (0.95–2.05) | 6 |
| `get_random` | `engine::GenIdMap` | `slotmap_dense` | 1.09 (0.99–1.30) | 6 |
| `get_random` | `engine::GenIdMap` | `thunderdome` | 1.18 (0.91–2.07) | 6 |
| `get_sequential` | `engine::GenIdMap` | `slotmap` | 1.32 (1.14–1.47) | 6 |
| `get_sequential` | `engine::GenIdMap` | `slotmap_dense` | 1.03 (1.00–1.19) | 6 |
| `get_sequential` | `engine::GenIdMap` | `thunderdome` | 1.32 (1.12–1.46) | 6 |
| `insert` | `engine::GenIdMap` | `slotmap` | 1.65 (0.88–2.01) | 6 |
| `insert` | `engine::GenIdMap` | `slotmap_dense` | 0.96 (0.87–1.08) | 6 |
| `insert` | `engine::GenIdMap` | `thunderdome` | 1.76 (1.27–2.17) | 6 |
| `iterate` | `engine::GenIdMap` | `slotmap` | 1.02 (0.88–1.08) | 6 |
| `iterate` | `engine::GenIdMap` | `slotmap_dense` | 1.00 (0.98–1.02) | 6 |
| `iterate` | `engine::GenIdMap` | `thunderdome` | 0.81 (0.76–0.86) | 6 |
| `iterate` | `engine::GenIdMap_values_slice` | `slotmap` | 0.38 (0.20–0.87) | 6 |
| `iterate` | `engine::GenIdMap_values_slice` | `slotmap_dense` | 0.37 (0.20–0.97) | 6 |
| `iterate` | `engine::GenIdMap_values_slice` | `thunderdome` | 0.29 (0.15–0.85) | 6 |
| `iterate_after_churn` | `engine::GenIdMap` | `slotmap` | 0.13 (0.06–0.34) | 6 |
| `iterate_after_churn` | `engine::GenIdMap` | `slotmap_dense` | 1.00 (0.99–1.00) | 6 |
| `iterate_after_churn` | `engine::GenIdMap` | `thunderdome` | 0.12 (0.06–0.24) | 6 |
| `iterate_after_churn` | `engine::GenIdMap_values_slice` | `slotmap` | 0.09 (0.02–0.17) | 6 |
| `iterate_after_churn` | `engine::GenIdMap_values_slice` | `slotmap_dense` | 0.38 (0.18–0.99) | 6 |
| `iterate_after_churn` | `engine::GenIdMap_values_slice` | `thunderdome` | 0.07 (0.02–0.14) | 6 |
| `map_get_hit` | `engine::IndexMap` | `btreemap` | 0.14 (0.03–0.30) | 3 |
| `map_get_hit` | `engine::IndexMap` | `indexmap_foldhash` | 0.98 (0.98–1.00) | 3 |
| `map_get_hit` | `engine::IndexMap` | `indexmap_sip` | 0.27 (0.26–0.35) | 3 |
| `map_get_miss` | `engine::IndexMap` | `btreemap` | 0.11 (0.10–0.18) | 3 |
| `map_get_miss` | `engine::IndexMap` | `indexmap_foldhash` | 1.00 (0.94–1.03) | 3 |
| `map_get_miss` | `engine::IndexMap` | `indexmap_sip` | 0.27 (0.26–0.33) | 3 |
| `map_insert` | `engine::IndexMap` | `btreemap` | 0.21 (0.06–0.29) | 3 |
| `map_insert` | `engine::IndexMap` | `indexmap_foldhash` | 1.00 (0.81–1.01) | 3 |
| `map_insert` | `engine::IndexMap` | `indexmap_sip` | 0.66 (0.61–0.67) | 3 |
| `map_iter` | `engine::IndexMap` | `btreemap` | 0.14 (0.11–0.14) | 3 |
| `map_iter` | `engine::IndexMap` | `indexmap_foldhash` | 1.00 (1.00–1.01) | 3 |
| `map_iter` | `engine::IndexMap` | `indexmap_sip` | 1.00 (1.00–1.01) | 3 |
| `prng/next_u64` | `engine::Prng` | `fastrand` | 2.90 (2.90–2.90) | 1 |
| `prng/next_u64` | `engine::Prng` | `rand_small_rng` | 1.00 (1.00–1.00) | 1 |
| `prng/next_u64` | `engine::Prng` | `rand_xoshiro` | 1.00 (1.00–1.00) | 1 |
| `prng/range_f32_0_1` | `engine::Prng` | `fastrand` | 1.16 (1.16–1.16) | 1 |
| `prng/range_f32_0_1` | `engine::Prng` | `rand_small_rng` | 1.06 (1.06–1.06) | 1 |
| `prng/range_f32_0_1` | `engine::Prng` | `rand_xoshiro` | 1.06 (1.06–1.06) | 1 |
| `prng/range_u32_0_1000` | `engine::Prng` | `fastrand` | 1.61 (1.61–1.61) | 1 |
| `prng/range_u32_0_1000` | `engine::Prng` | `rand_small_rng` | 0.34 (0.34–0.34) | 1 |
| `prng/range_u32_0_1000` | `engine::Prng` | `rand_xoshiro` | 0.34 (0.34–0.34) | 1 |
| `prng/shuffle_10k` | `engine::Prng` | `fastrand` | 1.53 (1.53–1.53) | 1 |
| `prng/shuffle_10k` | `engine::Prng` | `rand_small_rng` | 0.74 (0.74–0.74) | 1 |
| `prng/shuffle_10k` | `engine::Prng` | `rand_xoshiro` | 0.74 (0.74–0.74) | 1 |
| `remove_all` | `engine::GenIdMap` | `slotmap` | 1.91 (1.14–2.57) | 6 |
| `remove_all` | `engine::GenIdMap` | `slotmap_dense` | 1.10 (0.98–1.37) | 6 |
| `remove_all` | `engine::GenIdMap` | `thunderdome` | 1.55 (0.97–2.54) | 6 |
| `scenario/despawn_random_order` | `engine::GenIdMap` | `slotmap` | 2.07 (1.28–2.86) | 8 |
| `scenario/despawn_random_order` | `engine::GenIdMap` | `slotmap_dense` | 1.07 (1.04–1.21) | 8 |
| `scenario/despawn_random_order` | `engine::GenIdMap` | `thunderdome` | 1.52 (1.10–2.45) | 8 |
| `scenario/frame_update` | `engine::GenIdMap` | `slotmap` | 0.68 (0.48–0.87) | 8 |
| `scenario/frame_update` | `engine::GenIdMap` | `slotmap_dense` | 1.00 (0.99–1.00) | 8 |
| `scenario/frame_update` | `engine::GenIdMap` | `thunderdome` | 0.41 (0.25–0.84) | 8 |
| `scenario/frame_update` | `engine::GenIdMap_values_slice` | `slotmap` | 0.68 (0.48–0.88) | 8 |
| `scenario/frame_update` | `engine::GenIdMap_values_slice` | `slotmap_dense` | 1.00 (0.98–1.03) | 8 |
| `scenario/frame_update` | `engine::GenIdMap_values_slice` | `thunderdome` | 0.42 (0.25–0.85) | 8 |
| `scenario/get_stale_10pct` | `engine::GenIdMap` | `slotmap` | 1.33 (0.79–1.49) | 8 |
| `scenario/get_stale_10pct` | `engine::GenIdMap` | `slotmap_dense` | 1.15 (1.02–1.32) | 8 |
| `scenario/get_stale_10pct` | `engine::GenIdMap` | `thunderdome` | 1.34 (0.78–1.49) | 8 |
| `scenario/get_stale_50pct` | `engine::GenIdMap` | `slotmap` | 1.09 (0.88–1.17) | 8 |
| `scenario/get_stale_50pct` | `engine::GenIdMap` | `slotmap_dense` | 1.08 (1.01–1.25) | 8 |
| `scenario/get_stale_50pct` | `engine::GenIdMap` | `thunderdome` | 1.02 (0.94–1.23) | 8 |
| `scenario/resolve_target` | `engine::GenIdMap` | `slotmap` | 1.27 (0.98–1.46) | 8 |
| `scenario/resolve_target` | `engine::GenIdMap` | `slotmap_dense` | 1.01 (0.76–1.08) | 8 |
| `scenario/resolve_target` | `engine::GenIdMap` | `thunderdome` | 1.24 (0.99–1.46) | 8 |
| `scenario/steady_churn_frame` | `engine::GenIdMap` | `slotmap` | 0.95 (0.76–1.09) | 8 |
| `scenario/steady_churn_frame` | `engine::GenIdMap` | `slotmap_dense` | 1.05 (0.90–1.13) | 8 |
| `scenario/steady_churn_frame` | `engine::GenIdMap` | `thunderdome` | 0.66 (0.44–0.98) | 8 |

Across all comparable cases, `engine::GenIdMap` against `slotmap`: 54 of 90 more than 5% slower, 8 within 5%, 28 more than 5% faster; `slotmap_dense`: 34 of 90 more than 5% slower, 52 within 5%, 4 more than 5% faster; `thunderdome`: 48 of 90 more than 5% slower, 8 within 5%, 34 more than 5% faster.
<!-- /bench:ratios -->

## Interpretation

**The dense design does what it was chosen to do.** Iterating the map is a straight walk over
contiguous memory, so the per-frame update and iteration after churn are where it wins, and the
`values()` slice is the fastest way to process every value (36 µs versus 2.27 ms to iterate 1M `u64`s
after removing half of them, against `slotmap`). The mutating `frame_update` benchmark is the closest
proxy here for how a simulation uses the container, and `GenIdMap` leads the sparse containers on it
at every size.

**The price is on point operations.** A lookup reads the slot, then the dense position, so `get`,
`remove`, and resolving an ID to another entity cost more than in a sparse container that stores the
value in the slot. At 1M `u64` values, random `get` is 2.1x `slotmap`'s time. In realistic mixes this
largely washes out: the steady-state churn frame (2% despawned and respawned, then everything updated)
is at parity with `slotmap` (median ratio 0.95).

**Against the design it most resembles, we are close.** `DenseSlotMap` is the fair comparison. On
iteration, insert, sequential `get`, resolving a target, the frame update and the steady churn frame
the medians are within about 5% of equal or better (individual sequential-`get` cases range from 1.00
to 1.19); random `get`, stale lookups and the removal and churn paths are 6-15% slower. I have not
profiled the remaining gap, so I do not have a reason to offer, only the measurement.

**The allocator trades iteration for speed.** Allocating an ID with `GenIdAllocator` costs about what
`slotmap<()>` does (a little less). It cannot iterate its live IDs; a `GenIdMap<()>` does that, at the
cost of a dense list. Free/realloc churn and `is_alive` are 1.2-1.9x `slotmap`'s time; I have not
profiled why. `allocate_many` reserves all the room it needs up front, which is where its 1.3-2x gain
on fresh slots comes from; reusing freed slots means following the free list one slot at a time, so
the gain fades as the free list outgrows the cache (it is gone at 1M).

**PRNG.** On `next_u64`, `engine::Prng`, `rand_xoshiro`, and `SmallRng` are level, as expected for one
algorithm (xoshiro256++). Bounded integers are about 3x faster than `rand`'s, shuffling 10,000 elements
is 26% faster, and `f32` ranges are about 6% slower than `rand`'s (a candidate for tuning); I did not
investigate either difference. `fastrand` is faster on every measure (about 2.9x on `next_u64`). It uses
a different generator with a smaller state, and this comparison says nothing about statistical quality,
only speed.

**Collections.** `engine::IndexMap` uses the same backend and hasher as `indexmap` with `foldhash`, and
the numbers agree: the median ratios are 0.98-1.00 on `get`, insert and iteration. Against `indexmap`
with SipHash it is about 3.7x faster on lookups and 1.5x faster on insert, and against `BTreeMap` it is
several times faster on every operation. Those are comparisons of hashers and structures, not of our
code.

**Time and the sample.** One `FixedTimestep::accumulate` call with a 16 ms frame at 60 Hz costs about
11 ns; I did not profile it (integer division is the likely cost), and it is not a bottleneck today. The
`fixed_step_sim` default run (10,000 particles for 600 ticks, about 6 million particle-steps, including
`retain` and respawning) takes about 6.9 ms, roughly 1.1 ns per particle-step, with zero heap allocations
in steady state.

## Limitations

- One machine, one run, and medians only; the tables show no confidence intervals (the HTML report from
  `cargo xtask bench-report` does).
- The classic groups use a freshly built container; the scenario groups use more realistic mixes
  but are still synthetic. The real test is the M4 ECS.
- `GenIdMap` and its competitors were each driven through a thin common trait in the benchmark
  harness; differences in what each library's iteration API allows (for example `slotmap`
  exposing no values slice) are part of the comparison, not hidden.
- The reasons for the remaining gaps (removal paths versus `DenseSlotMap`, the allocator's churn and
  `is_alive` cost, random `get`) are hypotheses at best; none were profiled.
- The groups that ran in the first 16 minutes ran alongside a busy background process (see Run
  conditions), and there is no second run to check them against.

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
| `engine::GenIdAllocator` | 1.02 µs | 67.2 µs | 1.09 ms |
| `slotmap` | 1.16 µs | 76.3 µs | 1.14 ms |

#### `allocator/batch_fresh`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `batch` | 726 ns | 44.9 µs | 1.04 ms |
| `loop` | 1.25 µs | 91.1 µs | 1.32 ms |

#### `allocator/batch_recycled`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `batch` | 1.55 µs | 157 µs | 1.67 ms |
| `loop` | 2.92 µs | 207 µs | 1.67 ms |

#### `allocator/free_realloc_churn`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine::GenIdAllocator` | 2.01 µs | 442 µs | 5.85 ms |
| `slotmap` | 1.06 µs | 348 µs | 4.93 ms |

#### `allocator/is_alive`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine::GenIdAllocator` | 376 ns | 74.5 µs | 900 µs |
| `slotmap` | 316 ns | 57.5 µs | 677 µs |

#### `churn_then_get`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine::GenIdMap · 64B` | 508 ns | 116 µs | 4.02 ms |
| `engine::GenIdMap · u64` | 505 ns | 109 µs | 1.56 ms |
| `slotmap · 64B` | 398 ns | 125 µs | 5.02 ms |
| `slotmap · u64` | 366 ns | 70.1 µs | 1.01 ms |
| `slotmap_dense · 64B` | 449 ns | 116 µs | 3.84 ms |
| `slotmap_dense · u64` | 427 ns | 109 µs | 1.46 ms |
| `thunderdome · 64B` | 396 ns | 126 µs | 5.02 ms |
| `thunderdome · u64` | 366 ns | 70.1 µs | 956 µs |

#### `fixed_step_sim`

| subject | — |
|---|---|
| `default_config` | 6.86 ms |

#### `fixed_timestep`

| subject | — |
|---|---|
| `accumulate_16ms_at_60hz` | 11.4 ms |

#### `get_random`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine::GenIdMap · 64B` | 507 ns | 119 µs | 5.05 ms |
| `engine::GenIdMap · u64` | 514 ns | 116 µs | 2.01 ms |
| `slotmap · 64B` | 373 ns | 126 µs | 5.01 ms |
| `slotmap · u64` | 353 ns | 71.7 µs | 978 µs |
| `slotmap_dense · 64B` | 435 ns | 119 µs | 5.11 ms |
| `slotmap_dense · u64` | 425 ns | 113 µs | 1.54 ms |
| `thunderdome · 64B` | 372 ns | 126 µs | 5.01 ms |
| `thunderdome · u64` | 567 ns | 72.9 µs | 971 µs |

#### `get_sequential`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine::GenIdMap · 64B` | 513 ns | 84.4 µs | 926 µs |
| `engine::GenIdMap · u64` | 510 ns | 50.6 µs | 538 µs |
| `slotmap · 64B` | 376 ns | 67.2 µs | 813 µs |
| `slotmap · u64` | 348 ns | 38.0 µs | 412 µs |
| `slotmap_dense · 64B` | 439 ns | 82.5 µs | 901 µs |
| `slotmap_dense · u64` | 431 ns | 49.9 µs | 536 µs |
| `thunderdome · 64B` | 372 ns | 67.1 µs | 827 µs |
| `thunderdome · u64` | 349 ns | 37.8 µs | 414 µs |

#### `insert`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine::GenIdMap · 64B` | 4.00 µs | 738 µs | 8.78 ms |
| `engine::GenIdMap · u64` | 2.34 µs | 168 µs | 3.15 ms |
| `slotmap · 64B` | 2.47 µs | 838 µs | 6.87 ms |
| `slotmap · u64` | 1.30 µs | 83.6 µs | 1.88 ms |
| `slotmap_dense · 64B` | 4.13 µs | 719 µs | 8.15 ms |
| `slotmap_dense · u64` | 2.45 µs | 182 µs | 3.62 ms |
| `thunderdome · 64B` | 2.30 µs | 582 µs | 6.58 ms |
| `thunderdome · u64` | 1.28 µs | 77.7 µs | 1.77 ms |

#### `iterate`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine::GenIdMap · 64B` | 268 ns | 48.3 µs | 589 µs |
| `engine::GenIdMap · u64` | 266 ns | 27.0 µs | 270 µs |
| `engine::GenIdMap_values_slice · 64B` | 128 ns | 47.6 µs | 570 µs |
| `engine::GenIdMap_values_slice · u64` | 52.7 ns | 7.04 µs | 70.7 µs |
| `slotmap · 64B` | 267 ns | 54.6 µs | 673 µs |
| `slotmap · u64` | 259 ns | 25.0 µs | 257 µs |
| `slotmap_dense · 64B` | 273 ns | 49.0 µs | 596 µs |
| `slotmap_dense · u64` | 264 ns | 26.6 µs | 267 µs |
| `thunderdome · 64B` | 353 ns | 55.9 µs | 698 µs |
| `thunderdome · u64` | 350 ns | 33.0 µs | 337 µs |

#### `iterate_after_churn`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine::GenIdMap · 64B` | 138 ns | 24.1 µs | 254 µs |
| `engine::GenIdMap · u64` | 139 ns | 13.5 µs | 135 µs |
| `engine::GenIdMap_values_slice · 64B` | 68.9 ns | 24.1 µs | 250 µs |
| `engine::GenIdMap_values_slice · u64` | 25.5 ns | 3.58 µs | 35.9 µs |
| `slotmap · 64B` | 417 ns | 167 µs | 2.31 ms |
| `slotmap · u64` | 406 ns | 154 µs | 2.27 ms |
| `slotmap_dense · 64B` | 140 ns | 24.2 µs | 253 µs |
| `slotmap_dense · u64` | 139 ns | 13.5 µs | 136 µs |
| `thunderdome · 64B` | 612 ns | 166 µs | 2.44 ms |
| `thunderdome · u64` | 587 ns | 155 µs | 2.34 ms |

#### `map_get_hit`

| subject | 64 | 1.024K | 65.536K |
|---|---|---|---|
| `btreemap` | 263 ns | 8.48 µs | 3.99 ms |
| `engine::IndexMap` | 80.0 ns | 1.17 µs | 133 µs |
| `indexmap_foldhash` | 80.1 ns | 1.19 µs | 136 µs |
| `indexmap_sip` | 294 ns | 4.42 µs | 377 µs |

#### `map_get_miss`

| subject | 64 | 1.024K | 65.536K |
|---|---|---|---|
| `btreemap` | 314 ns | 9.27 µs | 892 µs |
| `engine::IndexMap` | 56.7 ns | 1.00 µs | 89.3 µs |
| `indexmap_foldhash` | 55.2 ns | 1.00 µs | 95.3 µs |
| `indexmap_sip` | 222 ns | 3.76 µs | 275 µs |

#### `map_insert`

| subject | 64 | 1.024K | 65.536K |
|---|---|---|---|
| `btreemap` | 763 ns | 17.1 µs | 4.34 ms |
| `engine::IndexMap` | 224 ns | 3.59 µs | 271 µs |
| `indexmap_foldhash` | 276 ns | 3.57 µs | 271 µs |
| `indexmap_sip` | 366 ns | 5.35 µs | 411 µs |

#### `map_iter`

| subject | 64 | 1.024K | 65.536K |
|---|---|---|---|
| `btreemap` | 40.7 ns | 654 ns | 113 µs |
| `engine::IndexMap` | 5.88 ns | 90.0 ns | 12.5 µs |
| `indexmap_foldhash` | 5.81 ns | 90.0 ns | 12.5 µs |
| `indexmap_sip` | 5.85 ns | 89.7 ns | 12.5 µs |

#### `prng/next_u64`

| subject | — |
|---|---|
| `engine::Prng` | 678 µs |
| `fastrand` | 233 µs |
| `rand_small_rng` | 677 µs |
| `rand_xoshiro` | 676 µs |

#### `prng/range_f32_0_1`

| subject | — |
|---|---|
| `engine::Prng` | 740 µs |
| `fastrand` | 636 µs |
| `rand_small_rng` | 696 µs |
| `rand_xoshiro` | 698 µs |

#### `prng/range_u32_0_1000`

| subject | — |
|---|---|
| `engine::Prng` | 780 µs |
| `fastrand` | 484 µs |
| `rand_small_rng` | 2.31 ms |
| `rand_xoshiro` | 2.33 ms |

#### `prng/shuffle_10k`

| subject | — |
|---|---|
| `engine::Prng` | 8.00 µs |
| `fastrand` | 5.24 µs |
| `rand_small_rng` | 10.8 µs |
| `rand_xoshiro` | 10.8 µs |

#### `remove_all`

| subject | 1K | 100K | 1M |
|---|---|---|---|
| `engine::GenIdMap · 64B` | 4.87 µs | 851 µs | 10.2 ms |
| `engine::GenIdMap · u64` | 1.47 µs | 137 µs | 2.93 ms |
| `slotmap · 64B` | 2.58 µs | 746 µs | 7.47 ms |
| `slotmap · u64` | 571 ns | 55.7 µs | 1.52 ms |
| `slotmap_dense · 64B` | 4.69 µs | 819 µs | 10.5 ms |
| `slotmap_dense · u64` | 1.07 µs | 105 µs | 2.50 ms |
| `thunderdome · 64B` | 3.98 µs | 878 µs | 8.84 ms |
| `thunderdome · u64` | 578 ns | 56.7 µs | 1.56 ms |

#### `scenario/despawn_random_order`

| subject | 100 | 1K | 10K | 100K |
|---|---|---|---|---|
| `engine::GenIdMap · 16B` | 303 ns | 2.27 µs | 27.1 µs | 394 µs |
| `engine::GenIdMap · 64B` | 569 ns | 5.20 µs | 57.4 µs | 1.24 ms |
| `slotmap · 16B` | 114 ns | 1.01 µs | 12.2 µs | 138 µs |
| `slotmap · 64B` | 299 ns | 2.82 µs | 30.0 µs | 975 µs |
| `slotmap_dense · 16B` | 286 ns | 2.02 µs | 23.9 µs | 325 µs |
| `slotmap_dense · 64B` | 543 ns | 5.00 µs | 53.3 µs | 1.17 ms |
| `thunderdome · 16B` | 157 ns | 1.34 µs | 15.0 µs | 161 µs |
| `thunderdome · 64B` | 429 ns | 4.14 µs | 43.0 µs | 1.13 ms |

#### `scenario/frame_update`

| subject | 100 | 1K | 10K | 100K |
|---|---|---|---|---|
| `engine::GenIdMap · 16B` | 13.4 ns | 128 ns | 2.07 µs | 22.0 µs |
| `engine::GenIdMap · 64B` | 20.0 ns | 197 ns | 5.85 µs | 58.9 µs |
| `engine::GenIdMap_values_slice · 16B` | 13.2 ns | 127 ns | 2.14 µs | 22.1 µs |
| `engine::GenIdMap_values_slice · 64B` | 19.8 ns | 197 ns | 5.79 µs | 59.4 µs |
| `slotmap · 16B` | 25.9 ns | 268 ns | 3.58 µs | 35.6 µs |
| `slotmap · 64B` | 25.9 ns | 267 ns | 6.72 µs | 67.6 µs |
| `slotmap_dense · 16B` | 13.5 ns | 128 ns | 2.08 µs | 22.0 µs |
| `slotmap_dense · 64B` | 19.9 ns | 197 ns | 5.89 µs | 58.7 µs |
| `thunderdome · 16B` | 53.5 ns | 494 ns | 4.94 µs | 49.2 µs |
| `thunderdome · 64B` | 53.8 ns | 495 ns | 6.95 µs | 69.9 µs |

#### `scenario/get_stale_10pct`

| subject | 100 | 1K | 10K | 100K |
|---|---|---|---|---|
| `engine::GenIdMap · 16B` | 55.7 ns | 581 ns | 7.43 µs | 113 µs |
| `engine::GenIdMap · 64B` | 54.7 ns | 584 ns | 7.93 µs | 119 µs |
| `slotmap · 16B` | 38.8 ns | 393 ns | 6.25 µs | 89.6 µs |
| `slotmap · 64B` | 38.9 ns | 393 ns | 10.1 µs | 129 µs |
| `slotmap_dense · 16B` | 43.7 ns | 439 ns | 7.01 µs | 110 µs |
| `slotmap_dense · 64B` | 43.8 ns | 444 ns | 7.63 µs | 117 µs |
| `thunderdome · 16B` | 38.8 ns | 394 ns | 6.18 µs | 88.0 µs |
| `thunderdome · 64B` | 39.1 ns | 393 ns | 10.1 µs | 126 µs |

#### `scenario/get_stale_50pct`

| subject | 100 | 1K | 10K | 100K |
|---|---|---|---|---|
| `engine::GenIdMap · 16B` | 42.7 ns | 577 ns | 7.42 µs | 279 µs |
| `engine::GenIdMap · 64B` | 43.3 ns | 670 ns | 7.99 µs | 285 µs |
| `slotmap · 16B` | 38.3 ns | 499 ns | 7.08 µs | 262 µs |
| `slotmap · 64B` | 38.2 ns | 573 ns | 7.98 µs | 322 µs |
| `slotmap_dense · 16B` | 40.0 ns | 523 ns | 7.24 µs | 275 µs |
| `slotmap_dense · 64B` | 39.8 ns | 534 ns | 7.37 µs | 281 µs |
| `thunderdome · 16B` | 37.9 ns | 567 ns | 7.19 µs | 288 µs |
| `thunderdome · 64B` | 38.0 ns | 545 ns | 8.05 µs | 302 µs |

#### `scenario/resolve_target`

| subject | 100 | 1K | 10K | 100K |
|---|---|---|---|---|
| `engine::GenIdMap · 16B` | 90.4 ns | 880 ns | 15.4 µs | 203 µs |
| `engine::GenIdMap · 64B` | 91.8 ns | 891 ns | 17.9 µs | 232 µs |
| `slotmap · 16B` | 92.5 ns | 700 ns | 10.6 µs | 143 µs |
| `slotmap · 64B` | 70.0 ns | 700 ns | 16.7 µs | 204 µs |
| `slotmap_dense · 16B` | 119 ns | 876 ns | 14.3 µs | 204 µs |
| `slotmap_dense · 64B` | 90.0 ns | 885 ns | 17.1 µs | 230 µs |
| `thunderdome · 16B` | 74.4 ns | 700 ns | 10.6 µs | 143 µs |
| `thunderdome · 64B` | 93.1 ns | 699 ns | 16.7 µs | 205 µs |

#### `scenario/steady_churn_frame`

| subject | 100 | 1K | 10K | 100K |
|---|---|---|---|---|
| `engine::GenIdMap · 16B` | 27.4 ns | 279 ns | 4.36 µs | 53.1 µs |
| `engine::GenIdMap · 64B` | 34.8 ns | 423 ns | 8.76 µs | 97.9 µs |
| `slotmap · 16B` | 33.5 ns | 368 ns | 4.97 µs | 56.0 µs |
| `slotmap · 64B` | 36.9 ns | 389 ns | 8.37 µs | 98.9 µs |
| `slotmap_dense · 16B` | 27.7 ns | 249 ns | 3.86 µs | 49.0 µs |
| `slotmap_dense · 64B` | 38.8 ns | 401 ns | 8.33 µs | 94.9 µs |
| `thunderdome · 16B` | 62.5 ns | 590 ns | 6.42 µs | 71.1 µs |
| `thunderdome · 64B` | 64.7 ns | 651 ns | 8.95 µs | 105 µs |
<!-- /bench:tables -->
