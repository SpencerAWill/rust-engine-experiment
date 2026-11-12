# M0+M1: Engineering Foundations and Core Primitives — Design Spec

- **Date:** 2026-11-11
- **Status:** Implemented. Approved 2026-11-12 and amended after implementation (amendments are marked "amended 2026-11-12" inline). The code and the ADRs are authoritative where they differ.
- **Working name:** `engine` (crates `engine_*`); rename before any publishing.
- **Scope:** First sub-project of the engine. Establishes the workspace, engineering policy, CI, test and benchmark infrastructure (M0), plus the first real crates: generational handles, deterministic PRNG, simulation time, math (M1).

---

**Reading guide.** This document is long, and other files cite it by section number (for example "spec §4.9"), so the numbering is stable. Find what you need:

| If you want | Read |
|---|---|
| Why the project exists, what is settled and what is deferred | §1 |
| The workspace layout and dependency graph | §3 |
| The rules every change must follow (lints, `unsafe`, determinism, panics, CI, no global state) | §4 |
| The API design of one crate | §5.1 `engine_handle`, §5.2 `engine_random`, §5.3 `engine_time`, §5.4 `engine_math`, §5.5 `engine`, §5.6 the sample |
| How failures are reported | §6 |
| How it is tested and benchmarked | §7, §8 |
| What "done" meant for M0+M1, and what comes next | §9, §10, §11 |

For decisions, prefer the ADRs in `docs/adr/`. The implementation plan for this milestone was removed when it merged; recover it with `git log --diff-filter=D --name-only -- docs/design/m0-m1-foundations/plan.md` and `git show <commit>^:docs/design/m0-m1-foundations/plan.md`.

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
| Build vs buy | Own the core (ECS, scheduler, FFI layer, PRNG, handles, time). Reuse mature crates at the edges (`glam`, later `winit`, `wgpu`, …) |
| Logic style (when the ECS arrives) | Systems over queries; read/write access sets drive parallelism. No entity relationships in the first ECS milestone |
| `unsafe` | Forbidden by default; named, audited exception crates only (see §4.3) |
| Originality | **No code copied from Bevy or any other project.** Designs, papers, talks, and blog posts may be studied and cited; all implementation is original. Algorithms are implemented from their published specifications |
| Crate granularity | Many fine-grained crates, layered, plus a facade crate. Split wherever a real boundary exists (§3.1) |
| License | Dual `MIT OR Apache-2.0` |
| World scale | **Large worlds are possible** (planet/space/large open-world sims). World placement uses f64 translation (`WorldTransform`); local/relative data uses f32 (`Transform`); conversion is explicit, relative to an origin (§5.4, ADR 0006) |
| Coordinate convention | **One internal convention: right-handed, +Y up, +X right, −Z forward (camera/look direction)**, matching glTF. SI units (meters, seconds, radians, kilograms). Other conventions are converted explicitly at import/export edges (§5.4, ADR 0005) |
| Determinism hygiene | Simulation code receives time, randomness, and input only as explicit parameters, never from ambient/global state. Nondeterministic std APIs are banned by lint in library crates. All transcendental math goes through `engine_math::scalar`, so cross-platform determinism can be enabled later as a contained switch (§4.9) |
| Instance isolation | **No global mutable state.** Every engine instance is self-contained, and many can coexist in one process (parallel tests, multi-match servers, editor + play-mode worlds, rollback worlds). The future C API takes an explicit context handle on every call (§4.10, ADR 0008) |
| Strings | UTF-8 everywhere inside the engine. FFI layers convert at the boundary (e.g. UTF-16 for C#) |
| Tooling & DX | A future game **editor** is a planned product, leaning **out-of-process** (it talks to a running engine over a protocol; UI technology undecided). **Tools and the editor use only public engine APIs, with no privileged backdoors** (ADR 0009). Debuggability is a first-class goal: `#[track_caller]` panics, actionable messages, readable `Debug`, invariant checks (§4.4) |
| Concurrency & cache | Correct, cache-efficient parallelism is a core requirement. M1 adds no threads but fixes the rules: determinism independent of thread count, concurrency primitives confined to designated crates with model-checked tests, false-sharing avoidance, and layout and cache-behavior measurement (§4.11) |
| CI cost | The repo is private for now, so CI is **tiered**: ubuntu on every push; macOS/Windows on PRs into `main` and nightly. Expected to go public soon, at which point the full matrix runs on every push (§4.7) |
| Dimensionality | **2D, 3D, and 2.5D games are all first-class targets.** 2D world space is the XY plane of the 3D convention (+X right, +Y up, Z = depth/layer, meters). Screen/UI space (+Y down, pixels) is a separate space, reached only through explicit conversion. M1 provides both 2D and 3D math vocabularies with bridges (§5.4). M2 decides storage |
| Loop ownership | **The host owns the loop (library-first).** The engine never takes over a thread it wasn't given. Hosts (Rust games, C# apps, editors, servers, tests, browsers) call into the engine, e.g. `step(...)`. A convenience "app runner" may be layered on top later, but nothing may require it (ADR 0007) |
| Snapshot-able state | All simulation state is plain data and fully capturable/restorable (Clone at minimum; raw-state accessors for value-like state). This is what enables replays, save/load, rollback, and "record a bug, replay it" |

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
3. **Scheduling.** Implicit ordering of conflicting systems produces nondeterminism. Bevy requires explicit ordering and reports ambiguities. Naughty Dog's fiber job system parks waiting jobs rather than blocking threads, uses atomic counters, and had no work stealing. → M3 concern; M1's `Prng::stream` is designed so randomness does not depend on execution order.
4. **Handles over pointers at boundaries.** Index plus generation handles are serializable, detect staleness, and are only dereferenced by the owning side, which is the right shape for FFI. → `engine_handle` in M1, with a stable 64-bit encoding.
5. **Determinism must be chosen up front.** Rapier offers only local determinism by default. Frontier's GDC 2024 talk shows cross-platform float determinism needs "extreme care" (they chose fixed-point). → We commit to same-build determinism and own our PRNG so dependency updates cannot change output.
6. **Benchmarking.** The classic `ecs_bench_suite` is archived. divan lacks built-in baseline save/compare. Criterion supports saved baselines (critcmp compares them). Gungraun (formerly iai-callgrind) gives CI-stable instruction-count benchmarks. → Criterion locally, Gungraun in CI (§8).
7. **Old-engine mistakes to avoid.** Individually allocated objects, data scattered in memory, logic scattered across "a million components", and OO gameplay layers tightly coupled to data layout. → Data-oriented crates with plain data and free functions where possible, and stable handles instead of object references.
8. **Hidden nondeterminism in std.** `std::collections::HashMap`'s default hasher is seeded from OS randomness per process (and per map), so iteration order differs between runs. Materialize bans std hash collections in deterministic code for this reason. → Lint bans (§4.9).
9. **Large-world precision is a one-way door.** f32 gives about 1 mm precision at 8 km from the origin and about 6 cm at 1,000 km. Unreal Engine 5 retrofitted double-precision "Large World Coordinates". Its pain points included serialized struct sizes changing, codebase-wide precision audits, jitter that persisted until 5.4, and physics/shader gaps. → f64 world placement from day one, f32 elsewhere (§5.4).
10. **Platform traps.** `std::time::Instant::now()` panics on `wasm32-unknown-unknown`. On 32-bit targets `usize` is 32 bits. → wasm32 CI checks; `SystemClock` is unavailable on that target (§4.7, §5.3).
11. **Reflection is the backbone engines add too late.** Serialization, editors, scripting, FFI, mods, and networking all need runtime type information. Rust's built-in reflection is minimal, which is why Bevy built `bevy_reflect` with a type registry. → The M2 component-identity design includes the type registry (§11).
12. **Transcendental functions are the cross-platform determinism hazard.** Rapier's networked desyncs were traced to `sin_cos` differences. The pure-Rust `libm` crate (a port of musl's libm) and glam's `libm` feature are the standard remedy. → Funnel now, switch later (§4.9).
13. **Private-repo CI pricing.** GitHub bills macOS at 10x and Windows at 2x against 2,000 included minutes/month (Free plan). → Tiered CI (§4.7).
14. **Coordinate conventions.** glTF 2.0 is right-handed, +Y up, meters/radians, with asset fronts facing +Z. → Adopted as the internal convention (§5.4).
15. **Concurrency correctness and cache behavior are measurable, not aspirational.** loom model-checks interleavings under the C11 memory model (with documented gaps). crossbeam-utils pads to 128 bytes on x86_64 and aarch64 to avoid false sharing. Reproducible parallel reductions require a fixed reduction order independent of core count. → §4.11, §4.9.
16. **Review-found pitfalls.** glam's SSE2 and NEON backends use different horizontal-sum orders (verified in source); proptest's default features don't build for WASI; libtest skips `#[should_panic]` on non-emscripten wasm. → §4.7, §4.9, §5.6.

---

## 3. Workspace structure

```
Cargo.toml                 workspace: [workspace.package], [workspace.dependencies], [workspace.lints]
rust-toolchain.toml        pinned stable toolchain
rustfmt.toml               stable options only
clippy.toml                determinism bans: disallowed types/methods (§4.9)
deny.toml                  cargo-deny: licenses, advisories, bans, sources
unsafe-allowlist.toml      crates permitted to use `unsafe` (empty in M0+M1)
concurrency-allowlist.toml crates permitted to implement atomics/lock-free sync (empty in M0+M1; §4.11)
LICENSE-MIT                (renamed from LICENSE)
LICENSE-APACHE
libs/
  engine_handle/           generational handles, HandlePool<T>, HandleAllocator<T>, DenseHandleAllocator<T>   [no_std + alloc]
  engine_random/           deterministic PRNG                                         [no_std]
  engine_time/             Tick, FixedTimestep, TimeSource, clocks                    [no_std, `std` feature]
  engine_math/             glam re-exports, Transform, scalar helpers                 [std]
  engine/                  facade: re-exports + `engine::prelude`
examples/
  fixed_step_sim/          headless deterministic sample (binary crate)
benches/                   `engine_benches`: cross-library comparison benchmarks (publish = false)
xtask/                     developer task runner (publish = false)
fuzz/                      cargo-fuzz targets (excluded from workspace; nightly) — arrives in M2 (§4.12)
docs/
  adr/                     architecture decision records
  benchmarks.md            methodology, hardware, recorded results
  design/<milestone>/      design spec for each milestone (this file); its plan sits beside it until the milestone merges
.github/
  workflows/ci.yml         PR/push CI
  workflows/nightly.yml    scheduled latest-stable CI run (fuzzing added in M2)
  dependabot.yml           devcontainers + cargo + github-actions
```

Workspace members: `libs/*`, `examples/*`, `benches`, `xtask`. `fuzz` is excluded.

All crates: `version = "0.1.0"`, `edition = "2024"`, `publish = false` (until a publishing decision), `license = "MIT OR Apache-2.0"`, `rust-version` and `repository` inherited from `[workspace.package]`.

### 3.1 Crate-splitting rule (ADR 0004)

A crate is Rust's unit of compilation, coherence (trait impls), visibility, and versioning, so crate boundaries are decided deliberately, using a two-sided checklist.

**Split into a separate crate when at least one of these holds:**

1. **Dependencies.** It needs a dependency others shouldn't carry, or must stay dependency-free.
2. **Permission boundary.** It needs an `unsafe` or concurrency exception (§4.3, §4.11), or must be provably free of them.
3. **FFI exposure.** It is exposed (or not) across the C ABI.
4. **Optionality.** Users should be able to omit it, and it has heavy dependencies or a distinct API. Small additive extensions are Cargo features instead.
5. **Platform.** `no_std` vs `std`, or platform-specific code.
6. **Stability.** A small, rarely-changing interface (core traits/types) that others implement or build against is separated from fast-moving implementation, so dependents don't churn (the `rand_core`/`futures-core` pattern). The mod/plugin-facing API (M4+) is the prime candidate.
7. **Enforced layering.** A lower layer must be provably unable to depend on a higher one. Crates make dependency cycles impossible; modules don't.
8. **Measured compile time.** `cargo build --timings` shows the crate on the critical path and a split improves parallelism. This requires measurement, not intuition.
9. **Forced by the language.** Procedural macros live in their own crate.

**Keep code in one crate when any of these holds** (these override the split reasons unless an ADR says otherwise):

1. **Coherence.** Splitting would make needed trait impls impossible under the orphan rule, or force newtype workarounds. Corollary: foundational traits that many crates implement (e.g. M2 reflection) live at the **bottom** of the dependency graph.
2. **Shared internals.** The pieces need each other's private details. Splitting would force `#[doc(hidden)] pub` back doors, which also contradicts ADR 0009.
3. **Lockstep change.** They always change together, so a split only adds release overhead.
4. **Too small to stand alone.** No boundary from the list above, and fixed per-crate costs (metadata, build-graph nodes, maintenance) would outweigh any benefit.

