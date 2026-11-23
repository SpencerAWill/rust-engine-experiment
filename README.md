# rust-engine-experiment

A high-performance, real-time game and simulation engine in Rust, built as a set of small libraries plus samples. It follows data-oriented design, will be ECS-based, and will expose a stable C ABI so games and apps can be written in other languages (C# first). The first workload is a headless, deterministic simulation core.

## Status

Experimental. All packages are `0.x`, `publish = false`, and their APIs are explicitly unstable. M0+M1 (workspace, policy and the foundation modules) is done; the milestone goals are in [docs/roadmap.md](docs/roadmap.md), and the scheduled work is on GitHub (issues, milestones and the "Engine backlog" Project). The design lives in `docs/chunks/` and `docs/README.md`, and decisions are recorded as ADRs in `docs/adr/`.

## Module map

The library is one crate, `engine`, and these are its modules. A separate crate is made only when [ADR 0004](docs/adr/0004-crate-splitting-rule.md) says so.

| Module | Purpose |
|---|---|
| `engine::genid` | Generational IDs: `GenId<T>`, `UntypedGenId`, the `GenIdMap<T>` container (`GenIdMap<()>` to iterate live IDs) and the lean `GenIdAllocator<T>` |
| `engine::collections` | Deterministic insertion-ordered `IndexMap<K, V>` and `IndexSet<T>`, the bitsets `BitSet` and `BitArray<WORDS>`, and `HierarchicalBitSet` with `Join` for iterating and joining sparse index sets |
| `engine::random` | Deterministic PRNG with frozen output streams |
| `engine::time` | `Tick`, `FixedTimestep`, `TimeSource`, clocks |
| `engine::math` | glam re-exports, `Transform` (local, large-world, 2D), coordinate conversion |
| `engine::prelude` | The everyday types, for `use engine::prelude::*;` |

Also in the workspace: `examples/fixed_step_sim` (a deterministic headless simulation that uses all
of the above), `examples/agents_2d` (thousands of 2D agents in a `winit` window, and the plain-`Vec` baseline for the M3 storage decision; `cargo run -p agents_2d --release`), `spikes/` (measured experiments kept for re-running, one directory per topic; the first is `spikes/ecs_storage/`, the harness and baselines of the M3 storage decision), `bench/compare/` (`engine_benches`: comparisons against `slotmap`, `thunderdome`, `rand`,
`rand_xoshiro`, and `fastrand`), `bench/regress/` (`engine_bench_regress`: instruction-count benchmarks for the regression gate), and `xtask` (the task runner).

Benchmark methodology and results are in [docs/benchmarks.md](docs/benchmarks.md).

## Quick start

```sh
cargo xtask ci          # every check CI gates on, in order (the coverage job, the nightly fuzz run and the weekly mutants run are report-only and run outside it)
cargo xtask ci --quick  # fmt check, clippy, tests only
```

`cargo xtask` is the single entry point for both CI and local runs; every command is listed in [xtask/README.md](xtask/README.md), and each tool's manual is linked from [CONTRIBUTING.md](CONTRIBUTING.md#tools).

## Using the engine in your project

A library cannot set its users' build profiles, so add this to your game's `Cargo.toml`. Unoptimized dev builds of game code are often unusably slow:

```toml
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3
```

glam's `fast-math` and `scalar-math` features are unsupported. Any crate in your dependency graph can enable them through Cargo feature unification. `scalar-math` changes the size and alignment of the math types, which the engine's layout assertions catch. `fast-math` silently changes numerics and cannot be detected (see [ADR 0003](docs/adr/0003-same-build-determinism.md)).

## Stability and compatibility

- **Versioning.** Every package is `0.x` and `publish = false` for now. Any release may contain breaking changes; they are listed in [CHANGELOG.md](CHANGELOG.md) and marked **breaking**. Once published, the crate will follow semver as `0.x` does: a minor bump may break, a patch bump will not.
- **Frozen for all of `0.x`:** the `Prng` output streams and `UntypedGenId::to_bits` (`(index << 32) | generation`; `GenId::to_bits` forwards to it). They are covered by golden tests, and changing one needs an ADR.
- **Minimum Rust version.** `rust-version` is `1.100`, which is also the pinned toolchain (`rust-toolchain.toml`), so the MSRV is whatever CI builds with. It may be raised freely during `0.x` and the change is noted in the changelog.
- **Determinism** means the same build gives the same results on the same platform; it is not bit-exactness across platforms (see [ADR 0003](docs/adr/0003-same-build-determinism.md)).

## Platform support

- Native: macOS, Linux, and Windows.
- `engine` builds for `no_std` targets, every module included (checked on `thumbv7em-none-eabihf`). `time::SystemClock` needs the default `std` feature. Without `std`, transcendental math comes from `libm` instead of `std` ([ADR 0003](docs/adr/0003-same-build-determinism.md)).
- `engine` builds for `wasm32-unknown-unknown`, and its tests run on `wasm32-wasip1` under `wasmtime`. That is a 32-bit target, so 32-bit `usize` is covered.
- WASM test notes:
  - proptest is used with `default-features = false`, because its default `fork` and `timeout` features do not compile for WASI.
  - libtest ignores `#[should_panic]` tests on wasm targets other than emscripten, and panics abort on WASI. The documented `# Panics` contracts are therefore tested on native targets only, and proptest failures on WASI abort without shrinking. Reproduce on a native target to shrink.

## Contributing and security

See [CONTRIBUTING.md](CONTRIBUTING.md) for the rules CI enforces and how to set up, and
[SECURITY.md](SECURITY.md) for how to report a vulnerability privately.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
