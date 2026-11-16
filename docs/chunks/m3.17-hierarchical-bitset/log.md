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