Topic alone is not a reason to split. Expect many crates over time, but no empty crates up front.

**Every crate states its justification** (the split criteria it meets) in the first paragraph of its crate-level docs and in ADR 0004's crate table. M0+M1:

| Crate | Justification |
|---|---|
| `engine_handle` | Platform (`no_std` + `alloc`), dependency-free; FFI exposure (handles cross the C ABI in M4); layering (foundation for the ECS, assets, FFI) |
| `engine_random` | Stability (output streams frozen for all of 0.x, a different change cadence); platform (`no_std`, no `alloc`); independent reuse (optional on its own) |
| `engine_time` | Platform (`no_std` core with a `std` feature for `SystemClock`); independent reuse; layering (no dependency on math or handles) |
| `engine_math` | Dependencies (`glam`); platform (`std`) |
| `engine` | Facade: the single dependency for users, re-exporting the above |

**Layout and versioning:**
- The layout stays flat (`libs/*`, `engine_` prefix), following common practice for large workspaces (matklad, "Large Rust Workspaces").
- When publishing is decided, the expected default is **lockstep versioning**: every `engine_*` crate shares one version and is released together, with the facade as the main entry point. This gets its own ADR at publish time.

### 3.2 Dependency graph (M0+M1)

```
engine_handle   (no deps)
engine_random   (no deps)
engine_time     (no deps)
engine_math  →  glam
engine       →  engine_handle, engine_random, engine_time, engine_math
fixed_step_sim → engine            (dev: dhat for allocation assertions)
engine_benches → engine_*, criterion, slotmap, thunderdome, rand, rand_xoshiro, fastrand   (dev-only; gungraun added in M2)
xtask        →  (std only; minimal deps)
```

Leaf crates do not depend on each other unless a real need appears.

---

## 4. Engineering policies

### 4.1 Toolchain and MSRV

- `rust-toolchain.toml` pins the exact stable version (currently `1.97.0`) with components `rustfmt`, `clippy`, and the `thumbv7em-none-eabihf` target for `no_std` checks. Bumps are deliberate PRs.
- `[workspace.package] rust-version` equals the pinned version. While at 0.x the MSRV moves freely. At publishing time, revisit (e.g. "latest stable − 2").
- Miri and cargo-fuzz (from M2, §4.12) use a separately pinned nightly, recorded in `xtask` and CI (not in `rust-toolchain.toml`).
- `Cargo.lock` is committed.

### 4.2 Lints

`[workspace.lints]`, inherited by every crate via `[lints] workspace = true`:

- **rustc:** `unsafe_code = "deny"`, `missing_docs = "warn"`, `unreachable_pub = "warn"`, `rust_2018_idioms = { level = "warn", priority = -1 }`, `missing_debug_implementations = "warn"`.
- **clippy:** `pedantic = { level = "warn", priority = -1 }`, `undocumented_unsafe_blocks = "deny"`, `unwrap_used = "warn"`, `dbg_macro = "warn"`, `todo = "warn"`, `print_stdout = "warn"` (libraries only; examples/xtask/benches allow it). Pedantic allowances are listed in `Cargo.toml` with a one-line justification each. Starting allowances: `module_name_repetitions`, `must_use_candidate` (we apply `#[must_use]` deliberately instead).
- **rustdoc:** `broken_intra_doc_links = "deny"`, `private_intra_doc_links = "warn"`.
- Tests, examples, benches, and xtask may use `unwrap`/`expect`. Library tests get this via `clippy.toml`'s `allow-unwrap-in-tests`/`allow-expect-in-tests` (§4.9); examples/benches/xtask allow it at the crate root.
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
- The Miri and fuzzing CI infrastructure is built at the start of M2, together with the first allowlisted crate (§4.12). M0+M1 has no `unsafe`, so these requirements have nothing to apply to yet.

### 4.4 API design guidelines

