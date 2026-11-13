# M2.4: Gungraun gate

- **Status:** Draft (2026-11-13).
- **Milestone:** M2 (process and verification infrastructure) · **Depends on:** M2.0 · **Decides:** no ADR
- **Estimated size:** about 800 changed lines, excluding tests, lockfiles and generated files (`xtask` about 350, the benchmark crate about 250, workflow, devcontainer and Pulumi about 70, docs about 130)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds an instruction-count regression gate: a new benchmark crate, `engine_bench_regress`, with Gungraun benchmarks for the `engine_handle` and `engine_random` hot paths; a `cargo xtask bench-regress` command that runs them under Valgrind at a base commit and at the head in the same job, and fails when any benchmark's instruction count rises by more than 5%; and a `bench-regress` workflow that runs it on every pull request and becomes a required check. It comes now because "Gungraun gate active on pull requests" is the last open M2 exit criterion, and M3 and M4 will add the storage hot paths that the gate exists to protect. At the end, a pull request that slows `Prng::next_u64` is blocked until it carries the `perf-regression-accepted` label, and a docs-only pull request passes the check in seconds.

## 2. Goals and non-goals

**Goals**

- Catch accidental instruction-count regressions in the handle and PRNG hot paths before they merge, with a result that does not depend on runner noise.
- One command that does the whole comparison, run the same way locally (in the devcontainer) and in CI.
- No stored baselines: the base is measured in the same job as the head, so nothing expires or drifts (M0+M1 spec §8).
- A way to accept an intended regression that is visible in the pull request, and that never hides a broken run.
- A required check that cannot block a pull request it has nothing to say about (docs-only changes).

**Non-goals**

- No gating on cache metrics. Callgrind's cache simulation is on and its estimates are reported, but only instruction counts (`ir`) have a limit (M0+M1 spec §8).
- No benchmarks for `engine_time`, `engine_math`, `fixed_step_sim` or the `engine` facade (decided 2026-11-13, §7). Later chunks add benchmarks when they add hot paths; M4 adds the storage ones.
- No published performance numbers. Instruction counts measure change between two commits; Criterion and `docs/benchmarks.md` remain the record of performance, and the two are never compared.
- No run on pushes to `main`, no trend history and no hosted benchmark service.
- No native macOS or Windows support for `bench-regress`: Valgrind runs on Linux only. macOS users use the devcontainer.
- No step in `cargo xtask ci` (§4.6).
- No change to library code, public APIs, frozen formats or `rust-toolchain.toml`.

## 3. Guide-level explanation

In the devcontainer (or any Linux machine with Valgrind 3.20 or later and the matching `gungraun-runner`), on a branch:

```sh
cargo xtask bench-regress                 # compare HEAD with its merge-base with origin/main
cargo xtask bench-regress --base HEAD~3   # compare with another commit
cargo xtask bench-regress --report-only   # report regressions without failing
```

The command first checks that anything relevant changed (§4.4). If so, it checks the base out into a temporary worktree, runs the benchmarks there and saves their counts, runs the benchmarks again in the working tree, and prints Gungraun's comparison for every benchmark: instruction count, change against the base, and cache estimates. When a benchmark's instruction count rose by more than 5%, it ends with:

```text
error: 1 benchmark regressed by more than 5% in instructions (prng::next_u64_10k).
If the regression is intended, add the `perf-regression-accepted` label to the pull request
and say why in its description.
```

On macOS it stops before building and says to use the devcontainer. If `gungraun-runner` is missing or its version differs from the `gungraun` library in `Cargo.lock`, it prints `cargo install --locked gungraun-runner --version <version>`.

On a pull request, the `bench-regress` workflow runs the same command against the current tip of `main`. Adding or removing any label re-runs it; with `perf-regression-accepted` present, a regression is reported as accepted and the check passes. A build failure or a Valgrind error fails the check whatever the labels.

To add a benchmark, add a function to `bench-regress/benches/handle.rs` or `prng.rs` (or a new `[[bench]]` file for a new crate). When you change what an existing benchmark measures, rename its ID: the base runs the base's copy of the benchmark, so an edited benchmark with the same ID compares two different things.

## 4. Reference-level design

### 4.1 Layout

```text
bench-regress/                       new workspace member, package engine_bench_regress
  Cargo.toml                         publish = false; dev-dependencies only
  src/lib.rs                         #![forbid(unsafe_code)] and crate docs; no code
  benches/handle.rs                  [[bench]] harness = false
  benches/prng.rs                    [[bench]] harness = false
xtask/src/bench_regress.rs           the command
.github/workflows/bench-regress.yml  the workflow
```

