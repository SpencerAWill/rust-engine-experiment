# bench/

Status: Living (2026-11-15). The benchmark packages: wall-clock comparisons and the instruction-count gate.

## When this applies

- You make, check or quote a performance claim (wall-clock numbers, `cargo xtask bench`).
- You add or change a benchmark, or the `bench-regress` check fails on your pull request.

## Contents

- `compare/`: package `engine_benches`, Criterion wall-clock benchmarks of the engine against `slotmap`, `thunderdome`, `rand`, `rand_xoshiro`, `fastrand`, `fixedbitset`, `hibitset` and `hi_sparse_bitset`. Results and methodology are in [docs/benchmarks.md](../docs/benchmarks.md). Its example `space_report` prints the heap bytes of the bitsets under `dhat`, one process per case (`cargo run -p engine_benches --release --example space_report`); the peak column misses the moment a `realloc` holds both buffers, because `dhat` 0.3's `realloc` records only the change of size (read in its source, not measured).
- `regress/`: package `engine_bench_regress`, the Gungraun benchmarks that the instruction-count gate counts.

## Wall-clock benchmarks

Performance claims need measurements. Wall-clock numbers from a laptop or shared CI runner are noisy (we have measured about 5% drift between runs on a quiet machine), so:

- Run on AC power with other apps closed, and record the machine and conditions. On macOS `cargo xtask bench` refuses to start on battery and keeps the machine awake; afterwards, check `pmset -g log` for a power change or sleep during the run. In Claude Code, `/bench-run` does this.
- Compare against a saved baseline: `cargo xtask bench --save NAME`, then `cargo xtask bench-report --baseline NEW --against OLD`. The report only calls a change real if it exceeds 5% and the confidence intervals do not overlap.
- Say what you measured and what you did not.

## Instruction-count gate

Wall-clock benchmarks are too noisy to gate a pull request, so the `bench-regress` check counts CPU instructions instead, under Valgrind through [Gungraun](https://docs.rs/crate/gungraun/). It builds and runs the benchmarks of `regress/` at the base commit and at the head, in the same job, and fails when any benchmark's instruction count rose by more than 5%. Nothing is stored between runs.

```sh
cargo xtask bench-regress                 # compare HEAD (and your working tree) with the merge base with origin/main
cargo xtask bench-regress --base HEAD~3   # compare with another commit
cargo xtask bench-regress --report-only   # report a regression without failing
```

- **Linux only.** Valgrind does not run on macOS, so on a Mac use the devcontainer. The command stops before building when the platform, Valgrind (3.20 or later) or the `gungraun-runner` version is wrong, and says what to install.
- **When it runs.** It skips itself, and passes, when no file that can change a count differs from the base (`libs/`, `bench/regress/`, the manifests, the toolchain, `.cargo/`, and the gate's own code and workflow; Markdown files never count). It compares a pull request with its parent only, so several small regressions across pull requests can add up; the milestone check in [docs/maintainers.md](../docs/maintainers.md) is the backstop.
- **The label.** An intended regression needs the `perf-regression-accepted` label and a justification in the pull request description. With the label the check reports the regression and passes; a build failure or a Valgrind error still fails it. The label is not tied to a commit and covers later pushes too, so the description must name the benchmarks it covers, and the review compares that list with the "Accepted by label" list of the final run ([Reviews before a pull request](../CONTRIBUTING.md#reviews-before-a-pull-request)). A toolchain bump often needs the label, because the compiler changed the code it generated.
- **A `gungraun` bump is not compared.** The runner installed for the head cannot run a benchmark binary built against another `gungraun` version, so a pull request that changes it (normally a Dependabot bump) passes without a comparison.
- **Architectures.** Counts from the aarch64 devcontainer are only comparable with other aarch64 runs; CI runs on x86_64. Never compare across the two.
- **Adding a benchmark.** Add a function to `regress/benches/genid.rs` or `prng.rs`, or a new `[[bench]]` file with `harness = false`. Build the input in a `setup` function (it is not counted) from a fixed seed, and put the operation in a separate `#[inline(never)]` function that the benchmark calls (the comment at the top of `genid.rs` says why). A benchmark must measure at least 10,000 instructions: with fewer, a harmless change in code generation exceeds 5%. `cargo xtask bench-smoke` runs each benchmark function once without Valgrind, so a broken benchmark fails ordinary CI.
- **Moving the package.** The base run looks for `engine_bench_regress` at `bench/regress/` and then at its earlier locations, `LEGACY_REGRESS_DIRS` in `xtask/src/bench_regress.rs`. If you move it, add the old directory there, or every comparison with an earlier commit (the milestone check included) reports every benchmark as new.
- **Renaming.** When you change what an existing benchmark measures, rename it. The base runs the base's copy of the benchmark, so an edited benchmark with the same name compares two different things.