Follow the [Rust API Guidelines checklist](https://rust-lang.github.io/api-guidelines/checklist.html). Specific rules:

- **Naming:** RFC 430 casing. Conversions are `as_`/`to_`/`into_`. Constructors are `new`, `with_*`, `from_*`. No `get_` prefix on getters.
- **Common traits:** implement `Debug` always. Implement `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `PartialOrd`, `Ord`, `Default` wherever semantically meaningful. Every public type is `Send + Sync` unless there is a documented reason not to be. Static assertions in tests verify this.
- **`#[must_use]`** on functions whose result being ignored is almost certainly a bug (e.g. `HandlePool::insert`, `Prng::stream`).
- **Errors:** each crate defines its own error enum with hand-written `Display` + `core::error::Error`, marked `#[non_exhaustive]`. No `thiserror`/`anyhow` in libraries.
- **Panics:** library code panics only on documented contract violations (e.g. an empty range passed to `Prng::range`), each with a `# Panics` doc section. Otherwise it returns `Option`/`Result`.
- **Docs:** every public item is documented. Every public type and most public functions include a runnable example (doctest). Each crate's root docs explain purpose, guarantees (determinism, complexity), and a quick start.
- **Prelude:** only the facade crate has a prelude. Leaf crates expose a flat, small public surface.
- **Ownership-first design:**
  - Cross-object references are `Copy` handles, never Rust references stored in data. Borrows happen only at the moment of access.
  - Return values that don't need to borrow their source don't (e.g. `Steps`).
  - Simultaneous mutable access to several elements is provided explicitly (`get_disjoint_mut`), so users aren't pushed toward `RefCell` or `unsafe`.
  - Builders take and return `self` by value and are `#[must_use]`.
  - Invalid states are made unrepresentable where it costs little (`NonZeroU32`, `Option`-returning constructors for state restoration).
- **Escape hatches are explicit:** no API silently erases or converts types (e.g. no `Handle::cast`).
- **`#[inline]` policy:** Rust only inlines non-generic functions across crate boundaries when they are marked `#[inline]` or LTO is on. Because the engine is many crates:
  - small, hot, non-generic public functions are `#[inline]` (e.g. `Prng::next_u64`, `Handle::index`/`to_bits`, `Tick::next`, `scalar::*`);
  - generic functions (e.g. `HandlePool<T>` methods) are compiled into the caller's crate and need no annotation;
  - `#[inline(always)]` is not used without a benchmark justifying it;
  - benchmarks verify the policy.
- **Dev-build performance:** game code is often unusably slow at `opt-level = 0`. The workspace sets `[profile.dev] opt-level = 1` and `[profile.dev.package."*"] opt-level = 3` (dependencies fully optimized). The README tells downstream users to do the same, since a library cannot set its users' profiles.
- **Workspace dependency gotcha:** `[workspace.dependencies]` entries for crates with a `std` feature declare `default-features = false`. Each member enables `std` explicitly, so `no_std` builds don't silently pull in `std` through Cargo feature unification.
- **Fallible twins:** every public API with a documented panic also has a non-panicking counterpart: std's `checked_*` naming for arithmetic-like operations (`Tick::checked_next`), `try_*` otherwise (`HandlePool::try_insert`, `Prng::try_range`, `FixedTimestep::try_new`, `ManualClock::try_set`). The panicking version stays as the ergonomic default for Rust callers. The fallible one serves FFI, mods, and untrusted input, so the M4 C layer never has to duplicate preconditions. `Index`'s twin is `get`.
- **Debuggability:**
  - **`#[track_caller]`** on every function with a documented panic (including `Index`/`IndexMut` impls), so the panic location points at the caller's line rather than the library.
  - **Actionable messages:** what was wrong, the offending value(s), and what to do instead. Example: `Prng::range: empty range 5..5 (start must be < end); use try_range to handle this without panicking`.
  - **`Debug` designed for humans:** `Handle<Particle>(3v2)`; `HandlePool` prints as a map `{Handle(..): value, ..}` rather than its slot vector; `Prng` prints its state (not secret; useful for replays).
  - **Invariant checking:** internal invariants are guarded by `debug_assert!`. Each stateful type has a test-only `check_invariants(&self) -> Result<(), InvariantViolation>` (amended 2026-11-12: it returns the violation instead of panicking, so a caller can report it) that the property tests call after every operation.
- **Tooling uses only public APIs (ADR 0009):** nothing the engine's own tools need (inspectors, editor, remote protocol, profilers) may require private access. If a tool needs it, it becomes a designed, documented public API. This keeps the API complete and honest for third-party tool builders.
- **Strings:** UTF-8 (`&str`/`String`) everywhere. No platform string types in public APIs. Encoding conversion happens only in FFI/platform layers.
- **Feature flags:** additive only. `std` is a default feature where applicable. No feature changes observable behavior of deterministic output.

### 4.5 Licensing and originality (ADR 0002)

- Rename `LICENSE` → `LICENSE-MIT`, add `LICENSE-APACHE` (Apache-2.0 text), and add a "License" section to `README.md` with the standard dual-license contribution clause.
- `deny.toml` allows only: `MIT`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`, `Unicode-3.0`. Unknown registries and git sources are denied. Duplicate versions warn. Security advisories deny.
- **AI-assisted development:** code is written from specifications and papers. Code from other repositories is never pasted in. Where an implementation closely follows a known published design, the docs cite that design.
- No code is copied from other projects. When a design is inspired by published work, the crate docs or ADR cite it. Published algorithms (xoshiro256++, SplitMix64, Lemire's bounded random integers, Fisher–Yates) are implemented from their papers/specifications. Their published test vectors may be used as golden values.

### 4.6 Process

- **ADRs** in `docs/adr/NNNN-kebab-title.md` with sections Status, Context, Decision, Consequences. M0+M1 delivers:
  - `0001-unsafe-policy.md`
  - `0002-originality-and-dependency-licensing.md`
  - `0003-same-build-determinism.md`
  - `0004-crate-splitting-rule.md`
  - `0005-coordinate-system-and-units.md`
  - `0006-large-world-precision.md`
  - `0007-host-owns-the-loop.md`
  - `0008-no-global-state.md`
  - `0009-tooling-uses-public-apis.md`

  ADR 0003 also covers the determinism guardrails (§4.9) and the snapshot-able state principle.
- **Commits:** conventional commits (`feat:`, `fix:`, `docs:`, `test:`, `perf:`, `ci:`, `chore:`).
- **Branching:** feature branches merged to `main` via PR. CI must be green.
- **README.md:** project purpose, status (experimental, 0.x), crate map, how to run `cargo xtask ci`, license.

### 4.7 CI (`.github/workflows/ci.yml`)

All jobs invoke `cargo xtask <task>` so local and CI runs are identical.

**Tiering (while the repo is private).** GitHub bills private-repo minutes at 10x for macOS and 2x for Windows, against 2,000 included minutes/month on the Free plan.
- **Every push:** all ubuntu jobs.
- **PRs into `main`, plus the nightly workflow:** the macOS and Windows legs of the clippy/test jobs.
- **All workflows:** `concurrency` groups cancel superseded runs, and the cargo registry, git dependencies, and build target directories are cached (`Swatinem/rust-cache`).

When the repo goes public (minutes become free), the macOS/Windows legs move to every push. This is a one-line trigger change, recorded in the workflow file as a comment.

| Job | Runner(s) | Command | Gate |
|---|---|---|---|
| fmt | ubuntu | `cargo xtask fmt --check` | yes |
| clippy | ubuntu, macOS, windows | `cargo xtask clippy` (all targets, all features) | yes |
| test | ubuntu, macOS, windows | `cargo xtask test` (`cargo nextest run` + `cargo test --doc`); on ubuntu also `cargo xtask test --release --include-ignored` for the statistical tests (§7) | yes |
| doc | ubuntu | `cargo xtask doc` | yes |
| no_std | ubuntu | `cargo xtask no-std` (build `engine_handle`, `engine_random`, `engine_time` with `--no-default-features` for `thumbv7em-none-eabihf`) | yes |
| wasm | ubuntu | `cargo xtask wasm`: build `engine` (default features) for `wasm32-unknown-unknown`; run the leaf crates' and `engine_math`'s tests (including the PRNG golden tests) on `wasm32-wasip1` under `wasmtime`. That is a 32-bit target, so 32-bit `usize` is covered. See the WASM test notes below | yes |
| check-unsafe | ubuntu | `cargo xtask check-unsafe` | yes |
| check-statics | ubuntu | `cargo xtask check-statics` (§4.10) | yes |
| miri | ubuntu (pinned nightly) | `cargo xtask miri` | **from M2** |
| deny | ubuntu | `cargo xtask deny` | yes |
| bench-build | ubuntu | `cargo bench --no-run` | yes |
| bench-regress | ubuntu (Valgrind via `gungraun/setup-gungraun`) | `cargo xtask bench-regress` (instruction counts vs the PR's merge-base, measured in the same job; fail if > 5%; see §8) | **from M2**; a `perf-regression-accepted` PR label downgrades it to report-only |
| coverage | ubuntu | `cargo llvm-cov nextest` → summary in job output | **from M2**, report only |

**WASM test notes:**
- proptest is declared as `proptest = { version = "…", default-features = false, features = ["std"] }` in `[workspace.dependencies]`. Its default `fork`/`timeout` features pull in `rusty-fork` → `wait-timeout`, which only compiles for unix/windows. A broken dev-dependency fails the entire test build for a target, so per-test `cfg` cannot work around it.
- libtest ignores `#[should_panic]` tests on every wasm target except emscripten, and panics abort on WASI. So the `# Panics` contracts are tested on native targets only, and proptest failures on WASI abort without shrinking (shrink by reproducing on native).
- The README's "Platform support" section records both facts.

`.github/workflows/nightly.yml` runs on a daily schedule: `cargo xtask ci` against the latest stable toolchain (an early warning for toolchain bumps), plus the macOS/Windows legs. From M2 it also runs each fuzz target for 10 minutes. It is report-only.

Dependabot gains the `cargo` (weekly, grouped) and `github-actions` (weekly) ecosystems alongside the existing `devcontainers` entry.

### 4.8 `xtask` commands

`ci` runs **every gating job in §4.7**, in sequence: fmt-check, clippy, test, doc, no-std, wasm, check-unsafe, check-statics, deny, bench-build (plus miri from M2). `bench-regress` is the exception: it needs a merge-base and Valgrind, so it runs via its own command. `ci --quick` runs only fmt-check, clippy, and test for fast local iteration. Other commands: `fmt [--check]`, `clippy`, `test`, `doc`, `no-std`, `wasm`, `check-unsafe`, `check-statics`, `deny`, `bench [--save <name>] [--compare <a> <b>]`, `prng-stats` (streams PRNG output to stdout for piping into PractRand; documents usage, does not bundle PractRand). Added in M2: `miri`, `fuzz <target> [--time <secs>]`, `bench-regress`, `coverage`.

`xtask` has minimal dependencies: std, at most a small argument parser, and `syn` for `check-statics`. It shells out to `cargo` and external tools, and prints an actionable install hint when a tool (`cargo-nextest`, `cargo-deny`, `critcmp`, `wasmtime`; from M2 also `cargo-llvm-cov`, `gungraun-runner`, `cargo-fuzz`) is missing.

### 4.9 Determinism guardrails (part of ADR 0003)

**Principle:** simulation code receives time, randomness, and input only as explicit parameters (a `TimeSource`, a `Prng`, input data). It never reads ambient or global state: no wall clocks, no thread-local generators, no OS randomness, no iteration over randomly-seeded hash collections.

**Enforcement:** the workspace-root `clippy.toml` applies to every crate:

```toml
allow-unwrap-in-tests = true
allow-expect-in-tests = true

disallowed-types = [
  { path = "std::collections::HashMap", reason = "iteration order is randomized per process; use BTreeMap (or engine_collections' deterministic map once it exists)" },
  { path = "std::collections::HashSet", reason = "iteration order is randomized per process; use BTreeSet (or engine_collections' deterministic set once it exists)" },
  { path = "std::hash::RandomState",    reason = "OS-seeded; breaks same-build determinism" },
]
disallowed-methods = [
  { path = "std::time::Instant::now",    reason = "ambient time; take a TimeSource parameter" },
  { path = "std::time::SystemTime::now", reason = "ambient time; take a TimeSource parameter" },
  # + the transcendental float methods listed below
]
disallowed-macros = [
  { path = "std::thread_local", reason = "global state; see ADR 0008" },
]
```

- **Sanctioned item-level exceptions** use `#[expect(clippy::disallowed_methods, reason = "...")]` on the smallest possible item. `expect`, not `allow`, so the exception is flagged once it becomes unnecessary. M1 has exactly two: `SystemClock` in `engine_time`, and the `engine_math::scalar` funnel.
- **Crate-level opt-outs** for non-simulation crates (`xtask`, `engine_benches`) use `#![allow(...)]` with a comment, not `#![expect(...)]`. A crate-level `expect` that never fires produces an `unfulfilled_lint_expectations` warning, which fails CI under `-D warnings`.
- **Tests** may use `unwrap`/`expect` via the `allow-*-in-tests` settings above (see §4.2).
- **Deterministic hash collections:** M1 needs none. The `std` path bans also cover `std::collections::HashMap<K, V, S>` with a custom hasher, which is intended. Deterministic maps will be `engine_collections`' own types (expected in M2), built on a fixed-seed hasher. Whenever a hashing crate is added (e.g. hashbrown, foldhash), its randomly-seeded default types are added to this ban list in the same PR.
- **Float notes for same-build determinism:**
  - Rust does not contract `a * b + c` into fused multiply-add (FMA) implicitly, and we never enable fast-math.
  - Basic IEEE operations (`+ − × ÷ sqrt`) are bit-reproducible.
  - Transcendental functions (`sin`, `exp`, …) call the platform's math library and may differ *across* platforms, but not within one build.
  - **SIMD reduction order differs across architectures.** glam's SIMD-backed types (`Vec3A`, `Vec4`, `Quat`, `Mat3A`, `Mat4`, `Affine3A`) compute horizontal sums in backend-specific orders. Verified in glam 0.30.10: SSE2's 4-lane dot product sums `(x+z)+(y+w)`, NEON uses a pairwise horizontal add, and the scalar fallback sums left to right. So `dot`, `length`, `normalize`, and quaternion/matrix products can differ in the last bit between x86_64 and aarch64, even with identical inputs. This is consistent with same-build determinism, but it limits what can share a cross-architecture golden value (§5.6).
- **Determinism is independent of thread count and timing.** The same build must produce identical results on a 4-core and a 14-core machine, and with 1 or N worker threads. Rules for M2/M3 and beyond:
  - parallel work uses **fixed partitioning** (chunk boundaries depend on data size, never on thread count);
  - **reductions combine partial results in a fixed tree/order** (floating-point addition is not associative);
  - deferred commands from parallel systems are **merged in a deterministic order** (e.g. by system order, then chunk index), never in completion order;
  - no floating-point accumulation through atomics.
- **Handle allocation order is simulation input.** Because PRNG streams are commonly keyed by `handle.to_bits()`, which handle an entity receives affects simulation output. Rules:
  1. Handles are allocated only at deterministic points: single-threaded code, or deferred commands applied in schedule order. No allocation order may depend on thread timing (relevant to M2/M3 parallel spawning).
  2. The `to_bits` layout (§5.1) is part of the deterministic output. Changing it is a breaking change to golden values and requires an ADR.

**Transcendental math funnel.** Same-build determinism rules out lockstep multiplayer across different platforms (e.g. Windows x86 vs macOS ARM). Server-authoritative and same-platform rollback models are unaffected. To keep a later upgrade to cross-platform determinism cheap:

- `engine_math::scalar` provides generic free functions over a sealed `Float` trait (implemented for `f32`, `f64`): `sin`, `cos`, `tan`, `sin_cos`, `asin`, `acos`, `atan`, `atan2`, `sinh`, `cosh`, `tanh`, `exp`, `exp2`, `exp_m1`, `ln`, `ln_1p`, `log2`, `log10`, `powf`, `powi`, `cbrt`, `hypot`. Usage: `scalar::sin(x)`, with the type inferred.
- In M1 these delegate to `std`.
- `clippy.toml` `disallowed-methods` bans the corresponding inherent methods (`f32::sin`, `f64::sin`, …, `f32::powi`, …) in all crates. `engine_math::scalar` carries the one sanctioned `#[expect]`. `xtask`, benches, and tests may opt out.
- Correctly-rounded IEEE operations stay allowed: `+ − × ÷`, `sqrt`, `mul_add`, `abs`, `floor`/`ceil`/`round`/`trunc`, `min`/`max`, `rem_euclid`, `copysign`.
- **Upgrade path (future ADR, not M1):** three changes together:
  1. back `scalar` with the pure-Rust `libm` crate;
  2. enable glam's `libm` feature (routes glam's internal transcendentals, e.g. `sin_cos` in rotation constructors, through libm);
  3. enable glam's `scalar-math` feature (removes the architecture-specific SIMD reduction orders).

  `scalar-math` **changes the size and alignment** of `Vec3A`/`Vec4`/`Quat`/`Mat3A`/`Mat4`, and therefore of `Transform`/`WorldTransform`. That breaks any layout C#, snapshots, or serialized data depend on (the same failure mode as Unreal's Large World Coordinates migration). The static layout assertions in §5.4 exist to make such a change loud.

  Because the switch changes output values and layout, it is a deliberate, versioned, engine-wide decision, not a Cargo feature (§4.4: features never change deterministic output). Golden values would be regenerated once and then become identical across all targets.
