# M3.2: Spike harness

- **Status:** Implemented (PR #36, 2026-11-14). Accepted (2026-11-14); amended before implementation (2026-11-14): one directory per spike topic (§4.1, §4.7, §4.8), and R8 narrowed because `add_remove`'s step is idempotent; amended 2026-11-26 by [M3.26](../m3.26-workload-checks-that-fail/spec.md): the `frag_iter` and `heavy_compute` steps (§4.4.2, §4.4.3), so their state changes every tick, and R8 points to M3.26's per-step `add_remove` test.
- **Milestone:** M3 (ECS storage decision) · **Depends on:** M3.0, M3.1 · **Decides:** no ADR
- **Estimated size:** about 1,500 changed lines, excluding tests, lockfiles and generated files (`spike_ecs_harness` about 450, `spike_ecs_vec` about 550, the two bench crates about 270, xtask about 60, documents about 170)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds `spikes/`, a home for spikes with one directory per topic, and its first spike, `spikes/ecs_storage/`: a harness that defines seven storage workloads precisely enough for any storage backend to reproduce them, a conformance check that compares a backend's checksums with a reference backend's tick by tick, the plain-`Vec` reference backend (array-of-structs for every workload, plus a struct-of-arrays variant of the agent workload), and two benchmark crates, one for wall-clock time (Criterion) and one for instruction counts and simulated cache misses (Gungraun with Callgrind's cache simulator). It comes now because every storage prototype (M3.3 to M3.7), the external ECS baselines (M3.10, split out of this chunk) and the decision ADRs (M3.8) are measured with it. At the end, `cargo xtask test` runs the conformance and rule tests, `cargo xtask spike-bench ecs` prints wall-clock numbers for the `Vec` backends, and `cargo bench -p spike_ecs_bench_cache` in the devcontainer prints cache-simulation numbers.

## 2. Goals and non-goals

**Goals**

- Seven workloads (§4.4), each defined by normative rules, a draw order for initial values, and a per-entity hash, so that a backend that follows the rules produces the reference's checksum exactly.
- One interface, the `Workload` trait (§4.2), that every backend implements once per workload, in that backend's own idiom. No backend is measured through an adapter.
- A conformance check that reports the first tick at which a candidate diverges from the reference (§4.3).
- The plain-`Vec` backends: the hand-written best case that each prototype's cost is read against (§4.5).
- Wall-clock and cache-simulation benchmarks over every workload, size and backend, with one entry point each (§4.6).
- Rules for crates under `spikes/` recorded in CONTRIBUTING.md (§4.8).

**Non-goals**

- No bevy_ecs or hecs baselines. They move to a new chunk, M3.10, which adds them as backends and owns their dependency trees.
- No ECS storage prototype: M3.3 onwards.
- No generic ECS trait (spawn, insert, query over component tuples). Each backend writes each workload directly (§7).
- No struct-of-arrays variant of any workload other than `agents`.
- No results recorded in `docs/benchmarks.md`: that is M3.8's, under the benchmark run protocol. The pull request shows one informal run.
- No CI gate on spike numbers. Neither benchmark crate fails a pull request on a measurement.
- No parallelism: every workload is single-threaded. Parallel execution is M7's.
- No change to `libs/`, no frozen format, no ADR.

## 3. Guide-level explanation

A backend is a crate under `spikes/ecs_storage/` that implements `Workload` for each workload's spec type. A sketch of the `add_remove` workload in the `Vec` backend:

```rust
use engine::prelude::*;
use spike_ecs_harness::{Params, Workload, add_remove::{self, AddRemove, A, B}};

pub struct VecAddRemove { entities: Vec<(A, Option<B>)> }

impl Workload for VecAddRemove {
    type Spec = AddRemove;
    const BACKEND: &'static str = "vec_aos";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let entities = (0..params.count as u64)
            .map(|i| (add_remove::spawn(&prng, i), None))
            .collect();
        Self { entities }
    }

    fn step(&mut self) {
        for (a, b) in &mut self.entities { *b = Some(B(a.0)); }
        for (_, b) in &mut self.entities { *b = None; }
    }

    fn checksum(&self) -> u64 {
        self.entities.iter().fold(0, |sum, (a, b)| sum.wrapping_add(add_remove::hash(a, b.as_ref())))
    }
}
```

The backend's tests check it against the reference, one line per workload:

```rust
#[test]
fn add_remove_matches_the_reference() {
    spike_ecs_harness::check_conformance::<spike_ecs_vec::VecAddRemove, my_backend::AddRemove>(
        Params { count: 1_000, seed: 42 },
        120,
    )
    .unwrap();
}
```

A failure reads, for example, `add_remove: my_backend diverges from vec_aos at tick 1 (expected 0x…, got 0x…)`. Tick 0 means the state right after `setup`.

To make the benchmarks see the backend, add one line per workload to each bench crate's backend list. Then:

```sh
cargo xtask spike-bench ecs              # wall clock, every workload, size and backend (macOS or Linux)
cargo xtask spike-bench ecs --save m3-run1 # the same, saved as a Criterion baseline
cargo bench -p spike_ecs_bench_cache     # in the devcontainer: instructions, L1/LL misses, estimated cycles
```

Criterion's report groups the results as `spike/ecs/<workload>/<backend>/<count>`, so backends for the same workload and size sit side by side.

## 4. Reference-level design

### 4.1 Layout

```
spikes/
  README.md                       # the rules for every spike, and the list of spikes
  ecs_storage/                    # this spike (M3); README.md: the workloads, adding a backend, the benches
    harness/                      # package spike_ecs_harness (library)
      src/lib.rs                  # Params, Spec, Workload, Fnv1a, check_conformance, Divergence, dt
      src/{simple_iter, frag_iter, heavy_compute, add_remove, entity_churn, sprites, agents}.rs
    vec/                          # package spike_ecs_vec (library): the reference backends
      src/lib.rs, one module per workload
    bench_compare/                # package spike_ecs_bench_compare: Criterion, benches/compare.rs
    bench_cache/                  # package spike_ecs_bench_cache: Gungraun, benches/cache.rs
```

`spikes/` holds one directory per spike topic, so later spikes that have nothing to do with ECS storage get their own `spikes/<topic>/`. Every directory one level below a topic is a workspace package (the workspace `members` glob is `spikes/*/*`), and its package name is `spike_<short topic>_<name>`; this spike's short topic is `ecs`. Later backends of this spike are added as sibling directories (`spikes/ecs_storage/sparse_set`, `spikes/ecs_storage/archetype`, …). Every crate root carries `#![forbid(unsafe_code)]` until M3.7, which is the only chunk that may change that.

`spike_ecs_harness` depends on `engine` and on `agents_2d`'s library (for the agent rules, `spawn_agent`, `agent_hash` and `Input::scripted`). `agents_2d` depends on `winit` and `softbuffer`, so they enter the spike crates' dependency tree. They are already in the workspace's tree and are never called by spike code (§7).

### 4.2 The interface

```rust
/// The size and seed of one workload instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Params {
    /// The number of entities, or for `frag_iter`, the number of entities per letter.
    pub count: usize,
    /// The seed of the `Prng` whose streams give every entity's initial values.
    pub seed: u64,
}

/// A workload's identity. Implemented by one marker type per workload module.
pub trait Spec {
    /// The workload's name, used in benchmark ids and conformance messages: `"simple_iter"`, ….
    const NAME: &'static str;
    /// The values of `Params::count` the benchmarks run.
    const SIZES: &'static [usize];
}

/// One backend's implementation of one workload.
pub trait Workload: Sized {
    /// The workload this implements.
    type Spec: Spec;
    /// The backend's name, used in benchmark ids and conformance messages: `"vec_aos"`, ….
    const BACKEND: &'static str;
    /// Builds the initial state. Not measured.
    fn setup(params: Params) -> Self;
    /// Advances the workload by one tick. The measured unit.
    fn step(&mut self);
    /// The order-independent checksum of the current state (§4.4).
    fn checksum(&self) -> u64;
}
```

- `setup` MUST create the entities with spawn indices `0..n` (where `n` is the workload's initial population) and the initial values that §4.4 gives for each index.
- `step` MUST apply the workload's rules exactly, including the order of floating-point operations. The order in which entities are visited and stored is the backend's choice: no rule for one entity reads another.
- A workload whose rules spawn entities during `step` MUST give them the next unused spawn indices, in increasing order. Entities spawned during a step are not updated in that step.
- `checksum` MUST be the wrapping sum, over every live entity, of the workload's per-entity hash (§4.4), plus any extra terms the workload names. It MUST NOT depend on storage order.
- `step` MUST NOT read wall-clock time, thread ids or any other ambient state.

`dt()` returns `FixedTimestep::from_hz(60).step().as_secs_f32()`, the step length `agents_2d` uses. §4.4 writes it `DT`; it is a function because `FixedTimestep` is not usable in a constant.

`Fnv1a` is a small builder over the 64-bit FNV-1a hash with the constants of M3.0 §4.3 (offset basis `0xcbf2_9ce4_8422_2325`, prime `0x0000_0100_0000_01b3`). It has `new`, `u8`, `u32`, `u64`, `f32` (hashes the 4 little-endian bytes of `to_bits`), `vec2`, `vec3` (the components in x, y, z order) and `finish`. Every per-entity hash in §4.4 is defined in terms of it, and each workload module exports its hash function, so backends do not write their own.

### 4.3 Conformance

```rust
pub fn check_conformance<R, C>(params: Params, ticks: u64) -> Result<(), Divergence>
where
    R: Workload,
    C: Workload<Spec = R::Spec>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergence {
    pub workload: &'static str,
    pub reference: &'static str,
    pub candidate: &'static str,
    /// 0 for the state after `setup`, otherwise the number of steps run.
    pub tick: u64,
    pub expected: u64,
    pub actual: u64,
}
```

`check_conformance` sets up both workloads with `params`, compares their checksums, then steps both `ticks` times and compares after every step. It returns the first `Divergence`, or `Ok(())`. `Divergence` implements `Display` with the message format of §3 and `std::error::Error`. The function does not panic, so tests can check that it detects divergence.

The reference for every workload is the `vec_aos` backend (§4.5). For `agents`, the `vec_aos` backend is a wrapper around `agents_2d::Sim`, so the existing golden checksum in `examples/agents_2d/tests/determinism.rs` also pins the reference.

### 4.4 Workloads

Common rules:

- Entity `i`'s initial values come from `r = Prng::from_seed(params.seed).stream(i)`, drawn in the order listed. Each module exports `spawn(prng: &Prng, i: u64)`, which returns entity `i`'s components; `agents` uses `agents_2d::spawn_agent`. Ranges use `Prng::range`; `a..b` is half-open and `a..=b` closed.
- Component types are defined in the workload's module in `spike_ecs_harness`, so every backend stores the same types. Each derives `Clone`, `Copy`, `Debug` and `PartialEq` where its fields allow.
- No rule uses a function on the transcendental ban list. Vector and matrix operations are glam's, named here by method.
- Hashes are written as the sequence of `Fnv1a` calls, starting from `Fnv1a::new()` and ending in `finish`.

#### 4.4.1 `simple_iter`

Sizes 1,000, 10,000 and 100,000.

- **Components.** `Matrix(Mat4)`, `Position(Vec3)`, `Rotation(Vec3)`, `Velocity(Vec3)`. Each entity has all four: 100 bytes, of which the step reads 24 and writes 12.
- **Spawn.** Position: 3 draws in `-100.0_f32..100.0` (x, y, z). Rotation: 3 draws in `-1.0_f32..1.0`. Velocity: 3 draws in `-1.0_f32..1.0`. Matrix: `Mat4::IDENTITY`, no draws.
- **Step.** For every entity: `position.0 = position.0 + velocity.0`.
- **Hash.** `vec3(position)`, `vec3(velocity)`.

#### 4.4.2 `frag_iter`

Size 20 per letter (520 entities).

- **Components.** 26 letter types `A` to `Z`, each `struct X(pub f32)`, and `Data(f32)`. Entity `i` has letter `i / count` (0 is `A`) and `Data`, so the entities fall into 26 groups that differ only in their letter type.
- **Spawn.** For `i` in `0..26 * count`: the letter's value, one draw in `0.0_f32..1.0`; then `Data`, one draw in `0.0_f32..1.0`.
- **Step.** For every entity with `Data`: `x = data.0 + 0.618_034`, then `data.0 = x - 1.0` if `x >= 1.0`, else `x`. Values stay in `0.0..1.0`. *Amended 2026-11-26 ([M3.26](../m3.26-workload-checks-that-fail/spec.md) §4.2): the step was `data.0 = data.0 * 0.5 + 1.0`. Every value settled on one number by tick 26, so the checksum stopped changing and conformance could not see a skipped or repeated step.*
- **Hash.** `u8(letter index)`, `f32(letter value)`, `f32(data)`.

#### 4.4.3 `heavy_compute`

Size 1,000.

- **Components.** As `simple_iter`.
- **Spawn.** Position, rotation and velocity as `simple_iter`. Then the matrix: for column `c` in 0 to 2, for row `r` in 0 to 2, one draw in `2.0_f32..4.0` if `r == c`, else in `-0.5_f32..0.5`. Then 3 draws in `-10.0_f32..10.0` for the translation (column 3, rows 0 to 2). Row 3 is `(0, 0, 0, 1)`. The 3 by 3 part is strictly diagonally dominant, so the matrix is invertible.
- **Step.** For every entity: 100 times, `matrix.0 = matrix.0.inverse()` (written back each time); then, for each row `r` in 0 to 2, `translation[r] = translation[r] + velocity.0[r] * dt` (multiply, then add; not fused), where `translation` is column 3 of the matrix and `dt` is `spike_ecs_harness::dt()`; then `position.0 = matrix.0.transform_point3(rotation.0)`. The translation accumulates; it is finite over the conformance run ([M3.26](../m3.26-workload-checks-that-fail/spec.md) §4.1). Inversion dominates the cost, so storage overhead should barely show here. *Amended 2026-11-26 ([M3.26](../m3.26-workload-checks-that-fail/spec.md) §4.2): the step had no translation move. An even number of inversions returned 963 of 1,000 matrices bit for bit by about tick 20, so only rounding drift in the other 37 changed the checksum.*
- **Hash.** `vec3(position)`, then the 16 matrix elements as `f32` in column-major order.

#### 4.4.4 `add_remove`

Sizes 1,000, 10,000 and 100,000.

- **Components.** `A(f32)`, `B(f32)`. Every entity has `A`; none has `B` after `setup` or after a step.
- **Spawn.** `A`: one draw in `0.0_f32..1.0`.
- **Step.** First, for every entity, insert `B(a.0)`. Then, for every entity, remove `B`. The two passes MUST be separate: every insert happens before the first removal.
- **Hash.** `f32(a)`, then `u8(0)` without `B`, or `u8(1)` and `f32(b)` with it. A backend that leaves a `B` behind changes the checksum.

#### 4.4.5 `entity_churn`

Sizes 1,000, 10,000 and 100,000.

- **Components.** `SpawnId(u64)`, `Position(Vec2)`, `Velocity(Vec2)`, `Lifetime(u32)`.
- **Spawn.** `SpawnId(i)`. Position: x in `0.0_f32..320.0`, then y in `0.0_f32..180.0`. Velocity: x, then y, each in `-1.0_f32..1.0`. Lifetime: one draw in `1_u32..=30`.
- **Step.** For every entity: `position.0 = position.0 + velocity.0`, then `lifetime.0 = lifetime.0 - 1`. Then despawn every entity whose lifetime is 0, and spawn entities with the next spawn indices until `count` are alive. Lifetimes average 15.5 ticks, so about 6.5% of entities are replaced per tick and the backend's entity ids are reused.
- **Hash.** `u64(spawn id)`, `vec2(position)`, `vec2(velocity)`, `u32(lifetime)`.

#### 4.4.6 `sprites`

Size 100,000.

- **Components.** `Transform2d` (from `engine::math`), `Velocity2d(Vec2)`, `Sprite { color: u32, half_extent: f32 }`, and the tag `Hidden`, which carries no data.
- **Spawn.** Translation: x in `0.0_f32..320.0`, then y in `0.0_f32..180.0`; rotation and scale stay at `Transform2d::IDENTITY`'s. Velocity: x, then y, each in `-20.0_f32..20.0`. Color: `r.next_u32() & 0x00FF_FFFF`. Half extent: one draw in `0.25_f32..2.0`. Hidden: `r.chance(0.1)`. Whether an entity is hidden never changes.
- **Step.** First, for every entity (hidden or not): `t = t + v * DT` on the translation (x then y, as `Vec2` operations), then wrap each axis with bound `B` (320 for x, 180 for y): if `p < 0.0`, `p = p + B`; else if `p >= B`, `p = p - B`. Then clear the draw list and push one `DrawItem { position, half_extent, color }` for every entity without `Hidden`. The draw list is a `Vec<DrawItem>` the workload owns; `setup` reserves `count` items in it. `DrawItem` is defined in the harness.
- **Hash.** Per entity: `vec2(translation)`, `vec2(velocity)`, `u32(color)`, `f32(half_extent)`, `u8(hidden as u8)`. Per draw item: `vec2(position)`, `f32(half_extent)`, `u32(color)`. The checksum is the wrapping sum of both.

#### 4.4.7 `agents`

Sizes 10,000 and 100,000.

- **Rules.** Exactly M3.0 §4.3 (spawn with `agents_2d::spawn_agent`, the four rules per agent, then removal and respawn), with `Params::count` as the agent count and `Params::seed` as the seed.
- **Step.** One step with `agents_2d::Input::scripted(tick)`, where `tick` is the number of steps already run by this instance.
- **Hash.** `agents_2d::agent_hash`.

### 4.5 The `Vec` backends

`spike_ecs_vec` is the reference and the best case. It is not an ECS: each workload keeps one `Vec` of plain structs and writes the loop a programmer would write by hand.

- **`vec_aos`** implements all seven workloads. In `frag_iter` the letter is an enum field. In `add_remove` `B` is an `Option<B>` field. In `entity_churn` and `sprites`, removal uses `retain`, and spawns are pushed. Its `agents` implementation wraps `agents_2d::Sim` and calls `Sim::step` and `Sim::checksum`.
- **`vec_soa`** implements `agents` only. It keeps one `Vec` per field (ids, `Transform2d`, velocity, ttl, `Option<Seeker>`), applies the M3.0 §4.3 rules itself, removes expired agents from every column with the same operation at the same index (for example `swap_remove`), and hashes by building an `agents_2d::Agent` on the stack for each index and calling `agent_hash`.

The two backends differ only in layout, so their ratio is the measured cost of array-of-structs against struct-of-arrays for that workload.

### 4.6 Benchmarks

**`spike_ecs_bench_compare`** (Criterion, `harness = false`):

- For each workload, each size in `Spec::SIZES` and each backend, one benchmark with id `spike/ecs/<NAME>/<BACKEND>/<count>`, seed 42.
- The workload is set up once per benchmark, outside the timed region; the timed routine is one `step`.
- Warm-up 1 s and measurement 3 s, as the M1 scenario groups use. These MAY be tuned in M3.8, which records the settings it used.
- One generic function, `bench::<W: Workload>(c)`, registers a benchmark, so adding a backend is one call per workload.

**`spike_ecs_bench_cache`** (Gungraun, `harness = false`):

- For each workload, size and backend, one library benchmark whose setup runs `setup` and one untimed `step` (so buffers have reached their steady size), and whose measured function is one `step` behind an `#[inline(never)]` wrapper, as CONTRIBUTING.md's `bench-regress` section asks.
- Callgrind runs with its cache simulator on (`--cache-sim=yes`). Gungraun then reports instructions, L1 hits, LL hits, RAM hits and estimated cycles for each benchmark.
- It is run by hand, in the devcontainer or any Linux machine with Valgrind and `gungraun-runner`. It is not a gate, and `bench-regress` does not look at it.

### 4.7 xtask

- **`spike-bench <topic> [--save <name>] [--allow-battery]`** runs `cargo bench -p spike_<topic>_bench_compare --bench compare`, passing `--save-baseline <name>` when asked. `ecs` is the only topic now; an unknown topic is an error that lists the known ones. It applies the same AC-power and system-load checks as `bench`, with the same `--allow-battery` escape.
- **`bench`** excludes `spike_ecs_bench_compare` and `spike_ecs_bench_cache`, so the M1 suite and the baselines `bench-doc` reads are unchanged. `bench-doc` MUST NOT include any benchmark whose id starts with `spike/`.
- **`bench-build`** includes `spike_ecs_bench_compare` and excludes `spike_ecs_bench_cache`, as it excludes `engine_bench_regress`.
- **`bench-smoke`** includes both. Criterion's `--test` mode runs each Criterion benchmark once; the Gungraun benchmarks run once without Valgrind, as `engine_bench_regress`'s do.
- **`coverage`** excludes the spike packages, as it excludes `engine_benches` and `engine_bench_regress`. `mutants` is unchanged: its triaged set is an explicit list.
- **`check-unsafe`** and **`check-statics`** cover the spike crates. If either finds crates through a list of directories rather than through the workspace, `spikes/` is added to that list.

### 4.8 Documents

- **`docs/roadmap.md`:**
  - M3.2's title becomes "Spike harness under `spikes/ecs_storage/`: the `Workload` interface and conformance check, seven workloads (simple and fragmented iteration, heavy compute, add/remove, entity churn, 100k 2D sprites, the M3.0 agent workload), the plain-`Vec` backends (array-of-structs, and struct-of-arrays for agents), Criterion and Gungraun cache-simulation benches".
  - Add the row M3.10, "bevy_ecs and hecs baselines as harness backends", depending on M3.2.
  - M3.8 depends on "M3.0 to M3.7, M3.10".
- **`CONTRIBUTING.md`:** a "Spike crates" section saying that crates under `spikes/` follow the rules of the bench crates. They are workspace members under the workspace lints, `forbid(unsafe_code)` (until an ADR says otherwise, M3.7), `deny` and the determinism lints. Their public items are documented. They are exempt from "every public type has a runnable doctest" and from the `try_` twin rule, and they are outside coverage and mutation testing. The section also says that a backend must pass `check_conformance` for every workload it implements before its numbers are quoted.
- **`spikes/README.md`:** what a spike is, the rules every spike crate follows (pointing to CONTRIBUTING.md), the layout and naming of §4.1, and a list of spikes with their milestone.
- **`spikes/ecs_storage/README.md`:** what a backend is, how to add one (crate, `Workload` impls, conformance tests, one line per workload in each bench crate), and the two bench commands.
- **`README.md`:** a line for `spikes/` in the layout paragraph.
- **`CHANGELOG.md`:** an entry.

## 5. Requirements

- **M3.2-R1** `spikes/ecs_storage/harness`, `spikes/ecs_storage/vec`, `spikes/ecs_storage/bench_compare` and `spikes/ecs_storage/bench_cache` MUST be workspace members, and each crate root MUST carry `#![forbid(unsafe_code)]`.
- **M3.2-R2** `spike_ecs_harness` MUST provide `Params`, `Spec`, `Workload`, `Fnv1a`, `dt`, `check_conformance` and `Divergence` as specified in §4.2 and §4.3.
- **M3.2-R3** `check_conformance` MUST compare checksums after `setup` and after every step, and MUST return the first divergence with the tick, both checksums and the three names; it MUST NOT panic on divergence.
- **M3.2-R4** Each of the seven workload modules MUST define the components, spawn function, hash function and `Spec` marker of §4.4, with `Spec::SIZES` as listed there.
- **M3.2-R5** The `vec_aos` backend MUST implement all seven workloads according to §4.4, and its `agents` implementation MUST wrap `agents_2d::Sim`.
- **M3.2-R6** The `vec_soa` backend MUST implement `agents` and MUST pass `check_conformance` against `vec_aos` for 1,000 agents over 1,300 ticks (which covers respawns and a period without a cursor).
- **M3.2-R7** Each `vec_aos` workload's checksum MUST NOT depend on storage order: permuting its storage MUST NOT change the checksum, after setup or after further steps.
- **M3.2-R8** `check_conformance` MUST detect, reporting tick 1: a candidate that skips one entity's update in `simple_iter` and in `entity_churn`; one that skips one entity's removal of `B` in `add_remove` (its step is idempotent, so a skipped insert or a repeated step leaves the same state and is not detectable; [M3.26](../m3.26-workload-checks-that-fail/spec.md) §4.3 adds a per-step test of the insert pass on every ECS backend); and one that applies `step` twice in `simple_iter` and in `entity_churn`.
- **M3.2-R9** Each workload's rules MUST have a test on `vec_aos`, as listed in §8.
- **M3.2-R10** `spike_ecs_bench_compare` MUST register one benchmark per workload, size and backend with the id of §4.6, timing only `step`.
- **M3.2-R11** `spike_ecs_bench_cache` MUST register one Gungraun benchmark per workload, size and backend, with Callgrind's cache simulation on, measuring only `step`.
- **M3.2-R12** `cargo xtask spike-bench ecs` MUST behave as §4.7 says, including the power and load checks; `bench` and `bench-doc` MUST exclude the spike benchmarks, and `bench-build`, `bench-smoke`, `coverage`, `check-unsafe` and `check-statics` MUST treat the spike packages as §4.7 says.
- **M3.2-R13** The change MUST pass `cargo xtask deny` without changes to `deny.toml`.
- **M3.2-R14** `cargo xtask ci` MUST pass, and CI MUST be green on Ubuntu, macOS and Windows without workflow changes.
- **M3.2-R15** The documents of §4.8 MUST be updated as described.
- **M3.2-R16** Every public item in the spike crates MUST be documented.

## 6. Cross-cutting concerns

- **Determinism:** conformance is checked within one build. Backends must agree with the reference when both run in the same binary, which is the determinism the project promises ([ADR 0003](../../adr/0003-same-build-determinism.md)). No new golden constants are added, because `heavy_compute` uses glam's `Mat4`, which is SIMD-backed and may round differently on x86_64 and aarch64. The agent workload stays pinned across platforms by `agents_2d`'s golden checksum, through the `vec_aos` wrapper. Initial values come from `prng.stream(spawn index)`, so they never depend on storage order. Checksums are wrapping sums, so they never depend on iteration order. No workload uses a transcendental function or ambient state.
- **`unsafe`:** none. Every spike crate root forbids it. M3.7 is the chunk that proposes the first exception.
- **Concurrency:** n/a, because every workload is single-threaded.
- **Heap allocation:** not checked in this chunk. `setup` may allocate freely. The `Vec` backends reserve their populations, and `sprites` reserves its draw list, so their steady-state steps should not allocate. If a backend allocates in `step`, both bench crates measure it, and M3.8 reports it if it matters.
- **`no_std` and wasm:** n/a, because the spike crates are not in the `no-std` or `wasm` builds, which name their packages.
- **Frozen formats:** none. The per-entity hashes are test definitions, not formats, and may change with this spec.
- **Public API changes:** none in `libs/`. The spike crates' APIs are used only by the spike crates.
- **FFI exposure:** n/a.
- **Performance:** this chunk builds the measuring tools and records no results. The pull request shows one informal `spike-bench` run, labelled as such. M3.8 records results under the benchmark run protocol, with saved baselines and the noise floor.
- **Documentation:** item docs (R16), `spikes/README.md` and `spikes/ecs_storage/README.md`, the CONTRIBUTING.md section, the roadmap, README and CHANGELOG changes (§4.8), and this spec and its plan.
- **Policy changes:** one. CONTRIBUTING.md gains the rules for spike crates (§4.8), including their exemption from the doctest and `try_` twin rules. The maintainer agreed to it during brainstorming (2026-11-14).

## 7. Alternatives considered

- **A generic ECS trait** (spawn, despawn, typed insert and remove, queries over component tuples), with the workloads written once against it. Rejected: typed mutable multi-component queries are hard to express in safe Rust. Each backend would also be measured through an adapter, whose cost would land in the numbers that M3.8 decides on.
- **A hybrid:** a shared trait for structural operations, with native iteration per backend. Rejected: it gives two concepts to learn instead of one, and it still puts an adapter in the measurement of `add_remove` and `entity_churn`, the workloads where storage models differ most.
- **bevy_ecs and hecs in this chunk**, as the roadmap first listed. Rejected for size: together they would take the chunk past 2,000 lines. They become M3.10, which can run alongside M3.3 and M3.4.
- **Golden checksums for every workload.** Rejected: glam's SIMD types may round differently between architectures. Cross-platform goldens would then fail for reasons that say nothing about storage, and same-build agreement is the property the decision needs.
- **Criterion only.** Rejected: the storage decision is about memory layout, and [the M0+M1 spec](../m0-m1-foundations/spec.md) §4.11 asks for cache-simulation metrics. Wall-clock time on one laptop cannot separate cache misses from other effects.
- **The cache-simulation bench in M3.8.** Rejected: every prototype chunk would then be measured on wall-clock time alone until the end of the milestone.
- **One bench crate holding both the Criterion and the Gungraun benches.** Rejected: xtask excludes Gungraun benches by package (`bench-build`, `bench`), so the two need separate packages, as `bench/compare` and `bench/regress` are.
- **Spike crates outside the workspace**, as `fuzz/` is. Rejected: CI would no longer run the conformance tests on every pull request, so prototypes could stop matching the reference without anyone noticing.
- **Full library rules for spike crates.** Rejected: doctests on every type and `try_` twins for prototype code are ceremony without readers. Conformance tests are what keep spike code honest.
- **One `Params` type per workload.** Rejected: every workload needs only a count and a seed.
- **A struct-of-arrays variant of every workload.** Rejected by the maintainer: the roadmap asks for it only for `agents`, and the prototypes are where layouts are compared.
- **`frag_iter` also at a larger size**, for example 1,000 per letter. Rejected by the maintainer: at 20 per letter, the workload measures the per-group overhead it exists to show. `simple_iter`'s sizes already cover scale.
- **Making `winit` and `softbuffer` optional in `agents_2d`**, behind a default feature, so the spike crates do not pull them in. Rejected: both are already in the workspace's tree and spike code never calls them. The change to the example would buy nothing measurable.
- **A flat `spikes/` with one directory per crate.** Rejected by the maintainer: later spikes unrelated to ECS storage would mix with this spike's crates. One directory per topic keeps each spike's crates, README and benches together.
- **Benchmarking `setup` (bulk insertion).** Rejected: it is not in the roadmap's workload list, and `entity_churn` measures spawning in the steady state, which is the case a simulation runs.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | the workspace `Cargo.toml`; `cargo xtask check-unsafe` |
| R2 | compilation of the backends and tests against the harness; review against §4.2 and §4.3 |
| R3 | harness unit tests: `check_conformance` of a workload against itself returns `Ok`; against a candidate whose `setup` differs it returns tick 0; the `Display` message contains every field |
| R4 | review against §4.4; spawn tests: each module's spawn function for index `i` equals direct draws from `prng.stream(i)` in the listed order |
| R5 | `vec_aos` rule tests (R9); the `agents` wrapper's checksum equals `Sim::checksum` |
| R6 | `spike_ecs_vec` test `vec_soa_agents_matches_vec_aos` |
| R7 | one `spike_ecs_vec` test per workload: clone, reverse one clone's storage after setup and after 10 steps, compare checksums after every one of 20 further steps |
| R8 | `spike_ecs_vec` tests with the broken candidates defined in its test module: a skip-one variant of each of the three workloads, and a generic `StepTwice<W>` |
| R9 | `vec_aos` unit tests. `simple_iter`: position moves by velocity, and matrix and rotation are unchanged. `frag_iter`: each of the 26 letters keeps `count` entities, and data follows `x = d + 0.618_034`, minus 1.0 if `x >= 1.0` (amended 2026-11-26 by [M3.26](../m3.26-workload-checks-that-fail/spec.md); was `d * 0.5 + 1.0`). `heavy_compute`: every spawned matrix's determinant is non-zero, and position equals `matrix.transform_point3(rotation)` after a step. `add_remove`: no entity has `B` after a step. `entity_churn`: the population stays at `count`, spawn ids are unique and increasing among newly spawned entities, and no lifetime is 0 after a step. `sprites`: every translation is inside the world after a step, and the draw list holds exactly the entities without `Hidden`. `agents`: the wrapper and `Sim` agree |
| R10 | `cargo xtask spike-bench ecs` output, pasted into the pull request; `cargo xtask bench-smoke` |
| R11 | `cargo bench -p spike_ecs_bench_cache` in the devcontainer, output pasted into the pull request; `cargo xtask bench-smoke` |
| R12 | xtask unit tests of the argument lists (as for `bench_args`) and of `spike-bench`'s flag parsing; a `bench-doc` unit test that a `spike/` benchmark id is skipped; `cargo xtask coverage` and `check-unsafe` output naming (or not naming) the spike packages as §4.7 says |
| R13 | `cargo xtask deny` |
| R14 | `cargo xtask ci`; the pull request's CI run |
| R15, R16 | documentation review (CONTRIBUTING.md "Reviews before a pull request"); `cargo xtask doc` |

## 9. Done when

- [x] `cargo xtask ci` passes locally, and CI is green on all three platforms (PR #36: every check passes, including the macOS and Windows `os-matrix` jobs).
- [x] Every `vec_aos` workload passes its rule, order-independence and broken-candidate tests, and `vec_soa` agents passes conformance against `vec_aos` (`spike_ecs_vec`: 47 tests, including `vec_soa_agents_matches_vec_aos`).
- [x] `cargo xtask spike-bench ecs` runs on macOS, and `cargo bench -p spike_ecs_bench_cache` runs under Valgrind on Linux. Both outputs are in the pull request, labelled as informal. (The cache bench ran in a throwaway `rust:1-bookworm` container, not the devcontainer: see the plan's Deviations.)
- [x] The roadmap shows the narrowed M3.2, the new M3.10 and M3.8's new dependency. CONTRIBUTING.md, README.md, CHANGELOG.md, `spikes/README.md` and `spikes/ecs_storage/README.md` are updated.
- [x] The eight reviews in CONTRIBUTING.md are recorded in the pull request.
- [x] The spec is `Implemented` and the plan `Historical`, with Deviations filled in.

## 10. Open questions

None.

## 11. Prior art and sources

- The [`ecs_bench_suite`](https://github.com/rust-gamedev/ecs_bench_suite) of the Rust gamedev working group. `simple_iter`, `frag_iter`, `heavy_compute` and `add_remove` follow the designs of its benchmarks of the same names (component shapes, 26 letter types with 20 entities each, 100 matrix inversions). The rules here are restated so that they are deterministic and checkable, and no code is taken from it ([ADR 0002](../../adr/0002-originality-and-dependency-licensing.md)).
- Michele Caini (skypjack), [ECS back and forth](https://skypjack.github.io/2019-02-14-ecs-baf-part-1/): sparse-set and archetype storage, and why structural changes and fragmented iteration separate them.
- The [Valgrind Callgrind manual](https://valgrind.org/docs/manual/cl-manual.html), on `--cache-sim=yes` and the L1 and LL cache model it simulates.
- [Gungraun documentation](https://docs.rs/crate/gungraun/): library benchmarks, setup functions excluded from the counts, and Callgrind arguments.
- [Criterion.rs user guide](https://bheisler.github.io/criterion.rs/book/): benchmark groups, `BenchmarkId` and saved baselines.
- [M3.0 spec](../m3.0-windowed-baseline/spec.md) §4.3: the agent rules and the order-independent checksum this harness generalizes.