`Cargo.toml` (workspace) MUST add `bench-regress` to `members` and `gungraun = { version = "=0.20.0", default-features = true }` to `[workspace.dependencies]`. The exact pin exists because `gungraun-runner` must have the same version as the library or the run aborts. Dependabot bumps it; the workflow (through `setup-gungraun`) and the devcontainer (through `cargo xtask gungraun-version`, §4.8) follow `Cargo.lock`, so a bump needs no other edit.

The benchmarks live in their own crate rather than in `libs/<crate>/benches/` as M0+M1 spec §8 says (decided 2026-11-13, §7). This keeps `gungraun` out of the library crates' dev-dependencies and lets the Criterion commands exclude one package. M0+M1 spec §8 MUST get a one-line amendment note pointing here.

### 4.2 The benchmarks

Each benchmark measures one operation at one size. Building its input (a filled pool, a shuffled handle list, a seeded `Prng`) MUST happen in Gungraun's setup function, which is not measured, and each measured function MUST pass its result through `core::hint::black_box`. All inputs are derived from fixed seeds, so the same commit always measures the same work.

| Target | ID | Measures |
|---|---|---|
| `handle` | `pool_insert_1k` | 1,000 `HandlePool<u64>::insert` into an empty pool with capacity reserved |
| | `pool_get_seq_1k` | `get` of 1,000 live handles in insertion order |
| | `pool_get_random_1k` | `get` of the same handles in a shuffled order |
| | `pool_remove_1k` | `remove` of 1,000 handles in a shuffled order |
| | `pool_iter_1k` | iterating 1,000 values and summing them |
| | `pool_churn_1k` | removing a shuffled half, then reinserting 500 values |
| | `allocator_allocate_1k` | 1,000 `HandleAllocator::allocate` |
| | `allocator_free_realloc_1k` | freeing a shuffled half, then allocating 500 (recycled slots) |
| | `allocator_is_alive_1k` | `is_alive` on 1,000 handles, half of them stale |
| | `dense_allocate_1k` | 1,000 `DenseHandleAllocator::allocate` |
| | `dense_free_1k` | freeing 1,000 handles in a shuffled order |
| `prng` | `next_u64_10k` | 10,000 `Prng::next_u64` |
| | `range_u32_10k` | 10,000 `range(0..1000)` |
| | `range_f64_10k` | 10,000 `range(0.0..1.0)` |
| | `shuffle_1k` | shuffling a 1,000-element slice |
| | `from_seed_1k` | constructing a `Prng` from each of 1,000 seeds |

The plan MAY adjust an operation to the exact API names (for example `try_insert` against `insert`), and MUST record any change of ID or size in its Deviations section. Sizes are small on purpose: under Valgrind each benchmark runs 20 to 50 times slower than natively, and every one runs twice per pull request.

**Minimum size.** Each benchmark MUST measure at least 10,000 instructions. A smaller one turns the 5% limit into a handful of instructions, which a harmless change in code generation can exceed; a benchmark below the floor gets a larger loop count (and its ID says so). The plan records each benchmark's instruction count from the first run.

Callgrind's cache simulation MUST be on (`--cache-sim=yes`), passed by `xtask` on the command line so that the base and head runs use the same settings whatever the benchmark files say.

**Platforms.** `cargo xtask clippy` (all targets) and `bench-smoke` compile the benchmark crate on macOS, and `clippy` also on Windows in the `os-matrix` job. The `gungraun` library MUST therefore compile on all three. If it does not compile on Windows, the dependency moves to `[target.'cfg(unix)'.dev-dependencies]` and each benchmark file compiles to an empty `main` on other targets; the plan checks this first and records the outcome in its Deviations section.

### 4.3 Separation from the Criterion commands

| Command | `engine_bench_regress` |
|---|---|
| `cargo xtask bench` | MUST be excluded (`--exclude engine_bench_regress`), so a Criterion run never starts Valgrind |
| `cargo xtask bench-build` | MUST be excluded, so the command means "the Criterion benchmarks compile"; `bench-smoke`, which runs next in `ci` and in the `bench-build` job, compiles and runs it |
| `cargo xtask bench-smoke` | MUST be included, run in Gungraun's test mode (`--test`, already passed to every bench target), which runs each benchmark function once without Valgrind, so a broken benchmark fails ordinary CI |
| `cargo xtask coverage` | MUST be excluded, like `engine_benches` |
| `cargo xtask bench-doc` | unaffected: it reads Criterion baselines only |

### 4.4 `cargo xtask bench-regress`

```text
cargo xtask bench-regress [--base <rev>] [--report-only]
```

- `--base <rev>`: any revision `git rev-parse` accepts. Default: `git merge-base origin/main HEAD`. CI passes `HEAD^1` (§4.7).
- `--report-only`: a regression does not fail the command (§4.5).
- Parsing MUST reject unknown flags, `--base` without a value, and a repeated flag.

