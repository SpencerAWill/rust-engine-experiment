# rust-engine-experiment

A high-performance, real-time game and simulation engine in Rust, built as a set of small libraries plus samples. It follows data-oriented design, will be ECS-based, and will expose a stable C ABI so games and apps can be written in other languages (C# first). The first workload is a headless, deterministic simulation core.

## Status

Experimental. All crates are `0.x`, `publish = false`, and their APIs are explicitly unstable. Current scope is milestones M0 (workspace and policy) and M1 (foundation crates). The design lives in `docs/superpowers/specs/`, and decisions are recorded as ADRs in `docs/adr/`.

## Crate map

| Crate | Purpose | Why it is a separate crate |
|---|---|---|
| `engine_handle` | Generational handles, `HandlePool<T>`, `HandleAllocator<T>` | Platform (`no_std` + `alloc`), dependency-free; FFI exposure; layering |
| `engine_random` | Deterministic PRNG with frozen output streams | Stability; platform (`no_std`, no `alloc`); independent reuse |
| `engine_time` | `Tick`, `FixedTimestep`, `TimeSource`, clocks | Platform (`no_std` core, `std` feature); independent reuse; layering |
| `engine_math` | glam re-exports, `Transform` (local, large-world, 2D), coordinate conversion | Dependencies (`glam`); platform (`std`) |
| `engine` | Facade and `engine::prelude` | The single dependency for users |

Also in the workspace: `examples/fixed_step_sim` (a deterministic headless simulation that uses all
of the above), `benches/` (`engine_benches`: comparisons against `slotmap`, `thunderdome`, `rand`,
`rand_xoshiro`, and `fastrand`), and `xtask` (the task runner). Rationale for the splitting rule is
in [ADR 0004](docs/adr/0004-crate-splitting-rule.md).

Benchmark methodology and results are in [docs/benchmarks.md](docs/benchmarks.md).

## Quick start

```sh
cargo xtask ci          # everything CI runs, in order
cargo xtask ci --quick  # fmt check, clippy, tests only
```

`cargo xtask` is the single entry point for both CI and local runs. Other commands: `bench [--save NAME] [--compare A B]`, `prng-stats` (streams PRNG output for PractRand), `no-std`, `wasm`, `check-unsafe`, `check-statics`. It prints an install hint when a required tool is missing (`cargo-nextest`, `cargo-deny`, `critcmp`, `wasmtime`).

## Using the engine in your project

A library cannot set its users' build profiles, so add this to your game's `Cargo.toml`. Unoptimized dev builds of game code are often unusably slow:

```toml
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3
```

glam's `fast-math` and `scalar-math` features are unsupported. Any crate in your dependency graph can enable them through Cargo feature unification. `scalar-math` changes the size and alignment of the math types, which the engine's layout assertions catch. `fast-math` silently changes numerics and cannot be detected (see [ADR 0003](docs/adr/0003-same-build-determinism.md)).

## Platform support

- Native: macOS, Linux, and Windows.
- `engine_handle`, `engine_random`, and `engine_time` build for `no_std` targets (checked on `thumbv7em-none-eabihf`).
- `engine` builds for `wasm32-unknown-unknown`, and the leaf crates' and `engine_math`'s tests run on `wasm32-wasip1` under `wasmtime`. That is a 32-bit target, so 32-bit `usize` is covered.
- WASM test notes:
  - proptest is used with `default-features = false`, because its default `fork` and `timeout` features do not compile for WASI.
  - libtest ignores `#[should_panic]` tests on wasm targets other than emscripten, and panics abort on WASI. The documented `# Panics` contracts are therefore tested on native targets only, and proptest failures on WASI abort without shrinking. Reproduce on a native target to shrink.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
