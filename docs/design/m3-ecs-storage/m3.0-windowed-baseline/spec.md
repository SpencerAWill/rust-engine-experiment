# M3.0: Windowed baseline sample

- **Status:** Draft (2026-11-13)
- **Milestone:** M3 (ECS storage decision) · **Depends on:** none · **Decides:** no ADR
- **Estimated size:** about 750 changed lines, excluding tests, lockfiles and generated files (`examples/agents_2d` library about 350, binary about 200, roadmap, README, CHANGELOG and this chunk's documents about 200)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds `examples/agents_2d`: thousands of 2D agents that wander, bounce, gain and lose an optional "seeker" part, die and respawn, stored in one plain `Vec` and drawn into a window through `winit` and `softbuffer`. It comes first in M3 because it defines the agent workload and the plain-`Vec` baseline that every ECS storage prototype (M3.3 to M3.7) is checked and measured against, and because it tests [ADR 0007](../../../adr/0007-host-owns-the-loop.md) (the host owns the loop) against a real windowing event loop before M4 depends on it. At the end, `cargo run -p agents_2d --release` opens a window where agents chase the cursor, and `cargo run -p agents_2d --release -- --headless` prints a checksum that the tests pin.

The chunk also breaks M3 into chunks in the roadmap, as the maintainers checklist asks at a milestone boundary.

## 2. Goals and non-goals

**Goals**

- A deterministic, headless agent simulation whose rules are written down here (§4.3) precisely enough that a different storage model can reproduce its checksum.
- A checksum that does not depend on the order agents are stored in, so storage models that iterate in different orders can be compared on correctness, not only speed.
- A component add/remove workload (agents gain and lose their seeker part) next to plain iteration and entity churn (death and respawn).
- A host that drives the simulation from a `winit` event loop with `SystemClock` and `FixedTimestep`, and passes input to it as plain data.
- A software renderer that is testable without a display.
- The M3 chunk table in `docs/roadmap.md`, M3 marked `Current`, and the M7 open question on events between systems.

**Non-goals**

- No ECS, no storage abstraction and no benchmark harness. M3.2 builds the harness and adds a struct-of-arrays variant of this workload; this chunk stays array-of-structs.
- No benchmarks. The window title shows an informal step time for watching, which is not a measurement (§4.6).
- No engine-side window, input, camera or rendering types. `winit` and `softbuffer` are dependencies of this example only (R2).
- No interpolation between steps when drawing (`FixedTimestep::alpha`), no frame-time smoothing, no GPU rendering, no vsync control, no HiDPI handling beyond drawing in physical pixels.
- No sprite rotation: agents are drawn as squares, and `Transform2d::rotation` and `scale` stay at their identity values (R5).
- No public library API change, no frozen format, no new ADR.

## 3. Guide-level explanation

To watch it:

```sh
cargo run -p agents_2d --release
cargo run -p agents_2d --release -- --agents 50000 --seed 7
```

A window opens showing a 320 by 180 meter world, letterboxed to the window. Blue squares are wanderers; orange squares are seekers, which steer toward the cursor. A seeker that reaches the cursor becomes a wanderer again; a wanderer that bounces off the edge of the world becomes a seeker. Agents live 1 to 10 seconds and are replaced as they die, so the population stays constant. Space pauses, Escape quits. The title shows the population, the number of seekers and the average time of one simulation step.

To run it without a window (CI, scripts, comparison with a prototype):

```sh
cargo run -p agents_2d --release -- --headless --ticks 600
# tick=600 agents=10000 seekers=… checksum=0x…
```

Headless runs replace the cursor with a scripted one (§4.4), so seekers still gain and lose their part.

From code, the library is a plain state machine that the host steps:

```rust
use agents_2d::{Input, Sim, SimConfig};

let mut sim = Sim::new(&SimConfig { agents: 1_000, ..SimConfig::default() });
for _ in 0..120 {
    let input = Input::scripted(sim.tick());
    sim.step(&input);
}
let summary = sim.summary();
assert_eq!(summary.agents, 1_000);
assert_eq!(summary.tick, 120);
```

A storage prototype in M3.3 or later reproduces the rules in §4.3 on its own storage, steps it with the same `Input` sequence, and compares its checksum with `Sim::checksum`. Equal checksums mean the prototype ran the same simulation.

## 4. Reference-level design

### 4.1 Layout

```
examples/agents_2d/
  Cargo.toml            # depends on engine, winit, softbuffer; dev-depends on dhat, proptest
  src/lib.rs            # Sim, Agent, Seeker, Input, SimConfig, SimSummary, run
  src/render.rs         # View, Frame, render
  src/main.rs           # winit host and command line
  tests/determinism.rs  # golden checksum, seeds, clone mid-run
  tests/allocations.rs  # no steady-state allocation (dhat), its own binary
```

The crate follows `examples/fixed_step_sim`: the library carries the behaviour and the tests, the binary is a thin host. Both crate roots carry `#![forbid(unsafe_code)]`. The binary allows `clippy::print_stdout` and `clippy::print_stderr`, as `fixed_step_sim` does.

### 4.2 Data

```rust
pub struct SimConfig { pub seed: u64, pub agents: usize }        // default: seed 42, 10,000 agents

pub struct Seeker { pub acceleration: f32 }                       // m/s², toward the cursor

pub struct Agent {
    pub id: u64,                 // spawn counter, unique for the life of a Sim
    pub transform: Transform2d,  // translation is the position, in meters
    pub velocity: Vec2,          // m/s
    pub ttl: u32,                // ticks left to live
    pub seeker: Option<Seeker>,
}

pub struct Input { pub cursor: Option<Vec2> }                    // world space; None: no cursor in the world

pub struct SimSummary { pub tick: u64, pub agents: usize, pub seekers: usize, pub checksum: u64 }
```

`Sim` holds `agents: Vec<Agent>`, `prng: Prng`, `next_id: u64`, `tick: u64`, `target: usize` and `dt: f32`. It derives `Clone` and `Debug`; cloning it snapshots the simulation. `Sim` exposes `new`, `step`, `tick`, `agents` (a shared slice), `checksum` and `summary`; nothing outside the crate can reorder or edit its agents.

`Transform2d` is used, rather than a bare `Vec2`, so that the M3 transform-layout decision is measured with the type a 2D game would store.

### 4.3 Simulation rules

These rules define the checksum. A storage prototype that claims to run this workload MUST implement them exactly, including the order of floating-point operations.

**Constants.** World width `W = 320.0` and height `H = 180.0` (meters, +X right, +Y up, origin bottom-left; [ADR 0005](../../../adr/0005-coordinate-system-and-units.md)). Tick rate 60 Hz, with `dt = FixedTimestep::from_hz(60).step().as_secs_f32()`. Maximum seeker speed `MAX_SPEED = 30.0` m/s. Catch radius `CATCH = 4.0` m. Acceleration of a seeker gained by bouncing `GAINED_ACCELERATION = 60.0` m/s².

**Spawning.** `spawn` takes `id = next_id`, increments `next_id`, creates `r = prng.stream(id)`, and draws, in this order:

1. `x = r.range(0.0_f32..W)`, then `y = r.range(0.0_f32..H)`;
2. `vx = r.range(-20.0_f32..20.0)`, then `vy = r.range(-20.0_f32..20.0)`;
3. `ttl = r.range(60_u32..=600)`;
4. `is_seeker = r.chance(0.25)`;
5. if `is_seeker`, `acceleration = r.range(40.0_f32..80.0)`.

The agent's transform is `Transform2d::IDENTITY` with translation `(x, y)`. `Sim::new` spawns `config.agents` agents, so their ids are `0..agents`. `prng` is `Prng::from_seed(config.seed)` and is never advanced; it is only used to derive streams. An agent's values therefore depend on its id alone, never on where it is stored.

**Step.** `step(&input)` applies, to every agent, in any order (the rules for one agent never read another agent):

1. **Steer or catch.** If the agent has a seeker part and `input.cursor` is `Some(c)`: let `d = c - p` (where `p` is the translation) and `len2 = d.x * d.x + d.y * d.y`. If `len2 <= CATCH * CATCH`, remove the seeker part and skip the rest of this rule. Otherwise let `len = len2.sqrt()` and `k = acceleration * dt`; set `v.x = v.x + (d.x / len) * k` and `v.y = v.y + (d.y / len) * k`. Then let `s2 = v.x * v.x + v.y * v.y`; if `s2 > MAX_SPEED * MAX_SPEED`, let `f = MAX_SPEED / s2.sqrt()` and set `v.x = v.x * f`, `v.y = v.y * f`.
2. **Move.** `p.x = p.x + v.x * dt`, then `p.y = p.y + v.y * dt`.
3. **Bounce.** For each axis, with bound `B` (`W` for x, `H` for y): if `p < 0.0`, set `p = 0.0` and `v = v.abs()`; else if `p > B`, set `p = B` and `v = -(v.abs())`. If either axis bounced and the agent has no seeker part, give it `Seeker { acceleration: GAINED_ACCELERATION }`. (An agent caught in rule 1 and bounced in rule 3 in the same tick ends the tick as a seeker.)
4. **Age.** `ttl = ttl - 1`.

Then, after every agent has been updated: remove every agent whose `ttl` is 0, spawn agents until there are `target` again, and increment `tick`. The order of agents in storage after a step is unspecified. Agents spawned during a step are not updated in that step.

`sqrt` is correctly rounded in IEEE 754, so it is deterministic and not on the transcendental ban list. The rules use no other function.

**Checksum.** For each agent, a 64-bit FNV-1a hash (offset basis `0xcbf2_9ce4_8422_2325`, prime `0x0000_0100_0000_01b3`) over, in order: `id` as 8 little-endian bytes; `translation.x`, `translation.y`, `velocity.x`, `velocity.y` as the little-endian bytes of `f32::to_bits`; `ttl` as 4 little-endian bytes; then one byte `0` for a wanderer, or one byte `1` followed by the little-endian bytes of `acceleration.to_bits()` for a seeker. The simulation's checksum is the wrapping sum of the per-agent hashes. Addition is commutative, so the checksum does not depend on storage order. `rotation` and `scale` are not hashed: no rule changes them.

### 4.4 Scripted input

`Input::scripted(tick)` gives every headless run, test and later benchmark the same cursor:

- If `(tick / 600) % 3 == 2`, the cursor is `None` (one period in three exercises the no-cursor branch).
- Otherwise, with `phase = tick % 600`, `f = phase as f32 / 300.0` when `phase < 300`, else `(600 - phase) as f32 / 300.0`; the cursor is `Some(Vec2::new(W * f, H * f))`. It moves along the world's diagonal and back.

`run(&config, ticks)` creates a `Sim` and calls `step(&Input::scripted(sim.tick()))` `ticks` times, then returns the summary. The headless binary is a wrapper around `run`.

### 4.5 Renderer

The renderer lives in `src/render.rs` and draws in `softbuffer`'s pixel format, `0x00RRGGBB` per `u32`.

- **`Frame<'a>`** wraps `&'a mut [u32]` with a width and a height. `Frame::new(pixels, width, height)` returns `None` unless `width * height == pixels.len()` (computed without overflow), so drawing never needs a bounds panic.
- **`View::fit(width, height)`** fits the world into the frame: `scale = min(width / W, height / H)` pixels per meter, with the world centered (`offset_x = (width - W * scale) / 2`, `offset_y` likewise), so the unused margin forms letterbox bars.
  - `world_to_screen(p) = (offset_x + p.x * scale, offset_y + (H - p.y) * scale)`. The flip of Y is the world-to-screen conversion of ADR 0005.
  - `screen_to_world(s)` is its inverse.
  - `cursor_to_world(s)` is `screen_to_world(s)` when the result lies inside `[0, W] × [0, H]`, else `None`. The host builds `Input` with it, so drawing and input use one conversion.
  - A zero width or height gives a view that draws nothing and maps every cursor to `None`.
- **`render(&sim, &view, &mut frame)`** fills the frame with the background colour, draws a one-pixel border around the world's screen rectangle, then draws every agent as a filled square of side `side = max(1, floor(scale))` pixels whose top-left corner is `(floor(sx) - side / 2, floor(sy) - side / 2)` (integer division), clipped to the frame. Wanderers are `0x6EC6FF`, seekers `0xFFB347`, the background `0x101418` and the border `0x3A4048`.

The renderer allocates nothing and reads the simulation only through `Sim::agents`.

### 4.6 Host

`src/main.rs` implements `winit::application::ApplicationHandler` (`winit` 0.30).

- **`resumed`:** create the window (title `agents_2d`), then the `softbuffer` context and surface, once. `winit` requires windows to be created here, not before the loop starts.
- **`window_event`:**
  - `CursorMoved` stores the physical cursor position; `CursorLeft` clears it.
  - `Resized` resizes the surface; a zero dimension skips drawing until the next non-zero size.
  - Space (on press) toggles pause; Escape or `CloseRequested` exits the loop.
  - `RedrawRequested` runs one frame (below).
- **A frame:**
  1. Read `SystemClock` and compute the time since the previous frame.
  2. If paused, discard it; otherwise pass it to `FixedTimestep::accumulate` (60 Hz, the default cap of 8 steps per update) and call `sim.step(&input)` once per returned step. `input.cursor` comes from `view.cursor_to_world` on the stored cursor position.
  3. Render into the surface's buffer, present it, and call `request_redraw`.
- **Title:** about once a second, the title is set to `agents_2d: <agents> agents, <seekers> seekers, step <mean> ms`, where `<mean>` is the mean wall time of the steps run since the last update, measured with `SystemClock` around `step`. This is a convenience for watching, not a benchmark.
- **Command line:** `--agents N` (default 10,000), `--seed N` (default 42), and `--headless [--ticks N]` (default 600 ticks), which runs `run` and prints `tick=<tick> agents=<agents> seekers=<seekers> checksum=<checksum as 0x and 16 hex digits>` without creating a window. An unknown flag or a bad number prints an error naming it and exits with a failure code.

The simulation never sees a `winit` or `softbuffer` type: the host turns events into `Input` and owns time, which is what ADR 0007 requires. The host owns the main thread because `winit` needs it on macOS, and the sample is the host.

### 4.7 Dependencies

- `winit = "0.30"` (0.30.13 at the time of writing, Apache-2.0) and `softbuffer = "0.4"` (0.4.8, MIT OR Apache-2.0), both declared in `examples/agents_2d/Cargo.toml` only. `winit` 0.31 is in beta and is not used.
- Both use `raw-window-handle` 0.6. On Linux both load X11 and Wayland at runtime by default, so building them should need no system packages; §8 checks this on CI rather than assuming it.
- The whole new dependency tree MUST pass `cargo xtask deny` with the current allow list. If a transitive crate's licence is outside it, the first remedy is to turn off the default feature that pulls it in (for example `winit`'s `wayland-csd-adwaita`, which draws window decorations with font crates). Changing the allow list is a policy change ([ADR 0002](../../../adr/0002-originality-and-dependency-licensing.md)) and stops the chunk for the maintainer.

