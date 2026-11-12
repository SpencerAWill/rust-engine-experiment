# M0+M1: Engineering Foundations and Core Primitives — Design Spec

- **Date:** 2026-11-11
- **Status:** Draft, awaiting review
- **Working name:** `engine` (crates `engine_*`); rename before any publishing.
- **Scope:** First sub-project of the engine. Establishes the workspace, engineering policy, CI, test and benchmark infrastructure (M0), plus the first real crates: generational handles, deterministic RNG, simulation time, math (M1).

---

## 1. Context and goals

We are building a high-performance, real-time game/simulation engine in Rust as a set of libraries plus samples. It follows data-oriented design and will be ECS-based. It will expose a stable C ABI so games, sims, and apps can be written in other languages (C# first).

Project trajectory: learning/experimentation first → foundation for the author's own projects → possibly open source. Consequences:

- Invest early in boundaries that are expensive to change later: data layout, threading model, the C ABI, the `unsafe` policy, determinism.
- Keep the public API at 0.x (explicitly unstable) for the foreseeable future.
- Hold to open-source-grade engineering hygiene from day one (docs, tests, licensing, CI), so a future release is a decision rather than a rewrite.

### Settled decisions (from brainstorming)

| Topic | Decision |
|---|---|
| First workload | Headless simulation core first; rendering layered on later |
| Interop | Stable C ABI first, C# as the first real binding; C++/Python/Go possible later |
| Platforms | macOS (native), Linux (devcontainer + CI), Windows. Web/mobile/console are desirable later and must not be precluded |
| Determinism | **Same-build determinism**: same binary + same platform + same inputs ⇒ identical results. Not cross-platform bit-exact |
| Build vs buy | Own the core (ECS, scheduler, FFI layer, RNG, handles, time). Reuse mature crates at the edges (`glam`, later `winit`, `wgpu`, …) |
| Logic style (when the ECS arrives) | Systems over queries; read/write access sets drive parallelism. No entity relationships in the first ECS milestone |
| `unsafe` | Forbidden by default; named, audited exception crates only (see §4.3) |
| Originality | **No code copied from Bevy or any other project.** Designs, papers, talks, and blog posts may be studied and cited; all implementation is original. Algorithms are implemented from their published specifications |
| Crate granularity | Many fine-grained crates, layered, plus a facade crate. Split wherever a real boundary exists (§3.1) |
| License | Dual `MIT OR Apache-2.0` |

### Deliberately deferred (decided in M2 with evidence)

- ECS storage model: sparse sets (EnTT-style) vs archetypes (flecs/Unity DOTS-style) vs per-component hybrid (Bevy-style).
- Runtime component registration, i.e. whether C# code and mods can define their own component types. Moddability is a stated goal and pushes toward "yes".
- How foreign-defined components are stored: type-erased raw columns (needs `unsafe`) vs safe byte-blob columns with copy-in/copy-out.

Nothing in M0+M1 may constrain these choices.

---

## 2. Research summary

These findings shaped this spec and the roadmap. Sources are listed in §12.

1. **ECS storage is a trade-off.** Sparse sets make component add/remove cheap but iterate slower at scale. Archetypes iterate fastest but pay on composition changes. Bevy offers per-component choice (table default, sparse opt-in). skypjack (EnTT author) argues hybrid schemes "look better than they are". → M2 spike benchmarks all three on identical workloads.
2. **Interop forces runtime component identity.** Rust `TypeId` is not stable across compilations, so it cannot cross FFI. Bevy retrofitted runtime `ComponentId`s for scripting. Flecs is C-first with runtime components and language-agnostic reflection, and its .NET binding is blittable with no marshalling. → If C#/mods define components, the core identity model must be runtime IDs plus layout descriptors, with Rust generics as a typed layer on top.
3. **Scheduling.** Implicit ordering of conflicting systems produces nondeterminism. Bevy requires explicit ordering and reports ambiguities. Naughty Dog's fiber job system parks waiting jobs rather than blocking threads, uses atomic counters, and had no work stealing. → M3 concern; M1's `Rng::stream` is designed so randomness does not depend on execution order.
4. **Handles over pointers at boundaries.** Index plus generation handles are serializable, detect staleness, and are only dereferenced by the owning side, which is the right shape for FFI. → `engine_handle` in M1, with a stable 64-bit encoding.
5. **Determinism must be chosen up front.** Rapier offers only local determinism by default. Frontier's GDC 2024 talk shows cross-platform float determinism needs "extreme care" (they chose fixed-point). → We commit to same-build determinism and own our RNG so dependency updates cannot change output.
6. **Benchmarking.** The classic `ecs_bench_suite` is archived. divan lacks built-in baseline save/compare. Criterion supports saved baselines (critcmp compares them). Gungraun (formerly iai-callgrind) gives CI-stable instruction-count benchmarks. → Criterion locally, Gungraun in CI (§8).
7. **Old-engine mistakes to avoid.** Individually allocated objects, data scattered in memory, logic scattered across "a million components", and OO gameplay layers tightly coupled to data layout. → Data-oriented crates with plain data and free functions where possible, and stable handles instead of object references.

---

## 3. Workspace structure

```
Cargo.toml                 workspace: [workspace.package], [workspace.dependencies], [workspace.lints]
rust-toolchain.toml        pinned stable toolchain
rustfmt.toml               stable options only
deny.toml                  cargo-deny: licenses, advisories, bans, sources
unsafe-allowlist.toml      crates permitted to use `unsafe` (empty in M0+M1)
LICENSE-MIT                (renamed from LICENSE)
LICENSE-APACHE
libs/
  engine_handle/           generational handles, HandlePool<T>, HandleAllocator<T>   [no_std + alloc]
  engine_random/           deterministic PRNG                                         [no_std]
  engine_time/             Tick, FixedTimestep, TimeSource, clocks                    [no_std, `std` feature]
  engine_math/             glam re-exports, Transform, scalar helpers                 [std]
  engine/                  facade: re-exports + `engine::prelude`
examples/
  fixed_step_sim/          headless deterministic sample (binary crate)
benches/                   `engine_benches`: cross-library comparison benchmarks (publish = false)
xtask/                     developer task runner (publish = false)
fuzz/                      cargo-fuzz targets (excluded from workspace; nightly)
docs/
  adr/                     architecture decision records
  benchmarks.md            methodology, hardware, recorded results
  superpowers/specs/       design specs (this file)
.github/
  workflows/ci.yml         PR/push CI
  workflows/nightly.yml    scheduled fuzzing
  dependabot.yml           devcontainers + cargo + github-actions
```

Workspace members: `libs/*`, `examples/*`, `benches`, `xtask`. `fuzz` is excluded.

All crates: `version = "0.1.0"`, `edition = "2024"`, `publish = false` (until a publishing decision), `license = "MIT OR Apache-2.0"`, `rust-version` and `repository` inherited from `[workspace.package]`.

### 3.1 Crate-splitting rule (ADR 0004)

Create a separate crate when at least one of these differs from its neighbours:

1. **Dependencies.** It needs a dependency others shouldn't carry, or must stay dependency-free.
2. **`unsafe` permission.** It needs an `unsafe` exception, or must be provably free of `unsafe`.
3. **FFI exposure.** It is exposed (or not) across the C ABI.
4. **Optionality.** Users should be able to omit it.
5. **Platform.** `no_std` vs `std`, or platform-specific code.

Topic alone is not a reason to split. Expect many crates over time, but no empty crates up front.

### 3.2 Dependency graph (M0+M1)

```
engine_handle   (no deps)
engine_random   (no deps)
engine_time     (no deps)
engine_math  →  glam
engine       →  engine_handle, engine_random, engine_time, engine_math
fixed_step_sim → engine
engine_benches → engine_*, criterion, gungraun, slotmap, thunderdome, rand, rand_xoshiro, fastrand   (dev-only)
xtask        →  (std only; minimal deps)
```

Leaf crates do not depend on each other unless a real need appears.

---

## 4. Engineering policies

### 4.1 Toolchain and MSRV

- `rust-toolchain.toml` pins the exact stable version (currently `1.97.0`) with components `rustfmt`, `clippy`, and the `thumbv7em-none-eabihf` target for `no_std` checks. Bumps are deliberate PRs.
- `[workspace.package] rust-version` equals the pinned version. While at 0.x the MSRV moves freely. At publishing time, revisit (e.g. "latest stable − 2").
- Miri and cargo-fuzz use a separately pinned nightly, recorded in `xtask` and CI (not in `rust-toolchain.toml`).
- `Cargo.lock` is committed.

### 4.2 Lints

`[workspace.lints]`, inherited by every crate via `[lints] workspace = true`:

- **rustc:** `unsafe_code = "deny"`, `missing_docs = "warn"`, `unreachable_pub = "warn"`, `rust_2018_idioms = { level = "warn", priority = -1 }`, `missing_debug_implementations = "warn"`.
- **clippy:** `pedantic = { level = "warn", priority = -1 }`, `undocumented_unsafe_blocks = "deny"`, `unwrap_used = "warn"`, `dbg_macro = "warn"`, `todo = "warn"`, `print_stdout = "warn"` (libraries only; examples/xtask/benches allow it). Pedantic allowances are listed in `Cargo.toml` with a one-line justification each. Starting allowances: `module_name_repetitions`, `must_use_candidate` (we apply `#[must_use]` deliberately instead).
- **rustdoc:** `broken_intra_doc_links = "deny"`, `private_intra_doc_links = "warn"`.
- Tests, examples, benches, and xtask may use `unwrap`/`expect` (`#![cfg_attr(test, allow(clippy::unwrap_used))]` in libraries).
- CI treats warnings as errors (`RUSTFLAGS="-D warnings"`, `RUSTDOCFLAGS="-D warnings"`).

### 4.3 `unsafe` policy (ADR 0001)

- The workspace lint is `deny`. Additionally, every crate root carries `#![forbid(unsafe_code)]`. Cargo cannot override one workspace lint per crate, so `forbid` in source is the real enforcement.
- `cargo xtask check-unsafe` fails CI if any workspace crate root lacks `#![forbid(unsafe_code)]` unless that crate is listed in `unsafe-allowlist.toml`:
  ```toml
  [[crate]]
  name = "engine_ffi"          # example; list is empty in M0+M1
  adr = "docs/adr/00NN-....md"
  ```
- Allowlisted crates must:
  1. put a `// SAFETY:` comment on every `unsafe` block (enforced by clippy);
  2. document invariants on every `unsafe fn` / `unsafe trait` (`# Safety` doc section);
  3. confine `unsafe` to the smallest possible modules, with `#[deny(unsafe_code)]` on the rest;
  4. run under Miri in CI;
  5. have at least one fuzz target.
- Miri also runs on `engine_handle` from day one (cheap, and it validates the CI job).

### 4.4 API design guidelines

Follow the [Rust API Guidelines checklist](https://rust-lang.github.io/api-guidelines/checklist.html). Specific rules:

- **Naming:** RFC 430 casing. Conversions are `as_`/`to_`/`into_`. Constructors are `new`, `with_*`, `from_*`. No `get_` prefix on getters.
- **Common traits:** implement `Debug` always. Implement `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `PartialOrd`, `Ord`, `Default` wherever semantically meaningful. Every public type is `Send + Sync` unless there is a documented reason not to be. Static assertions in tests verify this.
- **`#[must_use]`** on functions whose result being ignored is almost certainly a bug (e.g. `HandlePool::insert`, `Rng::stream`).
- **Errors:** each crate defines its own error enum with hand-written `Display` + `core::error::Error`, marked `#[non_exhaustive]`. No `thiserror`/`anyhow` in libraries.
- **Panics:** library code panics only on documented contract violations (e.g. an empty range passed to `Rng::range`), each with a `# Panics` doc section. Otherwise it returns `Option`/`Result`.
- **Docs:** every public item is documented. Every public type and most public functions include a runnable example (doctest). Each crate's root docs explain purpose, guarantees (determinism, complexity), and a quick start.
- **Prelude:** only the facade crate has a prelude. Leaf crates expose a flat, small public surface.
- **Feature flags:** additive only. `std` is a default feature where applicable. No feature changes observable behavior of deterministic output.

### 4.5 Licensing and originality (ADR 0002)

- Rename `LICENSE` → `LICENSE-MIT`, add `LICENSE-APACHE` (Apache-2.0 text), and add a "License" section to `README.md` with the standard dual-license contribution clause.
- `deny.toml` allows only: `MIT`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`, `Unicode-3.0`. Unknown registries and git sources are denied. Duplicate versions warn. Security advisories deny.
- No code is copied from other projects. When a design is inspired by published work, the crate docs or ADR cite it. Published algorithms (xoshiro256++, SplitMix64, Lemire's bounded random integers, Fisher–Yates) are implemented from their papers/specifications. Their published test vectors may be used as golden values.

### 4.6 Process

- **ADRs** in `docs/adr/NNNN-kebab-title.md` with sections Status, Context, Decision, Consequences. M0+M1 delivers:
  - `0001-unsafe-policy.md`
  - `0002-originality-and-dependency-licensing.md`
  - `0003-same-build-determinism.md`
  - `0004-crate-splitting-rule.md`
- **Commits:** conventional commits (`feat:`, `fix:`, `docs:`, `test:`, `perf:`, `ci:`, `chore:`).
- **Branching:** feature branches merged to `main` via PR. CI must be green.
- **README.md:** project purpose, status (experimental, 0.x), crate map, how to run `cargo xtask ci`, license.

### 4.7 CI (`.github/workflows/ci.yml`)

All jobs invoke `cargo xtask <task>` so local and CI runs are identical.

| Job | Runner(s) | Command | Gate |
|---|---|---|---|
| fmt | ubuntu | `cargo xtask fmt --check` | yes |
| clippy | ubuntu, macOS, windows | `cargo xtask clippy` (all targets, all features) | yes |
| test | ubuntu, macOS, windows | `cargo xtask test` (`cargo nextest run` + `cargo test --doc`); on ubuntu also `cargo xtask test --release --include-ignored` for the statistical tests (§7) | yes |
| doc | ubuntu | `cargo xtask doc` | yes |
| no_std | ubuntu | `cargo xtask no-std` (build `engine_handle`, `engine_random`, `engine_time` with `--no-default-features` for `thumbv7em-none-eabihf`) | yes |
| check-unsafe | ubuntu | `cargo xtask check-unsafe` | yes |
| miri | ubuntu (pinned nightly) | `cargo xtask miri` | yes |
| deny | ubuntu | `cargo xtask deny` | yes |
| bench-build | ubuntu | `cargo bench --no-run` | yes |
| bench-regress | ubuntu (Valgrind via `gungraun/setup-gungraun`) | `cargo xtask bench-regress` (instruction counts vs `main`, fail if > 5%) | yes, once a `main` baseline exists |
| coverage | ubuntu | `cargo llvm-cov nextest` → summary in job output | report only |

`.github/workflows/nightly.yml` runs on a daily schedule: each fuzz target for 10 minutes, plus `cargo xtask ci` against the latest stable toolchain (an early warning for toolchain bumps). It is report-only.

Dependabot gains the `cargo` (weekly, grouped) and `github-actions` (weekly) ecosystems alongside the existing `devcontainers` entry.

### 4.8 `xtask` commands

`ci` (runs fmt-check, clippy, test, doc, no-std, check-unsafe, deny in sequence), `fmt [--check]`, `clippy`, `test`, `doc`, `no-std`, `check-unsafe`, `miri`, `deny`, `bench [--save <name>] [--compare <a> <b>]`, `bench-regress`, `fuzz <target> [--time <secs>]`, `rng-stats` (streams RNG output to stdout for piping into PractRand; documents usage, does not bundle PractRand).

`xtask` has minimal dependencies (std plus, at most, a small argument parser). It shells out to `cargo` and external tools, and prints an actionable install hint when a tool (`cargo-nextest`, `cargo-deny`, `cargo-llvm-cov`, `gungraun-runner`, `cargo-fuzz`, `critcmp`) is missing.

---

## 5. Component designs

Code below shows the public API surface. Private fields are indicative.

### 5.1 `engine_handle`

**Purpose:** typed, generational, copyable identifiers and a pool that owns values behind them. Reused later for ECS entities (M2), assets, and FFI handles (M4).

```rust
#![no_std]
extern crate alloc;

/// A typed, generational identifier. 8 bytes. `Option<Handle<T>>` is also 8 bytes.
pub struct Handle<T> {
    index: u32,
    generation: core::num::NonZeroU32,
    _marker: core::marker::PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    /// Slot index (for diagnostics / dense side-tables). Not unique over time.
    pub const fn index(self) -> u32;
    /// Generation of the slot when this handle was issued.
    pub const fn generation(self) -> core::num::NonZeroU32;
    /// Stable encoding: `(generation as u64) << 32 | index as u64`. Never 0.
    pub const fn to_bits(self) -> u64;
    /// Inverse of `to_bits`. Returns `None` iff the generation half is 0.
    pub const fn from_bits(bits: u64) -> Option<Self>;
    /// Reinterpret as a handle to another type (explicit escape hatch; safe, purely type-level).
    pub const fn cast<U>(self) -> Handle<U>;
}
```

- `Copy`, `Clone`, `PartialEq`, `Eq`, `PartialOrd`, `Ord` (by `(index, generation)`), `Hash`, and `Debug` (`Handle<TypeName>(index v generation)`) are implemented manually, without bounds on `T`.
- `PhantomData<fn() -> T>` keeps the handle covariant in `T` and `Send + Sync` regardless of `T`, with no drop-check implications. A handle refers to a `T` but never owns one.

```rust
pub struct HandlePool<T> { slots: Vec<Slot<T>>, free_head: Option<u32>, len: u32 }

impl<T> HandlePool<T> {
    pub const fn new() -> Self;
    pub fn with_capacity(capacity: usize) -> Self;
    #[must_use] pub fn insert(&mut self, value: T) -> Handle<T>;
    pub fn get(&self, handle: Handle<T>) -> Option<&T>;
    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T>;
    pub fn remove(&mut self, handle: Handle<T>) -> Option<T>;
    pub fn contains(&self, handle: Handle<T>) -> bool;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn capacity(&self) -> usize;
    pub fn clear(&mut self);
    pub fn iter(&self) -> Iter<'_, T>;          // Item = (Handle<T>, &T), ascending slot index
    pub fn iter_mut(&mut self) -> IterMut<'_, T>; // Item = (Handle<T>, &mut T), ascending slot index
    pub fn drain(&mut self) -> Drain<'_, T>;     // Item = (Handle<T>, T), ascending slot index
}
// Also: Default, Debug (T: Debug), Clone (T: Clone), IntoIterator for &/&mut/owned, Index<Handle<T>> (panics on stale).
```

```rust
/// Handle allocation without value storage (ECS entity allocator shape).
pub struct HandleAllocator<T> { /* generations + free list */ }

impl<T> HandleAllocator<T> {
    pub const fn new() -> Self;
    pub fn with_capacity(capacity: usize) -> Self;
    #[must_use] pub fn allocate(&mut self) -> Handle<T>;
    /// Returns `false` if the handle was already stale.
    pub fn free(&mut self, handle: Handle<T>) -> bool;
    pub fn is_alive(&self, handle: Handle<T>) -> bool;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn clear(&mut self);
    pub fn iter(&self) -> AllocIter<'_, T>;    // live handles, ascending slot index
}
```

**Semantics (identical for pool and allocator):**

- New slots start at generation 1.
- Removing or freeing bumps the slot's generation, so every outstanding handle to it becomes stale (`get` → `None`, `contains` → `false`).
- Free slots are reused **LIFO**, which makes allocation order deterministic given the same operation sequence.
- **Generation exhaustion:** if bumping would overflow `u32::MAX`, the slot is permanently retired (never reused). This guarantees a stale handle can never become valid again, at the cost of one slot per 2³²−1 reuses.
- `clear()` bumps every occupied slot's generation (outstanding handles become stale) and returns all non-retired slots to the free list in ascending index order. Reuse order after `clear` is therefore deterministic.
- **Capacity:** at most `u32::MAX` slots. `insert`/`allocate` panic beyond that (`# Panics` documented).
- **Complexity:** `insert`/`allocate`/`get`/`remove`/`free`/`contains` are O(1). Iteration is O(capacity) (skips vacant slots). Dense iteration is deliberately not provided in M1; the ECS will own dense storage.
- Implementation is 100% safe Rust (`Vec<Slot<T>>` where `Slot` is an enum of occupied and vacant).

### 5.2 `engine_random`

**Purpose:** a fast, deterministic, versioned-stable PRNG for simulation. Not cryptographically secure (stated prominently in docs).

```rust
#![no_std]

/// xoshiro256++ generator; state seeded via SplitMix64.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Rng { s: [u64; 4] }

impl Rng {
    /// Expands `seed` into 256 bits of state using four successive SplitMix64 outputs.
    pub const fn from_seed(seed: u64) -> Self;
    pub fn next_u32(&mut self) -> u32;          // upper 32 bits of next_u64
    pub fn next_u64(&mut self) -> u64;
    pub fn f32(&mut self) -> f32;               // (next_u64 >> 40) as f32 * 2^-24, in [0, 1)
    pub fn f64(&mut self) -> f64;               // (next_u64 >> 11) as f64 * 2^-53, in [0, 1)
    pub fn range<R: SampleRange>(&mut self, range: R) -> R::Output;
    pub fn bool(&mut self) -> bool;             // top bit of next_u64
    pub fn chance(&mut self, p: f64) -> bool;   // f64() < p
    pub fn shuffle<T>(&mut self, slice: &mut [T]);
    pub fn choose<'a, T>(&mut self, slice: &'a [T]) -> Option<&'a T>;
    pub fn fill_bytes(&mut self, dest: &mut [u8]);
    #[must_use] pub fn stream(&self, id: u64) -> Rng;
}

/// Implemented for `Range` and `RangeInclusive` over u8..u64, usize, i8..i64, isize, f32, f64.
pub trait SampleRange: sealed::Sealed { type Output; /* sampling method is private */ }
```

**Semantics:**

- **Integers in ranges:** Lemire's nearly-divisionless bounded method (unbiased). An empty range panics (`# Panics`). An inclusive full-width range (e.g. `0..=u64::MAX`) is handled without overflow.
- **Floats in ranges:** `lo + (hi - lo) * unit`. `Range` guarantees results in `[lo, hi)`: if rounding produces `hi`, the sample is rejected and redrawn. `RangeInclusive` returns results in `[lo, hi]`. Non-finite bounds or `lo > hi` panic.
- **`chance(p)`:** `p <= 0.0` never true, `p >= 1.0` always true, `NaN` always false. It never panics.
- **`shuffle`:** Fisher–Yates using unbiased bounded integers.
- **`choose`:** `None` for an empty slice. Consumes one bounded draw otherwise.
- **`stream(id)`:** returns an independent generator that is a pure function of `(self's current state, id)` and does not advance `self`. Definition:
  `seed = mix(s0 ^ s1.rotate_left(17) ^ s2.rotate_left(31) ^ s3.rotate_left(47) ^ mix(id))`, then `Rng::from_seed(seed)`, where `mix` is the SplitMix64 finalizer. Typical use: `let mut r = world_rng.stream(entity.to_bits());`. The same entity gets the same sequence regardless of system execution order or thread assignment.
- **Stream stability guarantee:** for a given seed, every method's output sequence is frozen across all 0.x releases. Golden tests enforce this; any change is a breaking change requiring an ADR.
- No `rand_core` trait impls in M1 (YAGNI). An optional, additive `rand_core` feature may be added later.

### 5.3 `engine_time`

**Purpose:** simulation time primitives that make fixed-step loops correct, testable, and deterministic.

```rust
#![no_std]   // `std` default feature enables SystemClock

/// Monotonic simulation step counter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(u64);
impl Tick {
    pub const ZERO: Tick;
    pub const fn new(value: u64) -> Tick;
    pub const fn next(self) -> Tick;           // panics on overflow (documented; ~9.7e9 years at 60 Hz)
    pub const fn get(self) -> u64;
}

/// Converts variable elapsed time into a whole number of fixed simulation steps.
#[derive(Clone, Debug)]
pub struct FixedTimestep { step: Duration, accumulator: Duration, max_steps: u32, tick: Tick, dropped: Duration }

impl FixedTimestep {
    pub const DEFAULT_MAX_STEPS_PER_UPDATE: u32 = 8;
    /// # Panics: if `step` is zero.
    pub fn new(step: Duration) -> Self;
    /// # Panics: if `max_steps` is zero.
    pub fn with_max_steps_per_update(self, max_steps: u32) -> Self;
    /// Adds `elapsed`, commits up to `max_steps` whole steps, and returns them.
    /// Excess time beyond `max_steps * step` is discarded and added to `dropped_time()`.
    pub fn accumulate(&mut self, elapsed: Duration) -> Steps;
    /// Leftover fraction of a step in [0, 1), for render interpolation.
    pub fn alpha(&self) -> f32;
    pub fn step(&self) -> Duration;
    pub fn current_tick(&self) -> Tick;        // tick of the next step to be produced
    pub fn dropped_time(&self) -> Duration;    // total time discarded by the step cap
}

/// Ticks committed by one `accumulate` call. Steps are committed when `accumulate` returns,
/// whether or not the iterator is consumed.
#[derive(Clone, Debug)]
pub struct Steps { next: Tick, remaining: u32 }
impl Iterator for Steps { type Item = Tick; }
impl ExactSizeIterator for Steps {}
impl DoubleEndedIterator for Steps {}

/// A monotonic instant relative to an arbitrary, source-defined origin.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(Duration);
impl Timestamp {
    pub const fn from_duration(since_origin: Duration) -> Self;
    pub const fn as_duration(self) -> Duration;
    pub fn saturating_duration_since(self, earlier: Timestamp) -> Duration;
}

pub trait TimeSource { fn now(&self) -> Timestamp; }

#[cfg(feature = "std")]
#[derive(Clone, Debug)]
pub struct SystemClock { origin: std::time::Instant }   // new() captures origin; now() = elapsed since origin

#[derive(Clone, Debug, Default)]
pub struct ManualClock { now: Timestamp }
impl ManualClock {
    pub const fn new() -> Self;
    pub fn advance(&mut self, by: Duration);
    pub fn set(&mut self, to: Timestamp);       // # Panics: if `to` < current (monotonicity)
}
```

**Semantics:**

- Arithmetic uses `Duration` (integer nanoseconds), so accumulation is exact. Splitting `elapsed` differently across `accumulate` calls yields the same total sequence of ticks, as long as the cap is not hit (property-tested).
- `alpha()` is the only floating-point output and is never fed back into the simulation.

### 5.4 `engine_math`

**Purpose:** a single blessed math vocabulary for the engine.

- Re-exports from `glam` (version pinned in `[workspace.dependencies]`, default features plus no fast-math): `Vec2`, `Vec3`, `Vec3A`, `Vec4`, `Quat`, `Mat3`, `Mat3A`, `Mat4`, `Affine3A`, and the `IVec*`/`UVec*` families. `glam` is re-exported as `engine_math::glam` for escape-hatch access.
- glam's version is part of our public API. Bumping glam's major/minor version is a breaking change for us (acceptable at 0.x). The FFI layer (M4) defines its own `#[repr(C)]` plain-data math types and does not expose glam.

```rust
/// Translation, rotation, scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform { pub translation: Vec3, pub rotation: Quat, pub scale: Vec3 }

impl Transform {
    pub const IDENTITY: Transform;
    pub const fn from_translation(t: Vec3) -> Self;
    pub const fn from_rotation(r: Quat) -> Self;
    pub const fn from_scale(s: Vec3) -> Self;
    pub fn with_translation(self, t: Vec3) -> Self;  // + with_rotation, with_scale
    /// `self * child`. Exact for uniform scale; for non-uniform scale with rotation the result
    /// is the closest TRS approximation (documented, tested against Mat4 composition within eps).
    pub fn mul_transform(self, child: Transform) -> Transform;
    /// Exact inverse for uniform scale; documented approximation otherwise. Zero scale yields non-finite values.
    pub fn inverse(self) -> Transform;
    pub fn to_matrix(self) -> Mat4;
    pub fn to_affine(self) -> Affine3A;
    pub fn transform_point(self, p: Vec3) -> Vec3;
    pub fn transform_vector(self, v: Vec3) -> Vec3;  // no translation
}
impl Default for Transform { /* IDENTITY */ }
impl core::ops::Mul for Transform { /* mul_transform */ }

pub fn lerp(a: f32, b: f32, t: f32) -> f32;               // a + (b - a) * t; t not clamped
pub fn inverse_lerp(a: f32, b: f32, v: f32) -> f32;       // (v - a) / (b - a); a == b → NaN (documented)
pub fn remap(v: f32, from: (f32, f32), to: (f32, f32)) -> f32;
pub fn approx_eq(a: f32, b: f32, eps: f32) -> bool;       // (a - b).abs() <= eps
```

Only `f32` helpers in M1. `f64` variants are added when something needs them.

### 5.5 `engine` facade

- Re-exports each crate as a module: `engine::handle`, `engine::random`, `engine::time`, `engine::math`.
- `engine::prelude` exports: `Handle`, `HandlePool`, `HandleAllocator`, `Rng`, `Tick`, `FixedTimestep`, `TimeSource`, `Timestamp`, `ManualClock`, `SystemClock` (when `std`), `Transform`, `Vec2`, `Vec3`, `Vec4`, `Quat`, `Mat4`.
- Features: `std` (default), forwarded to leaf crates.
- The crate root docs are the engine's "start here" page: crate map, guarantees, and a short example.

### 5.6 `examples/fixed_step_sim`

A headless binary demonstrating the M1 crates together:

1. CLI arguments: `--seed <u64>` (default 42), `--ticks <u64>` (default 600), `--particles <usize>` (default 10 000).
2. Creates a `HandlePool<Particle { position: Vec3, velocity: Vec3, ttl: u32 }>` seeded via `Rng::from_seed(seed)`.
3. Drives a `FixedTimestep` (1/60 s) from a `ManualClock`, advancing by a deterministic pseudo-random frame time each loop iteration (simulating jittery frames). For each step:
   - integrate particles;
   - decrement `ttl` and remove expired particles;
   - respawn replacements using `rng.stream(handle.to_bits())`.
4. Prints the final tick, live count, slot capacity, and a 64-bit FNV-1a checksum over `(handle bits, position bits, velocity bits)` in iteration order.
5. Integration test (`examples/fixed_step_sim/tests/determinism.rs`) runs the simulation logic as a library function twice with the same seed and asserts equal checksums. A different seed must produce a different checksum. A different frame-time split must produce the same checksum (cap not hit).

The sim logic lives in the crate's `lib.rs`. `main.rs` is a thin CLI wrapper, so tests and benches can call it directly.

---

## 6. Error handling summary

| Crate | Fallible operations | Mechanism |
|---|---|---|
| `engine_handle` | stale or invalid handles | `Option` (`get`, `remove`, `from_bits`), `bool` (`free`, `contains`). Capacity overflow panics |
| `engine_random` | none at runtime | Contract violations (empty/invalid range) panic with `# Panics` docs |
| `engine_time` | zero step, non-monotonic `set` | Panic with `# Panics` docs |
| `engine_math` | none | Documented NaN/non-finite behavior |

No M1 crate needs an error enum. Per-crate `#[non_exhaustive]` error types are introduced when the first genuinely recoverable error appears (expected in M2/M4).

---

## 7. Testing strategy

All layers run in CI unless noted.

| Layer | Tool | M0+M1 coverage |
|---|---|---|
| Unit | `#[test]` in-module | Generation increments; retire-on-exhaustion (tested by constructing a slot near `u32::MAX` via a `#[cfg(test)]` constructor); LIFO reuse; `clear` semantics; `from_bits(0)` → `None`; `Tick::next`; `FixedTimestep` cap and `dropped_time`; `alpha` range; float-range rejection path; `chance` edge cases (`0`, `1`, `NaN`, negatives); `Transform` identities |
| Doc | rustdoc | Every public example runs. `compile_fail` doctests: `Handle<A>` passed where `Handle<B>` is expected; `Rng::range` with an unsupported type |
| Trait/auto-trait assertions | static asserts in tests | `Handle<T>: Send + Sync + Copy` for non-`Send` `T`; size assertions (`size_of::<Handle<T>>() == 8`, `size_of::<Option<Handle<T>>>() == 8`) |
| Property / model-based | `proptest` | `HandlePool` and `HandleAllocator` vs a `HashMap`/`BTreeMap` reference model over random op sequences (insert/remove/get/clear/iterate). Invariants: stale handles never resolve, `len` matches, iteration order is ascending by index, two runs of the same op sequence yield identical handles. `to_bits`/`from_bits` round-trips. `FixedTimestep` split-invariance. `Rng::range` results always within bounds. `Transform` compose/inverse round-trips within eps for uniform scale; `transform_point` matches `to_matrix` |
| Golden values | plain tests | xoshiro256++ and SplitMix64 outputs checked against published test vectors. First 16 outputs of every public `Rng` method for seeds `{0, 1, 42, u64::MAX}` frozen in `tests/golden.rs`. `stream` outputs frozen. `fixed_step_sim` checksum for the default arguments frozen |
| Statistical sanity | plain tests (release-mode, `#[ignore]` by default, run in CI via `--include-ignored` on ubuntu) | Chi-square goodness-of-fit for `range` over small and non-power-of-two bounds, `f32`/`f64` bucket uniformity, `bool` balance, `shuffle` permutation uniformity on 4 elements. Fixed seeds and thresholds, so tests are deterministic |
| Determinism | integration test | §5.6 |
| Undefined behavior | Miri | `engine_handle` (and all future allowlisted crates) |
| Fuzzing | `cargo-fuzz` (nightly workflow) | `handle_pool_ops` (arbitrary op sequences checked against the model), `handle_from_bits` |
| Test quality | `cargo-mutants` | Run manually or periodically; surviving mutants in M1 crates are triaged before M0+M1 is declared done. Not a CI gate |

Test-only dependencies (`proptest`, and `arbitrary`/`libfuzzer-sys` in `fuzz/`) live in `[workspace.dependencies]` and are dev-dependencies only.

---

## 8. Benchmarking strategy

**Tools:**

- **Criterion** (+ **critcmp**): local wall-clock benchmarks with named baselines. `cargo xtask bench --save <name>` runs `cargo bench -- --save-baseline <name>`; `cargo xtask bench --compare <a> <b>` runs critcmp.
- **Gungraun** (formerly iai-callgrind): instruction-count benchmarks under Valgrind for CI regression gating (Linux only; runs in CI and the devcontainer, not natively on macOS). On every push to `main`, CI saves the Gungraun summary as a workflow artifact. PR jobs download the most recent `main` artifact, compare against it, and fail on > 5% instruction-count regression in any benchmark. If no `main` artifact exists yet, the job reports only.

**Layout:**

- Micro-benchmarks for each crate live in `libs/<crate>/benches/` (Criterion, plus a Gungraun target for regression-critical paths).
- Comparisons against other libraries live in `benches/` (`engine_benches`). Competitor crates are dev-dependencies of that crate only.

**M1 benchmark matrix:**

| Subject | Compared against | Operations | Sizes |
|---|---|---|---|
| `HandlePool<T>` (T = `u64` and a 64-byte struct) | `slotmap::SlotMap`, `thunderdome::Arena` | insert N; get all (sequential and random handle order); remove all; iterate; churn (remove 50% randomly then reinsert) then get | 1k, 100k, 1M |
| `HandleAllocator` | `slotmap` with `()` values | allocate N; free/reallocate churn; `is_alive` checks | 1k, 100k, 1M |
| `Rng` | `rand_xoshiro::Xoshiro256PlusPlus`, `rand::rngs::SmallRng`, `fastrand::Rng` | `next_u64` ×1M; `range(0..1000)` ×1M; `range(0.0..1.0)` ×1M; `shuffle` 10k | — |
| `FixedTimestep` | — | `accumulate` + drain ×1M | — |
| `fixed_step_sim` | — | full run, default args | — |

**Reporting:** `docs/benchmarks.md` records methodology (toolchain, profile settings, CPU model, OS, power settings), results tables per milestone, and interpretation. It records results honestly, including where we lose. The release benchmark profile uses `lto = "thin"`, `codegen-units = 1`, `debug = true` (for profiling).

---

## 9. Definition of done (M0+M1)

1. `cargo xtask ci` passes locally (macOS) and in the devcontainer (Linux). The GitHub Actions matrix is green on ubuntu, macOS, and windows.
2. All gating CI jobs in §4.7 exist and pass. The bench-regress gate is active once the first `main` baseline is recorded.
3. Every public item in every crate is documented. Every public type has a runnable example.
4. All test layers in §7 are implemented. Golden values are frozen. Miri is clean. Both fuzz targets run in the nightly workflow.
5. The benchmark matrix in §8 has been run locally, with results and hardware recorded in `docs/benchmarks.md`.
6. ADRs 0001–0004 are written. README, dual license files, `deny.toml`, `unsafe-allowlist.toml` (empty), and dependabot config are in place.
7. `examples/fixed_step_sim` runs, and its determinism test passes.
8. A `cargo-mutants` run has been triaged.

---

## 10. Out of scope (M0+M1)

- Any ECS code: storage, queries, world, component registration (M2).
- Scheduler and job system (M3).
- C ABI, C# bindings, header generation (M4).
- Windowing, input, rendering, audio, assets, physics, scripting, networking, editor tooling.
- Publishing to crates.io, semver guarantees beyond "golden RNG streams are frozen in 0.x".
- Cross-platform bit-exact determinism, fixed-point math.
- `rand_core` trait interop, `f64` math helpers, dense iteration in `HandlePool`.

---

## 11. Roadmap after M0+M1

| Milestone | Summary | Key open question it resolves |
|---|---|---|
| **M2: ECS storage spike + decision** | Prototype sparse-set, archetype, and per-component hybrid storage behind one minimal API (spawn/despawn, insert/remove, typed query iteration, runtime-registered component). Benchmark against bevy_ecs, hecs, flecs_ecs on standard workloads (simple iteration, fragmented iteration, add/remove, heavy compute, entity churn). Produce an ADR choosing the storage model, the runtime component identity model, and the policy for foreign/mod-defined component storage (safe byte blobs vs audited type-erased columns) | Storage model; can C#/mods define components; which `unsafe` exceptions exist |
| M3: Scheduler | Systems with declared read/write access, explicit ordering, ambiguity detection, deterministic execution under same-build determinism, parallel execution | Threading model (thread pool vs fibers vs work stealing) |
| M4: C ABI + C# | `engine_ffi` crate (allowlisted `unsafe`), handle-based C API, generated header, C# binding (generator choice evaluated: Interoptopus vs cbindgen + hand-written P/Invoke vs csbindgen), C# sample | Binding generator; ownership/lifetime rules across the boundary |
| Later | Windowing/input (`winit`) → rendering (`wgpu`) → assets → audio → physics → modding/plugin loading | — |

---

## 12. Sources

- Cox et al., ECS storage comparison, EG UK CGVC 2025 — https://eprints.staffs.ac.uk/9315/1/cgvc20251224.pdf
- ADR-001: Archetype-Based ECS Storage — https://web-engine-dev-docs.pages.dev/architecture/decisions/001-archetype-based-ecs
- Archetypal ECS Considered Harmful? — https://moonside.games/posts/archetypal-ecs-considered-harmful/
- bevy_ecs storage module docs — https://docs.rs/crate/bevy_ecs/latest/source/src/storage/mod.rs
- skypjack, ECS back and forth (part 2: sparse sets and groups) — https://skypjack.github.io/2019-03-07-ecs-baf-part-2
- skypjack, ECS back and forth (part 9: sparse set internals) — https://skypjack.github.io/2020-08-02-ecs-baf-part-9
- skypjack, ECS back and forth (part 10: hybrid storage) — https://skypjack.github.io/2021-02-27-ecs-baf-part-10
- Bevy `ComponentId` docs — https://docs.rs/bevy/0.16.1/bevy/ecs/component/struct.ComponentId.html
- TypeId across FFI (Rust forum) — https://users.rust-lang.org/t/desperately-need-help-with-ffi-panicking/133572
- Flecs.NET — https://github.com/BeanCheeseBurrito/Flecs.NET
- flecs_ecs (Rust) — https://docs.rs/crate/flecs_ecs/0.2.2
- Bevy system-order ambiguity notes — https://hackmd.io/@alice-i-cecile/SJvmN1rAi
- Bevy cheatbook: system order — https://bevy-cheatbook.github.io/programming/system-order.html
- Gyrling, Parallelizing the Naughty Dog Engine Using Fibers (GDC 2015) — https://gdcvault.com/play/1022187/Parallelizing-the-Naughty-Dog-Engin
- Machinery: fiber-based job system — https://ruby0x1.github.io/machinery_blog_archive/post/fiber-based-job-system
- Handles are the better pointers (follow-up) — https://engineered.at/articles/handles-are-the-better-pointers-an-odin-gamedev-follow-up
- Llopis, Managing Data Relationships — https://www.gamedeveloper.com/programming/managing-data-relationships
- Interoptopus C# backend — https://docs.rs/interoptopus_csharp
- cbindgen issue mentioning csbindgen gaps — https://redirect.github.com/mozilla/cbindgen/issues/846
- Rapier determinism — https://rapier.rs/docs/user_guides/rust/determinism/
- Pollard & Bradley, Cross-Platform Determinism (GDC 2024) — https://media.gdcvault.com/gdc2024/Slides/GDC+slide+presentations/Pollard_Bradley_CrossPlatformDeterminism+2024-03-26+09.34.19.pdf
- Gaffer on Games, Floating Point Determinism — https://new.gafferongames.com/post/floating_point_determinism/
- flecs ecs_benchmark — https://github.com/flecs-hub/ecs_benchmark
- rust-gamedev ecs_bench_suite (archived) — https://relatedrepos.com/gh/rust-gamedev/ecs_bench_suite
- CodSpeed: Rust benchmarking — https://codspeed.io/docs/benchmarks/rust
- critcmp — https://awesome.ecosyste.ms/projects/github.com%2Fburntsushi%2Fcritcmp
- Gungraun — https://docs.rs/crate/gungraun/
- compiler-builtins Gungraun CI migration — https://github.com/rust-lang/compiler-builtins/pull/1033/files
- Aras Pranckevičius, ECS & DoD (2018) — https://aras-p.info/texts/files/2018Academy%20-%20ECS-DoD.pdf
- Unity DOTS roundtable Q&A — https://blog.unity.com/engine-platform/dots-webinar-roundtable-q-a
- Rust API Guidelines checklist — https://rust-lang.github.io/api-guidelines/checklist.html