- **Feature-unification hazard:** any crate in a downstream dependency graph can enable glam's `fast-math` or `scalar-math` features and silently change our numerics. `engine_math`'s static layout assertions catch `scalar-math`. `fast-math` cannot be detected, so the README documents it as unsupported.
- M1 code that needs trigonometry (e.g. `Transform2d::to_affine`, `to_2d`'s twist extraction) calls `scalar`, not std directly.

### 4.10 Instance isolation (ADR 0008)

**Principle:** there is no global mutable state anywhere in the engine. All state lives in instances that the caller creates and passes explicitly. Any number of instances can coexist in one process and on any threads, and none can observe another.

- **Lint:** `static mut` is already rejected (it requires `unsafe`).
- **Check:** `cargo xtask check-statics` parses every library crate (using `syn`) and fails on:
  - a `static` item whose type contains interior mutability (`Cell`, `RefCell`, `UnsafeCell`, `Mutex`, `RwLock`, `Atomic*`, `OnceCell`, `OnceLock`, `LazyLock`, `LazyCell`);
  - any `thread_local!` invocation.
- **Exceptions** require a preceding `// engine:allow-global-state: <reason>` comment, and `check-statics` reports each one. M1 has none.
- **The check is best-effort, not a proof.** It matches type names syntactically, so `type` aliases, user structs that wrap a `Mutex`, and third-party lazy-static macros can evade it. Code review remains the backstop, and `disallowed-macros` covers `thread_local!`.
- **Dependencies can carry global state** (logging/tracing dispatchers, rayon's global pool, the global allocator). Policy:
  - **Diagnostics facades** (logging, tracing, profiling) are the one sanctioned category of process-global state. Engine code only *emits* events; only the host installs subscribers. This carve-out is decided now because M3's profiling and the tooling milestone depend on it.
  - **Thread pools** are always explicit instances. When rayon (or similar) is added, its global-pool entry points (`rayon::spawn`, `rayon::join`, `par_iter` on the global pool, etc.) are added to `disallowed-methods`.
  - **The global allocator** belongs to the host application, never to an engine library.
- **Immutable statics and consts** (lookup tables, constants) are fine.
- **Consequences for later milestones:**
  - M2's type registry and component IDs are per-world (or per explicitly-shared registry instance), never process-global.
  - M3's thread pool is an instance the host creates.
  - M4's C API passes an explicit context handle to every function.

### 4.11 Concurrency, parallelism, and cache efficiency

M1 contains no threads, but the rules are fixed now so M2/M3 inherit them rather than retrofit them.

**Correctness of concurrent code:**
- **Confinement:** atomics, lock-free structures, and custom synchronization live only in designated crates (expected: the M3 scheduler and the M2 storage internals), listed in `concurrency-allowlist.toml` with an ADR link, mirroring the `unsafe` allowlist. Elsewhere, concurrency comes from those crates' safe APIs.
- **Verification required for every allowlisted crate:**
  - **`loom`** model tests for each synchronization protocol. loom exhaustively explores interleavings under the C11 memory model. It is not a proof (it has known gaps, e.g. some SeqCst and load-buffering behaviors), so it complements rather than replaces the other checks.
  - **Miri** (already required for `unsafe`), which also detects data races and some weak-memory behaviors.
  - **ThreadSanitizer** (`-Zsanitizer=thread`, pinned nightly) on the stress tests in CI.
  - **Stress tests** run under random scheduling. `shuttle`, a randomized scheduling-exploration tool, is an optional addition.
- **`Send`/`Sync` audits:** static assertions for every public type (already in §7). Any manual `unsafe impl Send/Sync` falls under the `unsafe` policy.

**Cache efficiency:**
- **Layout discipline:**
  - Hot types have static size and alignment assertions, and their docs state bytes per element (`Handle` = 8, transforms per §5.4).
  - `HandlePool` is dense (contiguous values, O(len) iteration); its per-slot and per-value overhead is documented (§5.1).
  - The data-oriented storage decision (structure-of-arrays columns vs archetype tables) is M2's to make with benchmarks.
- **False sharing:** mutable per-thread or per-worker data (counters, queues, accumulators) is padded and aligned to the destructive-interference size. That is 128 bytes on x86_64 (the spatial prefetcher pulls 64-byte line pairs) and on aarch64 including Apple Silicon (128-byte lines on performance cores). crossbeam-utils' `CachePadded` uses these values. M3 decides whether to depend on crossbeam-utils or define an equivalent.
- **Access patterns:** APIs favor linear, batched iteration over scattered per-element lookups. Per-element lookups (`HandlePool::get`) exist for convenience; bulk work goes through iterators/queries.
- **Measurement (§8):**
  - Benchmark sizes straddle the benchmark machine's cache hierarchy (L1/L2/SLC/DRAM, recorded in `docs/benchmarks.md`), with both sequential and random access.
  - Gungraun benchmarks enable Callgrind's cache simulation and report estimated L1/last-level-cache miss counts alongside instruction counts.
  - Hardware-counter profiling (Instruments CPU Counters on macOS, `perf stat` on Linux) is documented for investigations.

### 4.12 Infrastructure staging

Policies and decisions in this spec apply from day one. **Infrastructure arrives when the risk it guards against first appears.** Building it earlier would cost effort without catching anything, and adding it later requires no rework.

| When | Infrastructure | Trigger |
|---|---|---|
| **M0+M1** | Workspace, lints, `clippy.toml` bans, `cargo-deny`, `check-unsafe`, `check-statics`; tiered fmt/clippy/test/doc CI; `no_std` + wasm build checks; WASI test run (PRNG goldens on 32-bit); proptest; golden tests; Criterion comparison benchmarks + `docs/benchmarks.md`; the sample's determinism, snapshot, and allocation tests | Always needed |
| **Start of M2** | Miri job; `fuzz/` + nightly fuzzing; Gungraun instruction-count + cache-sim regression gate; coverage report; periodic `cargo-mutants` triage | First allowlisted `unsafe` crate and first performance-critical storage code |
| **Start of M3** | loom model tests, ThreadSanitizer job, stress-test harness for `concurrency-allowlist.toml` crates | First concurrency-allowlisted crate |

The allowlist files (`unsafe-allowlist.toml`, `concurrency-allowlist.toml`) exist, empty, from M0, so the rules are visible and enforced (`check-unsafe`) from the start.

---

## 5. Component designs

Code below shows the public API surface. Private fields are indicative.

### 5.1 `engine_handle`

> **Amended 2026-11-12, after implementation and review** (the API blocks below are updated in place and marked): the allocator is now two types, `HandleAllocator` (lean, no iteration) and `DenseHandleAllocator` (iterable), sharing one `SlotTable` with the pool ([ADR 0010](../../adr/0010-two-handle-allocators.md)); added `reserve`/`try_reserve`, `with_fixed_capacity`, `insert_with`, `allocate_many`, `From`/`TryFrom<u64>` for `Handle`, a `limit()` on `CapacityError`, and `check_invariants`; slots shrank to 8 bytes with a 2³¹−2 limit.

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
    /// Stable encoding: `(index as u64) << 32 | generation as u64`. Never 0.
    /// Index is in the high half, so numeric order of bits == `Ord` order of handles.
    /// This layout is frozen: it feeds PRNG stream ids, so it is part of deterministic output (§4.9).
    pub const fn to_bits(self) -> u64;
    /// Inverse of `to_bits`. Returns `None` iff the generation (low) half is 0.
    pub const fn from_bits(bits: u64) -> Option<Self>;
    // Also (amended 2026-11-12): `impl From<Handle<T>> for u64` (= to_bits) and
    // `impl TryFrom<u64> for Handle<T>` (= from_bits, with the rejected bits in `InvalidHandleBits`).
}
```

- **There is deliberately no `cast<U>()`.** It would silently defeat the type safety the phantom type exists to provide. Type-erased uses (FFI, serialization) go through `to_bits`/`from_bits`, where the type change is explicit at both ends.
- **Layout decision (2026-11-12): 32-bit index + 32-bit generation, no pool/world tag.** This matches mainstream practice: Bevy, Unity DOTS, flecs, and EnTT all use index + generation without a world tag, and treat IDs as meaningful only within the issuing world. Reserved ID bits in other engines (e.g. flecs' relationship-pair flags) carry ECS-level meaning. They belong to M2's own entity-ID type, which may have its own layout and is not bound by `Handle`'s frozen encoding.
- **Known limitation (documented, not fixed):** a `Handle<T>` issued by one pool, used on a *different* pool of the same `T`, can resolve to an unrelated element. Detecting this would need extra bits per handle (breaking the 8-byte size and the FFI encoding) or branded-lifetime tricks that make the API much harder to use. The M2 ECS may add per-world tagging if needed.

- `Copy`, `Clone`, `PartialEq`, `Eq`, `PartialOrd`, `Ord` (by `(index, generation)`, identical to the numeric order of `to_bits`), `Hash`, and `Debug` are implemented manually, without bounds on `T`. `Debug` prints `Handle<Particle>(3v2)`, i.e. slot 3, generation 2, using the short type name.
- `PhantomData<fn() -> T>` keeps the handle covariant in `T` and `Send + Sync` regardless of `T`, with no drop-check implications. A handle refers to a `T` but never owns one.

```rust
/// Dense ("sparse set with generations") storage: values are contiguous, so iteration is O(len).
pub struct HandlePool<T> {
    slots: SlotTable,           // shared with both allocators: per-index generation + link, LIFO free list, retirement, slot limit
    handles: Vec<Handle<T>>,    // dense, parallel to `values`
    values: Vec<T>,             // dense, contiguous
}

impl<T> HandlePool<T> {
    pub const fn new() -> Self;
    pub fn with_capacity(capacity: usize) -> Self;
    /// Amended 2026-11-12. Allocates every array up front and never grows: once `capacity` slots
    /// exist, `try_insert` returns `CapacityError`. Insert/remove/clear/retain/drain then never
    /// touch the heap (tested with dhat). `reserve` is still an explicit request to allocate.
    pub fn with_fixed_capacity(capacity: usize) -> Self;
    /// Amended 2026-11-12. Room for `additional` more values, counted from `len()`.
    /// # Panics: on capacity overflow or allocation failure (message names `try_reserve`). `#[track_caller]`.
    pub fn reserve(&mut self, additional: usize);
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError>;
    /// # Panics: if 2³¹−2 slots are already in use (see `try_insert`). `#[track_caller]`.
    #[must_use] pub fn insert(&mut self, value: T) -> Handle<T>;
    /// Fallible twin of `insert`; on failure, ownership of `value` is returned in the error.
    pub fn try_insert(&mut self, value: T) -> Result<Handle<T>, CapacityError<T>>;
    /// Amended 2026-11-12. `make` receives the handle its value will have (for values that store
    /// their own handle). It runs before the pool changes, so a panicking `make` leaves the pool
    /// untouched. Issues exactly the handle `insert` would. # Panics: if full, without calling
    /// `make` (use `try_insert_with`). `#[track_caller]`.
    #[must_use] pub fn insert_with(&mut self, make: impl FnOnce(Handle<T>) -> T) -> Handle<T>;
    /// `None`, without calling `make`, if the pool is full.
    pub fn try_insert_with(&mut self, make: impl FnOnce(Handle<T>) -> T) -> Option<Handle<T>>;
    pub fn get(&self, handle: Handle<T>) -> Option<&T>;
    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T>;
    pub fn remove(&mut self, handle: Handle<T>) -> Option<T>;
    /// Mutable access to several distinct elements at once (e.g. both bodies in a collision).
    /// Returns `None` if any handle is stale or two handles refer to the same slot.
    /// Built on std's `slice::get_disjoint_mut`; no `unsafe`.
    pub fn get_disjoint_mut<const N: usize>(&mut self, handles: [Handle<T>; N]) -> Option<[&mut T; N]>;
    pub fn contains(&self, handle: Handle<T>) -> bool;
    /// Keeps only the elements for which `keep` returns true, visiting in dense order.
    /// Removed elements behave exactly like `remove` (including swap-removal).
    pub fn retain(&mut self, keep: impl FnMut(Handle<T>, &mut T) -> bool);
    pub const fn len(&self) -> usize;       // len, is_empty and capacity are `const fn` (amended 2026-11-12)
    pub const fn is_empty(&self) -> bool;
    /// Number of values the pool can hold without reallocating its dense value storage
    /// (std `Vec::capacity` semantics for `values`).
    pub const fn capacity(&self) -> usize;
    /// Shrinks the dense value/handle storage to fit `len()`. The slot array cannot shrink,
    /// because indices must stay stable; it costs 8 bytes per slot ever used.
    pub fn shrink_to_fit(&mut self);
    /// All live values as one contiguous slice, in dense order (same order as `iter`).
    /// The fastest way to process every value when handles aren't needed.
    pub fn values(&self) -> &[T];
    pub fn values_mut(&mut self) -> &mut [T];
    pub fn clear(&mut self);
    pub fn iter(&self) -> Iter<'_, T>;          // Item = (Handle<T>, &T), dense order
    pub fn iter_mut(&mut self) -> IterMut<'_, T>; // Item = (Handle<T>, &mut T), dense order
    /// Removes all values, yielding them in dense order. When the `Drain` is
    /// dropped (fully consumed or not), the pool is in exactly the state `clear()` would produce.
    pub fn drain(&mut self) -> Drain<'_, T>;     // Item = (Handle<T>, T)
}

/// Returned by `try_insert` when the pool is full; carries the rejected value back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapacityError<T> { value: T, limit: u32 }   // amended 2026-11-12: carries the pool's own slot limit
impl<T> CapacityError<T> { pub fn into_inner(self) -> T; pub const fn limit(&self) -> u32; }
// + Display, core::error::Error (T: Debug)
// Also: Default, Debug (T: Debug), Clone (T: Clone), IntoIterator for &/&mut/owned,
// Index<Handle<T>> and IndexMut<Handle<T>> (panic on stale; documented, like HashMap's Index).
// All iterators are named types implementing ExactSizeIterator + FusedIterator
// (the remaining live count is tracked).
```

```rust
/// Lean handle allocation (ECS entity allocator shape). Amended 2026-11-12 (ADR 0010): one 8-byte
/// slot per slot ever used plus a free list, and no live-handle list, so no iteration.
pub struct HandleAllocator<T> { /* SlotTable + live count */ }

impl<T> HandleAllocator<T> {
    pub const fn new() -> Self;
    pub fn with_capacity(capacity: usize) -> Self;
    pub fn with_fixed_capacity(capacity: usize) -> Self;      // never grows; see HandlePool
    pub fn reserve(&mut self, additional: usize);              // + try_reserve, as for HandlePool
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError>;
    /// # Panics: if 2³¹−2 slots are in use. `#[track_caller]`.
    #[must_use] pub fn allocate(&mut self) -> Handle<T>;
    /// Fallible twin of `allocate`.
    pub fn try_allocate(&mut self) -> Option<Handle<T>>;
    /// `n` handles at once, exactly the ones `n` calls to `allocate` would issue. About 2x faster
    /// than a loop when it creates new slots; reusing freed slots gains 1.7-2x up to 100k handles
    /// and nothing at 1M (measured; see docs/benchmarks.md).
    /// # Panics: if fewer than `n` slots are available; nothing is allocated. `#[track_caller]`.
    #[must_use] pub fn allocate_many(&mut self, n: usize) -> Vec<Handle<T>>;
    /// All-or-nothing: `None`, with nothing allocated, if fewer than `n` slots are available.
    pub fn try_allocate_many(&mut self, n: usize) -> Option<Vec<Handle<T>>>;
    /// Returns `false` if the handle was already stale.
    pub fn free(&mut self, handle: Handle<T>) -> bool;
    pub fn is_alive(&self, handle: Handle<T>) -> bool;
    pub const fn len(&self) -> usize;
    pub const fn is_empty(&self) -> bool;
    pub fn clear(&mut self);                                   // O(slots), not O(len)
}

/// The previous design, kept for callers that need to iterate live handles: a `HandlePool<()>`
/// wrapper that also maintains the dense handle array. Same methods as `HandleAllocator`
/// (without `allocate_many`), plus:
pub struct DenseHandleAllocator<T> { /* HandlePool<()> */ }
impl<T> DenseHandleAllocator<T> {
    pub fn iter(&self) -> DenseAllocIter<'_, T>;    // live handles, dense order; ExactSize + Fused
    pub fn retain(&mut self, keep: impl FnMut(Handle<T>) -> bool);
}
```

**Semantics (identical for the pool and both allocators; for the same sequence of insert/allocate, remove/free and clear calls all three issue identical handles, which a model test checks):**

- New slots start at generation 1.
- Removing or freeing bumps the slot's generation, so every outstanding handle to it becomes stale (`get` → `None`, `contains` → `false`).
- Free slots are reused **LIFO**, which makes allocation order deterministic given the same operation sequence.
- **Generation exhaustion:** if bumping would overflow `u32::MAX`, the slot is permanently retired (never reused). This guarantees a stale handle can never become valid again, at the cost of one slot per 2³²−1 reuses.
- `clear()` bumps every occupied slot's generation (outstanding handles become stale) and returns all non-retired slots to the free list in ascending index order. Reuse order after `clear` is therefore deterministic.
- **Capacity:** at most 2³¹−2 (`0x7FFF_FFFE`) slots (amended 2026-11-12). A slot is exactly 8 bytes, because it packs its state into one `u32`: the top bit marks vacant, and the two largest values are sentinels (end of free list, retired). Handle indices still span the full `u32` range, so the `to_bits` layout is unaffected. `insert`/`allocate` panic beyond the limit (`# Panics` documented).
- **Dense storage (decision 2026-11-12):** values live in a contiguous `Vec<T>`, with a parallel dense `Vec<Handle<T>>`. Each slot records its value's dense position.
  - `insert` pushes to the dense arrays.
  - `remove` **swap-removes**: the last dense element moves into the hole, and its slot's dense position is updated.
- **Iteration order is dense order.** It is fully deterministic (a pure function of the operation sequence) but not ascending by slot index. Example: inserting a, b, c and removing a gives the order c, b.
- **Complexity:**
  - `insert`/`allocate`/`get`/`remove`/`free`/`contains` are O(1). `get` costs two array accesses (slot, then dense position).
  - Iteration, `retain`, `values()`, and `drain` are **O(len) over contiguous memory**.
- **Memory:** 8 bytes per slot ever used (a `u32` generation plus one packed `u32`: dense position, free link, or retired marker; slots never shrink; asserted by a unit test), plus 8 bytes per live value (its dense `Handle`), plus the values. `shrink_to_fit` releases dense capacity after a spike.
- Implementation is 100% safe Rust. The slot logic (generations, LIFO free list, retirement, slot limit) lives in one private `SlotTable` shared by all three types (amended 2026-11-12), so the rules cannot drift apart. A `Slot` holds a generation and an enum `Occupied { dense: u32 } | Vacant { next_free: Option<u32> } | Retired` packed into one `u32`.

### 5.2 `engine_random`

**Purpose:** a fast, deterministic, versioned-stable PRNG for simulation. Not cryptographically secure (stated prominently in docs).

**Naming:** the type is `Prng`, not `Rng`, so every call site states that the output is pseudorandom: a deterministic function of the seed, not a source of true or cryptographic randomness. Following Rust's convention for acronyms in type names (`Uuid`, `Tcp…`), it is written `Prng`. The crate keeps the broader name `engine_random`, so it can later hold related, non-generator code (distributions, noise) without a rename.

```rust
#![no_std]

/// xoshiro256++ generator; state seeded via SplitMix64.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Prng { s: [u64; 4] }

impl Prng {
    /// Expands `seed` into 256 bits of state using four successive SplitMix64 outputs.
    pub const fn from_seed(seed: u64) -> Self;
    // Raw draws are named `next_*`; operations are verbs.
    pub fn next_u32(&mut self) -> u32;          // upper 32 bits of next_u64
    pub fn next_u64(&mut self) -> u64;
    pub fn next_f32(&mut self) -> f32;          // (next_u64 >> 40) as f32 * 2^-24, in [0, 1)
    pub fn next_f64(&mut self) -> f64;          // (next_u64 >> 11) as f64 * 2^-53, in [0, 1)
    pub fn next_bool(&mut self) -> bool;        // top bit of next_u64
    /// # Panics: on an empty or invalid range (see below). `#[track_caller]`.
    pub fn range<R: SampleRange>(&mut self, range: R) -> R::Output;
    /// Fallible twin of `range`: `None` instead of panicking. Draws nothing when it returns `None`.
    pub fn try_range<R: SampleRange>(&mut self, range: R) -> Option<R::Output>;
    pub fn chance(&mut self, p: f64) -> bool;   // next_f64() < p
    pub fn shuffle<T>(&mut self, slice: &mut [T]);
    pub fn choose<'a, T>(&mut self, slice: &'a [T]) -> Option<&'a T>;
    pub fn fill_bytes(&mut self, dest: &mut [u8]);
    #[must_use] pub fn stream(&self, id: u64) -> Prng;
    /// Full generator state, for save/load, replays, and rollback.
    pub const fn to_state(&self) -> [u64; 4];
    /// Restores a state from `to_state`. Returns `None` iff all four words are zero, the one
    /// invalid xoshiro256++ state. `from_seed` can never produce it: SplitMix64's output
    /// function is a bijection, so at most one of four consecutive outputs can be zero.
    pub const fn from_state(state: [u64; 4]) -> Option<Self>;
}