### 4.8 Roadmap

`docs/roadmap.md` MUST:

- mark M3 `Current` and M4 `Next`;
- replace the M3 entry in "Later milestones" with a chunk table:

| Chunk | Title | Depends on |
|---|---|---|
| M3.0 | Windowed baseline sample: `examples/agents_2d` (`winit` and `softbuffer`, example-only), the agent workload and its order-independent checksum | none |
| M3.1 | `engine_collections`: deterministic map and set | none |
| M3.2 | Spike harness under `spikes/` (prototypes are kept there after the decision): one minimal storage interface, the workloads (simple and fragmented iteration, add/remove, heavy compute, entity churn, 100k 2D sprites, and the M3.0 agent workload), a struct-of-arrays variant of the agent workload, and bevy_ecs, hecs and plain-`Vec` baselines | M3.0, M3.1 |
| M3.3 | Sparse-set prototype (safe) | M3.2 |
| M3.4 | Archetype prototype (safe) | M3.2 |
| M3.5 | Per-component hybrid prototype (safe) | M3.3, M3.4 |
| M3.6 | Runtime-registered components in safe byte columns, in the leading prototype | M3.3, M3.4, M3.5 |
| M3.7 | `unsafe` type-erased variant of the leading prototype: an ADR for a spike-scoped exception, an `unsafe-allowlist.toml` entry and Miri in CI; measures the safe and `unsafe` gap | M3.3, M3.4, M3.5 |
| M3.8 | Decision ADRs (storage model, component identity, foreign and mod-defined component storage, transform layout) and the `benchmarks.md` section | M3.0 to M3.7 |

