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
