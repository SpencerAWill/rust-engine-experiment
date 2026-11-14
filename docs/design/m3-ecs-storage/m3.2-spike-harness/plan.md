# M3.2 plan: Spike harness

- **Status:** Draft (2026-11-14).
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both.

## 1. Approach

Ten commits on branch `m3.2-spike-harness`, after the spec commit already on it. The harness core comes first (interface, hash builder and conformance check) and is tested with a toy workload, so the check is proven to work before any real workload depends on it. Then come the workloads, each in the same commit as its `vec_aos` implementation and tests: the plain iteration workloads first, then the structural ones, then `sprites`, then `agents` with the struct-of-arrays backend. The two bench crates follow, each with the xtask changes it needs, then the remaining xtask coverage and scanning changes, and the documents last.

Choices the spec leaves to implementation:

- **The harness provides the per-entity rule functions as well as the hashes.** Each workload module exports `update` (and, where the step has one, an extra function such as `is_expired`) that applies §4.4's rule to one entity's components, marked `#[inline]`. Backends call them inside their own loops. The floating-point operation order is then written once, and backends differ only in storage and iteration. The one exception is `agents`: `harness::agents::update` is a separate implementation of M3.0 §4.3 for column storage, and the `vec_soa` conformance test against `agents_2d::Sim` (R6) is what proves it matches.
- **Spawn functions** take `(prng: &Prng, i: u64)` (plus `count` for `frag_iter`, which needs it to find the letter) and return a tuple of components, the shape an ECS spawn takes.
- **Order independence (R7)** uses a test-only trait in `spike_ecs_vec`: `trait Reversible { fn reverse(&mut self); }`, implemented by each `Vec` workload. That trait cannot reverse `agents_2d::Sim`'s private storage, so for `agents` R7 is checked on `vec_soa`. `agents_2d`'s own R4 test already covers the AoS layout.
- **Broken candidates (R8)** live in `spike_ecs_vec`'s test module. `StepTwice<W>` is a generic wrapper whose `step` calls `W::step` twice. The three skip-one variants are newtypes over the `Vec` workloads that loop over `entities[1..]` (or, in `add_remove`, skip one entity's removal).
- **Gungraun benchmarks** are generated per (workload, backend) by a `macro_rules!` in `benches/cache.rs`. The sizes come from `#[benches::sizes(iter = <W::Spec as Spec>::SIZES.iter().copied(), setup = prepared::<W>)]`. `prepared::<W>(count)` runs `setup` and one `step`, and the measured body calls an `#[inline(never)] fn step_once<W: Workload>(w: &mut W)` defined outside the benchmark function, because of the symbol toggle described in `bench/regress/benches/handle.rs`. If the `iter` form does not accept that expression, use `args = [...]` with the literal sizes, and add `debug_assert!(W::Spec::SIZES.contains(&count))` inside `prepared`.
- **Callgrind's cache simulation** is configured as `main!(config = LibraryBenchmarkConfig::default().tool(Callgrind::with_args(["--cache-sim=yes"])); library_benchmark_groups = …)`. Check the exact builder against gungraun 0.20's docs; what is fixed is that `--cache-sim=yes` reaches Callgrind for every benchmark in the crate.
- **The xtask spike packages** are named by constants in `xtask/src/main.rs`: `SPIKE_TOPICS: &[&str] = &["ecs"]`, `SPIKE_COMPARE_PACKAGES: &[&str] = &["spike_ecs_bench_compare"]` and `SPIKE_CACHE_PACKAGES: &[&str] = &["spike_ecs_bench_cache"]`. They are lists so a later spike adds one entry each.
- **`bench-doc`'s filter** is `pub(crate) fn drop_spikes(points: Vec<Point>) -> Vec<Point>`, which drops every point whose `group` starts with `spike/`. `run` calls it after `load::load`.
- **`check-statics`** scans `libs/*/src` today. It gains `spikes/*/*/src`, so the no-global-state rule covers spike code too. `check-unsafe` already reads the workspace members from `cargo metadata`, so it needs no change. A glob member next to a file (`spikes/README.md`) was checked in a scratch workspace: `cargo metadata` accepts it.

**Global constraints**