- record that the prototypes are kept under `spikes/`, which settles the M3 open question;
- add to M7's open questions: "How systems send events to each other: typed per-tick queues or short-lived entities";
- say that Miri arrives with M3.7, that `engine_ffi` (M8) is the first allowlisted `unsafe` crate outside `spikes/`, and that M4's dogfooding sample ports `agents_2d` to the ECS.

The M2 chunk table stays as a record.

## 5. Requirements

- **M3.0-R1** `examples/agents_2d` MUST be a workspace member with a library (`src/lib.rs`, `src/render.rs`) and a binary (`src/main.rs`), each with `#![forbid(unsafe_code)]`.
- **M3.0-R2** `winit` and `softbuffer` MUST be dependencies of `examples/agents_2d` only. No package under `libs/` may depend on them, directly or transitively.
- **M3.0-R3** `Sim::step` MUST implement the rules of §4.3 exactly, including spawn draw order, rule order within an agent, and floating-point operation order.
- **M3.0-R4** `Sim::checksum` MUST be the order-independent checksum of §4.3: permuting the agents in storage MUST NOT change it, before or after further steps.
- **M3.0-R5** No rule may change an agent's `rotation` or `scale` from their `Transform2d::IDENTITY` values.
- **M3.0-R6** The simulation MUST be deterministic: the same `SimConfig` and `Input` sequence MUST give the same checksum, and a `Sim` cloned mid-run MUST continue identically to the original.
- **M3.0-R7** The checksum of `run(&SimConfig::default(), 600)` MUST equal a golden constant in `tests/determinism.rs`, on every platform CI tests.
- **M3.0-R8** `Input::scripted` MUST follow §4.4, and headless runs MUST use it.
- **M3.0-R9** After warm-up, `Sim::step` and `render` MUST NOT allocate.
- **M3.0-R10** `Frame::new` MUST return `None` for a buffer whose length is not `width * height`; `render` MUST NOT panic for any `Frame` and `View`, including zero-sized ones.
- **M3.0-R11** `View::screen_to_world(View::world_to_screen(p))` MUST be within half a pixel (`0.5 / scale` meters) of `p` for every `p` in the world, and every agent square MUST be clipped to the frame.
- **M3.0-R12** The pixels produced by `render` for a fixed scene MUST match a golden hash.
- **M3.0-R13** The binary MUST drive the simulation through `FixedTimestep` fed by `SystemClock`, pass input only as `Input`, and expose the command line of §4.6, including `--headless`, which MUST print the summary line of §4.6 and create no window.
- **M3.0-R14** The new dependency tree MUST pass `cargo xtask deny` without changes to `deny.toml`.
- **M3.0-R15** `cargo xtask ci` MUST pass, and the CI matrix (Ubuntu, macOS, Windows) MUST be green without new system packages or workflow changes. If CI needs either, the pull request MUST say so and the spec's Deviations MUST record it.
- **M3.0-R16** `docs/roadmap.md` MUST contain the changes in §4.8; `README.md` MUST list the example; `CHANGELOG.md` MUST record it.
- **M3.0-R17** Every public item in the example's library MUST be documented, and every public type MUST have a runnable doctest.

