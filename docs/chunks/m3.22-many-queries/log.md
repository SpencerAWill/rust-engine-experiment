# M3.22 log: many_queries workload for the ECS storage harness

- **Status:** Living (2026-11-26)

<!-- Append-only. One or two lines per entry, written when it happens, not at the end:
- <date> <tag> <text>
Tags: done, decision, surprise, dead-end, lesson? (a candidate lesson for stage 6; see docs/lessons/README.md). -->

- 2026-11-26 done Picked by `/chunk next`: M3.22 (#107), order 12, the lowest unblocked Scheduled chunk in M3; no scheduled chores. Branch `m3.22-many-queries-workload` pushed.
- 2026-11-26 surprise How each backend starts a query today: the four prototypes keep no query state, so every run builds a `Plan` (a `Vec`, one `BTreeMap` lookup per type) and scans every archetype or set; `bevy_ecs` rows hold a `QueryState` built in `setup`; `hecs` caches matches per query type in `World` (lock, type lookup, downcast, `Arc` clone per run).
- 2026-11-26 decision (maintainer) Each backend starts its queries by its own repeated-query path as it is today: `bevy_ecs` holds one `QueryState` per query from `setup`, `hecs` plain `query_mut`/`query` (its internal cache), the prototypes rebuild per run. No cached query state is added to the prototypes; the caveat (their gap is largely the missing cache) goes to M3.8 with the run.
- 2026-11-26 decision (maintainer asked whether to build cached query state soon) It is already an M4 item in #84 (from M3.3 and M3.4); recommended keeping it there, with the `hecs` against `archetype` rows of this run as the estimate of what a cache saves on archetype storage.
- 2026-11-26 decision (maintainer) World shape: eight component types `C0..C7`, eight archetypes (group `g` holds the four cyclically consecutive types from `Cg`), entities spread evenly; the queries are every ordered pair "write `Ci`, read `Cj`", `i != j`: 56 queries per step, 8 of which (cyclic distance 4) match no entity.
- 2026-11-26 decision Main session's calls in the draft spec: sizes 100, 1,000 and 10,000 (adds 100 to the issue's two, so query start dominates every row at one size); named types `C0..C7`; `update` is `w * 0.5 + r * 0.25 + 0.125` (bounded, no subnormals); `hybrid` all table types; the query list defined once as `QUERIES` plus a `for_each_query!` macro every backend runs its step through, since a dropped empty query passes conformance.
- 2026-11-26 done Spec drafted (`spec.md`, Draft).
