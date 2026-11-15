# Roadmap

Status: Accepted (2026-11-12). The source of truth for what is planned and how far along it is.

Work is organized as **milestones** (a goal with exit criteria) broken into **chunks** (one small pull request each, with its own spec and plan in [`design/`](design/)). Only the current milestone is broken into chunks; later ones carry a rough list that is refined when they become current. Chunk IDs (`M2.1`) are never reused or renumbered; order comes from the dependency column. The process is described in [CONTRIBUTING.md](../CONTRIBUTING.md) and the first chunk's [spec](design/m2-infrastructure/m2.0-roadmap-and-process/spec.md); the templates are in [`design/_template/`](design/_template/).

## Milestones

| Milestone | Goal | Exit criteria | Status |
|---|---|---|---|
| M0+M1 | Engineering foundations and core primitives | Done (PR #3); see [its spec](design/m0-m1-foundations/spec.md) | Done |
| M2 | Process and verification infrastructure | Roadmap and templates in place; nightly fuzzing green; coverage report in CI; first mutation triage recorded; Gungraun gate active on pull requests | Done |
| M3 | ECS storage decision | ADRs accepted for the storage model, component identity, storage of foreign and mod-defined components, and the transform component layout, backed by results recorded in `benchmarks.md` | Current |
| M4 | ECS core | World, entities, components and queries on the chosen storage; registry core; the dogfooding sample runs and its determinism test passes; core operations within noise of the winning M3 prototype | Next |
| M5 | Reflection and persistence | The sample world saves, loads and reproduces its checksum; a separate tool prints a saved world without being compiled against its types | Later |
| M6 | Change tracking and edits | Applying edits then undoing them restores the original world hash; `diff(A, B)` applied to A yields B | Later |
| M7 | Scheduler | The sample runs its systems in parallel with identical checksums at thread counts 1 to N | Later |
| M8 | C ABI and C# | A C# program drives the world and defines a component at runtime | Later |
| M9 | Diagnostics and tooling | An external tool connects to the running sample, browses entities and edits a field | Later |

## M3 chunks

Row statuses are as for M2 below. The storage prototypes live under `spikes/` and are kept there after the decision, so the numbers in `benchmarks.md` can be re-run and M4 can start from the winner.

| Chunk | Title | Depends on | Status | Spec and plan |
|---|---|---|---|---|
| M3.0 | Windowed baseline sample: `examples/agents_2d` (`winit` and `softbuffer`, example-only), the agent workload and its order-independent checksum | none | Done (PR #24) | [spec](design/m3-ecs-storage/m3.0-windowed-baseline/spec.md), [plan](design/m3-ecs-storage/m3.0-windowed-baseline/plan.md) |
| M3.1 | `engine::collections`: deterministic map and set | none | Done (PR #26) | [spec](design/m3-ecs-storage/m3.1-collections/spec.md), [plan](design/m3-ecs-storage/m3.1-collections/plan.md) |
| M3.2 | Spike harness under `spikes/ecs_storage/`: the `Workload` interface and conformance check, seven workloads (simple and fragmented iteration, heavy compute, add/remove, entity churn, 100k 2D sprites, the M3.0 agent workload), the plain-`Vec` backends (array-of-structs, and struct-of-arrays for agents), Criterion and Gungraun cache-simulation benches | M3.0, M3.1 | Done (PR #36) | [spec](design/m3-ecs-storage/m3.2-spike-harness/spec.md), [plan](design/m3-ecs-storage/m3.2-spike-harness/plan.md) |
| M3.3 | Sparse-set prototype (safe) | M3.2 | Planned | |
| M3.4 | Archetype prototype (safe) | M3.2 | Planned | |
| M3.5 | Per-component hybrid prototype (safe) | M3.3, M3.4 | Planned | |
| M3.6 | Runtime-registered components in safe byte columns, in the leading prototype | M3.3, M3.4, M3.5 | Planned | |
| M3.7 | `unsafe` type-erased variant of the leading prototype: an ADR for a spike-scoped exception, an `unsafe-allowlist.toml` entry and Miri in CI; measures the safe and `unsafe` gap | M3.3, M3.4, M3.5 | Planned | |
| M3.8 | Decision ADRs (storage model, component identity, foreign and mod-defined component storage, transform layout) and the `benchmarks.md` section. Before it records results, it settles three measurement gaps M3.2 left open: the `agents` cache bench measures tick 1 only, so no removal or respawn is counted (warm it up past the first lifetimes); the Criterion `agents` bench steps one instance for a backend-dependent count, so cursor phases are weighted differently per backend (fix the tick count); and Callgrind takes its simulated cache sizes from the host (pin `--I1`, `--D1` and `--LL`) | M3.0 to M3.7, M3.10 | Planned | |
| M3.9 | Repository settings hardening (not part of M3's exit criteria): private vulnerability reporting, squash-only merging with linear history, Dependabot security updates and cooldown, CodeQL and Scorecard code scanning, a `check-workflows` allowlist for write scopes, wiki and Projects off | none | Done (PR #35) | [spec](design/m3-ecs-storage/m3.9-repository-settings/spec.md), [plan](design/m3-ecs-storage/m3.9-repository-settings/plan.md) |
| M3.10 | bevy_ecs and hecs baselines as harness backends (split out of M3.2, which keeps the harness and the plain-`Vec` backends) | M3.2 | Planned | |
| M3.11 | Generational IDs (not part of M3's exit criteria; #34, #39): remove `DenseHandleAllocator`, rename `engine::handle` to `engine::genid` and `Handle<T>` to `GenId<T>` over a new `UntypedGenId` core; bit layout unchanged; ADR 0012 supersedes ADR 0010 | a chore pull request renaming the regress bench target `handle` to `genid` | Done (PR #52) | [spec](design/m3-ecs-storage/m3.11-genid/spec.md), [plan](design/m3-ecs-storage/m3.11-genid/plan.md) |
| M3.13 | One library crate (not part of M3's exit criteria; #40): merge the five library crates into `engine` as its modules (`genid`, `collections`, `random`, `time`, `math`), with `math` buildable without `std`; ADR 0004 rewritten in place with a splitting rule (split only when Rust requires it, for a permission boundary, for configurations that cannot be features, or for measured compile time) and ADR 0003 edited for `no_std` builds, as a one-time exception; the historical records, benchmark labels and benchmark results rewritten as if there had always been one crate | none | Done (PR #54) | [spec](design/m3-ecs-storage/m3.13-one-engine-crate/spec.md), [plan](design/m3-ecs-storage/m3.13-one-engine-crate/plan.md) |
| M3.14 | Bitsets (not part of M3's exit criteria; #42): `engine::collections::BitSet` (heap, grows on insert) and `BitArray<WORDS>` (inline, fixed), our own code on `u64` words with a fixed `MAX_BITS` so behaviour does not depend on pointer width; `fixedbitset` as a benchmark baseline only | none | Done (PR #56) | [spec](design/m3-ecs-storage/m3.14-bitset/spec.md), [plan](design/m3-ecs-storage/m3.14-bitset/plan.md) |
| M3.15 | Agent workflow and lessons (not part of M3's exit criteria): one loop (brainstorm, grill, plan, work, review, compound, close out) stopping only for spec approval and the merge go-ahead; agent roles with pinned models in `.claude/agents/` (Sonnet builders, independent reviewers); `log.md` per chunk; `docs/lessons/` retrieved by path; `/chunk` and `/grill` skills; a push guard; the project knowledge in local memory moved into the repository | M3.14 | In progress | [spec](design/m3-ecs-storage/m3.15-agent-workflow/spec.md), [plan](design/m3-ecs-storage/m3.15-agent-workflow/plan.md) |
| M3.16 | Documentation layout (not part of M3's exit criteria): a README with an index in each directory that matters; the tool manuals moved out of `CONTRIBUTING.md` next to their tools; path-scoped `.claude/rules/`; `cargo xtask check-docs` as a CI gate (READMEs, indexes, lesson format, `AGENTS.md` length); the first chunk built entirely with the M3.15 loop | M3.15 | Planned | |

## M2 chunks

Row statuses: `Planned`, `Drafting`, `Accepted`, `In progress`, `Done (PR #n)`, `Dropped (reason)`. `Drafting` to `In progress` are set on the chunk's branch, so on `main` a row normally goes from `Planned` straight to `Done`.

| Chunk | Title | Depends on | Status | Spec and plan |
|---|---|---|---|---|
| M2.0 | Roadmap and process | none | Done (PR #12) | [spec](design/m2-infrastructure/m2.0-roadmap-and-process/spec.md), [plan](design/m2-infrastructure/m2.0-roadmap-and-process/plan.md) |
| M2.1 | Fuzzing: pinned nightly, `fuzz/` targets for `HandlePool`, the allocators (operation sequences against a model, plus `check_invariants`) and `Handle::from_bits`; `cargo xtask fuzz`; nightly CI job | M2.0 | Done (PR #14) | [spec](design/m2-infrastructure/m2.1-fuzzing/spec.md), [plan](design/m2-infrastructure/m2.1-fuzzing/plan.md) |
| M2.2 | Coverage: `cargo-llvm-cov` through xtask; report uploaded in CI; no threshold gate | M2.0 | Done (PR #16) | [spec](design/m2-infrastructure/m2.2-coverage/spec.md), [plan](design/m2-infrastructure/m2.2-coverage/plan.md) |
| M2.3 | Mutation testing: `cargo xtask mutants`, scheduled report-only workflow, first triage of `engine::handle` and `engine::random` | M2.2 | Done (PR #17) | [spec](design/m2-infrastructure/m2.3-mutation-testing/spec.md), [plan](design/m2-infrastructure/m2.3-mutation-testing/plan.md) |
| M2.4 | Gungraun gate: Valgrind and gungraun-runner in the devcontainer and CI; benchmarks for handle and PRNG hot paths; merge-base against head comparison; fails above 5%; `perf-regression-accepted` label declared in Pulumi | M2.0 | Done (PR #18) | [spec](design/m2-infrastructure/m2.4-gungraun-gate/spec.md), [plan](design/m2-infrastructure/m2.4-gungraun-gate/plan.md) |

**Miri is deferred out of M2.** [M0+M1 spec](design/m0-m1-foundations/spec.md) §4.12 placed it at the start of M2, but its trigger is the first allowlisted `unsafe` crate, and none exists. It arrives with the first chunk that proposes `unsafe`: M3.7.

## Later milestones

Rough lists, refined when each becomes current.

| Milestone | Rough chunks | Open question |
|---|---|---|
| M4 | The ECS as a module of `engine` (`engine::ecs`): World, Entity (on `engine::genid`), typed and runtime component registration, spawn/despawn, insert/remove; queries with with/without filters; invariant checks and readable `Debug`; debugger visualizers; Gungraun benchmarks for storage hot paths; the dogfooding sample (M3.0's `agents_2d` ported to the ECS; the windowing crate stays example-only) | Whether resources (singleton data) belong in M4 or M7. Entity relationships stay out |
| M5 | Reflection core traits as a module of `engine`, next to the types they are implemented for; field read and write by path; self-describing serialization; stable content hasher (a frozen format, so an ADR); persistent IDs with remapping on load; maps and sets are saved in iteration order and loaded by inserting in that order (order is state) | The serialization format: our own or an existing self-describing one |
| M6 | Change detection (added or changed since a tick); invertible edit commands (the basis for undo/redo); world diffing | Its first spec decides whether change detection is tick-based or relative to each system's last run; the latter moves M7 before M6 |
| M7 | Systems with access sets; explicit ordering and ambiguity detection; single-threaded mode; parallel execution on host-provided threads; loom, ThreadSanitizer, stress tests and the concurrency allowlist | The threading model (thread pool, fibers, work stealing). How systems send events to each other: typed per-tick queues or short-lived entities |
| M8 | `engine_ffi` (the first allowlisted `unsafe` crate outside `spikes/`; Miri arrives in M3.7); generated header; C# binding and sample; FFI panic policy; evaluation of WASM component-model mods | Binding generator; ownership across the boundary; native or WASM mods |
| M9 | Tracing facade; profiler integration; remote inspection protocol; record/replay with desync detection | Protocol transport and encoding |
| Later | Release infrastructure (a `v*` tag ruleset, a protected `release` environment, crates.io trusted publishing), when the first release is planned; windowing and input; 2D then 3D rendering; assets; audio; physics; mod loading; optional app runner; the editor | Unnumbered until scheduled |

## Milestone numbering note

Documents written before this roadmap (the M0+M1 spec, ADRs 0001 to 0011, `benchmarks.md`; all dated up to 2026-11-12) use the old milestone numbers. Read them through this table.

| Old | New |
|---|---|
| M0+M1 | M0+M1 |
| "start of M2" infrastructure (M0+M1 spec §4.12): Miri, fuzzing, Gungraun, coverage, mutation triage | M2 (Miri deferred, see above) |
| M2 (ECS storage spike and decision) | M3 |
| M2 (ECS, registry, dogfooding sample, reflection, persistence, change detection) | M4, M5, M6 |
| M3 (scheduler) | M7 |
| M4 (C ABI and C#) | M8 |
| M5 (diagnostics and tooling) | M9 |

Going public is not a milestone. It is the checklist in [maintainers.md](maintainers.md), run at a milestone boundary the maintainer chooses.