## 6. Cross-cutting concerns

- **Determinism:** the simulation uses only `+`, `-`, `*`, `/`, `abs`, comparison and `sqrt` on `f32`, and glam's `Vec2`, which is not SIMD-backed, with the operation order written down (§4.3). Per-agent randomness comes from `prng.stream(id)`, so no result depends on storage order or thread timing. The golden checksum is expected to match on aarch64 and x86_64, as `fixed_step_sim`'s does; if it does not, that is a finding about same-build determinism and is reported, not worked around (R7). Wall time from `SystemClock` reaches only the host, never the simulation.
- **`unsafe`:** none written. `winit` and `softbuffer` use `unsafe` internally; they are example-only dependencies, outside `unsafe-allowlist.toml`'s scope, which covers workspace crates.
- **Concurrency:** n/a, because the sample is single-threaded.
- **Heap allocation:** `Sim::new` reserves `target` agents; stepping and rendering allocate nothing afterwards (R9, checked with `dhat` as in `fixed_step_sim`). The host's `softbuffer` buffers may allocate on resize, which is outside the simulation.
- **`no_std` and wasm:** n/a, because examples are not part of the `no-std` or `wasm` builds (`cargo xtask no-std` and `wasm` name their packages). `winit` would need a web backend for wasm, which is out of scope.
- **Frozen formats:** none. The golden checksum and pixel hash pin this sample's behaviour; they are tests, not formats.
- **Public API changes:** none in `libs/`. The example's library is public only to its own binary, its tests, and the M3 spike crates.
- **FFI exposure:** n/a.
- **Performance:** not measured in this chunk. M3.2 measures this workload in the harness. The title's step time is informal.
- **Documentation:** module and item docs with doctests (R17), this spec and its plan, the roadmap changes (§4.8), a README line and a CHANGELOG entry.
- **Policy changes:** none expected. A licence outside the allow list would be one, and stops the chunk (§4.7).

