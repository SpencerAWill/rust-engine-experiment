# M3.25 log: rare_components workload for the ECS storage harness

- **Status:** Living (2026-11-27)

<!-- Append-only. One or two lines per entry, written when it happens, not at the end:
- <date> <tag> <text>
Tags: done, decision, surprise, dead-end, lesson? (a candidate lesson for stage 6; see docs/lessons/README.md). -->

- 2026-11-27 done Picked by `/chunk next`: #109, order 15, unblocked; M3.7 (#101, order 20) also eligible; M3.8 (#102) blocked by #101 and #109; no scheduled chores. Branch `m3.25-rare-components` pushed.
- 2026-11-27 decision #41 (M3.12, heap bytes) is open with no spec, so this chunk measures time only; the space half waits for M3.12.
- 2026-11-27 decision (maintainer) The step's query is `(&mut Common, &Rare)`: every entity has `Common`, about 1% also `Rare`, and the query writes `Common` from `Rare`, so `sparse_set` drives from `Rare` and probes `Common` at scattered indices.
- 2026-11-27 decision (maintainer) Rare entities are chosen by a per-entity draw (`stream(i)`'s second draw below 0.01), not every 100th or a block: irregular spacing defeats the stride prefetcher, as real rare tags would.
- 2026-11-27 decision (maintainer) Sizes 10,000, 100,000 and 1,000,000 (the sizes of `random_access`).
- 2026-11-27 decision (maintainer) `Rare` is a table type on `hybrid`: static and iterated, so a hybrid user would make it one; the sparse configuration (a walk over every `Common` row with a probe per row) is a caveat, unmeasured.
- 2026-11-27 decision (maintainer) The space half: a comment on #41 naming `rare_components` as a space case for M3.12; this chunk is time only.
- 2026-11-27 done Simulation (ecs-13) from a scratchpad crate outside the workspace (own `[workspace]`, pinned toolchain, the repo's `Cargo.lock`, depending on `engine` and `spike_ecs_harness` by path, release), seed 42, the rules of spec §4.2: rare entities 11 of 1,000 (1.100%), 78 of 10,000 (0.780%), 981 of 100,000 (0.981%), 10,009 of 1,000,000 (1.001%); smallest `Rare` value 7.84e-5 (at 1,000,000); largest index gap between rare entities 248, 684, 717, 858. Over 1,000 ticks at `count` 1,000 and at 10,000: every rare entity's `Common` changes at every tick, and no checksum equals an earlier tick's, tick 0 included.
- 2026-11-27 decision (my call) Conformance at `count` 10,000 (78 rare entities at seed 42), not the usual 1,000 (11): more rare entities for a backend to get wrong, at a test cost that stays small.
- 2026-11-27 done Grill cold read (`spec-reader`, registered agent): nothing important left; one point settled below. Declared harness API compiled as a stub on the pinned toolchain (scratchpad crate), no errors.
- 2026-11-27 decision Grill, main session's calls: (1) §4.3's "in one call" narrowed to backends with bundles; `sparse_set` and `dense` use `spawn` and `insert` (cold read). (2) §4.5 adds a direct `StorageKind` test on `hybrid` for `Common` and `Rare`, as M3.23's. (3) §4.5's allocation count passes when each backend's per-step count is steady after warm-up and equals its `simple_iter` one-query control's; a difference is explained before close-out (cold read, ecs-2).
- 2026-11-27 decision (maintainer) Spec approved as written; status Accepted.