- Every commit passes `cargo xtask ci --quick`. Steps 1, 7 and 8 also pass `cargo xtask deny`, with `deny.toml` unchanged (R13).
- Every spike crate root has `#![forbid(unsafe_code)]` and `[lints] workspace = true`. Its public items are documented (R16). Doctests are not required. Crate-level `#![allow(...)]` lists like those of `bench/regress` are allowed only in `benches/` files.
- Packages: `spike_ecs_harness` (dependencies `engine`, `agents_2d = { path = "../../../examples/agents_2d" }`), `spike_ecs_vec` (dependencies `engine`, `agents_2d`, `spike_ecs_harness`), `spike_ecs_bench_compare` (dev-dependency `criterion = { version = "0.8.2", default-features = false, features = ["cargo_bench_support"] }`), `spike_ecs_bench_cache` (dev-dependency `gungraun.workspace = true`). Every package has `publish = false` through the workspace. The spike packages are not added to `[workspace.dependencies]`; they use path dependencies.
- Workspace `members` gains `"spikes/*/*"`.
- Seed 42 everywhere a default is needed. Conformance and rule tests use `Params { count: 1_000, seed: 42 }` (`count: 20` for `frag_iter`) and 120 ticks, except `agents` (1,000 agents, 1,300 ticks, R6).
- Commit messages are conventional, end with ` (M3.2)`, and end with the session's attribution line. Stage by explicit path.
- Stop and ask the maintainer before the first push and before opening the pull request.