## 7. Alternatives considered

- **`pixels` instead of `softbuffer`.** Rejected: it scales a fixed framebuffer on the GPU through `wgpu`, which brings a large dependency tree into an example whose point is to show the simulation, and the scaling it does is instructive to write by hand.
- **`minifb` instead of `winit`.** Rejected: smallest code, but it brings its own windowing, so it would not test ADR 0007 against the `winit` event loop the engine plans to use.
- **`winit` 0.31 beta.** Rejected: its API is still changing; the example moves to 0.31 in a later chunk once it is released.
- **Boids-style flocking.** Rejected: neighbour search through a spatial grid would dominate the cost, so the workload would measure the grid more than the storage.
- **All agents identical, no optional part.** Rejected: it would not exercise fragmented iteration or component add and remove, which is where storage models differ most.
- **An order-dependent checksum (a hash over storage order, as in `fixed_step_sim`).** Rejected: a sparse set, an archetype table and this `Vec` iterate in different orders, so the prototypes could not be checked against the sample.
- **Sorting agents by id before hashing.** Rejected: also order-independent, but it allocates or reorders storage on every checksum; the wrapping sum needs neither.
- **One `Prng` advanced in storage order for spawns.** Rejected: spawn values would depend on removal order. A stream per id does not.
- **Storing a bare `Vec2` position.** Rejected: the transform-layout ADR in M3.8 needs the workload to carry the real `Transform2d`.
- **Struct-of-arrays in this chunk.** Rejected for now: array-of-structs is the more natural first version; M3.2 adds the struct-of-arrays variant next to it so both baselines exist before any prototype.
- **Pause and input inside the simulation.** Rejected: pause is the host not calling `step`, and ADR 0007 keeps such control with the host.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | `Cargo.toml` workspace members (`examples/*`); `cargo xtask check-unsafe` |
| R2 | `cargo tree -p engine -i winit` and `cargo tree -p engine -i softbuffer` both report the package is not in the tree; review of `Cargo.toml` files |
| R3 | unit tests in `src/lib.rs`, one per rule: steering moves a seeker toward the cursor and clamps speed to `MAX_SPEED`; within `CATCH` the part is removed and velocity is unchanged; each edge clamps position and reflects the right velocity component; a bouncing wanderer gains `Seeker { acceleration: 60.0 }`; expired agents are replaced and ids keep increasing; a spawned agent's values equal direct draws from `prng.stream(id)` |
| R4 | unit test: run two clones, reverse one's `agents` vector (from inside the crate) mid-run, continue both, compare checksums after every further step |
| R5 | unit test: after a run, every agent's `rotation` is 0 and `scale` is `Vec2::ONE` |
| R6 | `tests/determinism.rs`: same config same summary; a different seed gives a different checksum; a clone taken at tick 300 and run to 600 matches the original |
| R7 | `tests/determinism.rs` golden constant, run by CI on Ubuntu (x86_64), macOS (aarch64) and Windows (x86_64) |
| R8 | unit tests for `Input::scripted` at ticks 0, 150, 300, 450, 599, 1200 and 1799 |
| R9 | `tests/allocations.rs` (`dhat`): after 120 warm-up ticks, 600 more ticks with a render after each allocate zero blocks |
| R10 | unit tests for `Frame::new` with short, long and overflowing sizes; proptest rendering random small scenes into random sizes including 0 |
| R11 | proptest of the round trip over random points and window sizes; proptest that no drawn pixel falls outside the frame (by rendering into a frame with a sentinel border) |
| R12 | unit test hashing the pixels of `render` for `SimConfig { agents: 200, ..default }` after 60 scripted ticks into a 320 by 180 frame, against a golden constant |
| R13 | unit tests of command-line parsing; manual run on macOS (screenshot in the pull request); `cargo run -p agents_2d --release -- --headless` output pasted in the pull request. Not run by hand on Linux with a display or on Windows, which the pull request states |
| R14 | `cargo xtask deny` |
| R15 | `cargo xtask ci` locally; the pull request's CI run |
| R16, R17 | documentation review (CONTRIBUTING.md "Reviews before a pull request"); `cargo xtask doc` |