/// Implemented for `Range` and `RangeInclusive` over u8..u64, usize, i8..i64, isize, f32, f64.
pub trait SampleRange: sealed::Sealed { type Output; /* sampling method is private */ }
```

**Semantics:**

**Normative algorithms.** Because every output stream is frozen (below), these definitions are part of the contract. Two implementations following them must produce identical values on every target, including 32-bit wasm.

- **`bounded_u64(n)`** returns a uniform value in `[0, n)` for `n ≥ 1`, using Lemire's nearly-divisionless method in 64 bits:
  ```text
  m = (next_u64() as u128) * (n as u128); lo = m as u64
  if lo < n:
      t = n.wrapping_neg() % n
      while lo < t: m = (next_u64() as u128) * (n as u128); lo = m as u64
  return (m >> 64) as u64
  ```
- **Integer ranges, all integer types:** `u8`–`u64`, `i8`–`i64`, `usize`, `isize`.
  - Convert to an inclusive range `[lo, hi]`. For `Range`, `hi = end − 1`, and an empty range is invalid.
  - Widen both bounds to 64 bits: unsigned types via `as u64`; signed types via `as i64`, then reinterpret as `u64`.
  - Let `span = hi.wrapping_sub(lo)`, computed in `u64`.
  - If `span == u64::MAX` (full 64-bit range), the result is `lo.wrapping_add(next_u64())`. Otherwise it is `lo.wrapping_add(bounded_u64(span + 1))`.
  - Narrow back to the original type.
  - **Every integer type, including `usize`/`isize`, uses this 64-bit path on every target.** Results never depend on pointer width.
- **Float `Range` (`lo..hi`):**
  - Invalid unless `lo` and `hi` are finite, `lo < hi`, and `hi − lo` is finite. This rejects `lo..lo`, which would otherwise loop forever, and overflow cases such as `-MAX..MAX`, which would otherwise produce `inf`/NaN.
  - Loop: `u = next_f32()` (or `next_f64()`); `x = lo + (hi − lo) * u`; return `x` if `x < hi`, otherwise redraw. Since `u = 0` gives `lo < hi`, the loop terminates with probability 1.
- **Float `RangeInclusive` (`lo..=hi`):**
  - Invalid unless both bounds are finite, `lo ≤ hi`, and `hi − lo` is finite.
  - `u = (next_u64() >> 40) as f32 / (2^24 − 1) as f32` for `f32` (for `f64`: `>> 11`, divided by `2^53 − 1`), which is uniform over [0, 1] inclusive.
  - Return `min(lo + (hi − lo) * u, hi)`. `lo..=lo` returns `lo`.
- **`try_range`** performs exactly the validity checks above and returns `None` without drawing. `range` panics with an actionable message naming the bounds received.
- **`shuffle`:** Fisher–Yates, descending: `for i in (1..len).rev() { j = bounded_u64(i + 1); swap(i, j) }`. Empty and length-1 slices draw nothing.
- **`choose`:** `None` for an empty slice (no draw); otherwise `slice[bounded_u64(len)]`.
- **`fill_bytes`:**
  - Fill in 8-byte chunks, each `next_u64().to_le_bytes()`.
  - A final partial chunk of `k` bytes takes the first `k` bytes of one more `next_u64().to_le_bytes()` and discards the rest.
  - Little-endian on every target.
- **`chance(p)`:** `p <= 0.0` never true, `p >= 1.0` always true, `NaN` always false. It never panics.
- **`stream(id)`:** returns an independent generator that is a pure function of `(self's current state, id)` and does not advance `self`. Definition:
  `seed = mix(s0 ^ s1.rotate_left(17) ^ s2.rotate_left(31) ^ s3.rotate_left(47) ^ mix(id))`, then `Prng::from_seed(seed)`, where `mix` is the SplitMix64 finalizer. Typical use: `let mut r = world_prng.stream(entity.to_bits());`. The same entity gets the same sequence regardless of system execution order or thread assignment.
- **Stream stability guarantee:** for a given seed, every method's output sequence is frozen across all 0.x releases. Golden tests enforce this; any change is a breaking change requiring an ADR.
- No `rand_core` trait impls in M1 (YAGNI). An optional, additive `rand_core` feature may be added later.
- **Deliberate non-impls** (documented on the type so nobody "fixes" them):
  - **Not `Copy`:** an implicit copy silently duplicates the generator, which produces correlated "random" values. Duplication must be an explicit `clone()` (or `stream`).
  - **Not `Default`:** a default would hide the seed, which is the input that determines everything.

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
    /// # Panics: on overflow (~9.7e9 years at 60 Hz). `#[track_caller]`.
    pub const fn next(self) -> Tick;
    /// Fallible twin of `next` (std `checked_*` naming).
    pub const fn checked_next(self) -> Option<Tick>;
    pub const fn get(self) -> u64;
}

/// Converts variable elapsed time into a whole number of fixed simulation steps.
#[derive(Clone, Debug)]
pub struct FixedTimestep { step: Duration, accumulator: Duration, max_steps: NonZeroU32, tick: Tick, dropped: Duration }

impl FixedTimestep {
    pub const DEFAULT_MAX_STEPS_PER_UPDATE: NonZeroU32;   // 8
    /// # Panics: if `step` is zero.
    pub fn new(step: Duration) -> Self;           // #[track_caller]
    /// Fallible twin of `new`: `None` if `step` is zero.
    pub fn try_new(step: Duration) -> Option<Self>;
    /// Step of exactly `round(1e9 / hz)` nanoseconds, computed as `(1_000_000_000 + hz/2) / hz`
    /// in integer arithmetic (e.g. 60 Hz → 16_666_667 ns). Never panics.
    pub fn from_hz(hz: NonZeroU32) -> Self;
    /// `NonZeroU32` makes a zero cap unrepresentable instead of a runtime panic.
    #[must_use] pub fn with_max_steps_per_update(self, max_steps: NonZeroU32) -> Self;
    /// Adds `elapsed`, commits up to `max_steps` whole steps, and returns them.
    /// Excess time beyond `max_steps * step` is discarded and added to `dropped_time()`.
    pub fn accumulate(&mut self, elapsed: Duration) -> Steps;
    /// Leftover fraction of a step in [0, 1), for render interpolation. Computed as
    /// `(accumulator_ns as f64 / step_ns as f64) as f32`, then clamped to
    /// `1.0_f32.next_down()` (f32 rounding alone can produce exactly 1.0).
    pub fn alpha(&self) -> f32;
    pub fn step(&self) -> Duration;
    pub fn current_tick(&self) -> Tick;        // tick of the next step to be produced
    pub fn dropped_time(&self) -> Duration;    // total time discarded by the step cap
    pub fn to_state(&self) -> FixedTimestepState;
    /// Returns `None` if `step` is zero or `accumulator >= step`
    /// (unreachable through normal use).
    pub fn from_state(state: FixedTimestepState) -> Option<Self>;
}

/// Complete, plain-data state of a `FixedTimestep`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FixedTimestepState {
    pub step: Duration,
    pub accumulator: Duration,
    pub max_steps_per_update: NonZeroU32,
    pub next_tick: Tick,
    pub dropped: Duration,
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

pub trait TimeSource { fn now(&self) -> Timestamp; }   // object-safe: `&dyn TimeSource` works
// Blanket impls, following std's `impl Read for &mut R` pattern, so `impl TimeSource`
// parameters accept borrowed clocks:
impl<T: TimeSource + ?Sized> TimeSource for &T { .. }
impl<T: TimeSource + ?Sized> TimeSource for &mut T { .. }

