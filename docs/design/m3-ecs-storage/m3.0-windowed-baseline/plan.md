# M3.0 plan: Windowed baseline sample

- **Status:** Accepted (2026-11-13)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both.

## 1. Approach

Five commits on branch `m3.0-windowed-baseline`, after the spec and roadmap commit already on it. The simulation comes first because it carries the chunk's normative content (the rules and the checksum) and needs no window to test. The renderer follows, as pure functions over a pixel slice. The `winit` host comes last, after a dependency probe, so that a licence problem is found before any host code is written. The docs commit closes the chunk.

Choices the spec leaves to implementation:

- **Probe before the host (no commit).** At the start of Step 4, add `winit = "0.30"` and `softbuffer = "0.4"` to the example's manifest and run `cargo xtask deny` and `cargo tree -p engine -i winit`. If `deny` rejects a licence, try `winit = { version = "0.30", default-features = false, features = ["x11", "wayland", "wayland-dlopen"] }` (dropping `wayland-csd-adwaita`) and run it again. If it still fails, stop and ask the maintainer (spec §4.7); do not touch `deny.toml`.
- **Public surface of the library.** The rule constants and two per-agent functions are public, because M3.2's harness and the prototypes reuse them instead of copying numbers: `spawn_agent` (the spawn rule) and `agent_hash` (one agent's checksum term). The per-agent update stays private (`fn update`); prototypes reimplement it on their own storage, which is the point of the comparison.
- **Module layout.** `src/lib.rs` holds the simulation; `src/render.rs` is `pub mod render`, and `lib.rs` re-exports `Frame`, `View` and `render` at the crate root. Unit tests are inline `#[cfg(test)]` modules (coverage rule in CONTRIBUTING).
- **Pixel arithmetic.** Screen coordinates are `f32`; `floor` then `as i64` (a saturating cast), and clipping happens in `i64`, so no window size or scale overflows.
- **Golden values.** The golden checksum and the golden pixel hash are captured from the first passing implementation, after the rule tests pass, as `fixed_step_sim` did. Each test is first written against `0`, run to see it fail with the actual value, and the value is pinned only after reading the summary it belongs to (agents equal to the target, a plausible seeker count). R7 is then confirmed by CI on all three platforms.

**Global constraints**

- Every commit passes `cargo xtask ci --quick`; Step 4 also passes `cargo xtask deny`.
- Commit messages are conventional, end with ` (M3.0)`, and end with the session's attribution line. Stage by explicit path.
- No change under `libs/`, to `deny.toml`, to `.github/` or to `rust-toolchain.toml` (spec R2, R14, R15).
- `winit` 0.30 and `softbuffer` 0.4, in `examples/agents_2d/Cargo.toml` only.
- Every crate root has `#![forbid(unsafe_code)]`; the library uses no banned method (`clippy.toml`).
- Stop and ask the maintainer before the first push and before opening the pull request.

