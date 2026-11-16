# Spikes

Status: Accepted (2026-11-14), added by M3.2; reshaped by M3.16.

A **spike** is a measured experiment. Its code is kept after the decision it informed, so the numbers can be re-run and the next chunk can start from the winner: the storage prototypes of M3 are the first. Nothing under `spikes/` is part of the engine's public API.

## When this applies

- You add or change a spike crate, a backend or a workload.
- You quote a number a spike produced.

## Contents

| Topic | Milestone | What it measures |
|---|---|---|
| [`ecs_storage/`](ecs_storage/README.md) | M3 | Storage models for an ECS, on seven shared workloads |

## Rules

Spike crates follow the rules of the bench crates, not the library rules:

- **They keep** the workspace lints, `#![forbid(unsafe_code)]` (until an ADR says otherwise: M3.7 is the first chunk that may), `cargo deny`, and the determinism lints. Every public item is documented.
- **They are exempt from** "every public type has a runnable doctest", from the `try_` twin rule for documented panics, and from the inline-test-module rule of [Tests](../CONTRIBUTING.md#tests) (a spike may keep its tests in `src/tests.rs`). They are outside coverage and mutation testing: conformance tests are what keep spike code honest, and CI runs them with the rest of the workspace.
- **Backends are checked before they are measured.** A backend must pass `check_conformance` against the reference for every workload it implements before its numbers are quoted anywhere.
- **Benchmarks are manual.** `cargo xtask spike-bench <topic>` runs a spike's Criterion bench and `cargo bench -p <package>` its Gungraun bench; neither is a CI gate. `cargo xtask bench` and `bench-doc` leave them out, so the library suites and `docs/benchmarks.md` do not change. Quote spike numbers under the rules for [wall-clock benchmarks](../bench/README.md#wall-clock-benchmarks).

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

A new topic adds its entries to the constants `SPIKE_TOPICS`, `SPIKE_COMPARE_PACKAGES` and `SPIKE_CACHE_PACKAGES` in `xtask/src/main.rs`, and its packages to `SPIKE_PACKAGES` in `xtask/src/coverage.rs`, and a row to Contents above.
