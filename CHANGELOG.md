# Changelog

All notable changes are recorded here, in the [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) style. The crates are `0.x` and `publish = false`, so any release may break compatibility; breaking changes are marked **breaking**. The PRNG output streams and `Handle::to_bits` are frozen for all of `0.x` and will not change.

## [Unreleased]

### Added

- **Workspace and policy (M0):** pinned toolchain, lint policy, determinism bans in `clippy.toml`, `cargo-deny`, dual MIT/Apache licensing, ADRs 0001-0011, and `cargo xtask` (fmt, clippy, test, doc, deny, no-std, wasm, check-unsafe, check-statics, check-workflows, bench, bench-smoke, bench-report, prng-stats, ci). GitHub Actions are pinned to commit hashes and use a read-only token.
- **`engine_handle`:** typed generational `Handle<T>`; dense `HandlePool<T>` with 8-byte slots; the lean `HandleAllocator<T>` and the iterable `DenseHandleAllocator<T>`; `reserve`/`try_reserve`, `with_fixed_capacity`, `insert_with`, `allocate_many`; `From`/`TryFrom<u64>` for `Handle`.
- **`engine_random`:** `Prng` (xoshiro256++ seeded by SplitMix64) with normative sampling algorithms and frozen output streams.
- **`engine_time`:** `Tick`, `Timestamp`, `TimeSource`, `ManualClock`, `SystemClock`, `FixedTimestep`.
- **`engine_math`:** glam re-exports, a transcendental funnel (`scalar`), `Transform`, large-world `WorldTransform`, 2D variants, coordinate conversion (`ENGINE`, `GLTF`, `UNITY` presets).
- **`engine`:** facade crate and prelude. **`examples/fixed_step_sim`:** a deterministic headless simulation with a golden checksum that is identical on aarch64 and x86_64.
- Comparison benchmarks against `slotmap`, `thunderdome`, `rand`, `rand_xoshiro` and `fastrand`, and `cargo xtask bench-report` for HTML reports of saved results.