/// Not available on `wasm32-unknown-unknown`, where `std::time::Instant::now()` panics.
/// Platforms without a std clock supply their own `TimeSource` (e.g. one backed by `performance.now()`).
#[cfg(all(feature = "std", not(all(target_arch = "wasm32", target_os = "unknown"))))]
#[derive(Clone, Debug)]
pub struct SystemClock { origin: std::time::Instant }   // new() captures origin; now() = elapsed since origin

#[derive(Clone, Debug, Default)]
pub struct ManualClock { now: Timestamp }
impl ManualClock {
    pub const fn new() -> Self;
    pub const fn starting_at(now: Timestamp) -> Self;   // restore from a snapshot
    pub fn advance(&mut self, by: Duration);
    pub fn set(&mut self, to: Timestamp);       // # Panics: if `to` < current (monotonicity). #[track_caller]
    /// Fallible twin of `set`.
    pub fn try_set(&mut self, to: Timestamp) -> Result<(), NonMonotonicError>;
}
```

**Semantics:**

- Arithmetic uses `Duration` (integer nanoseconds), so accumulation is exact. Splitting `elapsed` differently across `accumulate` calls yields the same total sequence of ticks, as long as the cap is not hit (property-tested).
- `alpha()` is the only floating-point output and is never fed back into the simulation.

### 5.4 `engine_math`

**Purpose:** a single blessed math vocabulary for the engine.

- Re-exports from `glam` (version pinned in `[workspace.dependencies]`, default features plus no fast-math):
  - f32: `Vec2`, `Vec3`, `Vec3A`, `Vec4`, `Quat`, `Mat2`, `Mat3`, `Mat3A`, `Mat4`, `Affine2`, `Affine3A`;
  - f64: `DVec2`, `DVec3`, `DVec4`, `DQuat`, `DMat2`, `DMat3`, `DMat4`, `DAffine2`, `DAffine3`;
  - the `IVec*`/`UVec*` families.

  `glam` is re-exported as `engine_math::glam` for escape-hatch access.
- **Layout assertions:** `const` assertions pin `size_of`/`align_of` for `Transform`, `WorldTransform`, `Transform2d`, and `WorldTransform2d` on each CI target, with the expected values recorded in ADR 0006. Any change (e.g. a glam `scalar-math` feature flip via feature unification, or a glam upgrade) fails the build loudly instead of silently changing snapshot/FFI layouts.
- glam's version is part of our public API. Bumping glam's major/minor version is a breaking change for us (acceptable at 0.x). The FFI layer (M4) defines its own `#[repr(C)]` plain-data math types and does not expose glam.

```rust
/// Translation, rotation, scale. `#[repr(C)]` fixes field order; layout is pinned by static assertions.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct Transform { pub translation: Vec3, pub rotation: Quat, pub scale: Vec3 }

