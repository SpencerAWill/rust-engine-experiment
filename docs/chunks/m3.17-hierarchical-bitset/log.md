# M3.17 log: Hierarchical bitset

- **Status:** Living (2026-11-16).

<!-- Append-only. One or two lines per entry, written when it happens, not at the end:
- <date> <tag> <text>
Tags: done, decision, surprise, dead-end, lesson? (a candidate lesson for stage 6; see docs/lessons/README.md). -->

- 2026-11-16 done Brainstorm started from #44 (sub-issue of #38). ID M3.17: M3.12 is reserved for space metrics (#41), which is not yet on the roadmap.
- 2026-11-16 surprise #44 expects bytes per element and peak bytes "using the layers from the space-metrics chunk (#41)", but M3.12 has no roadmap row or spec yet.
- 2026-11-16 surprise hibitset 0.6.4 stores every layer as `usize` (5 bits per level on 32-bit, 6 on 64-bit), so its largest index and layout depend on pointer width; a wrapper cannot fix that. hi_sparse_bitset 0.10.0 has fixed-width configs (`_64bit` caps at 262,144 indices) but 256 `unsafe` occurrences, a `wide` SIMD dependency by default, and MSRV 1.95.
- 2026-11-16 decision Own implementation on the M3.14 `u64` word layout (maintainer). `hibitset` and `hi_sparse_bitset` are benchmark baselines in `bench/compare` only: hibitset's layers are pointer-width, hi_sparse_bitset brings 256 `unsafe` occurrences, `wide` and a generic `Config` API, and the join API is what we need to own.
- 2026-11-16 decision Dense only (maintainer). Layers grow lazily to the highest index, like `BitSet`. The time-vs-space trade is measured against `hi_sparse_bitset` in `bench/compare` at several occupancies; a sparse variant of our own is a follow-up issue, decided on those numbers.
- 2026-11-16 decision Six fixed 64-wide layers, `MAX_BITS = u32::MAX as usize` like `BitSet` and `GenId`'s index (maintainer). Upper layers change only when a word goes empty to non-empty or back; `contains` reads the bottom layer only. A grow-with-height design was rejected: joins across sets of different heights get harder.
- 2026-11-16 decision Space measured with `dhat` in `bench/compare`, report only (maintainer): heap bytes at the end and peak bytes, for our set, `hibitset` and `hi_sparse_bitset`, at several occupancies and spreads. No public size API, so M3.12 (#41) still designs `HeapSize` and can add this set to its retrofit list.
- 2026-11-16 decision Joins over slices (maintainer): `Join` over required and excluded sets, ANDing required words at every layer and applying exclusions at the bottom layer only; no allocation, no public layer trait. Plus in-place union, intersection and difference, and subset and disjoint tests, as on `BitSet`. Lazy OR joins and composable views are deferred.
- 2026-11-16 decision Names `HierarchicalBitSet`, `HierarchicalIter`, `Join`, `EmptyJoinError`, `BitSetInvariantError`; `symmetric_difference_with` left out; layer 0 is not a `BitSet` field (my calls, spec §4.3, §7).
- 2026-11-16 done Grill cold read (spec-reader, registered agent): three questions, all settled as main-session calls; two path fixes (allocation test file, fuzz list in `xtask/src/fuzz.rs`).
- 2026-11-16 decision Main-session calls from the cold read: model and fuzz indices capped below 1,048,576, with top-of-range unit tests on 64-bit hosts only (layer 0 near `MAX_BITS` is about 512 MiB); `union_with` grows to `other`'s trimmed length, as `BitSet`; the space report builds by one-at-a-time insertion in seeded random order with no presizing, plus a bytes-after-`shrink_to_fit` column.
- 2026-11-16 done `cargo xtask deny` passes with `hibitset` 0.6.4 (default features off) and `hi_sparse_bitset` 0.10.0 added to `bench/compare` (checked in a throwaway worktree, not committed): its `MIT/Apache-2.0` licence string is accepted.
- 2026-11-16 decision Spec accepted by the maintainer.
- 2026-11-16 done Plan written and accepted (10 steps). Lessons (lessons-researcher, registered agent): testing-1 and specs-2 followed; bench-1, specs-1, docs-1 not applicable.
- 2026-11-16 done Step 1 (builder, registered agent): layers, single-bit operations, `check_invariants`, `BitSetInvariantError`. Main session re-ran `ci --quick` (700 passed, 6 skipped), `wasm` (engine lib 173 passed) and `no-std`.
- 2026-11-16 surprise Spec §4.8's top-of-range members could not set `top`'s bit 0 (layer 4's word index is `i >> 30`); member 0 added, spec parenthesis corrected (plan §5).
- 2026-11-16 lesson? Under wasm32, `MAX_BITS + 1` in a test is a deny-by-default overflow lint; use `checked_add`. Clippy pedantic rejects `usize as u32` and `u64 as usize` casts in layer arithmetic (builder report).
- 2026-11-16 done Step 2 (builder): `WordSource`/`WordCursor` in `descent.rs`, `HierarchicalIter`, construction, `BitSet` conversions, `hierarchical_matches_model`. Main session re-ran `ci --quick` (708 passed, 6 skipped) and `wasm` (engine lib 180 passed). `iteration_skips_empty_ranges` asserts at most 16 word reads for `{0, 1_000_000}`.
- 2026-11-16 done Step 3 (builder): `Join` through a private `JoinSource` on the shared `WordCursor`, `EmptyJoinError`, the `collections` module paragraph and "Choosing a collection" row. Main session re-ran `ci --quick` (716 passed, 6 skipped) and `wasm` (engine lib 187 passed).
- 2026-11-16 done Step 4 (builder): union, intersection, difference, subset, superset, disjoint; model ops and laws. Main session checked the mutate-while-descending argument (the cursor holds copies of unvisited upper bits; `clear_parents` touches only the consumed path) and re-ran `ci --quick` (722 passed, 6 skipped) and `wasm` (engine lib 191 passed).
- 2026-11-16 done Step 5 (builder): allocation checks for spec §4.6, all 0 blocks with no library change. Removing `ensure_capacity` from a setup made the check fail (`left: 9, right: 0`), so it can detect an allocation. Main session re-ran `ci --quick` (722 passed, 6 skipped) and `no-std`.
- 2026-11-16 done Step 6 (builder): fuzz target `hierarchical_bit_set_ops`. The xtask `targets_match_the_files_on_disk` test failed before `TARGETS` was updated. 600 s run (builder): `Done 1038875 runs in 601 second(s)`, final `cov: 870 ft: 5573 corp: 1436/319Kb`, no crash; main session checked `fuzz/artifacts/hierarchical_bit_set_ops/` is empty and re-ran `fuzz-build` and `ci --quick` (722 passed, 6 skipped).
- 2026-11-16 done Step 7 (builder): Gungraun group `hierarchical_bit_set` and 128 Criterion benchmarks; `bench-smoke` runs all 128; `deny` passes with `deny.toml` unchanged. `bench-regress` (main session, throwaway `debian:trixie` container on the M4 Pro host, aarch64, Valgrind 3.24.0, `--security-opt seccomp=unconfined`, branch at 49585db against base 873152b): "no regression"; new: `hierarchical_contains_4k` 45,750, `hierarchical_insert_4k` 153,531, `hierarchical_iter_sparse_256k` 71,534, `hierarchical_join_3x1_64k` 82,464 instructions, all above the 10,000 minimum.
- 2026-11-16 surprise Step 7's join inputs use independent seeds, so at 0.1% and 1% occupancy the joins are nearly empty and measure skipping, not yielding; recorded for the pull request.
- 2026-11-16 done Step 8 (builder): `bench/compare/examples/space_report.rs`. Main session re-ran its test and `ci --quick`. Table (builder's run, `cargo run -p engine_benches --release --example space_report`, arm64 Apple M4 Pro, at the Step 8 working tree on 49585db; bytes after building and peak exclude the member list; inserted one at a time in seeded random order, no presizing):

  | structure | case | members | after building | peak | after shrink_to_fit | bytes/member |
  |---|---|---:|---:|---:|---:|---:|
  | HierarchicalBitSet | one at 1,000,000 | 1 | 127,064 | 127,064 | 127,016 | 127,064 |
  | HierarchicalBitSet | 0.1% uniform | 1,048 | 148,512 | 148,512 | 133,144 | 141.71 |
  | HierarchicalBitSet | 1% uniform | 10,485 | 148,512 | 148,512 | 133,168 | 14.16 |
  | HierarchicalBitSet | 50% uniform | 524,288 | 148,512 | 148,512 | 133,168 | 0.28 |
  | HierarchicalBitSet | 99% uniform | 1,038,090 | 148,512 | 148,512 | 133,168 | 0.14 |
  | HierarchicalBitSet | 0.1% clustered | 1,024 | 198,392 | 198,392 | 99,992 | 193.74 |
  | HierarchicalBitSet | 1% clustered | 10,496 | 177,504 | 177,504 | 122,704 | 16.91 |
  | HierarchicalBitSet | 50% clustered | 524,288 | 242,704 | 242,704 | 133,072 | 0.46 |
  | HierarchicalBitSet | 99% clustered | 1,038,080 | 175,392 | 175,392 | 133,168 | 0.17 |
  | BitSet | one at 1,000,000 | 1 | 125,008 | 125,008 | 125,008 | 125,008 |
  | BitSet | 0.1% uniform | 1,048 | 146,128 | 146,128 | 131,048 | 139.44 |
  | BitSet | 1% uniform | 10,485 | 146,128 | 146,128 | 131,072 | 13.94 |
  | BitSet | 50% uniform | 524,288 | 146,128 | 146,128 | 131,072 | 0.28 |
  | BitSet | 99% uniform | 1,038,090 | 146,128 | 146,128 | 131,072 | 0.14 |
  | BitSet | 0.1% clustered | 1,024 | 196,752 | 196,752 | 98,400 | 192.14 |
  | BitSet | 1% clustered | 10,496 | 174,672 | 174,672 | 120,768 | 16.64 |
  | BitSet | 50% clustered | 524,288 | 238,864 | 238,864 | 130,976 | 0.46 |
  | BitSet | 99% clustered | 1,038,080 | 172,592 | 172,592 | 131,072 | 0.17 |
  | hibitset | one at 1,000,000 | 1 | 127,000 | 127,000 | — | 127,000 |
  | hibitset | 0.1% uniform | 1,048 | 148,448 | 148,448 | — | 141.65 |
  | hibitset | 1% uniform | 10,485 | 148,448 | 148,448 | — | 14.16 |
  | hibitset | 50% uniform | 524,288 | 148,448 | 148,448 | — | 0.28 |
  | hibitset | 99% uniform | 1,038,090 | 148,448 | 148,448 | — | 0.14 |
  | hibitset | 0.1% clustered | 1,024 | 198,328 | 198,328 | — | 193.68 |
  | hibitset | 1% clustered | 10,496 | 177,440 | 177,440 | — | 16.91 |
  | hibitset | 50% clustered | 524,288 | 242,640 | 242,640 | — | 0.46 |
  | hibitset | 99% clustered | 1,038,080 | 175,328 | 175,328 | — | 0.17 |
  | hi_sparse_bitset `_128bit` | one at 1,000,000 | 1 | 1,152 | 1,152 | — | 1,152 |
  | hi_sparse_bitset `_128bit` | 0.1% uniform | 1,048 | 51,200 | 132,000 | — | 48.85 |
  | hi_sparse_bitset `_128bit` | 1% uniform | 10,485 | 165,888 | 165,888 | — | 15.82 |
  | hi_sparse_bitset `_128bit` | 50% uniform | 524,288 | 296,960 | 296,960 | — | 0.57 |
  | hi_sparse_bitset `_128bit` | 99% uniform | 1,038,090 | 296,960 | 296,960 | — | 0.29 |
  | hi_sparse_bitset `_128bit` | 0.1% clustered | 1,024 | 2,432 | 17,408 | — | 2.38 |
  | hi_sparse_bitset `_128bit` | 1% clustered | 10,496 | 10,752 | 16,384 | — | 1.02 |
  | hi_sparse_bitset `_128bit` | 50% clustered | 524,288 | 165,888 | 165,888 | — | 0.32 |
  | hi_sparse_bitset `_128bit` | 99% clustered | 1,038,080 | 165,888 | 165,888 | — | 0.16 |

- 2026-11-16 surprise `dhat` counts a `realloc` as a change of size (`Delta::new(old_size, new_size)` in dhat 0.3's `realloc`), so peak misses the moment both buffers exist during growth; that is why peak equals "after building" for the dense sets. Noted in `bench/README.md`.
- 2026-11-16 lesson? clippy's pedantic `cast_precision_loss` fires on `u64 as f64` in bench examples; allow it in the file header (builder report).
- 2026-11-16 done Wall-clock run 1 (`cargo xtask bench --save m3.17-hierarchical-a`, Apple M4 Pro, AC power, 13:22:56 to 14:52:21, `pmset -g log` shows no sleep, wake or battery event in the window). Ratios, our median time over each baseline's (below 1 is faster):

  | group | case | ours (ns) | BitSet | hibitset | hi_sparse |
  |---|---|---:|---:|---:|---:|
  | hierarchical_contains | clustered_0.1pct | 1,224 | 0.96 | 0.93 | 0.43 |
  | hierarchical_contains | clustered_1pct | 1,231 | 1.00 | 0.96 | 0.37 |
  | hierarchical_contains | clustered_50pct | 1,324 | 1.00 | 1.07 | 0.40 |
  | hierarchical_contains | clustered_99pct | 1,311 | 1.00 | 0.96 | 0.32 |
  | hierarchical_contains | uniform_0.1pct | 1,312 | 1.00 | 1.05 | 0.39 |
  | hierarchical_contains | uniform_1pct | 1,446 | 1.10 | 1.16 | 0.39 |
  | hierarchical_contains | uniform_50pct | 1,447 | 1.10 | 1.16 | 0.35 |
  | hierarchical_contains | uniform_99pct | 1,310 | 0.91 | 1.05 | 0.32 |
  | hierarchical_insert | clustered_0.1pct | 2,398 | 1.25 | 1.45 | 1.18 |
  | hierarchical_insert | clustered_1pct | 20,991 | 1.21 | 1.24 | 1.07 |
  | hierarchical_insert | clustered_50pct | 1,015,700 | 1.19 | 1.16 | 1.06 |
  | hierarchical_insert | clustered_99pct | 2,009,300 | 1.19 | 1.19 | 1.06 |
  | hierarchical_insert | uniform_0.1pct | 3,489 | 1.93 | 1.92 | 1.03 |
  | hierarchical_insert | uniform_1pct | 36,060 | 4.76 | 2.25 | 1.86 |
  | hierarchical_insert | uniform_50pct | 751,570 | 2.63 | 2.37 | 0.95 |
  | hierarchical_insert | uniform_99pct | 1,405,200 | 2.51 | 2.38 | 0.88 |
  | hierarchical_iter | clustered_0.1pct | 528 | 0.11 | 0.35 | 0.77 |
  | hierarchical_iter | clustered_1pct | 5,185 | 0.46 | 0.34 | 0.74 |
  | hierarchical_iter | clustered_50pct | 249,680 | 0.78 | 0.33 | 0.71 |
  | hierarchical_iter | clustered_99pct | 493,900 | 0.79 | 0.32 | 0.71 |
  | hierarchical_iter | uniform_0.1pct | 1,530 | 0.28 | 0.64 | 1.26 |
  | hierarchical_iter | uniform_1pct | 14,831 | 1.04 | 0.56 | 1.40 |
  | hierarchical_iter | uniform_50pct | 376,300 | 1.08 | 0.54 | 1.04 |
  | hierarchical_iter | uniform_99pct | 566,640 | 0.92 | 0.38 | 0.81 |
  | hierarchical_join | clustered_0.1pct | 5 | - | 4.40 | 1.45 |
  | hierarchical_join | clustered_1pct | 10 | - | 1.56 | 1.33 |
  | hierarchical_join | clustered_50pct | 41,450 | - | 0.37 | 0.65 |
  | hierarchical_join | clustered_99pct | 30,749 | - | 0.51 | 1.17 |
  | hierarchical_join | uniform_0.1pct | 553 | - | 0.82 | 6.18 |
  | hierarchical_join | uniform_1pct | 2,690 | - | 0.54 | 0.38 |
  | hierarchical_join | uniform_50pct | 53,949 | - | 0.41 | 0.80 |
  | hierarchical_join | uniform_99pct | 31,245 | - | 0.57 | 1.13 |

  The `engine::BitSet` join column is empty because its label contains spaces, which the extraction script did not match.
- 2026-11-16 decision Insert is 1.2 to 2.4 times `hibitset`'s time (same design) and up to 4.8 times `BitSet`'s; Gungraun shows about 37 instructions per insert against `BitSet`'s 19 even when the word exists, so the overhead is ours. One focused fix attempt in this chunk (maintainer agreed to fix gaps that point at our code), then one more full run: it measures the new insert, and the unchanged groups give the run-to-run noise floor. The gaps to `hi_sparse_bitset` (near-empty joins, uniform 1% iteration) come from its 128-bit blocks and SIMD and go to the sparse follow-up issue.
- 2026-11-16 dead-end Insert fix (builder, dev-loop Criterion runs of `hierarchical_insert` only, M4 Pro): baseline medians uniform_1pct 35.5 µs (hibitset 15.8, BitSet 7.54), uniform_50pct 750 µs (333, 284), clustered_50pct 1.014 ms (871, 867). Attempt 1 (`get_mut` on layer 0, cold growth, `top` after the parent loop, no `k == 5` branch): no gain (uniform_50pct +5.6%, clustered 0%). Attempt 2 (parent bits set unconditionally, no `before == 0` branch): uniform_1pct −25% but uniform_50pct +53%, uniform_99pct +63%. No change kept; `hierarchical.rs` is as committed. The cost looks like data-dependent branches and the dependent parent writes, not the instruction count (inferred, not confirmed in asm). Goes to the follow-up issue.
- 2026-11-16 decision Run 2 is a pure noise-floor run (code unchanged since run 1): some comparisons fall in the 5–15% band (`contains` against `hibitset`, insert against `hi_sparse_bitset`).
- 2026-11-16 decision Run 2 stopped about 10 minutes in (maintainer agreed) for one insert experiment shaped like `hibitset`'s: parent climb out of line, unconditional parent writes on the slow path only. Partial `m3.17-hierarchical-b` baseline deleted; run 2 restarts on the final code.
- 2026-11-16 decision Further optimizations stay out of this chunk (maintainer). At close-out, one follow-up issue "hierarchical bitset performance and space" carries the ranked list (internal `fold` iteration; join ordering and bounds; `clear` through the hierarchy; a "full" summary layer; 256-bit blocks with 4 layers; batch insert and remove; paged layer 0; one allocation), the measured gaps and the insert attempts; items 4 to 6 wait on M3.8's storage decision.
- 2026-11-16 done Insert fix, variant A kept (builder experiment; main session installed and verified it). Asm (aarch64, `--emit=asm` probe): `insert` was not inlined, one `bl` per insert with a 6-register, 80-byte prologue, layer-0 length checked twice, the parent climb inlined into the same function. Variant A: `get_mut` hit path (load, OR, store), growth in a `#[cold] #[inline(never)] insert_growing`, `set_parents` `#[inline(never)]` and called only when a word goes from zero. Variant B (A plus unconditional parent writes) was slower than A and is not kept. Dev-loop medians, µs, two runs each (M4 Pro; `cargo bench -p engine_benches --bench collections -- hierarchical_insert`):

  | case | HEAD | A | A+B | hibitset | BitSet |
  |---|---:|---:|---:|---:|---:|
  | uniform_1pct | 36.8 / 36.7 | 13.3 / 13.3 | 19.3 / 19.3 | 14.8–16.1 | 7.7 |
  | uniform_50pct | 764 / 768 | 310 / 308 | 338 / 338 | 314–325 | 287–289 |
  | uniform_99pct | 1,426 / 1,427 | 587 / 583 | 629 / 626 | 595–619 | 566–573 |
  | clustered_50pct | 1,032 / 1,031 | 942 / 941 | 942 / 942 | 931–941 | 936–939 |

  Re-ran `ci --quick` (722 passed, 6 skipped, including the allocation checks), `wasm` (engine lib 191 passed) and `no-std`.
- 2026-11-16 lesson? `#[inline]` on a method does not make it inlined: a cold or rare path inlined into it (here the parent climb, and the panic formatting) can push it past LLVM's budget, so every call pays a full prologue. Keep hot paths tiny and put rare work in `#[inline(never)]` functions; check with `--emit=asm`, not by reading the source.
- 2026-11-16 done `bench-regress` after the insert fix (same container setup, 5f1ebca against 873152b): no regression; `hierarchical_insert_4k` 88,374 instructions (153,531 before the fix), the other three new benchmarks unchanged.
- 2026-11-16 done Opened #62 (maintainer asked): move long verification (wall-clock, mutants, fuzzing) out of the local chunk loop, with the research behind it. For this chunk, mutation testing comes from the pull request's `mutants` job (`--in-diff`) instead of a local `--module collections` run; recorded as a deviation from R15 at close-out.
- 2026-11-16 done Wall-clock run 2 (`cargo xtask bench --save m3.17-hierarchical-b`, 5f1ebca with the insert fix, M4 Pro, AC power, 15:45:22 to 17:15:00, `pmset -g log` clean). Noise floor from the 644 benchmarks whose code did not change between runs 1 and 2: median |change| 0.7%, 90th percentile 6.1%, 95th 10.4%; the large movers (up to 59%) are the 100,000 to 1,000,000-element handle and B-tree benchmarks, and one sub-microsecond join (`hierarchical_join/engine::HierarchicalBitSet/uniform_0.1pct`, +30%). Insert moved run 1 to run 2 by −14% (clustered) to −64% (uniform 1%). Run 2 ratios, our median over each baseline's (the `BitSet` join is clone plus `intersect_with` plus `difference_with`):

  | group | case | ours | /BitSet | /hibitset | /hi_sparse |
  |---|---|---:|---:|---:|---:|
  | contains | clustered_0.1pct | 1.2 µs | 1.01 | 0.97 | 0.43 |
  | contains | clustered_1pct | 1.2 µs | 1.00 | 0.96 | 0.37 |
  | contains | clustered_50pct | 1.3 µs | 1.00 | 1.06 | 0.40 |
  | contains | clustered_99pct | 1.3 µs | 1.00 | 1.05 | 0.32 |
  | contains | uniform_0.1pct | 1.3 µs | 1.00 | 1.05 | 0.39 |
  | contains | uniform_1pct | 1.3 µs | 1.00 | 1.05 | 0.36 |
  | contains | uniform_50pct | 1.3 µs | 1.00 | 1.05 | 0.32 |
  | contains | uniform_99pct | 1.3 µs | 1.00 | 1.05 | 0.32 |
  | insert | clustered_0.1pct | 2.1 µs | 1.08 | 1.27 | 1.02 |
  | insert | clustered_1pct | 18.0 µs | 1.03 | 1.06 | 0.92 |
  | insert | clustered_50pct | 871.4 µs | 1.03 | 1.02 | 0.91 |
  | insert | clustered_99pct | 1,717.3 µs | 1.03 | 1.02 | 0.91 |
  | insert | uniform_0.1pct | 2.5 µs | 1.42 | 1.41 | 0.74 |
  | insert | uniform_1pct | 13.2 µs | 1.74 | 0.90 | 0.68 |
  | insert | uniform_50pct | 304.7 µs | 1.07 | 0.97 | 0.39 |
  | insert | uniform_99pct | 573.9 µs | 1.02 | 0.97 | 0.37 |
  | iter | clustered_0.1pct | 524 ns | 0.11 | 0.34 | 0.67 |
  | iter | clustered_1pct | 5.2 µs | 0.46 | 0.33 | 0.63 |
  | iter | clustered_50pct | 248.5 µs | 0.77 | 0.32 | 0.61 |
  | iter | clustered_99pct | 492.2 µs | 0.79 | 0.32 | 0.68 |
  | iter | uniform_0.1pct | 1.5 µs | 0.26 | 0.65 | 1.25 |
  | iter | uniform_1pct | 15.0 µs | 1.06 | 0.60 | 1.36 |
  | iter | uniform_50pct | 376.8 µs | 1.09 | 0.53 | 1.01 |
  | iter | uniform_99pct | 564.2 µs | 0.91 | 0.39 | 0.77 |
  | join | clustered_0.1pct | 6 ns | 0.00 | 4.89 | 1.58 |
  | join | clustered_1pct | 11 ns | 0.00 | 1.68 | 1.46 |
  | join | clustered_50pct | 45.7 µs | 0.70 | 0.38 | 0.65 |
  | join | clustered_99pct | 34.1 µs | 1.30 | 0.56 | 1.23 |
  | join | uniform_0.1pct | 720 ns | 0.04 | 0.96 | 7.68 |
  | join | uniform_1pct | 3.2 µs | 0.18 | 0.59 | 0.42 |
  | join | uniform_50pct | 57.4 µs | 0.67 | 0.41 | 0.79 |
  | join | uniform_99pct | 34.0 µs | 1.20 | 0.64 | 1.17 |
- 2026-11-16 done Step 10 (main session): `lib.rs` module map, README row, research catalog entry, CHANGELOG entry. Full `cargo xtask ci` passes (nextest 722 passed, 6 skipped; 728 passed in the second test pass), `doc` and `check-docs` clean.
- 2026-11-16 done Simplify pass (builder, inline single pass): one `WORD_SHIFT`, and a shared private `WordBits::next_member` for the bit loop of `HierarchicalIter` and `Join`. `ci --quick` re-run by main session (722 passed, 6 skipped). Instruction counts before and after are checked with `bench-regress --base b667ca6` in the container.
- 2026-11-16 done `bench-regress --base b667ca6` on 306bf57 (simplify), same container setup: no regression; `hierarchical_iter_sparse_256k` 71,534 → 66,979 (−6.4%), `hierarchical_join_3x1_64k` 82,526 → 77,311 (−6.3%), insert and contains unchanged. The wall-clock tables predate this commit.
- 2026-11-16 decision Review triage so far. Docs: fix the CHANGELOG's "mutation triage" (added back at close-out once the CI triage exists), shorten the catalog citation, say the `bench/README.md` `realloc` note comes from reading dhat 0.3's source; not fixed: the plan restating the spec (the plan is the executed record of what builders were briefed with), tables kept in the log (reviewer accepts). Spec: fix the R8 gap (panic tests for `ensure_capacity`, `Extend<usize>`, `Extend<&usize>`, `From<[usize; N]>`), re-run fuzz (600 s), wasm and full `ci` on the final commit; the `0.00` join ratios against `BitSet` are explained in the PR (its join clones the 128 KB set). Security: fix, cite `hibitset` as the followed design in the `bit_set` module docs (ADR 0002).
- 2026-11-16 decision Code review triage: fix all four. The space report's peak includes input generation (dhat's `max_bytes` cannot be reset; the 131,072-byte `taken` bitmap and 16,384-byte runs vector show in `hi_sparse_bitset`'s peaks), so the logged table's peak column is wrong for those rows and is regenerated; R8 panic tests (as the spec review); `union_with` uses `words::union_into` and `summarize` moves to `words.rs` for `check_invariants` (spec §4.1); the `Join` proptest's repeated-exclusion option is weighted lower.
- 2026-11-16 done Review fixes (builder): space report inputs generated before the profiler starts (single case before: `1048 51200 132000`, after: `1048 51200 51200`); regenerated table has peak equal to "after building" in every row, and only three `hi_sparse_bitset` peaks change from the logged table: 0.1% uniform 132,000 → 51,200, 0.1% clustered 17,408 → 2,432, 1% clustered 16,384 → 10,752 (all other figures identical). Four panic tests added (`ensure_capacity` on 64-bit hosts, `Extend<usize>`, `Extend<&usize>`, `From<[usize; N]>`), passing first time. `summarize` in `words.rs`, `union_with` through `words::union_into`; `Join` proptest repeated exclusion at 20%; short `HierarchicalIter` `Debug`; `Join` borrow note; `hibitset` cited in the `bit_set` module docs (ADR 0002); bounded walks in the allocation test. Main-session docs fixes: CHANGELOG, catalog sources, `bench/README.md` note. Main session re-ran `ci --quick` (726 passed, 6 skipped) and the space report test and single case.
- 2026-11-16 lesson? `dhat`'s `max_bytes` cannot be reset: generate inputs before starting the profiler, or the peak measures the input generator (code review).
- 2026-11-16 lesson? A CHANGELOG line naming a deliverable (here "a mutation triage") is written only after it exists (docs and spec reviews).
- 2026-11-16 decision Spec §4.3 amended (maintainer approved the recommendation): `capacity()` is the bound across every layer, so every insert below it does not allocate (conditional review found `insert(128_000)` allocating in layer 1 while `capacity()` reported 128,128).
- 2026-11-16 done `capacity()` per amended §4.3 (builder): minimum over layers of allocated words × 64^(k+1), saturating, capped at `MAX_BITS`. With the old body restored, `inserts_below_capacity_never_grow_any_layer` failed (`insert(100000) grew a layer, capacity 128128`) and the new dhat check failed; both pass now. Main session re-ran `ci --quick` and `wasm`.