**Directories.** `<target>` below is the effective Cargo target directory: `$CARGO_TARGET_DIR` when set, otherwise `<repo>/target`. Everything the command writes (the base worktree, the base build, `GUNGRAUN_HOME`) goes under `<target>/bench-regress/`. In the devcontainer that is `/target`, a volume, rather than the slow bind mount of the repository; and in the devcontainer the worktree is not nested inside the repository, so Cargo does not merge the repository's `.cargo/config.toml` into the worktree's.

The command MUST run these steps in order:

1. **Preflight.** Fail with an actionable message, before building anything, when: the host is not Linux (the message names the devcontainer); `valgrind --version` is missing or below 3.20; `gungraun-runner` is missing, or its `--version` differs from the `gungraun` version in `Cargo.lock` (the message is the `cargo install` line of §3). Resolve `--base` to a commit hash, and fail naming the revision if it does not resolve.
2. **Relevance.** List the tracked files that differ between the base and the working tree (`git diff --name-only <base>`; in CI the working tree is the checked-out commit, locally it includes uncommitted changes, which the head run also measures). If none matches any of the patterns below, print `bench-regress: skipped, no relevant changes since <short base>` and exit 0.

   ```text
   libs/**   bench-regress/**   Cargo.toml   Cargo.lock   rust-toolchain.toml   .cargo/**
   xtask/src/bench_regress.rs   .github/workflows/bench-regress.yml
   ```
3. **Clean.** Delete `<target>/bench-regress/home/` so a baseline left by an earlier run can never be compared against.
4. **Base run.** The base run is skipped, with a printed reason, in two cases, and the head run (step 5) then reports every benchmark with no comparison:
   - the base has no `bench-regress/Cargo.toml` (`git cat-file -e <base>:bench-regress/Cargo.toml`): "every benchmark is new";
   - the `gungraun` version in the base's `Cargo.lock` (`git show <base>:Cargo.lock`) differs from the head's: "not compared: gungraun changed from X to Y". The runner installed for the head cannot run a bench binary built against another library version, and baselines from different Gungraun versions are not assumed comparable. Such a pull request (normally a Dependabot bump) passes the gate without a comparison.

   Otherwise, create a detached worktree of the base in `<target>/bench-regress/base-worktree/` (`git worktree add --detach`), run `rustup toolchain install` in it (installs the toolchain named by the base's `rust-toolchain.toml`, which differs from the head's on a toolchain bump, and does nothing when it is already installed), and run the bench targets in the worktree as described below, with `CARGO_TARGET_DIR=<target>/bench-regress/base-target` (so the base build does not evict the head build) and the Gungraun arguments

   ```text
   --save-baseline=base --callgrind-args=--cache-sim=yes
   ```

   Any failure fails the command (§4.5). The worktree MUST be removed (`git worktree remove --force`) whether the run passed, failed or was interrupted by an error in `xtask`.
5. **Head run.** Delete every `summary.json` under `GUNGRAUN_HOME`, then run the bench targets in the repository, with the default target directory and the Gungraun arguments

   ```text
   --baseline=base --callgrind-limits=ir=5% --save-summary=json --callgrind-args=--cache-sim=yes
   ```

   `--regression-fail-fast` MUST NOT be passed, so every regressed benchmark within a target is reported. A benchmark with no baseline (new, or the base was skipped) reports no change and does not fail.
6. **Verdict** (§4.5) and **summary** (§4.6).

**Running the bench targets.** The command does not run `cargo bench` for the measurement, because Cargo replaces a bench binary's exit code with 101, the same code as a build failure, and would hide Gungraun's regression code (3). Instead, for both runs:

- **Build:** `cargo bench -p engine_bench_regress --no-run --message-format=json`. The command reads Cargo's JSON messages (with `serde_json`, already a dependency of `xtask` through `bench-report`) and takes, from each `compiler-artifact` message whose target kind is `bench`, the target name and the `executable` path. A build failure, or no bench executable, is a failure.
- **Run:** each executable, in target-name order, with `--bench` followed by the Gungraun arguments, from the package directory (`bench-regress/` in the worktree or the repository), with `GUNGRAUN_HOME=<target>/bench-regress/home` and the environment Cargo gives a bench binary that Gungraun needs (at least `CARGO_MANIFEST_DIR` and `CARGO_PKG_NAME`; the plan's probe records the exact set). Every target runs, even after an earlier one failed or regressed. Each target's exit code is kept for §4.5.

The exact flag spellings (`--callgrind-args`, `--callgrind-limits`, `--save-baseline`, `--baseline`, `--save-summary`) and the `summary.json` schema are those of gungraun 0.20.0's documentation; the plan's first task confirms them, confirms that a bench executable started directly behaves as it does under `cargo bench` (same benchmark IDs, exit code 3 on a regression), and confirms that a baseline saved from the worktree is found by the head run from the repository (benchmarks are keyed by package, file, group, function and ID, not by path). If any of these does not hold, the plan stops and this spec is amended before anything else is built.

### 4.5 Exit codes

The head run is classified from the exit codes of its bench targets (§4.4), which are Gungraun's own: 0 success, 3 "one or more regressions", anything else (including death by a signal) an error. The head run is a **regression** when, and only when, every target exited 0 or 3 and at least one exited 3. It is a **failure** when any target exited with another code, even if another target regressed, or when the build failed.

| Outcome | Without `--report-only` | With `--report-only` |
|---|---|---|
| Skipped (step 2) | 0 | 0 |
| Every head target exits 0 | 0 | 0 |
| Regression (as defined above) | 1, with the message of §3 naming each regressed benchmark | 0, printing `REGRESSION ACCEPTED BY LABEL` and each regressed benchmark |
| Any failure: preflight, worktree, base run, build, a head target with any code other than 0 or 3 | 1 | 1 |

`--report-only` MUST NOT turn a failure into success, so a target that crashed or did not build can never be accepted by the label.

**Naming the regressed benchmarks.** After the head run, the command reads every `summary.json` under `GUNGRAUN_HOME` (with `serde_json`) and takes, for each benchmark, its ID, the base and head instruction counts and whether it regressed. The fields used are those of gungraun 0.20.0's summary schema, recorded by the plan's probe; the text output is shown but never parsed. The exit code, not the summaries, decides the verdict: if a target exited 3 but the summaries cannot be read or name no regressed benchmark, the verdict is still a regression, and the message says the names are unavailable and to read the output above.

### 4.6 Output and relationship to `cargo xtask ci`

Gungraun's output from both runs goes to the terminal unchanged. After the head run, the command prints a table built from the summaries: one row per benchmark, ordered by target and ID, with the base and head instruction counts, the change in percent (`new` when there is no base count), and a marker on each regressed row. When `GITHUB_STEP_SUMMARY` is set, the command MUST append to it: a heading, the base and head commit hashes, the verdict line (passed, skipped, not compared, regressed, or regression accepted), the list of regressed benchmarks when there are any (under "Accepted by label" with `--report-only`), and the table.

**The label is not scoped to a commit.** `perf-regression-accepted` stays on the pull request across later pushes, so a later commit that regresses a different benchmark is accepted too. The guard is review, not mechanism (decided 2026-11-13, §7): the pull request's Reviews section names the benchmarks the label covers, and the code review compares that list with the "Accepted by label" list of the final run.

`bench-regress` MUST NOT be a step of `cargo xtask ci` or `ci --quick`. It needs Valgrind (unavailable on macOS) and a base commit, so it runs through its own command, as M0+M1 spec §4.8 already says.

### 4.7 CI

`.github/workflows/bench-regress.yml`:

- Triggers: `pull_request` on `main` with `types: [opened, synchronize, reopened, labeled, unlabeled]`. No `push`, no `schedule`, no `paths` filter: a required check that never reports blocks the pull request, so relevance is decided by step 2, inside the job.
- Top-level `permissions: contents: read`; `concurrency` group per ref with `cancel-in-progress: true`.
- One job, named exactly `bench-regress`, on `ubuntu-latest`, `timeout-minutes: 30` (adjusted to the measured first run, recorded in the plan's Deviations if changed).
- Steps: `actions/checkout` (pinned, `persist-credentials: false`, `fetch-depth: 2`); `rustup show`; `Swatinem/rust-cache` (pinned); `gungraun/setup-gungraun` (pinned to a commit hash, default inputs: Valgrind, the runner matching `Cargo.lock`, libc debug symbols); then `cargo xtask bench-regress --base HEAD^1`, plus `--report-only` when the label is present.
- The label MUST be read as `contains(github.event.pull_request.labels.*.name, 'perf-regression-accepted')` into an environment variable, and the shell MUST compare that variable with `true`. No label text is interpolated into the script.
- The job MUST NOT be skipped by an `if:` on any event. GitHub reports a skipped job as passing a required check, so skipping on unrelated label events would let any label clear a failed gate.

**Why `HEAD^1`.** On `pull_request`, `actions/checkout` checks out the merge of the pull request into the current tip of `main`, whose first parent is that tip. Comparing `HEAD^1` with `HEAD` measures exactly what merging the pull request changes. It keeps the M0+M1 spec §8 property (base and head in the same job, nothing stored) and leaves out commits that reached `main` after the branch point. Locally the default is the merge-base, because a local `HEAD` is the branch, not a merge.

The workflow MUST pass `cargo xtask check-workflows`.

### 4.8 Devcontainer

- The image MUST move from `mcr.microsoft.com/devcontainers/base:bookworm` to `base:trixie`: bookworm's Valgrind is 3.19, below the 3.20 Gungraun is tested with; trixie ships 3.24.
- `install-tools.sh` MUST install `valgrind` with `apt-get`, and `gungraun-runner` with `cargo install --locked gungraun-runner --version "$(cargo xtask gungraun-version)"`, in an optional block like the fuzzing, coverage and mutants tools.
- New command `cargo xtask gungraun-version` prints the `gungraun` version recorded in `Cargo.lock` and nothing else, failing if there is none (the pattern of `fuzz-toolchain`).
- Because the image change refreshes every system package, the plan MUST re-run `install-tools.sh` and `cargo xtask ci` in the rebuilt container.

### 4.9 Repository settings (Pulumi)

`infra/github/index.ts` MUST:

- add the label `{ name: "perf-regression-accepted", color: "fbca04", description: "An instruction-count regression in bench-regress is intended; justified in the PR description" }`;
- add `"bench-regress"` to `requiredChecks`.

`pulumi up` MUST NOT run before the workflow is on `main`, and MUST NOT run without the maintainer's go-ahead: it changes live repository settings, and a required check added before its workflow exists blocks every open pull request. The M2 exit criterion is met once both have happened; the roadmap row moves to `Done` then.

### 4.10 Docs

- `CONTRIBUTING.md`: a new "Instruction-count gate" section: the commands; Linux and Valgrind only, the devcontainer on macOS; what the label means, that the description must justify it, and that it covers later pushes too (§4.6); the rename-the-ID rule (§3); that a toolchain bump often needs the label, because the compiler changed the code; that a `gungraun` bump is not compared (§4.4); the minimum size of a benchmark (§4.2); that the gate compares a pull request with its parent only, so small regressions can add up across pull requests (the milestone check below is the backstop); aarch64 devcontainer counts are only comparable with other aarch64 runs; how to add a benchmark. The tool list under "Setting up" names `valgrind` and `gungraun-runner`. The **Performance** row of "Reviews before a pull request" adds: when `perf-regression-accepted` is used, name the benchmarks it covers and check them against the final run's "Accepted by label" list.
- `docs/maintainers.md`: replace "planned for M2 (chunk M2.4)" with the gate's description and its measured CI time per pull request (the budget later chunks add benchmarks against); add the post-merge `pulumi up` step; and add to "When a milestone completes", before tagging: in the devcontainer or CI, run `cargo xtask bench-regress --report-only --base m<N-1>` (the previous milestone's tag) and record the result in `docs/benchmarks.md`. The first such run is at the end of M3, against `m2`; at the end of M2 there is no earlier tag containing the benchmarks.
- `docs/benchmarks.md`: a short methodology note: instruction counts gate change between two commits and are never compared with Criterion figures; and an empty "Instruction counts across milestones" section, filled by the milestone check above, recording the machine (architecture), both tags and every change of more than 5%.
- `README.md` command list, `AGENTS.md` (the gate among the checks that run outside `ci`), `CHANGELOG.md`, the roadmap row, and the M0+M1 spec §8 amendment note (§4.1).

## 5. Requirements

- **M2.4-R1** The workspace MUST contain the `engine_bench_regress` package in `bench-regress/` as §4.1 describes: `publish = false`, a `src/lib.rs` with `#![forbid(unsafe_code)]` and crate docs, dev-dependencies limited to `gungraun` (pinned `=0.20.0` in `[workspace.dependencies]`), `engine_handle` and `engine_random`, and the `handle` and `prng` bench targets with `harness = false`.
- **M2.4-R2** The bench targets MUST contain the benchmarks of §4.2 under those IDs (or the IDs recorded in the plan's Deviations), each building its input in an unmeasured setup function from fixed seeds, passing its result through `black_box`, and measuring at least 10,000 instructions.
- **M2.4-R3** `cargo xtask bench`, `bench-build` and `coverage` MUST exclude `engine_bench_regress`; `bench-smoke` MUST run it in Gungraun's test mode and pass without Valgrind.
- **M2.4-R4** `cargo xtask bench-regress` MUST accept `--base <rev>` and `--report-only`, default the base to the merge-base of `HEAD` and `origin/main`, and reject unknown flags, a missing value and a repeated flag.
- **M2.4-R5** The preflight MUST fail before building, with an actionable message, on a non-Linux host, a missing or older-than-3.20 Valgrind, a missing `gungraun-runner` or one whose version differs from `Cargo.lock`'s `gungraun`, and an unresolvable base.
- **M2.4-R6** When no file changed between the base and `HEAD` matches the patterns of §4.4 step 2, the command MUST print the skip message and exit 0 without building.
- **M2.4-R7** The base run MUST use a detached worktree, its own target directory and the shared `GUNGRAUN_HOME`, all under `<target>/bench-regress/`, and MUST install the base's toolchain first, as §4.4 step 4 describes; MUST be skipped with the stated reason when the base has no `bench-regress/Cargo.toml` or a different `gungraun` version in `Cargo.lock`; and the worktree MUST be removed on every path out of the command. `GUNGRAUN_HOME` MUST be emptied before the base run.
- **M2.4-R8** Both runs MUST build the bench targets with `cargo bench --no-run --message-format=json` and run each bench executable directly, every target even after an earlier one failed, as §4.4 describes. The head run MUST compare against the base run's baseline with an instruction-count limit of 5% and write JSON summaries, MUST NOT pass `--regression-fail-fast`, and both runs MUST enable Callgrind's cache simulation.
- **M2.4-R9** The command MUST classify the head run from its targets' exit codes as §4.5 defines, and its exit code MUST follow the table of §4.5; `--report-only` MUST change only the outcome of a regression.
- **M2.4-R10** On a regression, the command MUST name each regressed benchmark from the JSON summaries (or say the names are unavailable, §4.5) and, without `--report-only`, print the label instruction of §3. The text output MUST NOT be parsed.
- **M2.4-R11** The command MUST print the per-benchmark table of §4.6 and, when `GITHUB_STEP_SUMMARY` is set, append the summary of §4.6, including the list of regressed or accepted benchmarks.
- **M2.4-R12** `bench-regress` MUST NOT be a step of `cargo xtask ci` or `cargo xtask ci --quick`.
- **M2.4-R13** `.github/workflows/bench-regress.yml` MUST meet §4.7: the triggers and event types listed, no `paths` filter, no job-level or step-level `if:` that can skip the `bench-regress` job, `fetch-depth: 2`, `setup-gungraun` pinned to a commit hash, `--base HEAD^1`, and the label read only through `contains(...)` into an environment variable.
- **M2.4-R14** `cargo xtask check-workflows` MUST pass.
- **M2.4-R15** `infra/github/index.ts` MUST declare the `perf-regression-accepted` label and list `bench-regress` in `requiredChecks`. `pulumi up` MUST NOT be run before merge, nor without the maintainer's go-ahead.
- **M2.4-R16** The devcontainer MUST use the `base:trixie` image and install Valgrind and the matching `gungraun-runner` as §4.8 describes; `cargo xtask gungraun-version` MUST print `Cargo.lock`'s `gungraun` version.
- **M2.4-R17** The `xtask` change MUST include unit tests for flag parsing, the default and explicit base, the effective target directory (with and without `CARGO_TARGET_DIR`), the two base-skip conditions (no crate; different `gungraun` versions read from two `Cargo.lock` texts), the relevance matcher (relevant and irrelevant paths of every pattern), the Valgrind and runner version parsing and comparison, reading the version from `Cargo.lock`, the base and head argument lists and environment, reading bench executables from Cargo's JSON messages (bench artifacts among library and build-script artifacts; none at all), the classification of §4.5 (all targets 0; one and two targets 3; a 3 alongside a 101 or a signal; a build failure) and the exit-code mapping over every row of its table, reading `summary.json` files (a captured fixture from the probe: a regressed, an unchanged and a new benchmark; a malformed file), the table and job-summary renderers, and the package exclusions of R3.
- **M2.4-R18** The pull request MUST record, with output: the cross-directory baseline check (§4.4); `bench-regress --base HEAD` reporting no instruction-count change in any benchmark, both in the devcontainer and in CI (through a temporary commit that makes the workflow pass `--base HEAD`, reverted before review); each benchmark's instruction count; a local run against a throwaway commit that adds work to `Prng::next_u64` exiting 1 and naming the benchmark, the same run with `--report-only` exiting 0, and a deliberately broken head build exiting 1 with `--report-only`; and the CI run's duration.
- **M2.4-R21** `cargo xtask deny` MUST pass with `gungraun` and its dependency tree, with no new entry in `deny.toml`'s exceptions or ignores unless the pull request justifies it under ADR 0002.
- **M2.4-R19** The documentation changes of §4.10 MUST be made.
- **M2.4-R20** No file under `libs/` and no `rust-toolchain.toml` may change, and no public API or frozen format may change.

## 6. Cross-cutting concerns

- **Determinism:** n/a, because no library code changes. The benchmarks use fixed seeds so the same commit always measures the same work; that is what makes a zero-change self-comparison (R18) possible.
- **`unsafe`:** none in our code; the new crate root has `#![forbid(unsafe_code)]`. `gungraun` contains `unsafe` and is a dev-dependency of a tooling crate, as `criterion` is; nothing in the engine links it.
- **Concurrency:** n/a, because nothing is concurrent.
- **Heap allocation:** n/a, because no library code changes. Allocations inside a measured function are counted as instructions, which is intended.
- **`no_std` and wasm:** n/a, because the crate is not part of the `no_std` or wasm builds and the library crates do not change.
- **Frozen formats:** none change. The benchmark IDs are not a frozen format; renaming one only makes it report as new once.
- **Public API changes:** none.
- **FFI exposure:** n/a.
- **Performance:** this chunk adds measurement, not optimisation. The noise floor of the gate is checked by the self-comparison of R18, which must show no change; the first CI run's time is recorded.
- **Documentation:** §4.10.
- **Policy changes:** a new required status check; the `perf-regression-accepted` label and the rule that its use is justified in the pull request description; an exact version pin on `gungraun`; the benchmark location differs from M0+M1 spec §8 (amended); the devcontainer moves to Debian trixie; a milestone-close step compares instruction counts with the previous milestone's tag (§4.10). Valgrind (GPL) is an external tool that is never linked, like `wasmtime`; `gungraun` and its dependency tree MUST pass `cargo xtask deny` under ADR 0002.

## 7. Alternatives considered

- **`xtask` computes the changes and applies the 5% rule itself** from both runs' JSON summaries. Rejected (2026-11-13): it reimplements the limit logic Gungraun already has. *Corrected 2026-11-13, before the plan: the first draft also cited a new JSON dependency, but `xtask` already depends on `serde_json` (`bench-report`). The design now uses it to read the summaries for naming and reporting only (§4.5, §4.6); Gungraun still decides what is a regression.*
- **Parsing Gungraun's text output** (its `Gungraun result: ... M regressed` line) to tell a regression from a failure under `cargo bench`, which reports both as 101. Rejected (2026-11-13) in the same correction: the human-readable format of a 0.x tool may change on any bump, whereas running the bench executables directly gives Gungraun's exit code itself.
- **Benchmarks in `libs/<crate>/benches/`** (M0+M1 spec §8). Rejected (2026-11-13): `gungraun` would become a dev-dependency of two library crates, and the Criterion commands would have to filter bench targets by a naming convention instead of excluding one package.
- **Benchmarks inside `engine_benches`.** Rejected: that crate compares the engine with other libraries; mixing in the regression gate and its dependency would blur both purposes.
- **Separate base and head jobs with a baseline artifact, or a baseline saved on pushes to `main`.** Rejected in M0+M1 spec §8: different runners, artifacts that expire, and baselines that go stale.
- **A hosted service (CodSpeed, Bencher).** Not scheduled (maintainers.md); the in-job comparison needs no account or secret.
- **An advisory, non-required check, or advisory first.** Rejected (2026-11-13): the exit criterion says "gate", and the label already gives an escape for intended regressions.
- **Skipping the job on label events other than `perf-regression-accepted`.** Rejected: a skipped job satisfies a required check, so any label would clear a failed gate.
- **Comparing with the merge-base in CI.** Rejected in favour of `HEAD^1`: the same "measured in the same job" property, and it measures the merge result against current `main` rather than an older commit.
- **Benchmarks for `engine_time` and `engine_math`.** Deferred (2026-11-13): neither has a hot path worth gating yet, and each benchmark costs CI time twice per pull request.
- **Keeping bookworm and building Valgrind from source, or no devcontainer support.** Rejected (2026-11-13): several minutes per container build and more script to maintain, or contradicting the roadmap row.
- **Gating cache-miss estimates.** Rejected in M0+M1 spec §8: Callgrind's cache model is a simulation, useful for spotting a change, not precise enough to fail a build on.
- **Installing a second runner when a pull request changes the `gungraun` version**, so that the comparison still runs. Rejected (2026-11-13): a slower job and more code, for a comparison across two versions of the measuring tool that is not assumed meaningful. Such pull requests pass uncompared (§4.4).
- **Accepting drift across pull requests without a backstop, or making the check strict** (`strictRequiredStatusChecksPolicy`). Rejected (2026-11-13) in favour of the milestone check (§4.10): a strict check catches only two concurrent pull requests, and forces a rebase before every merge.
- **A mechanical allow-list for the label**: the pull request description names the accepted benchmark IDs and the command fails on any other regression. Rejected (2026-11-13): the workflow would parse untrusted pull request text, for a risk that review already covers with a sole maintainer (§4.6).

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | Read `bench-regress/Cargo.toml` and the workspace `Cargo.toml`; `cargo xtask check-unsafe` passes. |
| R2 | Read the bench files; a `bench-regress` run lists every ID. Code review checks that setup work is outside the measured functions. |
| R3 | Unit tests on the `bench`, `bench-build` and `coverage` argument lists; `cargo xtask bench-smoke` on macOS without Valgrind passes and its output names the Gungraun benchmarks. |
| R4 | Unit tests on flag parsing and the default base. |
| R5 | Unit tests on version parsing and comparison; on macOS, `cargo xtask bench-regress` prints the devcontainer message; in the devcontainer, with `gungraun-runner` hidden from `PATH`, it prints the install line. |
| R6 | Unit tests on the relevance matcher; a local run with `--base` set to a commit that differs only in docs prints the skip message without building. |
| R7 | Unit tests on the base argument list and environment, the target directory and both skip conditions; a local run leaves no worktree (`git worktree list`), also after a deliberately failing base build; a run with a base that predates the crate reports every benchmark as new; a local run against a throwaway commit that changes the `gungraun` version in `Cargo.lock` prints "not compared". |
| R8 | Unit tests on the build and run argument lists and on reading executables from Cargo's JSON; the run's output shows cache estimates; a local run with a deliberately panicking benchmark in the first target still runs the second. |
| R9 | Unit tests on the classification and the exit-code mapping over every row of §4.5; the local runs of R18. |
| R10 | Unit test on reading the probe's captured `summary.json` fixtures; the throwaway-commit run of R18; `grep` finds no parsing of Gungraun's text output in `xtask`. |
| R11 | Unit tests on the table and job-summary renderers; the job summary of the pull request's `bench-regress` run. |
| R12 | Unit tests on `ci_steps(false)` and `ci_steps(true)`. |
| R13 | Read the workflow; the pull request's `bench-regress` run is green and reports every benchmark as new. |
| R14 | `cargo xtask check-workflows`. |
| R15 | Read the `infra/` diff; `pulumi preview` (run by the maintainer, or with their go-ahead) shows exactly one label and one required check added. |
| R16 | Rebuild the devcontainer; `valgrind --version` reports 3.20 or later; `gungraun-runner --version` equals `cargo xtask gungraun-version`; `cargo xtask ci` passes inside it. |
| R17 | `cargo xtask test` runs the new tests. |
| R18 | The pull request description. |
| R19 | Documentation review. |
| R20 | `git diff --stat main -- libs/ rust-toolchain.toml` is empty. |
| R21 | `cargo xtask deny`; `git diff main -- deny.toml`. |

## 9. Done when

- [ ] All requirements in §5 are verified as in §8, with the evidence in the pull request description.
- [ ] `cargo xtask ci` passes locally on macOS, in the rebuilt devcontainer, and in CI on all legs; the `bench-regress` job is green on the pull request.
- [ ] The eight reviews of CONTRIBUTING.md are listed in the pull request.
- [ ] The documentation of §4.10 is updated, the roadmap row is `In progress` on the branch, and this spec's status is `Implemented (PR #n, date)`.
- [ ] The pull request title is `feat(xtask): instruction-count regression gate through Gungraun (M2.4)` and says an assistant was used.
- [ ] After merge and with the maintainer's go-ahead: `pulumi up` applied, a throwaway draft pull request with the `Prng::next_u64` slowdown is blocked by the required check and passes once labelled, then is closed unmerged; the roadmap row moves to `Done (PR #n)` and M2 to `Done`.

## 10. Open questions

None. The facts this design depends on and has not yet observed (`gungraun`'s dependency tree passes `cargo deny`; a bench executable started directly behaves as under `cargo bench`; a baseline saved from a worktree is found by a run from the repository; the flag spellings and `summary.json` schema of gungraun 0.20.0; `gungraun` compiles on Windows) are the plan's first tasks, each with a stated consequence (R21, §4.4, §4.5, §4.2).

## 11. Prior art and sources

- M0+M1 spec §8 (benchmarking strategy) and §4.12 (staged infrastructure).
- Gungraun documentation, 0.20.0: [baselines](https://gungraun.github.io/gungraun/latest/html/cli_and_env/baselines.html), [regressions](https://gungraun.github.io/gungraun/latest/html/regressions.html), [basic usage and exit codes](https://gungraun.github.io/gungraun/latest/html/cli_and_env/basics.html), [installation in CI](https://gungraun.github.io/gungraun/latest/html/installation/ci.html), [best practices](https://gungraun.github.io/gungraun/latest/html/best_practices.html).
- [`gungraun/setup-gungraun`](https://github.com/gungraun/setup-gungraun), the GitHub Action used in §4.7.
- GitHub documentation on [skipped jobs and required status checks](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/troubleshooting-required-status-checks#handling-skipped-but-required-checks) and on the [`pull_request` merge ref](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#pull_request).
- Valgrind's [Callgrind manual](https://valgrind.org/docs/manual/cl-manual.html) (instruction counts, `--cache-sim`).