impl Transform {
    pub const IDENTITY: Transform;
    pub const fn from_translation(t: Vec3) -> Self;
    pub const fn from_rotation(r: Quat) -> Self;
    pub const fn from_scale(s: Vec3) -> Self;
    pub fn with_translation(self, t: Vec3) -> Self;  // + with_rotation, with_scale
    /// `self * child`, by per-component composition:
    ///   translation = self.translation + self.rotation * (self.scale * child.translation)
    ///   rotation    = self.rotation * child.rotation
    ///   scale       = self.scale * child.scale
    /// Matches Mat4 composition exactly (within float eps) when `self.scale` is uniform. With
    /// non-uniform parent scale and a rotated child, the shear a matrix would carry is dropped
    /// (documented; not an approximation within eps).
    pub fn mul_transform(self, child: Transform) -> Transform;
    /// Inverse by per-component inversion. Exact (`t * t.inverse() ≈ IDENTITY`) when scale is
    /// uniform; documented otherwise. Zero scale yields non-finite values.
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

Scalar helpers are `f32`-only in M1. `f64` variants are added when something needs them.

#### Large-world placement (ADR 0006)

```rust
/// High-precision placement in the world. Translation is f64. Rotation and scale stay f32:
/// their magnitudes are bounded, so they don't lose precision far from the origin.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct WorldTransform { pub translation: DVec3, pub rotation: Quat, pub scale: Vec3 }

impl WorldTransform {
    pub const IDENTITY: WorldTransform;
    pub const fn from_translation(t: DVec3) -> Self;
    /// Re-express relative to a nearby origin (e.g. the camera or a physics island center).
    /// translation = (self.translation - origin) narrowed to f32; rotation and scale copied.
    pub fn relative_to(self, origin: DVec3) -> Transform;
    /// Place a local (f32) child under this world-space parent.
    pub fn mul_transform(self, child: Transform) -> WorldTransform;
    /// Local f32 point → world f64 point.
    pub fn transform_point(self, p: Vec3) -> DVec3;
    pub fn to_daffine(self) -> DAffine3;
}
impl Default for WorldTransform { /* IDENTITY */ }

impl Transform {
    /// Inverse of `WorldTransform::relative_to`.
    pub fn to_world(self, origin: DVec3) -> WorldTransform;
}
```

- **Rule:** `WorldTransform` is for absolute placement. `Transform` is for local/relative data: parent-to-child offsets, GPU data, physics-local space.
- Narrowing from f64 to f32 happens only through `relative_to`. There is no `From` impl, so precision loss is always explicit at the call site.
- **Precision guarantee (tested):** for any absolute position with magnitude up to 1e9 m (beyond the Earth–Moon distance), and a local offset within 10 km of the chosen origin, `relative_to` followed by `to_world` round-trips with error ≤ 1 mm.
- Which of these the ECS stores per entity is decided in M2. M1 provides the vocabulary.

#### 2D and 2.5D

2D types mirror the 3D ones. The 2D world is the XY plane of the 3D convention, and rotation is an angle about +Z.

```rust
/// 2D transform in the XY plane. `rotation` is radians, counter-clockwise (about +Z).
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct Transform2d { pub translation: Vec2, pub rotation: f32, pub scale: Vec2 }

impl Transform2d {
    pub const IDENTITY: Transform2d;
    pub const fn from_translation(t: Vec2) -> Self;
    pub const fn from_rotation(radians: f32) -> Self;
    pub const fn from_scale(s: Vec2) -> Self;
    pub fn with_translation(self, t: Vec2) -> Self;  // + with_rotation, with_scale
    /// `self * child`, per-component composition with the same contract as 3D
    /// (rotation = sum of angles; exact vs `to_affine` composition when `self.scale` is uniform).
    /// The resulting rotation is wrapped into (-π, π].
    pub fn mul_transform(self, child: Transform2d) -> Transform2d;
    pub fn inverse(self) -> Transform2d;
    pub fn to_affine(self) -> Affine2;
    pub fn to_mat3(self) -> Mat3;                  // homogeneous 2D matrix
    pub fn transform_point(self, p: Vec2) -> Vec2;
    pub fn transform_vector(self, v: Vec2) -> Vec2;
    /// Exact lift into 3D at depth `z` (rotation about +Z; scale.z = 1).
    pub fn to_3d(self, z: f32) -> Transform;
    pub fn to_world(self, origin: DVec2) -> WorldTransform2d;
}
impl Default for Transform2d { /* IDENTITY */ }
impl core::ops::Mul for Transform2d { /* mul_transform */ }

/// Large-world 2D placement (f64 translation), mirroring `WorldTransform`.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct WorldTransform2d { pub translation: DVec2, pub rotation: f32, pub scale: Vec2 }

impl WorldTransform2d {
    pub const IDENTITY: WorldTransform2d;
    pub const fn from_translation(t: DVec2) -> Self;
    pub fn relative_to(self, origin: DVec2) -> Transform2d;
    pub fn mul_transform(self, child: Transform2d) -> WorldTransform2d;
    pub fn transform_point(self, p: Vec2) -> DVec2;
    pub fn to_daffine(self) -> DAffine2;
    pub fn to_3d(self, z: f64) -> WorldTransform;
}

impl Transform {
    /// Projection onto the XY plane: keeps translation.xy, the twist about +Z (swing–twist
    /// decomposition), and scale.xy. Lossy: drops z, tilt, and scale.z. Documented as such.
    pub fn to_2d(self) -> Transform2d;
}
impl WorldTransform {
    pub fn to_2d(self) -> WorldTransform2d;  // same projection rules
}
```

- **2.5D** is supported by construction. A 2D object is a 3D object on the XY plane, and Z gives depth or layer. `to_3d` is exact; `to_2d` is a documented projection.
- **Screen/UI space** (+Y down, pixel units, origin top-left) is defined by convention in ADR 0005. Conversion functions between world and screen need camera/viewport data, so they ship with the camera in the rendering milestone, not in M1.
- **2D axis constants:** `axis::RIGHT_2D = Vec2::X`, `axis::UP_2D = Vec2::Y` (defined in the `axis` module above).

#### Coordinate system and units (ADR 0005)

- **Internal convention (fixed, not configurable):** right-handed, identical to glTF 2.0's axes. Units are meters, seconds, radians, kilograms. Matrices follow glam: column-major storage, column vectors, `M * v`.
- **Two distinct meanings of "forward" are both named, never conflated:**
  - **View forward = −Z.** The direction a default camera looks, with +X to the viewer's right and +Y up (glam `look_to_rh`).
  - **Asset front = +Z.** The direction an imported asset's front faces (glTF: "the front of a glTF asset faces +Z"). Assets therefore face a default camera. From the asset's own point of view its right is −X, which is exactly how the glTF spec phrases it ("+Z forward, −X right").
  - Conventions disagree on these: Unity's camera and asset fronts both point along +Z, for example. That is why `CoordinateSystem` below avoids the word "forward" entirely.

```rust
/// Named directions in the engine's convention.
pub mod axis {
    pub const RIGHT: Vec3 = Vec3::X;    pub const LEFT: Vec3 = Vec3::NEG_X;
    pub const UP: Vec3 = Vec3::Y;       pub const DOWN: Vec3 = Vec3::NEG_Y;
    /// View forward: the direction a default camera looks.
    pub const VIEW_FORWARD: Vec3 = Vec3::NEG_Z;
    /// Asset front: the direction an imported asset's front faces (glTF convention).
    pub const ASSET_FRONT: Vec3 = Vec3::Z;
    pub const RIGHT_2D: Vec2 = Vec2::X;  pub const UP_2D: Vec2 = Vec2::Y;
}

/// Explicit conversion from other conventions, for import/export edges only.
pub mod convert {
    /// A source convention, described purely as a basis: which signed *source* axis corresponds
    /// to each engine axis. Engine +Y is "up"; engine +Z is the asset-front direction.
    /// No semantic "forward" is involved, so there is no camera-vs-asset ambiguity.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct CoordinateSystem { pub x: SignedAxis, pub y: SignedAxis, pub z: SignedAxis }
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum SignedAxis { PosX, NegX, PosY, NegY, PosZ, NegZ }

    impl CoordinateSystem {
        pub const ENGINE: CoordinateSystem;    // identity
        pub const GLTF: CoordinateSystem;      // identity (asserted ENGINE == GLTF by test)
        // BLENDER and UNREAL are omitted (amended 2026-11-12): their axis conventions could not be
        // verified against vendor documentation (ADR 0005). Only ENGINE, GLTF and UNITY ship.
        pub const UNITY: CoordinateSystem;
        /// Returns `None` if two fields name the same source axis (ignoring sign).
        pub const fn new(x: SignedAxis, y: SignedAxis, z: SignedAxis) -> Option<Self>;
        pub const fn is_right_handed(self) -> bool;
        /// Signed permutation matrix mapping vectors in `self` to engine space.
        pub fn to_engine_matrix(self) -> Mat3;
    }

    pub fn vec3_to_engine(v: Vec3, from: CoordinateSystem) -> Vec3;       // + dvec3_to_engine
    pub fn quat_to_engine(q: Quat, from: CoordinateSystem) -> Quat;       // via M·R·Mᵀ (a proper rotation even when handedness flips)
    pub fn scale_to_engine(s: Vec3, from: CoordinateSystem) -> Vec3;      // permutes components
    pub fn transform_to_engine(t: Transform, from: CoordinateSystem) -> Transform;
    // + `*_from_engine` inverses of each
}
```

- **Preset rule:** a preset maps the source's up axis to engine +Y and the source's asset-front axis to engine +Z. The remaining axis follows from handedness; when handedness flips, the flipped axis is the asset-right axis, matching common glTF importers (e.g. Unity's convention of negating X).
- Each preset's mapping cites the vendor's documentation and that tool's glTF importer/exporter behavior in ADR 0005. A preset that cannot be verified is omitted rather than guessed.
- **Tests:** `ENGINE == GLTF`; a glTF asset's +Z front stays +Z and +Y up stays +Y through `*_to_engine(_, GLTF)`; for every preset, the source's documented up and asset-front axes land on engine +Y and +Z.
- **Why this convention:** see ADR 0005. In short, it gives zero-conversion import of glTF (the primary planned asset format), works with glam's `*_rh` helpers and the Rust graphics ecosystem, and makes right-handed math and physics formulas apply directly.

### 5.5 `engine` facade

- Re-exports each crate as a module: `engine::handle`, `engine::random`, `engine::time`, `engine::math`.
- `engine::prelude` exports: `Handle`, `HandlePool`, `HandleAllocator`, `DenseHandleAllocator`, `Prng`, `Tick`, `FixedTimestep`, `TimeSource`, `Timestamp`, `ManualClock`, `SystemClock` (when available), `Transform`, `WorldTransform`, `Transform2d`, `WorldTransform2d`, `Vec2`, `Vec3`, `Vec4`, `DVec2`, `DVec3`, `Quat`, `Mat4`, and the `axis` module.
- Features: `std` (default), forwarded to leaf crates.
- The crate root docs are the engine's "start here" page: crate map, guarantees, and a short example.

### 5.6 `examples/fixed_step_sim`

A headless binary demonstrating the M1 crates together:

1. CLI arguments: `--seed <u64>` (default 42), `--jitter-seed <u64>` (default 7), `--ticks <u64>` (default 600), `--particles <usize>` (default 10 000).
2. Creates a `HandlePool<Particle { position: Vec3, velocity: Vec3, ttl: u32 }>` seeded via `Prng::from_seed(seed)`.
3. Drives a `FixedTimestep::from_hz(60)` (exactly 16_666_667 ns) from a `ManualClock`. Each loop iteration advances the clock by a pseudo-random frame time in [4 ms, 40 ms], simulating jittery frames. This stays below the step cap, so no time is dropped. Frame times come from a **separate** `Prng::from_seed(jitter_seed)`, so the frame-time split never consumes the simulation's random numbers. For each step:
   - integrate particles;
   - decrement `ttl` and remove expired particles via `retain`;
   - respawn replacements using `prng.stream(handle.to_bits())`.
4. Prints the final tick, live count, slot capacity, and a 64-bit FNV-1a checksum over `(handle bits, position bits, velocity bits)` in iteration order.
5. Integration test (`examples/fixed_step_sim/tests/determinism.rs`) runs the simulation logic as a library function twice with the same seed and asserts equal checksums. A different seed must produce a different checksum. A different `--jitter-seed` (i.e. a different frame-time split) must produce the same checksum.

6. **Snapshot test** (`tests/snapshot.rs`): run 300 ticks, clone the complete sim state (pool, PRNG, timestep, clock), run both copies 300 more ticks, and assert equal checksums. Also round-trip `Prng`/`FixedTimestep`/`ManualClock` through their raw-state accessors mid-run.
7. **Allocation test** (`tests/allocations.rs`, its own test binary, using `dhat` in testing mode, so no `unsafe` is needed in our code): pre-size the pool, warm up 120 ticks, then assert **zero new heap allocations** across the next 600 ticks.
8. **Golden checksum across architectures:** the sim uses only `Vec3`/`f32` basic IEEE operations: no transcendental functions, and no SIMD-backed glam types (`Vec3A`, `Vec4`, `Quat`, `Mat4`), whose reduction order differs per architecture (§4.9). So its frozen checksum must match on aarch64 (local Apple Silicon) and x86_64 (CI). If a future change needs `sin`/`cos`/etc., the golden value becomes per-target, with the reason documented.

The sim logic lives in the crate's `lib.rs`. `main.rs` is a thin CLI wrapper, so tests and benches can call it directly.

---

## 6. Error handling summary

| Crate | Fallible operations | Mechanism |
|---|---|---|
| `engine_handle` | stale or invalid handles; capacity | `Option` (`get`, `remove`, `from_bits`, `get_disjoint_mut`, `try_allocate`, `try_allocate_many`, `try_insert_with`), `Result<(), TryReserveError>` (`try_reserve`), `bool` (`free`, `contains`), `Result<_, CapacityError<T>>` (`try_insert`). `insert`/`allocate` panic on capacity |
| `engine_random` | invalid ranges | `try_range` → `Option`; `range` panics (`# Panics`, `#[track_caller]`) |
| `engine_time` | zero step, non-monotonic `set`, tick overflow, invalid restored state | `try_new`/`from_state` → `Option`, `try_set` → `Result<(), NonMonotonicError>`, `checked_next` → `Option`; panicking twins documented |
| `engine_math` | none | Documented NaN/non-finite behavior |

M1 introduces these small error types: `engine_handle::CapacityError<T>`, which returns the rejected value and the pool's limit; `engine_handle::InvalidHandleBits` (amended 2026-11-12), the error of `TryFrom<u64> for Handle<T>`; `engine_handle::InvariantViolation` (test-only, from `check_invariants`); and `engine_time::NonMonotonicError`, which reports the current and requested timestamps. Both implement `Display` + `core::error::Error`. Broader `#[non_exhaustive]` error enums arrive when a crate has several failure modes (expected in M2/M4).

---

## 7. Testing strategy

All layers run in CI unless noted.

| Layer | Tool | M0+M1 coverage |
|---|---|---|
| Unit | `#[test]` in-module | Generation increments; retire-on-exhaustion (tested by constructing a slot near `u32::MAX` via a `#[cfg(test)]` constructor); LIFO reuse; `clear` semantics; `from_bits(0)` → `None`; `Tick::next`; `FixedTimestep` cap and `dropped_time`; `alpha` range; float-range rejection path; `chance` edge cases (`0`, `1`, `NaN`, negatives); `Transform` identities |
| Doc | rustdoc | Every public example runs. `compile_fail` doctests: `Handle<A>` passed where `Handle<B>` is expected; `Prng::range` with an unsupported type |
| Trait/auto-trait assertions | static asserts in tests | `Handle<T>: Send + Sync + Copy` for non-`Send` `T`; size assertions (`size_of::<Handle<T>>() == 8`, `size_of::<Option<Handle<T>>>() == 8`) |
| Property / model-based | `proptest` | `HandlePool` and `HandleAllocator` vs a `HashMap`/`BTreeMap` reference model over random op sequences (insert/remove/get/get_disjoint_mut/retain/clear/iterate; get_disjoint_mut with duplicate or stale handles returns None). `assert_invariants()` runs after every operation. Invariants: stale handles never resolve, `len` matches, iteration visits exactly the live set, and two runs of the same op sequence yield identical handles **and identical iteration order**. Every live slot's dense position points at a dense handle equal to that slot's handle. `to_bits`/`from_bits` round-trips. `FixedTimestep` split-invariance. `Prng::range` results always within bounds. `Transform` compose/inverse round-trips within eps for uniform scale; `transform_point` matches `to_matrix` |
| Golden values | plain tests | xoshiro256++ and SplitMix64 outputs checked against published test vectors. First 16 outputs of every public `Prng` method for seeds `{0, 1, 42, u64::MAX}` frozen in `tests/golden.rs`. `stream` outputs frozen. `fixed_step_sim` checksum for the default arguments frozen |
| Statistical sanity | plain tests (release-mode, `#[ignore]` by default, run in CI via `--include-ignored` on ubuntu) | Chi-square goodness-of-fit for `range` over small and non-power-of-two bounds, `next_f32`/`next_f64` bucket uniformity, `next_bool` balance, `shuffle` permutation uniformity on 4 elements. Fixed seeds and thresholds, so tests are deterministic |
| Determinism | integration test | §5.6, including the cross-architecture golden checksum |
| Snapshot/restore | integration + unit | §5.6 item 6. `Prng::from_state(p.to_state())` continues identically. `FixedTimestep::from_state` rejects invalid states. `from_state([0; 4])` → `None` |
| Allocation discipline | `dhat` (testing mode) | §5.6 item 7: zero steady-state allocations per tick |
| Large-world precision | property tests | `relative_to`/`to_world` round-trip ≤ 1 mm for magnitudes up to 1e9 m and offsets ≤ 10 km. `WorldTransform::transform_point` agrees with `to_daffine` |
| Coordinate conversion | unit + property | Every preset: `*_from_engine(*_to_engine(x)) == x`. Converted quaternions stay unit-length and proper (det = +1). Handedness is detected correctly. `new` rejects non-perpendicular axes. Each preset's up and asset-front axes land on engine +Y/+Z (§5.4) |
| 2D / 2.5D | unit + property | `Transform2d` compose/inverse round-trips (uniform scale). `transform_point` agrees with `to_affine`/`to_mat3`. Rotation wrapping into (-π, π]. `t.to_3d(z).to_2d() == t` within eps. `to_2d` of a pure-Z rotation is exact and drops tilt as documented. `WorldTransform2d` precision guarantee identical to 3D |
| Panic contracts & fallible twins | unit (native only; see WASM notes) | Every documented panic is triggered and its message checked. A panic hook asserts that `#[track_caller]` reports the *test file* as the location. Each `try_*`/`checked_*` twin returns the error on exactly the same inputs where the panicking version panics, and leaves state untouched (`try_range` draws nothing) |
| PRNG normative algorithms | golden + unit | Goldens cover every integer type including `usize`/`isize` (identical on wasm32), `shuffle`, `choose`, `fill_bytes` (lengths 0–17), and float `Range`/`RangeInclusive`. Edge cases: `lo..lo` is invalid, `-MAX..MAX` is invalid, `lo..=lo` returns `lo`, inclusive bounds attainable |
| Time edge cases | unit | `alpha()` stays < 1.0 when the accumulator is `step - 1ns` (for 1/60 s and 100 ms steps). `from_hz(60)` is exactly 16_666_667 ns |
| Layout | `const` assertions + tests | Sizes/alignments of `Handle`, `Option<Handle>`, transforms per ADR 0006 |
| Debug output | unit | Snapshot tests of `Debug` for `Handle`, `HandlePool`, `Prng` |
| 32-bit / WASM | CI `wasm` job | Leaf-crate tests on `wasm32-wasip1`; `engine` builds for `wasm32-unknown-unknown` |
| Undefined behavior | Miri | From M2: all allowlisted crates (and `engine_handle` once the job exists) |
| Fuzzing | `cargo-fuzz` (nightly workflow, from M2) | First targets in M2: `handle_pool_ops` (arbitrary op sequences checked against the model), `handle_from_bits`, plus the M2 storage code |
| Test quality | `cargo-mutants` | Periodic from M2 (the M1 crates are included in the first run). Not a CI gate |

Test-only dependencies (`proptest`; from M2 `arbitrary`/`libfuzzer-sys` in `fuzz/`) live in `[workspace.dependencies]` and are dev-dependencies only.

---

## 8. Benchmarking strategy

**Tools:**

- **Criterion** (+ **critcmp**): local wall-clock benchmarks with named baselines. `cargo xtask bench --save <name>` runs `cargo bench -- --save-baseline <name>`; `cargo xtask bench --compare <a> <b>` runs critcmp.
- **Gungraun** (formerly iai-callgrind), **from M2 (§4.12)**: instruction-count benchmarks under Valgrind for CI regression gating (Linux only; runs in CI and the devcontainer, not natively on macOS). The PR job builds and runs the benchmarks at the PR's **merge-base**, then at the PR head, in the **same job on the same runner**, and compares the two. No artifacts are needed, so nothing expires and there is no stale-baseline drift. It fails on a > 5% instruction-count regression in any benchmark. The `perf-regression-accepted` PR label downgrades the gate to report-only for intended trade-offs; the PR description must justify it. Callgrind's cache simulation is enabled, so estimated L1/last-level-cache misses are reported too, but they are not gated. Instruction counts from the aarch64 devcontainer are not comparable with x86_64 CI counts, and are never mixed.

**Layout:**

- Micro-benchmarks for each crate live in `libs/<crate>/benches/` (Criterion; from M2 also Gungraun targets for regression-critical paths).
- Comparisons against other libraries live in `benches/` (`engine_benches`). Competitor crates are dev-dependencies of that crate only.

**M1 benchmark matrix:**

| Subject | Compared against | Operations | Sizes |
|---|---|---|---|
| `HandlePool<T>` (T = `u64` and a 64-byte struct) | `slotmap::SlotMap`, `slotmap::DenseSlotMap`, `thunderdome::Arena` | insert N; get all (sequential and random handle order); remove all; iterate; churn (remove 50% randomly then reinsert) then get | 1k, 100k, 1M |
| `HandleAllocator`, `DenseHandleAllocator` (amended 2026-11-12) | `slotmap` with `()` values | allocate N; free/reallocate churn; `is_alive` checks; `allocate_many` against a loop (fresh and recycled slots) | 1k, 100k, 1M |
| `Prng` | `rand_xoshiro::Xoshiro256PlusPlus`, `rand::rngs::SmallRng`, `fastrand::Rng` | `next_u64` ×1M; `range(0..1000)` ×1M; `range(0.0..1.0)` ×1M; `shuffle` 10k | — |
| `FixedTimestep` | — | `accumulate` + drain ×1M | — |
| `fixed_step_sim` | — | full run, default args | — |

**Reporting:** `docs/benchmarks.md` records methodology (toolchain, profile settings, CPU model, OS, power settings), results tables per milestone, and interpretation. It records results honestly, including where we lose. In M1, losing to a competitor is recorded and analyzed, not fixed: optimization waits for real workloads (M2+). Wall-clock (Criterion) numbers come from the author's Apple M4 Pro (aarch64 macOS). Instruction counts (Gungraun) come from x86_64 Linux CI. The two measure different things and are never compared with each other. The release benchmark profile uses `lto = "thin"`, `codegen-units = 1`, `debug = true` (for profiling).

---

## 9. Definition of done (M0+M1)

1. `cargo xtask ci` passes locally (macOS) and in the devcontainer (Linux). The GitHub Actions matrix is green on ubuntu, macOS, and windows.
2. All CI jobs marked for M0+M1 in §4.7/§4.12 exist and pass.
3. Every public item in every crate is documented. Every public type has a runnable example.
4. All test layers in §7 that are scheduled for M0+M1 are implemented, and golden values are frozen.
5. The benchmark matrix in §8 has been run locally, with results and hardware recorded in `docs/benchmarks.md`.
6. ADRs 0001–0004 are written. README, dual license files, `deny.toml`, `unsafe-allowlist.toml` (empty), and dependabot config are in place.
7. `examples/fixed_step_sim` runs, and its determinism test passes.
8. (Moved to M2: Miri, fuzzing, Gungraun gate, coverage, mutation triage; §4.12.)
9. ADRs 0005 (coordinates/units, including 2D and screen space), 0006 (large-world precision), 0007 (host owns the loop), and 0008 (no global state) are written. The `check-statics` job is green. The transcendental-method bans are active, with only the `engine_math::scalar` exception. The devcontainer has every tool `cargo xtask ci` needs. `clippy.toml` determinism bans are active, with only the sanctioned `SystemClock` exception.
10. The `wasm` CI job is green. The snapshot, allocation, large-world precision, and coordinate-conversion tests pass.
11. Every documented-panic API has a fallible twin and `#[track_caller]`. The panic-contract, PRNG-normative, layout, and `Debug` tests pass. ADR 0009 is written, and `concurrency-allowlist.toml` exists (empty).

---

## 10. Out of scope (M0+M1)

- Any ECS code: storage, queries, world, component registration (M2).
- Scheduler and job system (M3).
- C ABI, C# bindings, header generation (M4).
- Windowing, input, rendering, audio, assets, physics, scripting, networking, editor tooling.
- Publishing to crates.io, semver guarantees beyond "golden PRNG streams are frozen in 0.x". When publishing is decided, add `cargo-semver-checks` to CI. Optionally, commit per-crate public-API snapshots (`cargo public-api`) so API changes appear as reviewable diffs.
- Cross-platform bit-exact determinism, fixed-point math.
- `rand_core` trait interop, `f64` scalar helpers, dense iteration in `HandlePool`.
- On-disk serialization of `HandlePool`/`HandleAllocator` internals and `serde` support. In-memory snapshots via `Clone` are in scope; persistent serialization is designed in M2 together with the type registry.
- Floating-origin shifting, planet-centered and local tangent frames. M1 provides `WorldTransform`/`Transform`; spatial frames come with the systems that need them.

---

## 11. Roadmap after M0+M1

| Milestone | Summary | Key open question it resolves |
|---|---|---|
| **M2: ECS storage spike + decision** | Prototype sparse-set, archetype, and per-component hybrid storage behind one minimal API (spawn/despawn, insert/remove, typed query iteration, runtime-registered component). Benchmark against bevy_ecs, hecs, flecs_ecs on standard workloads (simple iteration, fragmented iteration, add/remove, heavy compute, entity churn). Produce an ADR choosing the storage model, the runtime component identity model, and the policy for foreign/mod-defined component storage (safe byte blobs vs audited type-erased columns). **Also in scope:** the type registry (reflection) designed together with component identity, because it is the backbone for serialization, editor, scripting, FFI, mods, and networking; runtime handles vs persistent IDs, with ID remapping on load; deterministic hash collections (`engine_collections`); which transform type(s) entities store, including one unified transform vs separate 2D/3D transform components, decided with a **2D-heavy benchmark workload** (e.g. 100k sprites) alongside the 3D ones. **Editor-driven requirements:** change detection and world diffing (inspector refresh, undo/redo, network deltas, incremental saves); reflection whose core traits sit at the bottom of the crate graph (orphan rule, §3.1) and that is writable (set a field by path) and serializes to a self-describing format (an out-of-process editor can show types it wasn't compiled against); edits applied as commands (the basis for undo/redo); a stable content hasher (desync detection, persistent IDs, deterministic maps); debugger visualizers (natvis, LLDB/GDB formatters) for the storage types. **Concurrency:** storage designed for fixed-partition parallel iteration and cache-friendly layout. **Infrastructure arriving with M2** (§4.12): Miri, fuzzing, Gungraun gate, coverage, mutation triage. **Dogfooding sample:** a minimal visual example (e.g. thousands of 2D agents drawn into a simple pixel-buffer window; the windowing crate is an example-only dependency, never an engine dependency). It validates ECS ergonomics with a real consumer, doubles as a realistic benchmark, and makes progress visible | Storage model; can C#/mods define components; which `unsafe` exceptions exist; type registry shape |
| M3: Scheduler | Systems with declared read/write access, explicit ordering, ambiguity detection, deterministic execution under same-build determinism, parallel execution. **Required:** a single-threaded execution mode (for WASM without threads, and determinism debugging). Profiling instrumentation behind one macro convention. Honors ADR 0007: the scheduler runs on threads it is given (or a pool the host creates), driven by host calls, and never takes over the caller's thread. Determinism independent of thread count (§4.9). The concurrency allowlist, loom, TSan, and stress-test tooling arrive here and apply (§4.11, §4.12). Per-worker state is cache-padded. The work-distribution strategy (work stealing vs fixed assignment) is chosen with benchmarks, and its merges must stay deterministic | Threading model (thread pool vs fibers vs work stealing) |
| M4: C ABI + C# | `engine_ffi` crate (allowlisted `unsafe`), handle-based C API, generated header, C# binding (generator choice evaluated: Interoptopus vs cbindgen + hand-written P/Invoke vs csbindgen), C# sample. **Also:** evaluate the WebAssembly component model (WIT + `wit-bindgen`) for sandboxed, multi-language mods alongside the C ABI for embedding. Define the FFI panic policy: a panic unwinding out of `extern "C"` aborts the host, so the FFI layer validates inputs before calling APIs with documented panics | Binding generator; ownership/lifetime rules across the boundary; native vs WASM for mods |
| M5: Diagnostics & tooling foundation | Diagnostics facade (logging/tracing; host-installed subscribers per §4.10); profiler integration; **remote inspection protocol** (localhost by default, auth token; entity/component browsing via reflection, change subscriptions, edit commands); record/replay with automatic desync detection (per-tick state checksums) | Protocol transport and encoding; the editor's main integration path |
| Later | Windowing/input (`winit`) → rendering (`wgpu`; 2D sprite/batching path, pixel-perfect cameras, world↔screen conversion, 3D path, 2.5D via orthographic/perspective cameras over the same world) → assets → audio → physics (2D and 3D are separate solvers in practice) → modding/plugin loading → optional convenience app runner → **editor product** (separate app over the M5 protocol and public APIs only) | — |

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
- matklad, Large Rust Workspaces — https://matklad.github.io/2021/08/22/large-rust-workspaces.html
- Microsoft Rust project guidelines — https://microsoft.github.io/rust-guidelines/guidelines/project/
- Materialize, Compile Times and Code Graphs — https://materialize.com/blog/compile-times-and-code-graphs/
- extern-impl RFC (orphan-rule constraint on crate factoring) — https://github.com/jsgf/rfcs/blob/extern-impl/text/0000-extern-impl.md
- DZone, architectural cost of the orphan rule — https://dzone.com/articles/architectural-cost-rust-orphan-rule
- std `RandomState` source — https://doc.rust-lang.org/src/std/hash/random.rs.html
- rust-lang/rust#36481 (HashMap iteration order) — https://github.com/rust-lang/rust/issues/36481
- Materialize deterministic collections — https://dev.materialize.com/api/rust/mz_ore/collections/hash/index.html
- Epic, LWC project conversion guidelines — https://dev.epicgames.com/documentation/en-us/unreal-engine/large-world-coordinates-project-conversion-guidelines-in-unreal-engine-5
- Epic, Large World Coordinates in UE5 — https://dev.epicgames.com/documentation/unreal-engine/large-world-coordinates-in-unreal-engine-5
- UE5 LWC forum report (rendering artifacts) — https://forums.unrealengine.com/t/lwc-large-world-coordinates-do-not-work-for-first-person-shooters-driving-games-wobbly-rendering-artefacts-ue5-only-ue4-works-fine/652748
- web-time (Instant::now on wasm32-unknown-unknown) — https://docs.rs/crate/web-time/1.1.0/source/src/lib.rs
- bevy_reflect — https://docs.rs/crate/bevy_reflect/0.19.1
- bevy_reflect design PR — https://github.com/bevyengine/bevy/pull/926
- glTF coordinate conventions (summary) — https://asam-ev.github.io/OpenMATERIAL/asamopenmaterial/latest/specification/geometry/file-format-support.html
- wit-bindgen — https://lib.rs/crates/wit-bindgen-cli
- WASI language support — https://wasi.dev/languages
- glam discussion on libm / Rapier `sin_cos` desync — https://github-redirect.dependabot.com/bitshifter/glam-rs/discussions/350
- glam features — https://lib.rs/crates/glam/features
- libm crate — https://docs.rs/crate/libm/latest
- GitHub Actions billing — https://docs.github.com/en/billing/concepts/product-billing/github-actions
- glTF 2.0 specification (coordinate system) — https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html
- crossbeam-utils `CachePadded` — https://docs.rs/crossbeam-utils/latest/src/crossbeam_utils/cache_padded.rs.html
- loom — https://docs.rs/crate/loom/0.7.2/source/README.md
- Core-count independent reproducible reduce — https://ae.iti.kit.edu/english/4480.php
- ReproBLAS — https://bebop.cs.berkeley.edu/reproblas

---

## 13. Implementation phasing

The implementation plan (removed at merge, see the reading guide) split M0+M1 into separately reviewable phases. Each ends with `cargo xtask ci` green and a commit/PR. Each phase summary explains the key Rust and engine decisions made, for learning: ownership choices, why a trait is sealed, what `PhantomData` variance does, and similar. Rust basics are skipped.

1. **Workspace and policy (M0):** workspace layout, toolchain, lints, `clippy.toml`, `deny.toml`, unsafe allowlist + `check-unsafe`, licenses, README, ADRs 0001–0004 and 0007–0009, `concurrency-allowlist.toml` (empty), `xtask` (including `check-unsafe` and `check-statics`), devcontainer tooling (wasmtime, cargo-nextest, cargo-deny, critcmp, the extra rustup targets; Valgrind, gungraun-runner, cargo-llvm-cov, cargo-fuzz, and the pinned nightly with Miri arrive in M2) so `cargo xtask ci` runs there unchanged, CI workflows, dependabot.
2. **`engine_handle`:** handles, pool, allocator; unit and property tests; benchmarks.
3. **`engine_random`:** `Prng`; golden, statistical, and state round-trip tests; benchmarks.
4. **`engine_time`:** `Tick`, `FixedTimestep`, clocks; split-invariance and state tests.
5. **`engine_math`:** re-exports, `Transform`, `WorldTransform`, `Transform2d`, `WorldTransform2d`, `axis`, `convert`; ADRs 0005–0006; precision and conversion tests.
6. **Facade, sample, docs:** `engine` prelude, `fixed_step_sim` with determinism/snapshot/allocation tests, the comparison benchmark run, `docs/benchmarks.md`.
