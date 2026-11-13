# rust-engine-experiment

A high-performance, real-time game and simulation engine in Rust, built as a set of small libraries plus samples. It follows data-oriented design, will be ECS-based, and will expose a stable C ABI so games and apps can be written in other languages (C# first). The first workload is a headless, deterministic simulation core.

## Status

Experimental. All crates are `0.x`, `publish = false`, and their APIs are explicitly unstable. M0+M1 (workspace, policy and the foundation crates) is done; what comes next is in [docs/roadmap.md](docs/roadmap.md). The design lives in `docs/design/`, and decisions are recorded as ADRs in `docs/adr/`.

## Crate map

| Crate | Purpose | Why it is a separate crate |
|---|---|---|
| `engine_handle` | Generational handles, `HandlePool<T>`, `HandleAllocator<T>` (lean), `DenseHandleAllocator<T>` (iterable) | Platform (`no_std` + `alloc`), dependency-free; FFI exposure; layering |
| `engine_collections` | Deterministic insertion-ordered `IndexMap<K, V>` and `IndexSet<T>` | Dependencies (`indexmap`, `foldhash`); platform (`no_std` + `alloc`); layering |
| `engine_random` | Deterministic PRNG with frozen output streams | Stability; platform (`no_std`, no `alloc`); independent reuse |
| `engine_time` | `Tick`, `FixedTimestep`, `TimeSource`, clocks | Platform (`no_std` core, `std` feature); independent reuse; layering |
| `engine_math` | glam re-exports, `Transform` (local, large-world, 2D), coordinate conversion | Dependencies (`glam`); platform (`std`) |
| `engine` | Facade and `engine::prelude` | The single dependency for users |

Also in the workspace: `examples/fixed_step_sim` (a deterministic headless simulation that uses all
of the above), `examples/agents_2d` (thousands of 2D agents in a `winit` window, and the plain-`Vec` baseline for the M3 storage decision; `cargo run -p agents_2d --release`), `benches/` (`engine_benches`: comparisons against `slotmap`, `thunderdome`, `rand`,
`rand_xoshiro`, and `fastrand`), `bench-regress/` (`engine_bench_regress`: instruction-count benchmarks for the regression gate), and `xtask` (the task runner). Rationale for the splitting rule is
in [ADR 0004](docs/adr/0004-crate-splitting-rule.md).

Benchmark methodology and results are in [docs/benchmarks.md](docs/benchmarks.md).

## Quick start

```sh
cargo xtask ci          # every check CI gates on, in order (the coverage job, the nightly fuzz run and the weekly mutants run are report-only and run outside it)
cargo xtask ci --quick  # fmt check, clippy, tests only
```

`cargo xtask` is the single entry point for both CI and local runs. Other commands: `bench [--save NAME] [--compare A B] [--allow-battery]` (on macOS it refuses to start on battery power and keeps the machine awake with `caffeinate`), `bench-smoke` (runs every benchmark once in criterion's and Gungraun's test modes, measuring nothing, so a broken benchmark fails CI; needs `gungraun-runner`), `bench-regress [--base REV] [--report-only]` (the instruction-count gate: counts instructions under Valgrind at the base and at the head and fails above +5%; Linux only, so use the devcontainer on macOS; see CONTRIBUTING.md), `gungraun-version` (prints the `gungraun` version that `gungraun-runner` must match), `bench-report [--baseline NAME] [--against NAME] [--notes FILE] [--out FILE]` (turns saved criterion results into one self-contained HTML page with charts, and with `--against` a noise-aware before/after view; it writes `target/bench-report/index.html`), `prng-stats` (streams PRNG output for PractRand), `no-std`, `wasm`, `check-unsafe`, `check-statics`, `hack` (builds every crate with each feature alone and with none), `typos` (spell-check), `bench-doc` (regenerates the data sections of `docs/benchmarks.md` from a saved baseline), `check-workflows` (every GitHub Action pinned to a commit hash, read-only token permissions, no persisted checkout credentials, no `pull_request_target`), `fuzz <target|all> [--time SECS] [--replay PATH]` (coverage-guided fuzzing of `engine_handle` on a pinned nightly; see CONTRIBUTING.md), `fuzz-build` (compiles the fuzz targets), `fuzz-toolchain` (prints the pinned nightly) and `coverage [--open]` (line and function coverage of the library and example sources through `cargo-llvm-cov`, with a per-crate table and an HTML report in `target/llvm-cov/`; see CONTRIBUTING.md) and `mutants [-p PACKAGE]... [-j N] [--list] [--in-diff FILE]` (mutation testing of the triaged crates through `cargo-mutants`, with a per-crate table and the missed mutants; see CONTRIBUTING.md). It prints an install hint when a required tool is missing (`cargo-nextest`, `cargo-deny`, `critcmp`, `wasmtime`, `cargo-fuzz`, `cargo-llvm-cov`, `cargo-mutants`, `gungraun-runner`).

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

- **Versioning.** Every crate is `0.x` and `publish = false` for now. Any release may contain breaking changes; they are listed in [CHANGELOG.md](CHANGELOG.md) and marked **breaking**. Once published, the crates will follow semver as `0.x` does: a minor bump may break, a patch bump will not.
- **Frozen for all of `0.x`:** the `Prng` output streams and `Handle::to_bits` (`(index << 32) | generation`). They are covered by golden tests, and changing one needs an ADR.
- **Minimum Rust version.** `rust-version` is `1.100`, which is also the pinned toolchain (`rust-toolchain.toml`), so the MSRV is whatever CI builds with. It may be raised freely during `0.x` and the change is noted in the changelog.
- **Determinism** means the same build gives the same results on the same platform; it is not bit-exactness across platforms (see [ADR 0003](docs/adr/0003-same-build-determinism.md)).

## Platform support

- Native: macOS, Linux, and Windows.
- `engine_collections`, `engine_handle`, `engine_random`, and `engine_time` build for `no_std` targets (checked on `thumbv7em-none-eabihf`).
- `engine` builds for `wasm32-unknown-unknown`, and the leaf crates' (including `engine_collections`) and `engine_math`'s tests run on `wasm32-wasip1` under `wasmtime`. That is a 32-bit target, so 32-bit `usize` is covered.
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
