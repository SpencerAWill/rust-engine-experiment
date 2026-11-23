# M3.10 log: bevy_ecs and hecs baselines as harness backends

- **Status:** Living (2026-11-23)

<!-- Append-only. One or two lines per entry, written when it happens, not at the end:
- <date> <tag> <text>
Tags: done, decision, surprise, dead-end, lesson? (a candidate lesson for stage 6; see docs/lessons/README.md). -->

- 2026-11-23 done Picked by `/chunk next`: #100, Order 10, M3, unblocked; M3.7 (#101) next, M3.8 (#102) blocked by 2 open issues; no scheduled chores. Branch `m3.10-ecs-baselines` pushed.
- 2026-11-23 done Probe (scratchpad crate outside the repository, rustc 1.100.0): `bevy_ecs` 0.20.1 (`default-features = false, features = ["std"]`, MSRV 1.95) and `hecs` 0.11.2 (MSRV 1.81) build together; `#[derive(Component)]` with and without `#[component(storage = "SparseSet")]` compiles under `#![forbid(unsafe_code)]`; about 60 crates in the normal dependency tree.
- 2026-11-23 done Probe: `cargo deny check licenses bans sources` with this repository's `deny.toml` passes every dependency of both crates (only the unlicensed probe crate itself fails); duplicates `hashbrown` and `syn` warn. A `[bans] deny = [{ crate = "hecs", wrappers = ["nobody"] }]` entry fails with "crate 'hecs = 0.11.2' is explicitly banned" and one naming the direct parent passes (cargo-deny 0.20.2). Clippy pedantic with `-D warnings` raises nothing from the `Component` derive.
- 2026-11-23 decision (maintainer) The baselines are normal `[dependencies]` of two unpublished spike crates, `spike_ecs_bevy` and `spike_ecs_hecs`; ADR 0002 is not changed; a `deny.toml` `[bans]` entry with `wrappers` lets only `spike_ecs_bevy` depend on `bevy_ecs` and only `spike_ecs_hecs` on `hecs`.
- 2026-11-23 decision (maintainer) Each baseline drives its `World` directly, single-threaded, with no schedule or systems, so the numbers measure storage as the prototypes' do.
- 2026-11-23 decision (maintainer) A second bevy backend, `bevy_ecs_sparse`, for `add_remove` and `agents`, with `B` and `Seeker` in `SparseSet` storage, as a like-for-like baseline for `hybrid`.
- 2026-11-23 decision (maintainer) The informal run is the Criterion bench on the Mac under the protocol; the cache bench is registered and built, not run.
- 2026-11-23 surprise Grill stub (scratchpad, bevy_ecs 0.20.1): a new `World` reports `entities().count_spawned()` 1 (an entity of bevy's own), so a bevy backend cannot use it as the alive count; and without `clear_trackers`, `World::removed::<B>()` held 10, 20, 30 entities after three remove steps on 10 entities, 0 with it. Both written into spec §4.4.
- 2026-11-23 done Cold read (`spec-reader`, registered agent): two questions, hecs `query_mut` against `PreparedQuery`, and whether `bevy_ecs_sparse` is like-for-like with `hybrid`. Both premises checked: hecs 0.11.2 caches query matches in `World::query_cache` with `std` and has `PreparedQuery`; in the stub, inserting a `SparseSet` component took bevy's archetype count from 3 to 4 with the table count unchanged. The wrong "hecs has no cached query state" and its `frag_iter` expectation are corrected in §4.4 and §4.5.
