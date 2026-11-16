# xtask/

Status: Living (2026-11-15). The task runner: `cargo xtask <command>` is the single entry point for CI and local runs.

## When this applies

- You run a check, a benchmark, coverage, fuzzing or mutation testing.
- You change a command, or a check that CI runs.

## Contents

- `Cargo.toml`: the `xtask` package (not published).
- `src/`: one module per command that needs more than a few lines (`check_docs`, `check_workflows`, `bench_regress`, `coverage`, `mutants`, `fuzz`, ...); `main.rs` parses the command line and holds the `ci` steps.

## Commands

Every command rejects flags it does not know, and prints an install hint when a tool it needs is missing. The tools are listed in [Setting up](../CONTRIBUTING.md#setting-up).

| Command | What it does |
|---|---|
| `ci [--quick]` | Every check CI gates on, in order. `--quick`: fmt check, clippy and tests only. Coverage, the nightly fuzz run and the weekly mutants run are report-only and outside it |
| `fmt [--check]`, `clippy`, `doc` | Formatting, clippy with warnings denied, docs without warnings |
| `test [--release] [--include-ignored]` | nextest plus doctests |
| `no-std`, `wasm` | `engine` without `std` (`thumbv7em-none-eabihf`), and for wasm (tests on `wasm32-wasip1` under `wasmtime`) |
| `hack` | Builds every package with each feature alone and with none |
| `deny` | `cargo deny`: licences, advisories, sources |
| `typos` | Spell-check |
| `check-unsafe`, `check-statics` | `#![forbid(unsafe_code)]` in every crate root; no global mutable state |
| `check-workflows` | Every GitHub Action pinned to a commit hash, read-only token permissions (a `write` scope only for a job in `WRITE_ALLOWLIST`), no persisted checkout credentials, no `pull_request_target` |
| `check-docs` | Required READMEs and their Contents, relative links and anchors, lesson and rule files, the length of `AGENTS.md` (the conventions are in [docs/README.md](../docs/README.md#conventions)) |
| `bench [--save NAME] [--compare A B] [--allow-battery]` | Criterion wall-clock benchmarks; on macOS it refuses battery power and keeps the machine awake ([bench/README.md](../bench/README.md)) |
| `bench-report [--baseline NAME] [--against NAME] [--notes FILE] [--out FILE]` | One self-contained HTML page of saved Criterion results, and with `--against` a noise-aware before/after view (`target/bench-report/index.html`) |
| `bench-doc [--baseline NAME] [--env FILE]... [--doc FILE]` | Regenerates the data sections of `docs/benchmarks.md` from a saved baseline |
| `bench-build`, `bench-smoke` | Build the benchmarks; run each once in test mode, measuring nothing, so a broken benchmark fails CI (needs `gungraun-runner`) |
| `bench-regress [--base REV] [--report-only]` | The instruction-count gate; Linux only ([bench/README.md](../bench/README.md#instruction-count-gate)) |
| `gungraun-version` | The `gungraun` version that `gungraun-runner` must match |
| `spike-bench <topic> [--save NAME] [--allow-battery]` | A spike's Criterion bench ([spikes/README.md](../spikes/README.md)) |
| `prng-stats [--seed N] [--bytes N]` | Streams PRNG output for PractRand |
| `fuzz <target\|all> [--time SECS] [--replay PATH]`, `fuzz-build`, `fuzz-toolchain` | Fuzzing on the pinned nightly ([fuzz/README.md](../fuzz/README.md)) |
| `coverage [--open]` | Line and function coverage ([below](#coverage)) |
| `mutants [--module NAME]... [-j N] [--list] [--in-diff FILE]` | Mutation testing ([below](#mutation-testing)) |
| `guard-push` | The push guard behind `.claude/hooks/guard-push.sh`; reads a hook's JSON on stdin |

## Coverage

```sh
cargo xtask coverage          # measure, print the per-module table, write the reports
cargo xtask coverage --open   # the same, then open the HTML report
```

You need `cargo-llvm-cov` and `rustup component add llvm-tools-preview`; the command prints the line for whichever is missing. It runs the workspace tests under instrumentation (the same tests and features as `cargo xtask test --include-ignored`, minus doctests, `xtask`, `engine_benches`, `engine_bench_regress` and the spike packages) in a separate build under `target/llvm-cov-target/`, so the first run is as slow as a clean build. It writes `target/llvm-cov/html/index.html` (annotated source, for triage), `target/llvm-cov/lcov.info` (for editor gutters) and `target/llvm-cov/summary.md` (the table).

- **What the table counts.** Lines and functions in `libs/*/src/` and `examples/*/src/`, with one row per module of `engine` (`engine::genid` and so on, and `engine` for the crate root) and one per example. It leaves out doctests (not measured: `cargo-llvm-cov` supports them only on nightly, so code that only a doctest runs shows as uncovered), `tests/`, `benches/`, `bench/`, `spikes/` and `xtask/`, and every item marked `#[test]` or `#[cfg(test)]` (including `cfg(all(test, ...))`); the heading says how many instrumented lines that removed. Code under `#[cfg(any(test, feature = "test-invariants"))]` is counted. The HTML and lcov reports are unfiltered apart from the directories, so their percentages are higher.
- **No threshold.** CI's `coverage` job writes the same table to the job summary and uploads the reports for 30 days. It fails only when a test fails, and it is not a required check. Use the report to find behaviour no test exercises, not to raise the number.
- **What library code must do** for the report to be right (no coverage attributes, inline test modules) is in [Tests](../CONTRIBUTING.md#tests).
- **Platforms.** CI measures on Ubuntu only. The command is verified on macOS and Linux, not on Windows.

## Mutation testing

Run mutated code only in a memory-capped container: in Claude Code through `/mutants-run`, never bare on the host.

```sh
cargo xtask mutants --list            # check the exclusions and list the mutants; builds nothing
cargo xtask mutants                   # test the triaged modules (collections, genid and random)
cargo xtask mutants --module random   # one module (repeatable)
cargo xtask mutants -j 2              # two mutants at a time
cargo xtask mutants --in-diff FILE    # only mutants in code that a unified diff changes
```

You need `cargo-mutants` at the pinned version; the command prints the install line when it is missing and warns when the version differs, because results from another version are not comparable. The command checks the exclusion list, runs `cargo mutants` with the settings in `.cargo/mutants.toml` (nextest, all features, ignored tests included, `cap_lints`), writes the output to `target/mutants/mutants.out/` and the table to `target/mutants/summary.md`, and prints it. The first run builds a copy of the workspace per job, so it is as slow as a clean build.

- **What the outcomes mean.** *Caught*: a test failed with the mutant in place, which is what should happen. *Missed*: every test passed with it, so a test is missing or the mutant changes nothing observable. *Timeout*: the tests did not finish in time, usually an endless loop; this detects the mutant. *Unviable*: the mutant does not compile and says nothing about the tests. Doctests do not run (as in coverage), so a mutant only a doctest catches shows as missed.
- **Report only.** A missed mutant never fails anything and there is no target score: a score to hit invites tests written for the score. The command fails only when it could not produce a report. CI's `mutants` workflow runs every mutant on Sundays (on `main`) and on manual dispatch (about 20 minutes). On a pull request that changes the sources of a triaged module, or the mutants tooling, it runs only the mutants in the changed code (`cargo xtask mutants --in-diff FILE`) after checking the exclusion list, which takes a few minutes. Both write the table to the job summary and upload `mutants.out/` for 30 days. Neither is a required check.
- **Ad hoc runs.** A plain `cargo mutants` uses the same settings. For one mutant: `cargo mutants -p engine -f 'libs/engine/src/genid/**' -F 'shrink_to_fit'`. For changed code: `git diff main > target/changes.diff`, then `cargo mutants --in-diff target/changes.diff`. Read a missed mutant's diff in `target/mutants/mutants.out/diff/`.
- **Triage.** Give each missed mutant exactly one class: *test gap, fixed* (a test at the public-API level that fails with the mutant and passes without it; never one that only pins the mutant's diff), *test gap, deferred* (an issue; the mutant stays missed), *bug* (an issue, fixed in its own pull request, test first), *equivalent* (no input tells it from the original; exclude it) or *not worth a test* (real behaviour no caller depends on, such as a speed-only fast path; exclude it and say why). A readable `Debug` impl is behaviour, not a candidate for exclusion. The record is [docs/mutation-triage.md](../docs/mutation-triage.md); it is written by hand.
- **Exclusions.** Only in `exclude_re` in `.cargo/mutants.toml` (library code has no `mutants::` attributes: [Tests](../CONTRIBUTING.md#tests)). The list is written in a restricted form (`exclude_re = [`, one `'single-quoted'` regex per line followed by `,`, `]`), with a comment directly above each entry giving the reason. The command rejects an entry without a reason, an entry that contains a line or column number (a `:` followed by a digit; it would stop matching when the file changes), and an entry that matches no mutant. `cargo xtask mutants --list` prints how many mutants each entry removes; check that against what you meant. `cargo-mutants` never applies a name filter to its "delete field" mutants, so those cannot be excluded.
- **The triaged set** is a constant in `xtask/src/mutants.rs` and grows by chunk: a chunk that triages another module adds it there and to the record.
