# Spikes

Status: Accepted (2026-11-14), added by M3.2.

A **spike** is a measured experiment. Its code is kept after the decision it informed, so the numbers can be re-run and the next chunk can start from the winner. Nothing under `spikes/` is part of the engine's public API.

## Rules

Every spike crate follows [CONTRIBUTING.md](../CONTRIBUTING.md#spike-crates): the workspace lints, `#![forbid(unsafe_code)]` (until an ADR says otherwise), `cargo deny`, documented public items, and conformance tests that pass before any number is quoted. Spike crates are exempt from doctests on every public type and from `try_` twins, and they are outside coverage and mutation testing.

## Layout and names

`spikes/` holds one directory per topic. Every directory one level below a topic is a workspace package (the workspace `members` glob is `spikes/*/*`), named `spike_<short topic>_<name>`:

```
spikes/
  README.md                this file
  ecs_storage/             topic; its short name is `ecs`
    README.md
    harness/               spike_ecs_harness
    vec/                   spike_ecs_vec
    bench_compare/         spike_ecs_bench_compare
    bench_cache/           spike_ecs_bench_cache
```

A new topic adds its entries to the constants `SPIKE_TOPICS`, `SPIKE_COMPARE_PACKAGES` and `SPIKE_CACHE_PACKAGES` in `xtask/src/main.rs`, and its packages to `SPIKE_PACKAGES` in `xtask/src/coverage.rs`.

## Spikes

| Topic | Milestone | What it measures |
|---|---|---|
| [`ecs_storage`](ecs_storage/README.md) | M3 | Storage models for an ECS, on seven shared workloads |