**Review focus** (inputs the spec implies but no requirement's test pins on its own; each has a test in the step that owns the code)

1. **An agent exactly on a world edge** (`x == 0.0` or `x == W`) must not bounce: the comparisons are strict. Step 2: `exactly_on_the_edge_does_not_bounce`.
2. **The cursor exactly on a seeker, or exactly at the catch radius**: `len2 == 0` and `len2 == CATCH * CATCH` both catch, and no `NaN` appears (the division only happens when `len2 > CATCH * CATCH`). Step 2: `cursor_on_the_agent_catches_without_nan` and `exactly_at_the_catch_radius_catches`.
3. **A minimised window** (zero width or height) and a 1 by 1 window: nothing panics, `cursor_to_world` is `None` at zero size, and the host skips drawing. Step 3: `zero_sized_view_draws_nothing_and_maps_no_cursor`, plus the proptest over sizes from 0.
4. **Zero agents** (`--agents 0`): the simulation steps, its checksum is 0, and the renderer draws only the background and border. Step 1: `empty_sim_has_zero_checksum`; Step 3: `empty_sim_renders_background_and_border`.
5. **A seeker caught and bounced in the same tick** ends it as a seeker with `GAINED_ACCELERATION` (spec §4.3 rule 3). Step 2: `caught_then_bounced_is_a_seeker_again`.

## 2. Steps

### Step 1: agents, spawning and the checksum (R1, R4, R5, R17)

- **Files:** `examples/agents_2d/Cargo.toml` (new: `engine = { workspace = true }`; dev-dependency `proptest = { workspace = true }`; `[lints] workspace = true`; description "2D agents in a window: the M3 storage baseline"), `examples/agents_2d/src/lib.rs` (new).
- **Interfaces produced** (all in `lib.rs`, all documented, every public type with a doctest):
  - Constants: `WORLD_WIDTH: f32 = 320.0`, `WORLD_HEIGHT: f32 = 180.0`, `TICK_HZ: u32 = 60`, `SPAWN_SPEED: f32 = 20.0` (velocity components in `-SPAWN_SPEED..SPAWN_SPEED`), `TTL_MIN: u32 = 60`, `TTL_MAX: u32 = 600`, `SEEKER_CHANCE: f64 = 0.25`, `SEEKER_ACCELERATION_MIN: f32 = 40.0`, `SEEKER_ACCELERATION_MAX: f32 = 80.0`, `GAINED_ACCELERATION: f32 = 60.0`, `MAX_SPEED: f32 = 30.0`, `CATCH_RADIUS: f32 = 4.0`.
  - `SimConfig { pub seed: u64, pub agents: usize }`, `Default` = seed 42, 10,000 agents; derives `Clone, Debug, PartialEq, Eq`.
  - `Seeker { pub acceleration: f32 }`, `Agent { pub id: u64, pub transform: Transform2d, pub velocity: Vec2, pub ttl: u32, pub seeker: Option<Seeker> }`; both derive `Clone, Copy, Debug, PartialEq`.
  - `SimSummary { pub tick: u64, pub agents: usize, pub seekers: usize, pub checksum: u64 }`; derives `Clone, Copy, Debug, PartialEq, Eq`.
  - `pub fn spawn_agent(prng: &Prng, id: u64) -> Agent`: spec §4.3 "Spawning", draws in the spec's order.
  - `pub fn agent_hash(agent: &Agent) -> u64`: spec §4.3 "Checksum", one agent's FNV-1a term.
  - `Sim` (derives `Clone, Debug`; fields `agents: Vec<Agent>`, `prng: Prng`, `next_id: u64`, `tick: u64`, `target: usize`, `dt: f32`): `pub fn new(config: &SimConfig) -> Sim` (reserves `config.agents`, spawns ids `0..agents`, `dt = FixedTimestep::from_hz(NonZeroU32 60).step().as_secs_f32()`), `pub fn tick(&self) -> u64`, `pub fn dt(&self) -> f32`, `pub fn agents(&self) -> &[Agent]`, `pub fn checksum(&self) -> u64` (wrapping sum of `agent_hash`), `pub fn summary(&self) -> SimSummary`.
- **Test first** (inline `#[cfg(test)] mod tests`):
  - `spawn_draws_in_spec_order`: for ids 0, 1 and 12345 with `Prng::from_seed(42)`, build the expected agent by drawing from `prng.stream(id)` by hand in the spec's order (`range(0.0..W)`, `range(0.0..H)`, two `range(-20.0..20.0)`, `range(60..=600)`, `chance(0.25)`, then `range(40.0..80.0)` only if a seeker) and assert `spawn_agent` equals it.
  - `new_spawns_ids_in_order`: `Sim::new(&SimConfig { agents: 100, .. })` has ids `0..100`, `tick() == 0`, and every transform has `rotation == 0.0` and `scale == Vec2::ONE` (R5).
  - `agent_hash_matches_hand_computed_fnv`: for one fixed wanderer and one fixed seeker, compare with an FNV-1a computed in the test over the byte sequence the spec lists, written out as a `Vec<u8>`.
  - `checksum_ignores_storage_order`: a `Sim` with 1,000 agents and a clone whose `agents` vector is reversed (from inside the crate) have equal checksums.
  - `empty_sim_has_zero_checksum`: `agents: 0` gives `checksum() == 0` and `summary().agents == 0` (review focus 4).
  - Run `cargo xtask test`: it fails to compile because the crate's items do not exist.
- **Change:** implement the items above. `SimSummary::seekers` counts agents with `seeker.is_some()`. `Sim::new` documents a `# Panics` section stating it never panics in practice, as `fixed_step_sim` does for its 60 Hz `expect`.
- **Verify:** `cargo xtask test` passes the five tests and the doctests; `cargo xtask ci --quick` passes.
- **Commit:** `feat(examples): add agents_2d agents, spawning and checksum (M3.0)`

### Step 2: the step rules, scripted input and determinism (R3, R4, R5, R6, R7, R8)

- **Files:** `examples/agents_2d/src/lib.rs`, `examples/agents_2d/tests/determinism.rs` (new).
- **Interfaces produced:**
  - `Input { pub cursor: Option<Vec2> }`, derives `Clone, Copy, Debug, Default, PartialEq`; `pub fn scripted(tick: u64) -> Input` (spec §4.4).
  - `pub fn step(&mut self, input: &Input)` on `Sim`: calls `update` on every agent, then `retain(|a| a.ttl > 0)`, then spawns up to `target`, then increments `tick`.
  - private `fn update(agent: &mut Agent, cursor: Option<Vec2>, dt: f32)`: rules 1 to 4 of spec §4.3, written component by component in the spec's operation order (no `Vec2::normalize` or `length`, whose internal order the spec does not fix).
  - `pub fn run(config: &SimConfig, ticks: u64) -> SimSummary`.
- **Test first** (unit tests call `update` directly with `dt = 1.0 / 60.0` written as the `Sim`'s `dt`):
  - `seeker_steers_toward_the_cursor`: a seeker at rest at `(100, 90)`, acceleration 60, cursor `(200, 90)`: after `update`, `velocity.x > 0`, `velocity.y == 0`, and `velocity.x == 60.0 * dt` exactly.
  - `steering_clamps_speed`: a seeker moving at `(30, 0)` toward a cursor ahead ends with `vx * vx + vy * vy <= MAX_SPEED * MAX_SPEED * (1 + 1e-6)`.
  - `exactly_at_the_catch_radius_catches` and `cursor_on_the_agent_catches_without_nan` (review focus 2): the seeker part is removed, velocity is unchanged and finite.
  - `wanderer_ignores_the_cursor`: velocity is unchanged by rule 1.
  - `each_edge_clamps_and_reflects`: four cases (left, right, bottom, top) moving out of the world: position equals the bound, the crossing component's sign is reversed, the other is unchanged, and a wanderer gains `Seeker { acceleration: GAINED_ACCELERATION }`; a seeker keeps its own acceleration.
  - `exactly_on_the_edge_does_not_bounce` (review focus 1).
  - `caught_then_bounced_is_a_seeker_again` (review focus 5).
  - `expired_agents_are_replaced_with_new_ids`: a `Sim` of 10 agents with every `ttl` set to 1 (from inside the crate): after one `step`, 10 agents, ids `10..20`, `tick() == 1`.
  - `step_ignores_storage_order`: two clones, one reversed at tick 50, both stepped with `Input::scripted` to tick 300; checksums equal after every step (R4).
  - `transforms_keep_identity_rotation_and_scale`: after `run`-equivalent 600 steps, every agent has rotation 0 and scale one (R5).
  - `scripted_input_follows_the_spec`: ticks 0 → `(0, 0)`, 150 → `(160, 90)`, 300 → `(320, 180)`, 450 → `(160, 90)`, 599 → `(W * f, H * f)` with `f = 1.0_f32 / 300.0` (the spec's formula, so the comparison is exact), 1200 and 1799 → `None` (R8).
  - `tests/determinism.rs`: `same_config_same_summary`; `different_seed_different_checksum` (seed 43); `clone_mid_run_continues_identically` (clone at 300, both to 600); `golden_checksum` against `const GOLDEN: u64 = 0` first (R6, R7).
  - Run `cargo xtask test`: compile failure (`Input`, `step`, `run` missing).
- **Change:** implement the interfaces. Then run the golden test, read the actual summary (`agents == 10_000`, `0 < seekers < 10_000`), pin `GOLDEN`, and add a doc comment saying it must match on every architecture (spec R7).
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(examples): add agents_2d step rules and scripted input (M3.0)`

### Step 3: the renderer and the allocation test (R9, R10, R11, R12)

- **Files:** `examples/agents_2d/src/render.rs` (new), `examples/agents_2d/src/lib.rs` (`pub mod render; pub use render::{Frame, View, render};`), `examples/agents_2d/tests/allocations.rs` (new), `examples/agents_2d/Cargo.toml` (dev-dependency `dhat = "0.3"`, as `fixed_step_sim`).
- **Interfaces produced** (in `render.rs`):
  - Colour constants `BACKGROUND = 0x10_1418`, `BORDER = 0x3A_4048`, `WANDERER = 0x6E_C6FF`, `SEEKER = 0xFF_B347` (`u32`, `0x00RRGGBB`).
  - `Frame<'a>`: `pub fn new(pixels: &'a mut [u32], width: u32, height: u32) -> Option<Frame<'a>>` (`None` unless `usize` `width * height`, computed with `checked_mul`, equals `pixels.len()`), `width()`, `height()`, `pixels(&self) -> &[u32]`. `Debug` is implemented by hand and prints the size only.
  - `View` (derives `Clone, Copy, Debug, PartialEq`): `pub fn fit(width: u32, height: u32) -> View`, `scale(&self) -> f32`, `world_to_screen(&self, p: Vec2) -> Vec2`, `screen_to_world(&self, s: Vec2) -> Vec2`, `cursor_to_world(&self, s: Vec2) -> Option<Vec2>` (`None` when the scale is 0 or the point is outside `[0, W] × [0, H]`).
  - `pub fn render(sim: &Sim, view: &View, frame: &mut Frame<'_>)`: spec §4.5. Returns after the background fill when the frame or the view has a zero dimension.
- **Test first:**
  - `frame_new_checks_the_length`: `Frame::new` with a buffer one short, one long, and `width = height = u32::MAX` on a small buffer all give `None`; an exact length gives `Some` (R10).
  - `fit_letterboxes`: `View::fit(640, 480)` has scale 2.0, and `world_to_screen((0, 180)) == (0, 60)`, `world_to_screen((320, 0)) == (640, 420)`.
  - `zero_sized_view_draws_nothing_and_maps_no_cursor` (review focus 3): `View::fit(0, 100)` and `View::fit(100, 0)` map every cursor to `None`; rendering a 0 by 0 frame does not panic.
  - `cursor_outside_the_world_is_none`: a point in the letterbox bar maps to `None`.
  - `empty_sim_renders_background_and_border` (review focus 4): with zero agents, every pixel is `BACKGROUND` or `BORDER`, and at least one is `BORDER`.
  - proptest `round_trip_within_half_a_pixel`: sizes `1..2000` by `1..2000`, points in the world: `screen_to_world(world_to_screen(p))` within `0.5 / scale` of `p` (R11).
  - proptest `render_never_panics_or_spills`: sizes `0..64` by `0..64`, a view fitted to a size that may differ from the frame's, 50 agents after a few scripted steps. Render into a frame embedded in a larger buffer whose surrounding pixels hold a sentinel; the sentinel is intact afterwards (R10, R11).
  - `golden_pixels`: FNV-1a over the little-endian bytes of the pixels of `render` for `SimConfig { agents: 200, ..Default::default() }` after 60 scripted ticks, `View::fit(320, 180)`, a 320 by 180 frame; against `0` first, then pinned (R12).
  - `tests/allocations.rs`: `zero_allocations_in_steady_state`, in the `fixed_step_sim` style: default config, 120 warm-up ticks with a render after each into a preallocated 640 by 360 frame, then 600 more; `total_blocks` unchanged (R9).
  - Run `cargo xtask test`: compile failure (`render` module missing).
- **Change:** implement `render.rs` per spec §4.5. Square side `max(1, floor(scale))`; top-left `(floor(sx) - side / 2, floor(sy) - side / 2)` in `i64`; clip to the frame before writing. Border: the outline of the rectangle from `world_to_screen((0, H))` to `world_to_screen((W, 0))`, floored, clipped. Pin the golden pixel hash as described in §1.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(examples): add the agents_2d software renderer (M3.0)`

### Step 4: the `winit` host and command line (R2, R13, R14, R15)

- **Files:** `examples/agents_2d/Cargo.toml` (`winit = "0.30"`, `softbuffer = "0.4"`, possibly with the feature change from the probe), `examples/agents_2d/src/main.rs` (new), `Cargo.lock`.
- **Probe first:** as described in §1. Record the outcome (which features, whether `deny` passed first time) for §4 Deviations if it differs from the spec.
- **Interfaces produced** (private to the binary):
  - `enum Mode { Window, Headless { ticks: u64 } }`, `struct Args { config: SimConfig, mode: Mode }`, `fn parse(args: &[String]) -> Result<Args, String>`.
  - `fn summary_line(summary: &SimSummary) -> String`: `tick={} agents={} seekers={} checksum={:#018x}`.
  - `struct App` implementing `winit::application::ApplicationHandler`: holds the `Sim`, a `View`, `FixedTimestep::from_hz(60)`, `SystemClock`, the last frame's `Timestamp`, the last physical cursor position, `paused`, the title statistics, and `Option<Gfx>` where `Gfx` holds `Rc<Window>` and `softbuffer::Surface<Rc<Window>, Rc<Window>>`.
- **Test first** (unit tests in `main.rs`):
  - `parse_defaults`: no arguments give `SimConfig::default()` and `Mode::Window`.
  - `parse_agents_and_seed`: `--agents 5 --seed 7`.
  - `parse_headless`: `--headless` gives 600 ticks; `--headless --ticks 10` gives 10; `--ticks 10 --headless` also gives 10.
  - `parse_errors_name_the_problem`: an unknown flag, `--agents` with no value, `--agents x`, and `--ticks` without `--headless` each give an `Err` containing the flag (and the bad value where there is one).
  - `summary_line_format`: `SimSummary { tick: 600, agents: 3, seekers: 1, checksum: 0xAB }` gives `tick=600 agents=3 seekers=1 checksum=0x00000000000000ab`.
  - Run `cargo xtask test`: compile failure (`parse` missing).
- **Change:** implement `main.rs` per spec §4.6. `main` returns `ExitCode`: parse errors print `agents_2d: <message>` to stderr and fail; headless prints `summary_line(&run(..))`; window mode builds the `EventLoop`, runs `run_app`, and reports an event-loop or `softbuffer` error to stderr with a failure code. Space toggles pause on a non-repeat press of `Key::Named(NamedKey::Space)`. On `Resized`, store the size; the surface is resized and `View` refitted at the start of the next redraw when both dimensions are non-zero. The title updates when at least one second has passed since the last update.
- **Verify:**
  - `cargo xtask deny` passes with `deny.toml` unchanged (R14).
  - `cargo tree -p engine -i winit` and `cargo tree -p engine -i softbuffer` both report that the package is not in `engine`'s tree (R2).
  - `cargo xtask ci --quick` passes.
  - `cargo run -p agents_2d --release -- --headless` prints the golden checksum from Step 2.
  - Manual, on macOS: `cargo run -p agents_2d --release` opens the window; seekers chase the cursor and turn blue on reaching it; wanderers turn orange on bouncing; Space pauses and resumes; resizing letterboxes; Escape quits. Take a screenshot for the pull request.
  - In the devcontainer: `cargo build -p agents_2d` succeeds without installing system packages (it cannot open a window there).
- **Commit:** `feat(examples): add the agents_2d winit host (M3.0)`

### Step 5: documentation (R16, R17)

- **Files:** `README.md` (the "Also in the workspace" paragraph gains `examples/agents_2d`), `CHANGELOG.md` (an "Added" entry under Unreleased: the example, its workload and checksum, `winit` and `softbuffer` as example-only dependencies; and a "Changed" entry for the roadmap: M3 current and chunked, prototypes kept under `spikes/`, the M7 events question), `docs/roadmap.md` (M3.0 row: `In progress`, plan link), this plan's §4.
- **Test first:** the documentation review checklist in CONTRIBUTING.md fails before the change: the README does not mention the example and the CHANGELOG has no entry.
- **Change:** as listed. Spec and plan statuses become `Accepted (date)` at the maintainer's approval (before Step 1), and `Implemented` and `Historical` at merge, as the maintainers checklist says.
- **Verify:** `cargo xtask ci` (the full run, including `doc`, `typos`, `deny`); relative links in the changed files resolve.
- **Commit:** `docs: record agents_2d in the README and changelog (M3.0)`

## 3. Final verification

- `cargo xtask ci` passes locally on macOS.
- With the maintainer's go-ahead: push, open the pull request as a draft, and confirm CI is green on Ubuntu, macOS and Windows, in particular the golden checksum (R7) and the build without new system packages (R15). If a platform's golden value differs, stop and report it (spec §6, Determinism).
- Run the reviews in CONTRIBUTING.md ("Reviews before a pull request"): `/code-review`, `/security-review` (two new dependencies and their trees), documentation, spec conformance (each requirement against §8 of the spec), public API (n/a: no change under `libs/`), determinism (the rule implementation against spec §4.3), performance (n/a: no hot path in `libs/`, no claim), portability (the three CI platforms; the manual run was macOS only).
- The pull request says an assistant was used, includes the macOS screenshot and the headless output, lists all eight review results, and states that the window was not run by hand on Linux or Windows.
- Walk the spec's §9 "Done when" list.

## 4. Deviations

- **`winit` feature set (Step 4).** The probe with `winit = "0.30"` failed `cargo xtask deny`, but on an advisory, not a licence: `ttf-parser` is unmaintained (RUSTSEC-2026-0192), reached through `wayland-csd-adwaita` -> `sctk-adwaita` -> `ab_glyph`. Licences passed. The plan's second step applied: `winit = { version = "0.30", default-features = false, features = ["rwh_06", "x11", "wayland", "wayland-dlopen"] }` (`rwh_06` kept because it is one of winit's default features and `softbuffer` needs it). `cargo xtask deny` then passed with `deny.toml` unchanged. Cost: Wayland compositors that do not draw server-side window decorations show an undecorated window.
- **Golden values.** `GOLDEN` in `tests/determinism.rs` is `0xa352_0200_2876_ad91` (10,000 agents, 3,984 seekers after 600 ticks); the pixel hash in `render::tests::golden_pixels` is `0x57e2_497e_8128_338b`. Both were captured on macOS (aarch64); R7 awaits CI on Ubuntu and Windows.
- **Manual check.** The maintainer ran `cargo run -p agents_2d --release` on macOS and confirmed the behaviour listed under Step 4 (seekers chase, colours change, pause, resize, title statistics). No screenshot has been taken yet. `cargo build -p agents_2d` succeeded in the devcontainer image (aarch64 Linux) without new system packages. Not run by hand on Linux with a display or on Windows.