## 9. Done when

- [ ] `cargo xtask ci` passes on macOS, and CI is green on all three platforms.
- [ ] `cargo run -p agents_2d --release` opens a window on macOS where seekers chase the cursor, pause works and the title updates.
- [ ] `--headless` prints the golden checksum.
- [ ] The roadmap shows M3 `Current` with its chunk table, and M7's new open question.
- [ ] The eight reviews in CONTRIBUTING.md are recorded in the pull request.
- [ ] The spec is `Implemented` and the plan `Historical`, with Deviations filled in.

## 10. Open questions

None.

## 11. Prior art and sources

- Craig Reynolds, [Steering Behaviors For Autonomous Characters](https://www.red3d.com/cwr/steer/gdc99/) (GDC 1999): the "seek" behaviour, of which rule 1 is a speed-clamped variant.
- Glenn Fiedler, [Fix Your Timestep!](https://gafferongames.com/post/fix_your_timestep/): the accumulator loop that `FixedTimestep` already implements.
- [`winit` 0.30 documentation](https://docs.rs/winit/0.30.13/winit/), in particular `ApplicationHandler` and the rule that windows are created in `resumed`.
- [`softbuffer` documentation](https://docs.rs/softbuffer/0.4.8/softbuffer/): the `0x00RRGGBB` pixel format and the `Context` / `Surface` / `present` cycle.
- Fowler, Noll and Vo, [FNV hash](http://www.isthe.com/chongo/tech/comp/fnv/): FNV-1a 64-bit, as used by `fixed_step_sim`.
- Michele Caini (skypjack), [ECS back and forth](https://skypjack.github.io/2019-02-14-ecs-baf-part-1/): why component add and remove is the operation that separates sparse-set and archetype storage, which motivates the seeker part.