**Review focus** (inputs the spec implies but no requirement's test pins on its own; each has a test in the step that owns the code)

1. **`count: 0`.** Every workload sets up with no entities, has checksum 0, and steps without panicking. `entity_churn` spawns nothing, and the `sprites` draw list stays empty. Steps 2 to 6: `zero_count_is_valid` for each workload.
2. **Long runs stay finite.** Criterion calls `step` millions of times on one instance. No value may become NaN or infinite: positions in `simple_iter` saturate through rounding, `heavy_compute`'s matrices must stay invertible after many inversions, and `frag_iter` converges. Steps 2, 3 and 5: `long_run_stays_finite` (10,000 steps at `count: 10`, then every float is finite and, for `heavy_compute`, every determinant is non-zero).
3. **The `sprites` wrap can land exactly on the bound.** For `p = -1e-6`, `p + 320.0` rounds to `320.0` in `f32`, so after a step a translation is in `[0, B]`, not `[0, B)`. The rule test MUST assert the closed range, and a unit test pins the case: Step 5, `wrap_of_a_tiny_negative_lands_on_the_bound`.
4. **Churn with tiny populations.** At `count: 1`, the single entity expires and is replaced repeatedly, and spawn ids keep increasing. Step 4: `single_entity_churns_forever` (200 steps, population 1 throughout, each new spawn id larger than the last).
5. **Divergence at setup.** A candidate whose initial state differs must be reported at tick 0, not tick 1. Step 1: `divergence_after_setup_is_tick_zero`.

## 2. Steps

### Step 1: harness core: interface, hash builder and conformance check (R1, R2, R3, R13)

- **Files:** `Cargo.toml` (members), `spikes/ecs_storage/harness/Cargo.toml`, `spikes/ecs_storage/harness/src/lib.rs`, `src/fnv.rs`, `src/conformance.rs` (new), `Cargo.lock`.
- **Interfaces produced:**
  - `pub struct Params { pub count: usize, pub seed: u64 }` (derives `Clone, Copy, Debug, PartialEq, Eq`), `pub trait Spec { const NAME: &'static str; const SIZES: &'static [usize]; }` and `pub trait Workload: Sized { type Spec: Spec; const BACKEND: &'static str; fn setup(params: Params) -> Self; fn step(&mut self); fn checksum(&self) -> u64; }`, all as in spec §4.2.
  - `pub fn dt() -> f32`.
  - `pub struct Fnv1a(u64)` with `pub const fn new() -> Self` and `#[must_use]` chaining methods `u8(self, u8) -> Self`, `u32`, `u64`, `f32` (hashes `to_bits().to_le_bytes()`), `vec2(self, Vec2)`, `vec3(self, Vec3)`, `finish(self) -> u64`. `Default` calls `new`.
  - `pub fn check_conformance<R, C>(params: Params, ticks: u64) -> Result<(), Divergence>` where `R: Workload, C: Workload<Spec = R::Spec>`, and `pub struct Divergence { pub workload, pub reference, pub candidate: &'static str, pub tick: u64, pub expected: u64, pub actual: u64 }` with `Display` (`"{workload}: {candidate} diverges from {reference} at tick {tick} (expected {expected:#018x}, got {actual:#018x})"`) and `std::error::Error`.
- **Test first** (inline test modules):
  - `fnv1a_matches_the_published_vectors`: `Fnv1a::new().finish() == 0xcbf2_9ce4_8422_2325`, and the hash of byte `b'a'` is `0xaf63_dc4c_8601_ec8c`.
  - `f32_hashes_its_bits`: `Fnv1a::new().f32(1.5).finish() == Fnv1a::new().u32(1.5_f32.to_bits()).finish()`, and `vec3(v)` equals `.f32(v.x).f32(v.y).f32(v.z)`.
  - `dt_is_sixty_hz`: `dt() == agents_2d::Sim::new(&SimConfig::default()).dt()`.
  - With a toy workload in the test module (`Counter`: state `u64`, `step` adds 1, `checksum` returns the state; `Spec` `"counter"`, sizes `[1]`) and two candidates (`OffByOneAtSetup` starting at 1; `BreaksAtTick3` adding 2 on its third step):
    - `a_workload_conforms_to_itself`: `check_conformance::<Counter, Counter>(p, 10) == Ok(())`.
    - `divergence_after_setup_is_tick_zero` (review focus 5): `OffByOneAtSetup` gives `Err` with `tick == 0`, `expected == 0`, `actual == 1`.
    - `divergence_reports_the_first_tick`: `BreaksAtTick3` gives `tick == 3`, with both names and the workload name set.
    - `divergence_message_names_everything`: `to_string()` contains `"counter"`, both backend names, `"tick 3"` and both checksums in `0x` form.
  - Run `cargo xtask test`: compile failure (the package does not exist).
- **Change:** implement the items above. Crate docs say what the harness is for and point to the spec.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes; `cargo xtask deny` passes; `cargo xtask check-unsafe` lists `spike_ecs_harness`.
- **Commit:** `feat(spikes): add the ECS storage harness core and conformance check (M3.2)`

### Step 2: `simple_iter` and the `Vec` backend crate (R4, R5, R7, R8, R9)

- **Files:** `spikes/ecs_storage/harness/src/simple_iter.rs` (new), harness `lib.rs` (module), `spikes/ecs_storage/vec/Cargo.toml`, `spikes/ecs_storage/vec/src/lib.rs`, `src/simple_iter.rs`, `src/tests.rs` (new), `Cargo.lock`.
- **Interfaces produced:**
  - `spike_ecs_harness::simple_iter`: `pub struct SimpleIter;` (`Spec`: `"simple_iter"`, `[1_000, 10_000, 100_000]`); `pub struct Matrix(pub Mat4)`, `Position(pub Vec3)`, `Rotation(pub Vec3)`, `Velocity(pub Vec3)`; `pub fn spawn(prng: &Prng, i: u64) -> (Matrix, Position, Rotation, Velocity)`; `pub fn update(position: &mut Position, velocity: &Velocity)`; `pub fn hash(position: &Position, velocity: &Velocity) -> u64`.
  - `spike_ecs_vec::VecSimpleIter` (`BACKEND = "vec_aos"`), with `pub(crate) entities: Vec<SimpleIterEntity>` and `pub(crate) struct SimpleIterEntity { matrix, position, rotation, velocity }`.
  - Test module, used by later steps: `trait Reversible`, `fn assert_order_independent<W: Workload + Clone + Reversible>(params: Params)` (spec §8 R7: reverse after setup and after 10 steps, compare over 20 more steps), `struct StepTwice<W>`, `fn assert_zero_count_is_valid<W: Workload>()`.
- **Test first:**
  - `simple_iter_spawn_follows_the_draw_order`: for `i = 7`, the components equal nine direct draws from `prng.stream(7)` in the order of spec §4.4.1, with the matrix `Mat4::IDENTITY`.
  - `simple_iter_moves_position_by_velocity`: after one step, each position equals its old position plus its velocity, and matrix and rotation are unchanged.
  - `simple_iter_is_order_independent` (R7), `simple_iter_zero_count_is_valid` (review focus 1), `simple_iter_long_run_stays_finite` (review focus 2).
  - `skipping_one_entity_is_detected` (R8): `SkipOneSimpleIter` makes `check_conformance` return `tick == 1`.
  - `stepping_twice_is_detected` (R8): `StepTwice<VecSimpleIter>` returns `tick == 1`.
  - Run `cargo xtask test`: compile failure.
- **Change:** the module and the backend. `setup` reserves `count` entities.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the simple_iter workload and the Vec backend (M3.2)`

### Step 3: `frag_iter` and `heavy_compute` (R4, R5, R7, R9)

- **Files:** harness `src/frag_iter.rs`, `src/heavy_compute.rs` (new), harness `lib.rs`; vec `src/frag_iter.rs`, `src/heavy_compute.rs` (new), vec `lib.rs`, `src/tests.rs`.
- **Interfaces produced:**
  - `frag_iter`: `pub struct FragIter;` (`"frag_iter"`, `[20]`); `pub const LETTERS: u8 = 26`; 26 letter types `A` to `Z`, each `pub struct X(pub f32)`, generated by a `macro_rules!`; `pub enum Letter { A(A), …, Z(Z) }` with `pub fn index(&self) -> u8` and `pub fn value(&self) -> f32`; `pub struct Data(pub f32)`; `pub fn spawn(prng: &Prng, i: u64, count: usize) -> (Letter, Data)`; `pub fn update(data: &mut Data)`; `pub fn hash(letter: &Letter, data: &Data) -> u64`. The population is `26 * count`.
  - `heavy_compute`: `pub struct HeavyCompute;` (`"heavy_compute"`, `[1_000]`); `pub const INVERSIONS: u32 = 100`; it reuses `simple_iter`'s component types; `pub fn spawn(prng: &Prng, i: u64) -> (Matrix, Position, Rotation, Velocity)`; `pub fn update(matrix: &mut Matrix, position: &mut Position, rotation: &Rotation)`; `pub fn hash(position: &Position, matrix: &Matrix) -> u64`.
  - `spike_ecs_vec::VecFragIter`, `VecHeavyCompute`.
- **Test first:**
  - `frag_iter_spawn_follows_the_draw_order` (entity `i = 45` at `count = 20` is letter `C`, index 2), `frag_iter_has_count_entities_per_letter`, `frag_iter_data_follows_the_map` (`d * 0.5 + 1.0`), `frag_iter_is_order_independent`, `frag_iter_zero_count_is_valid`, `frag_iter_long_run_stays_finite`.
  - `heavy_compute_spawn_follows_the_draw_order` (the 9 then 9 then 3 draws of spec §4.4.3; row 3 is `(0, 0, 0, 1)`), `heavy_compute_matrices_are_invertible` (`determinant() != 0.0` for all 1,000), `heavy_compute_position_is_the_transformed_rotation` (after a step, `position == matrix.transform_point3(rotation)` with the written-back matrix), `heavy_compute_is_order_independent`, `heavy_compute_zero_count_is_valid`, `heavy_compute_long_run_stays_finite` (review focus 2, including determinants).
  - Run `cargo xtask test`: compile failure.
- **Change:** the modules and the backends. `VecFragIter` stores `Vec<(Letter, Data)>`.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the frag_iter and heavy_compute workloads (M3.2)`

### Step 4: `add_remove` and `entity_churn` (R4, R5, R7, R8, R9)

- **Files:** harness `src/add_remove.rs`, `src/entity_churn.rs` (new), harness `lib.rs`; vec `src/add_remove.rs`, `src/entity_churn.rs` (new), vec `lib.rs`, `src/tests.rs`.
- **Interfaces produced:**
  - `add_remove`: `pub struct AddRemove;` (`"add_remove"`, `[1_000, 10_000, 100_000]`); `pub struct A(pub f32)`, `pub struct B(pub f32)`; `pub fn spawn(prng: &Prng, i: u64) -> A`; `pub fn make_b(a: &A) -> B`; `pub fn hash(a: &A, b: Option<&B>) -> u64`.
  - `entity_churn`: `pub struct EntityChurn;` (`"entity_churn"`, `[1_000, 10_000, 100_000]`); `pub struct SpawnId(pub u64)`, `Position(pub Vec2)`, `Velocity(pub Vec2)`, `Lifetime(pub u32)`; `pub fn spawn(prng: &Prng, i: u64) -> (SpawnId, Position, Velocity, Lifetime)`; `pub fn update(position: &mut Position, velocity: &Velocity, lifetime: &mut Lifetime)`; `pub fn is_expired(lifetime: &Lifetime) -> bool`; `pub fn hash(id: &SpawnId, position: &Position, velocity: &Velocity, lifetime: &Lifetime) -> u64`.
  - `spike_ecs_vec::VecAddRemove` (`Vec<(A, Option<B>)>`) and `VecEntityChurn` (a `Vec` of entity structs plus `next_id: u64` and `count: usize`; removal with `retain`, then pushes).
- **Test first:**
  - `add_remove_spawn_follows_the_draw_order`, `add_remove_leaves_no_b_after_a_step`, `add_remove_is_order_independent`, `add_remove_zero_count_is_valid`.
  - `skipping_one_removal_is_detected` (R8): `SkipOneRemoval` leaves entity 0's `B`, and `check_conformance` reports `tick == 1`.
  - `entity_churn_spawn_follows_the_draw_order`, `entity_churn_keeps_count_alive` (after every one of 120 steps, `len == count`, no lifetime is 0, ids are unique), `entity_churn_new_ids_increase` (the ids spawned in one step are exactly `next_id_before..next_id_after`), `entity_churn_replaces_some_entities` (over 120 steps at 1,000 entities, between 4% and 9% replaced per step on average: a sanity range around spec §4.4.5's 6.5%), `entity_churn_is_order_independent`, `entity_churn_zero_count_is_valid`, `single_entity_churns_forever` (review focus 4).
  - `skipping_one_churn_entity_is_detected` and `stepping_churn_twice_is_detected` (R8).
  - Run `cargo xtask test`: compile failure.
- **Change:** the modules and the backends. `VecAddRemove::step` is two separate loops, insert then remove (spec §4.4.4); no test can observe the order from outside, so code review checks it.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the add_remove and entity_churn workloads (M3.2)`

### Step 5: `sprites` (R4, R5, R7, R9)

- **Files:** harness `src/sprites.rs` (new), harness `lib.rs`; vec `src/sprites.rs` (new), vec `lib.rs`, `src/tests.rs`.
- **Interfaces produced:** `pub struct Sprites;` (`"sprites"`, `[100_000]`); `pub struct Velocity2d(pub Vec2)`; `pub struct Sprite { pub color: u32, pub half_extent: f32 }`; `pub struct Hidden;`; `pub struct DrawItem { pub position: Vec2, pub half_extent: f32, pub color: u32 }`; `pub fn spawn(prng: &Prng, i: u64) -> (Transform2d, Velocity2d, Sprite, Option<Hidden>)`; `pub fn update(transform: &mut Transform2d, velocity: &Velocity2d, dt: f32)`; `pub fn wrap(p: f32, bound: f32) -> f32`; `pub fn draw_item(transform: &Transform2d, sprite: &Sprite) -> DrawItem`; `pub fn hash_entity(transform: &Transform2d, velocity: &Velocity2d, sprite: &Sprite, hidden: bool) -> u64`; `pub fn hash_draw_item(item: &DrawItem) -> u64`. World bounds come from `agents_2d::{WORLD_WIDTH, WORLD_HEIGHT}`. `spike_ecs_vec::VecSprites` holds the entities and `draw_list: Vec<DrawItem>`, reserved to `count`.
- **Test first:**
  - `sprites_spawn_follows_the_draw_order` (including `next_u32() & 0x00FF_FFFF` and `chance(0.1)`), `sprites_stay_inside_the_world` (after each of 120 steps, every coordinate is in the closed range `[0, B]`, review focus 3), `wrap_of_a_tiny_negative_lands_on_the_bound` (`wrap(-1e-6, 320.0) == 320.0`; `wrap(320.0, 320.0) == 0.0`; `wrap(-1.0, 320.0) == 319.0`), `sprites_draw_list_holds_exactly_the_visible` (the multiset of draw items equals the visible entities' `draw_item`s; its length equals the number without `Hidden`), `sprites_hidden_sprites_still_move`, `sprites_is_order_independent`, `sprites_zero_count_is_valid`, `sprites_long_run_stays_finite`.
  - Run `cargo xtask test`: compile failure.
- **Change:** the module and the backend. The checksum is the wrapping sum of the entity hashes and the draw-item hashes.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the sprites workload (M3.2)`

### Step 6: `agents`, array-of-structs and struct-of-arrays (R4, R5, R6, R7, R9)

- **Files:** harness `src/agents.rs` (new), harness `lib.rs`; vec `src/agents.rs` (new), vec `lib.rs`, `src/tests.rs`.
- **Interfaces produced:** `pub struct Agents;` (`"agents"`, `[10_000, 100_000]`); re-exports `agents_2d::{Agent, Input, Seeker, agent_hash, spawn_agent}`; `pub fn update(transform: &mut Transform2d, velocity: &mut Vec2, ttl: &mut u32, seeker: &mut Option<Seeker>, cursor: Option<Vec2>, dt: f32)`, which implements M3.0 §4.3 rules 1 to 4 for one agent from the spec text, not by calling into `agents_2d`. `spike_ecs_vec::VecAgents` (`BACKEND = "vec_aos"`) wraps `agents_2d::Sim`. `spike_ecs_vec::SoaAgents` (`BACKEND = "vec_soa"`) has the columns `ids: Vec<u64>`, `transforms: Vec<Transform2d>`, `velocities: Vec<Vec2>`, `ttls: Vec<u32>`, `seekers: Vec<Option<Seeker>>`, plus `prng`, `next_id`, `tick`, `target` and `dt`. It removes expired agents with `swap_remove` on every column at the same index, walking from the end, and hashes by building an `Agent` per index for `agent_hash`.
- **Test first:**
  - `vec_aos_agents_is_sim`: after 600 steps, `VecAgents::checksum()` equals `agents_2d::run(&SimConfig { agents: 1_000, seed: 42 }, 600).checksum`.
  - `vec_soa_agents_matches_vec_aos` (R6): `check_conformance::<VecAgents, SoaAgents>(Params { count: 1_000, seed: 42 }, 1_300) == Ok(())`.
  - `soa_agents_is_order_independent` (R7, with `Reversible` reversing every column together), `agents_zero_count_is_valid` (both backends).
  - Run `cargo xtask test`: compile failure.
- **Change:** the module and the two backends.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes. If `vec_soa_agents_matches_vec_aos` fails, the message names the tick. Fix the harness's `update` against M3.0 §4.3, never `agents_2d`.
- **Commit:** `feat(spikes): add the agents workload with array-of-structs and struct-of-arrays backends (M3.2)`

### Step 7: Criterion bench crate and `spike-bench` (R10, R12)

- **Files:** `spikes/ecs_storage/bench_compare/Cargo.toml`, `src/lib.rs` (one line of docs, empty), `benches/compare.rs` (new); `xtask/src/main.rs`; `xtask/src/bench_doc.rs`; `Cargo.lock`.
- **Interfaces produced:** in `benches/compare.rs`, `fn bench<W: Workload>(c: &mut Criterion)`. It opens group `format!("spike/ecs/{}", W::Spec::NAME)` with warm-up 1 s and measurement 3 s, and for each size in `W::Spec::SIZES` sets up `W` with seed 42 outside the timing and registers `BenchmarkId::new(W::BACKEND, count)` timing `w.step()` through `b.iter`. One `criterion_group!` calls `bench::<…>` for each of the 8 (workload, backend) pairs. In xtask: `Command::SpikeBench { topic: String, save: Option<String>, allow_battery: bool }`, `fn spike_bench_args(topic: &str, save: Option<&str>) -> Result<Vec<String>, String>`, and the constants of §1. `bench_args` and `bench_build_args` exclude `SPIKE_CACHE_PACKAGES`, and `bench_args` also excludes `SPIKE_COMPARE_PACKAGES`. `bench_doc::drop_spikes`.
- **Test first** (xtask unit tests next to the existing ones):
  - `spike_bench_parses_its_flags`: `["spike-bench", "ecs"]`, `["spike-bench", "ecs", "--save", "m3"]` and `["spike-bench", "ecs", "--allow-battery"]` parse. `["spike-bench"]` and `["spike-bench", "nope"]` are errors, and the second error's message contains `ecs`.
  - `spike_bench_args_select_the_topic_package`: `spike_bench_args("ecs", Some("m3"))` is `["bench", "-p", "spike_ecs_bench_compare", "--bench", "compare", "--", "--save-baseline", "m3"]`.
  - `bench_excludes_the_spike_packages`: `bench_args(None)` contains `--exclude spike_ecs_bench_compare` and `--exclude spike_ecs_bench_cache`. `bench_build_args()` contains `--exclude spike_ecs_bench_cache` but not `--exclude spike_ecs_bench_compare`.
  - `bench_doc_drops_spike_points`: of points with groups `spike/ecs/simple_iter` and `scenario/frame_update`, only the second remains.
  - Run `cargo test -p xtask`: compile failure.
- **Change:** the bench crate; the xtask command, which shares `bench`'s battery guard and `caffeinate` wrapping (factor the wrapping into a helper both call); the exclusions; and `drop_spikes` in `bench_doc::run`.
- **Verify:** `cargo test -p xtask` passes. `cargo xtask bench-smoke` runs the spike benches once. `cargo xtask spike-bench ecs` prints 16 results, one for each (workload, size, backend) entry: 14 for `vec_aos` and 2 for `vec_soa`. Keep the output for the pull request, labelled as informal. `cargo xtask deny` passes. `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the Criterion spike bench and cargo xtask spike-bench (M3.2)`

### Step 8: Gungraun cache-simulation bench crate (R11, R12)

- **Files:** `spikes/ecs_storage/bench_cache/Cargo.toml`, `src/lib.rs`, `benches/cache.rs` (new); `Cargo.lock`.
- **Interfaces produced:** in `benches/cache.rs`: `fn prepared<W: Workload>(count: usize) -> W` (setup with seed 42, then one `step`); `#[inline(never)] fn step_once<W: Workload>(w: &mut W)`; a `macro_rules! cache_bench` that emits, for one (name, type), a `#[library_benchmark]` with the `benches::sizes` attribute of §1 and a body that calls `step_once(black_box(&mut w))`. There is one group per workload, and `main!` carries the Callgrind configuration of §1. Benchmark function names are `<workload>_<backend>`, for example `simple_iter_vec_aos`.
- **Test first:** `cargo xtask bench-smoke` before the change has no `spike_ecs_bench_cache` entries. After it, every one of the 8 functions runs once for each of its sizes, without Valgrind.
- **Change:** the bench crate. It needs no xtask change beyond Step 7's exclusions.
- **Verify:** `cargo xtask bench-smoke` passes. In the devcontainer, `cargo bench -p spike_ecs_bench_cache` prints, for each of the 16 benchmarks, instructions, L1 hits, LL hits, RAM hits and estimated cycles. Keep that output for the pull request, labelled as informal. `cargo xtask deny` passes. `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the Gungraun cache-simulation spike bench (M3.2)`

### Step 9: coverage and static scanning cover the spikes (R12)

- **Files:** `xtask/src/coverage.rs`, `xtask/src/check_statics.rs`.
- **Test first:**
  - `test_args_match_the_spec` (existing, extended): the arguments also exclude `spike_ecs_harness`, `spike_ecs_vec`, `spike_ecs_bench_compare` and `spike_ecs_bench_cache`.
  - `ignore_regex_lists_the_excluded_dirs` (existing, extended): the regex matches `<root>/spikes/ecs_storage/harness/src/lib.rs`.
  - A new `check_statics` test, `spike_sources_are_scanned`: the function that lists the files to scan (factor it out of `run` as `fn scanned_dirs(root: &Path) -> Vec<PathBuf>` if it is inline) includes `spikes/ecs_storage/harness/src` in a temporary directory tree.
  - Run `cargo test -p xtask`: the extended tests fail.
- **Change:** add the four packages to the coverage exclusions, as one constant `SPIKE_PACKAGES` in `coverage.rs` that names them. Add `spikes` to the regex. Scan `spikes/*/*/src` in `check_statics`.
- **Verify:** `cargo test -p xtask` passes. `cargo xtask check-statics` passes over the spike sources. `cargo xtask coverage`'s table lists no spike file. `cargo xtask ci --quick` passes.
- **Commit:** `feat(xtask): keep spikes out of coverage and scan them for global state (M3.2)`

### Step 10: documents (R15)

- **Files:** `docs/roadmap.md`, `CONTRIBUTING.md`, `README.md`, `CHANGELOG.md`, `spikes/README.md`, `spikes/ecs_storage/README.md` (new).
- **Test first:** n/a for prose. The check before the change: `grep -n "M3.10" docs/roadmap.md` finds nothing, and `grep -n "Spike crates" CONTRIBUTING.md` finds nothing.
- **Change:** the text of spec §4.8. The roadmap's M3.2 row stays `Drafting` until the pull request, where it becomes `In progress`. The CONTRIBUTING section sits next to the coverage and mutation sections, which it refers to. `spikes/ecs_storage/README.md` walks through adding a backend, using the spec's §3 example.
- **Verify:** `cargo xtask ci` passes (full: `typos`, `doc` and the rest). The links in the new documents resolve (open each one).
- **Commit:** `docs: record the spike rules and the ECS storage harness (M3.2)`

## 3. Final verification

- `cargo xtask ci` passes locally on macOS. `cargo xtask bench-regress` is not needed: no file under `libs/` or `bench/regress/` changes.
- The pull request's CI is green on Ubuntu, macOS and Windows with no workflow change (R14).
- The spec's §9 checklist is ticked, with evidence: the test names for R6 to R9; the `spike-bench ecs` and `spike_ecs_bench_cache` outputs pasted in the pull request, labelled informal; the eight reviews of CONTRIBUTING.md recorded.
- After merge approval: the spec status becomes `Implemented (PR #n, date)`, this plan becomes `Historical`, the roadmap row becomes `Done (PR #n)`, and §4 is filled in.

## 4. Deviations

- **Eleven commits, not ten.** Step 8's benchmark sizes were first written as `iter = Spec::SIZES` (§1). Run under Valgrind, Gungraun named each benchmark after the setup expression's text (`sizes_0:(prepared :: < VecSimpleIter >(100_000))` with the type, never the size, truncated at its own length), so the sizes are now literal `args` in each `cache_bench!` line, the fallback §1 allows. `prepared` asserts (not `debug_assert!`, which is off in the bench profile) that the size is in `Spec::SIZES`. This shipped as a follow-up commit, `fix(spikes): write the cache bench sizes out so benchmark names show them`.
- **`agents_2d` is a dev-dependency of the harness in steps 1 to 4**, and a normal dependency from step 5 (where `sprites` uses `WORLD_WIDTH` and `WORLD_HEIGHT`). An unused normal dependency fails `clippy -D warnings`. `spike_ecs_vec` takes it as a normal dependency in step 6.
- **The two bench crates also depend on `spike_ecs_harness` and `spike_ecs_vec`** (dev-dependencies), which §1's package list omits.
- **Two existing xtask tests changed** (`bench_args_select_only_bench_targets_before_passing_flags_through` and `criterion_commands_leave_the_regress_benchmarks_alone`): they pin the exact argument lists that R12 changes.
- **`cargo xtask check-unsafe` prints only `ok`**, so step 1's "lists `spike_ecs_harness`" could not be observed; the command passes with the spike crates in the workspace.
- **Step 9's `cargo xtask coverage` check was not run**: the machine lacks `llvm-tools-preview` and the toolchain was not changed. The exclusion list and the regex are covered by the extended unit tests.
- **Step 8's Valgrind run** used a throwaway `rust:1-bookworm` container (aarch64) with Valgrind from apt, not the repository's devcontainer, and needed `--security-opt seccomp=unconfined` (Docker's default profile blocks `setarch`). The caveat is in `spikes/ecs_storage/README.md`.
- **`CONTRIBUTING.md`'s "Spike crates" section** also exempts spike crates from the inline-test-module rule of coverage (the vec crate keeps its tests in `src/tests.rs`, as step 2 says).
- **Extra public items** the plan did not list: `Letter::from_index` in `frag_iter`, and `pub(crate) draw_vectors` in `simple_iter` (the shared first nine draws of `simple_iter` and `heavy_compute`).
