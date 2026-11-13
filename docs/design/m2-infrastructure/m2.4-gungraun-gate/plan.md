# M2.4 plan: Gungraun gate

- **Status:** Historical: executed in PR #18 (2026-11-13); the spec and the code are authoritative.
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both.

## 1. Approach

The work is on branch `m2.4-gungraun-gate`, which already holds the spec. The devcontainer moves to trixie first, because every Valgrind step (the probe, the manual verification) runs inside it: the host is macOS. The benchmark crate comes next, with the Criterion commands taught to leave it alone. Then `xtask` is built in three layers: pure helpers (flags, versions, relevance, skips), pure reporting (Cargo's JSON, Gungraun's summaries, classification, rendering), then the command that wires them to `git`, `cargo` and the bench executables. The workflow and Pulumi follow, then the docs, then the manual verification of R18 and the pull request.

**Running in the devcontainer.** From the host: `docker compose -f .devcontainer/docker-compose.yaml up -d`, then `docker compose -f .devcontainer/docker-compose.yaml exec -u vscode -w /workspace dev bash -lc '<command>'`. VS Code's "Reopen in Container" is equivalent. The container is aarch64 Linux; its counts are only compared with each other.

**Probe before code (Step 1, no commit for the probe itself).** In the trixie container, with `gungraun-runner 0.20.0` and Valgrind installed and a draft of the Step 2 crate (two benchmarks are enough), confirm what the spec took from the documentation, and write the result of each into §4 Deviations:

1. `cargo xtask deny` passes with `gungraun` added (R21). If it does not, stop and ask.
2. `cargo clippy -p engine_bench_regress --benches --target x86_64-pc-windows-msvc` passes on the host after `rustup target add x86_64-pc-windows-msvc` (spec §4.2, Windows). If it does not, use the `cfg(unix)` fallback of spec §4.2.
3. `cargo bench -p engine_bench_regress -- --test` runs on macOS without Valgrind, and prints each benchmark (spec §4.3).
4. The flags `--save-baseline`, `--baseline`, `--callgrind-limits=ir=5%`, `--save-summary=json`, `--callgrind-args=--cache-sim=yes` are accepted with these spellings.
5. A bench executable from `cargo bench --no-run --message-format=json`, started directly with `--bench <args>` from `bench-regress/`, runs the same benchmark IDs as `cargo bench`. Find the smallest environment it needs by starting it with `env -i PATH=$PATH HOME=$HOME` plus candidates (`CARGO_MANIFEST_DIR`, `CARGO_PKG_NAME`, `CARGO_PKG_VERSION`, `CARGO`), and record the set.
6. With a baseline saved by a run from a `git worktree` (its own `CARGO_TARGET_DIR`, shared `GUNGRAUN_HOME`), a run from the repository compares against it (the output shows a base count, not `N/A`).
7. With a deliberate slowdown in one benchmark, the executable exits 3; with a panicking benchmark, it exits with something other than 0 or 3.
8. The `summary.json` files: where they land under `GUNGRAUN_HOME`, and the JSON paths of the benchmark's ID, module path, the `ir` counts (new and old) and the regression flag. Copy three of them (a regressed, an unchanged and a new benchmark) to `xtask/src/bench_regress/fixtures/` for Step 4's tests.
9. `gungraun-runner --version` and `valgrind --version` output, verbatim, for Step 3's parsers.
10. Whether setup functions are excluded from the counts: an empty benchmark with an expensive setup reports a near-zero `ir`.

If item 5 or 6 fails, stop and ask: the spec is amended before anything else is built (spec §4.4).

**Module layout** (spec §4.1):

- `xtask/src/bench_regress.rs`: `Options`, flag parsing, version and relevance helpers, the command.
- `xtask/src/bench_regress/cargo_json.rs`: bench executables from Cargo's JSON messages.
- `xtask/src/bench_regress/report.rs`: summaries, classification, verdict, rendering.
- Running processes: `cmd::run` turns a non-zero exit into an error, but here 3 is a result. The command starts the bench executables with `std::process::Command` itself, prints the command line as `cmd::run` does, and records `status.code()` (`None` is a signal). `git` and `cargo` calls that only need success use `cmd::run`/`cmd::run_in`; calls whose output is needed use `Command::output()`.

**Global constraints**

- Every commit passes `cargo xtask ci --quick`; commits touching a workflow also pass `cargo xtask check-workflows`.
- Commit messages are conventional, end with ` (M2.4)`, and end with the attribution lines the session requires. Stage by explicit path.
- `xtask/Cargo.toml` gets no new dependency (`serde_json` is already there).
- No change under `libs/` or `examples/`, nor to `rust-toolchain.toml` (R20).
- `gungraun` is pinned `=0.20.0` in `[workspace.dependencies]` only.
- Stop and ask the maintainer before: the first push of the branch; opening the pull request; the temporary CI noise-floor commit's push; `pulumi preview` or `pulumi up`; opening the throwaway draft pull request after merge.

**Review focus.** Failure modes the spec implies that are easy to miss. Each has a test or check in the step that owns the code.

1. **A crash next to a regression accepted by the label.** One target exits 3, another panics; with `--report-only` this must still fail. Test in Step 4: `a_crash_beside_a_regression_is_a_failure`.
2. **A stale baseline or summary compared as if fresh.** A second local run with a base that has no crate must report every benchmark as new, not compare with the first run's baseline. Check in Step 8.
3. **A worktree left behind after a failure**, which makes the next `git worktree add` fail. Check in Step 8 (`git worktree list` after a failing base build).
4. **The version check matching the wrong package.** `Cargo.lock` also lists `gungraun-runner` and `gungraun-macros`. Test in Step 3: `lock_version_matches_the_exact_package_name`.
5. **A relevant change judged irrelevant**, so the gate skips a pull request that changes counts. Test in Step 3: `relevance_table` covers every pattern of spec §4.4 step 2 and near misses (`xtask/src/mutants.rs`, `docs/benchmarks.md`, `benches/benches/prng.rs`).

## 2. Steps

### Step 1: devcontainer on Debian trixie, and the probe (R16)

- **Files:** `.devcontainer/docker-compose.yaml` (image `mcr.microsoft.com/devcontainers/base:trixie`); `.devcontainer/install-tools.sh` (an optional block: `sudo apt-get update && sudo apt-get install -y valgrind`, then `cargo install --locked gungraun-runner --version "$(cargo xtask gungraun-version)"`, with the same warning style as the other blocks). The `gungraun-version` command arrives in Step 3, so until then the block fails softly with its warning; the probe installs the runner by hand at `0.20.0`.
- **Test first:** on the old image, `valgrind --version` in the container is missing or 3.19.
- **Change:** the two files; rebuild the container (`docker compose ... up -d --force-recreate --build`, or VS Code "Rebuild Container").
- **Verify:** in the container, `valgrind --version` prints 3.24 or later; `bash .devcontainer/install-tools.sh` finishes (warnings allowed for the runner block); `cargo xtask ci` passes. Then run the probe (§1) and record its findings in §4.
- **Commit:** `build(devcontainer): move to Debian trixie and install Valgrind (M2.4)`

### Step 2: the benchmark crate, kept out of the Criterion commands (R1, R2, R3, R21)

- **Files:** new `bench-regress/Cargo.toml`, `bench-regress/src/lib.rs`, `bench-regress/benches/handle.rs`, `bench-regress/benches/prng.rs`; root `Cargo.toml` (member, `gungraun = "=0.20.0"` in `[workspace.dependencies]`); `xtask/src/main.rs` (`bench_args`, new `bench_build_args`, `bench_smoke_args` unchanged); `xtask/src/coverage.rs` (`test_args`).
- **Interfaces produced:** `fn bench_build_args() -> Vec<String>` in `main.rs`, returning `["bench", "--workspace", "--exclude", "engine_bench_regress", "--no-run"]`; `Command::BenchBuild` uses it. `const REGRESS_PACKAGE: &str = "engine_bench_regress"` in `main.rs`, `pub(crate)` so `coverage.rs` and Step 3 use it.
- **Test first:** update the existing argument-list tests in `main.rs` and `coverage.rs` so `bench_args(None)`, `bench_args(Some("x"))`, `bench_build_args()` and coverage's `test_args()` each contain `--exclude engine_bench_regress`, and add `bench_smoke_includes_the_regress_benchmarks` (its arguments contain no `--exclude engine_bench_regress`). Run `cargo xtask test`: the updated tests fail.
- **Change:** the argument lists; then the crate. `src/lib.rs` follows `benches/src/lib.rs` (the `#![forbid(unsafe_code)]` line, the same lint allowances, crate docs saying how to run it). Each bench file has one `library_benchmark_group!` per subject (`pool`, `allocator`, `dense`; `prng`) and one `#[library_benchmark]` function per spec §4.2 ID, named after the ID, with its input built in a `setup` function from a fixed seed (`Prng::from_seed`), and the result passed through `core::hint::black_box`.
- **Verify:** `cargo xtask test` passes; `cargo xtask bench-smoke` on macOS passes and lists all 16 IDs; `cargo xtask bench-build` builds no `engine_bench_regress` target (read the output); `cargo xtask deny` and `cargo xtask check-unsafe` pass; in the container, `cargo bench -p engine_bench_regress` runs every benchmark and each reports at least 10,000 instructions (raise a loop count and rename the ID where not; record any change in §4).
- **Commit:** `feat(bench-regress): Gungraun benchmarks for handle and PRNG hot paths (M2.4)`

### Step 3: `bench-regress` helpers and `gungraun-version` (R4, R5, R6, R7, R16, R17)

- **Files:** new `xtask/src/bench_regress.rs`; `xtask/src/main.rs` (`mod bench_regress;`, `Command::BenchRegress(bench_regress::Options)`, `Command::GungraunVersion`, parse arms `"bench-regress"` and `"gungraun-version"`, the usage string).
- **Interfaces produced** (all in `bench_regress.rs`):
  - `pub(crate) struct Options { pub(crate) base: Option<String>, pub(crate) report_only: bool }` (derive `Debug, PartialEq, Eq`), and `pub(crate) fn parse_flags(flags: &[&str]) -> Result<Options, String>`;
  - `fn lock_version(cargo_lock: &str, package: &str) -> Option<String>`: the `version` of the `[[package]]` whose `name` is exactly `package`;
  - `fn valgrind_version(output: &str) -> Option<(u32, u32)>` and `fn runner_version(output: &str) -> Option<String>`, against the probe's verbatim output (§1 item 9);
  - `const MIN_VALGRIND: (u32, u32) = (3, 20)`;
  - `const RELEVANT: &[&str]` holding the spec §4.4 step 2 patterns, and `fn is_relevant(path: &str) -> bool` (a pattern ending in `/**` matches that directory's contents; any other pattern matches the path exactly);
  - `fn target_dir(cargo_target_dir: Option<&str>, repo: &Path) -> PathBuf`;
  - `enum BaseSkip { NoCrate, VersionChanged { base: String, head: String } }` and `fn base_skip(base_has_crate: bool, base_lock: &str, head_lock: &str) -> Option<BaseSkip>`;
  - `pub(crate) fn print_gungraun_version() -> Result<(), String>`: reads `Cargo.lock`, prints `lock_version(.., "gungraun")`, fails if absent.
- **Test first** (unit tests in `bench_regress.rs` and `main.rs`):
  - `parses_base_and_report_only`; `rejects_unknown_missing_and_repeated_flags` (`--bogus`, `--base` alone, `--base a --base b`, `--report-only --report-only`);
  - `lock_version_matches_the_exact_package_name` (a lock with `gungraun-runner 0.19.0`, `gungraun-macros 0.20.0` and `gungraun 0.20.0` gives `0.20.0`; one without `gungraun` gives `None`);
  - `parses_valgrind_and_runner_versions` (the probe's outputs; `valgrind-3.19.0` is below `MIN_VALGRIND`, `valgrind-3.24.0` is not; garbage gives `None`);
  - `relevance_table`: relevant: `libs/engine_handle/src/pool.rs`, `bench-regress/benches/prng.rs`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/config.toml`, `xtask/src/bench_regress.rs`, `xtask/src/bench_regress/report.rs`, `.github/workflows/bench-regress.yml`; not relevant: `docs/benchmarks.md`, `README.md`, `xtask/src/mutants.rs`, `benches/benches/prng.rs`, `libs` (no slash), `examples/fixed_step_sim/src/main.rs`;
  - `target_dir_prefers_cargo_target_dir`;
  - `base_skip_cases`: no crate gives `NoCrate`; equal versions give `None`; `0.20.0` against `0.21.0` gives `VersionChanged`; a base lock without `gungraun` but with the crate gives `VersionChanged` with base `none`;
  - in `main.rs`: `parse(&["bench-regress", "--base", "HEAD^1", "--report-only"])` and `parse(&["gungraun-version"])` parse; `ci_steps(false)` and `ci_steps(true)` contain neither (R12).

  Run `cargo xtask test`: it fails to compile.
- **Change:** the helpers above. `Command::BenchRegress` dispatches to a stub `bench_regress::bench_regress(&Options) -> Result<(), String>` that returns `Err("not implemented yet")`; Step 5 replaces it.
- **Verify:** `cargo xtask test` passes; `cargo xtask gungraun-version` prints `0.20.0`; the container's `install-tools.sh` now installs the runner without a warning.
- **Commit:** `feat(xtask): bench-regress flags, version checks and relevance (M2.4)`

### Step 4: reading Cargo's JSON and Gungraun's summaries; verdict and rendering (R8, R9, R10, R11, R17)

- **Files:** new `xtask/src/bench_regress/cargo_json.rs`, `xtask/src/bench_regress/report.rs`, `xtask/src/bench_regress/fixtures/*.json` (from the probe); `xtask/src/bench_regress.rs` (`mod cargo_json; mod report;`).
- **Interfaces produced:**
  - `cargo_json.rs`: `pub(super) struct BenchExe { pub(super) name: String, pub(super) path: PathBuf }`; `pub(super) fn bench_executables(messages: &str) -> Result<Vec<BenchExe>, String>`, sorted by `name`, error when there are none;
  - `report.rs`: `pub(super) struct Row { pub(super) target: String, pub(super) id: String, pub(super) base_ir: Option<u64>, pub(super) head_ir: u64, pub(super) regressed: bool }`; `pub(super) fn read_summary(json: &str) -> Result<Row, String>` (the JSON paths recorded by the probe); `pub(super) enum Verdict { Passed, Regressed, Failed(String) }`; `pub(super) fn classify(exits: &[(String, Option<i32>)]) -> Verdict`; `pub(super) fn finish(verdict: &Verdict, report_only: bool, rows: &[Row]) -> Result<String, String>` (`Ok` carries the closing message, `Err` the failure message); `pub(super) fn render_table(rows: &[Row]) -> String`; `pub(super) fn step_summary(base: &str, head: &str, verdict_line: &str, rows: &[Row], report_only: bool) -> String`.
- **Test first:**
  - `bench_executables_picks_bench_artifacts`: three captured-shape messages (a `lib` artifact, a `build-script` artifact, two `bench` artifacts out of name order) give the two benches, sorted; `no_bench_executable_is_an_error`; `a_non_json_line_is_skipped` (Cargo interleaves a final `build-finished` message and may print plain text).
  - `reads_the_fixtures`: the regressed fixture has `regressed: true` and both counts; the new one has `base_ir: None`; a malformed string gives `Err`.
  - `classify_cases`: `[0, 0]` gives `Passed`; `[3, 0]` and `[3, 3]` give `Regressed`; `[3, 101]` and `[3, None]` give `Failed`; and `a_crash_beside_a_regression_is_a_failure` asserts that `finish(&classify([3, None]), true, ..)` is `Err`.
  - `finish_cases`: every row of spec §4.5. `Regressed` without `--report-only` is `Err`, contains each regressed ID and the words `perf-regression-accepted`; with `--report-only` it is `Ok` and contains `REGRESSION ACCEPTED BY LABEL` and each ID; `Regressed` with no regressed row says the names are unavailable.
  - `table_orders_by_target_then_id` and marks regressed rows; a new benchmark shows `new`; the change is printed with one decimal (`+5.3%`).
  - `step_summary_has_shas_verdict_and_accepted_list`.

  Run `cargo xtask test`: it fails to compile.
- **Change:** the two modules. `classify` follows spec §4.5 exactly; the change is `(head - base) / base * 100` in `f64`, rounded for display only.
- **Verify:** `cargo xtask test` passes; `cargo xtask clippy` passes; `grep -n "Gungraun result" xtask/src` finds nothing (R10).
- **Commit:** `feat(xtask): bench-regress verdicts and reports from JSON (M2.4)`

### Step 5: the `bench-regress` command (R5, R6, R7, R8, R9, R11, R12)

- **Files:** `xtask/src/bench_regress.rs`.
- **Interfaces consumed:** everything from Steps 3 and 4.
- **Interfaces produced:** `pub(crate) fn bench_regress(options: &Options) -> Result<(), String>` (replaces the stub); private `fn build_args() -> Vec<String>` (`["bench", "-p", "engine_bench_regress", "--no-run", "--message-format=json"]`), `fn base_args() -> Vec<String>` and `fn head_args() -> Vec<String>` (the spec §4.4 Gungraun arguments, each list starting with `--bench`), and `fn run_targets(package_dir: &Path, exes: &[BenchExe], args: &[String], env: &[(&str, String)]) -> Vec<(String, Option<i32>)>`.
- **Test first:** `base_and_head_arguments` (exact lists from spec §4.4; the head list contains `--callgrind-limits=ir=5%` and `--save-summary=json`, neither contains `--regression-fail-fast`); `build_arguments`. Run `cargo xtask test`: they fail.
- **Change:** the steps of spec §4.4 in order: preflight (`std::env::consts::OS`, the versions); resolve the base (`git rev-parse --verify <rev>^{commit}`, default from `git merge-base origin/main HEAD`); relevance (`git diff --name-only <base>`); clean `<target>/bench-regress/home`; base skip (`git cat-file -e`, `git show <base>:Cargo.lock`); base run in a worktree with a guard value whose `Drop` runs `git worktree remove --force` (so every exit path, including `?`, removes it); delete old `summary.json` files; head run; read every `summary.json` under the home (a missing or unreadable file only loses names, spec §4.5); print the table; append `step_summary` to the file named by `GITHUB_STEP_SUMMARY` when set; return `finish(..)`. The environment for the executables is the set the probe recorded, plus `GUNGRAUN_HOME`.
- **Verify:** `cargo xtask test` passes. On macOS, `cargo xtask bench-regress` fails at once with the devcontainer message. In the container, `cargo xtask bench-regress --base HEAD` runs both sides and reports 0.0% for every benchmark.
- **Commit:** `feat(xtask): bench-regress compares base and head instruction counts (M2.4)`

### Step 6: the workflow and the repository settings (R13, R14, R15)

- **Files:** new `.github/workflows/bench-regress.yml`; `infra/github/index.ts`.
- **Test first:** none runnable before the file exists; `cargo xtask check-workflows` is the check after.
- **Change:** the workflow exactly as spec §4.7. Pin `gungraun/setup-gungraun` to the commit of its current `v1` release (`gh api repos/gungraun/setup-gungraun/git/ref/tags/<tag>`; dereference an annotated tag), with the tag in a trailing comment, like the other actions. The `rust-cache` step uses `key: bench-regress`. The last step:

  ```yaml
  - run: |
      if [ "$ACCEPTED" = "true" ]; then
        cargo xtask bench-regress --base HEAD^1 --report-only
      else
        cargo xtask bench-regress --base HEAD^1
      fi
    env:
      ACCEPTED: ${{ contains(github.event.pull_request.labels.*.name, 'perf-regression-accepted') }}
  ```

  In `index.ts`: the label of spec §4.9 in `labels`, and `"bench-regress"` in `requiredChecks` with a comment that it lives in `bench-regress.yml`, not `ci.yml`.
- **Verify:** `cargo xtask check-workflows` passes; `grep -n "if:" .github/workflows/bench-regress.yml` finds nothing; `pnpm -C infra/github exec tsc --noEmit` passes (if the infra package has no such script, `npx tsc -p infra/github --noEmit`). Do not run `pulumi`.
- **Commit:** `ci: required bench-regress job and perf-regression-accepted label (M2.4)`

### Step 7: documentation (R19)

- **Files:** `CONTRIBUTING.md`, `docs/maintainers.md`, `docs/benchmarks.md`, `README.md`, `AGENTS.md`, `CHANGELOG.md`, `docs/design/m0-m1-foundations/spec.md` (§8 amendment note), `docs/roadmap.md` (row to `In progress`, plan link).
- **Test first:** `grep -n "planned for M2 (chunk M2.4)" docs/maintainers.md` finds the line to replace; `grep -n "bench-regress" CONTRIBUTING.md README.md AGENTS.md` finds nothing.
- **Change:** the content listed in spec §4.10, item by item. The CI time per pull request in `maintainers.md` is left as `(measured in the pull request)` until Step 8 fills it in.
- **Verify:** the two greps now find the new text; `cargo xtask typos` passes; every relative link added resolves (`ls` each target).
- **Commit:** `docs: instruction-count gate (M2.4)`

### Step 8: manual verification and the pull request (R7, R18)

All runs in the container unless marked; paste each command and its relevant output into the pull request description.

1. Noise floor: `cargo xtask bench-regress --base HEAD` reports 0.0% everywhere. Record each benchmark's instruction count.
2. Regression: on a throwaway local commit adding a loop of `black_box` work to `Prng::next_u64`, `cargo xtask bench-regress --base HEAD~1` exits 1 and names `next_u64_10k` (and any other benchmark that calls it); with `--report-only` it exits 0 with `REGRESSION ACCEPTED BY LABEL`. Drop the commit.
3. Broken head: a throwaway commit that panics in one benchmark exits 1 with `--report-only`, and the other target still ran (its rows are in the table). Drop the commit.
4. Worktree cleanup: a throwaway base commit that fails to compile `bench-regress` makes the command fail, and `git worktree list` shows only the main tree afterwards.
5. Skips: `--base` at a commit before this branch reports every benchmark as new, even straight after run 1 (review focus 2); a throwaway commit that only edits `README.md` gives the skip message without building; for "not compared", make a throwaway commit on top of `HEAD` that only edits `gungraun`'s version in `Cargo.lock` by hand (never built), note its hash `C`, reset back to `HEAD`, and run `--base C`: the base is skipped with "not compared: gungraun changed from <edited> to 0.20.0" and the head runs normally.
6. Fill in the CI time in `maintainers.md` after item 8, and commit it with `docs: record bench-regress CI time (M2.4)`.
7. **Ask the maintainer**, then push the branch and open the pull request (title `feat(xtask): instruction-count regression gate through Gungraun (M2.4)`, saying an assistant was used). The `bench-regress` job must be green, reporting every benchmark as new.
8. **Ask the maintainer**, then push a temporary commit that changes the workflow's `--base HEAD^1` to `--base HEAD`; record that the x86_64 run reports 0.0% everywhere and the job's duration; revert it with a second commit before review.

### Step 9: reviews and status

- Run the eight reviews of CONTRIBUTING.md and list them in the pull request; fix each finding or say why not.
- Set the spec's status to `Implemented (PR #n, date)`, this plan's to `Historical: executed in PR #n (date); the spec and the code are authoritative`, and the roadmap row's spec and plan links; commit as `docs: mark M2.4 implemented (M2.4)`.
- After merge, with the maintainer's go-ahead: `pulumi preview` then `pulumi up` in `infra/github`; then the throwaway draft pull request with the `Prng::next_u64` slowdown, blocked, then green once labelled, then closed; roadmap row to `Done (PR #n)` and M2 to `Done` (spec §9).

## 3. Final verification

- `cargo xtask ci` passes on macOS and in the container; CI is green on all legs, including `bench-regress`.
- Every box of spec §9 is ticked, with evidence in the pull request, except the post-merge ones, which wait for the maintainer.

## 4. Deviations

The probe (§1, Step 1) ran on 2026-11-13 in the trixie container (aarch64, Valgrind 3.24.0, `gungraun-runner 0.20.0`) and on the macOS host.

### 4.1 Probe findings

1. **`cargo xtask deny`** passes with `gungraun` added (R21).
2. **Windows.** `cargo clippy -p engine_bench_regress --benches --target x86_64-pc-windows-msvc` passes on the host, so the `cfg(unix)` fallback of spec §4.2 is not needed.
3. **Test mode.** `cargo bench -p engine_bench_regress -- --test` runs on macOS without Valgrind and exits 0, but it needs `gungraun-runner` on the `PATH` (without it: "Failed to run benchmarks: No such file or directory"), and it prints nothing on success, not each benchmark. It does run each benchmark function: a `panic!` in one fails the command. See §4.2 below for what follows.
4. **Flags.** `--save-baseline=base`, `--baseline=base`, `--callgrind-limits=ir=5%`, `--save-summary=json` and `--callgrind-args=--cache-sim=yes` are accepted with these spellings, after `--bench`.
5. **Direct execution.** The executable from `cargo bench --no-run --message-format=json` (the `compiler-artifact` message whose `target.kind` is `["bench"]`; `target.name` and `executable`), started from `bench-regress/` with `--bench <args>`, runs the same benchmarks as `cargo bench`. It needs no environment beyond `PATH`, `HOME` and `GUNGRAUN_HOME`: it also ran with `env -i PATH=$PATH HOME=$HOME GUNGRAUN_HOME=...` alone, without `CARGO_MANIFEST_DIR`, `CARGO_PKG_NAME`, `CARGO_PKG_VERSION` or `CARGO`. Step 5 therefore inherits the environment and adds only `GUNGRAUN_HOME`.
6. **Baseline across a worktree.** A baseline saved by a run from a `git worktree` (own `CARGO_TARGET_DIR`, shared `GUNGRAUN_HOME`) is found by a run from the repository: the output shows the base count (`Baselines: |base`, `120010|120010 (No change)`), not `N/A`.
7. **Exit codes.** A deliberate +10% slowdown makes the executable exit 3 (`Regressed. ... 1 regressed`). A benchmark that panics makes it exit 1 (`Error running 'callgrind': Exit code was: '101'`). Neither is 0 or 3 for the panic, as spec §4.5 assumes.
8. **`summary.json`.** It lands at `<GUNGRAUN_HOME>/<bench file>/<group>/<function>.<id>/summary.json` (for example `prng/prng/next_u64_10k.fixed/`). The fields used: `module_path` (`prng::prng::next_u64_10k`: file, group, function), `function_name`, `id`, `package_dir`; `profiles[0].data.total.metrics.Ir.values.new` and `.old` (`old` is absent for a benchmark with no baseline); `profiles[0].data.total.regressions`, a non-empty array exactly when the limit was exceeded. Fixtures: `xtask/src/bench_regress/fixtures/{regressed,unchanged,new}.json` (Step 4).
9. **Versions.** `gungraun-runner --version` prints `gungraun-runner 0.20.0`; `valgrind --version` prints `valgrind-3.24.0`.
10. **Setup is not counted.** `pool_iter_10k` builds a 10,000-element pool in `setup` (about 150,000 instructions) and reports about 11,000.

### 4.2 Rulings and deviations

- **Node feature (Step 1).** On trixie the `claude-code` devcontainer feature fails: Debian's `nodejs` package has no `npm`. Step 1 also adds `ghcr.io/devcontainers/features/node:1` to `devcontainer.json` and its entry to `devcontainer-lock.json`. The container is built with the devcontainer CLI (`npx @devcontainers/cli@0.89.0 up --workspace-folder . --remove-existing-container`); plain `docker compose up` does not apply features, so it has no Rust.
- **Test mode needs the runner (Step 3, Step 6).** Because of probe item 3, `bench-smoke` (in `ci` and in the `bench-build` job) needs `gungraun-runner`. Step 3 makes `bench-smoke` fail early with the install line when the runner is missing; Step 6 installs the runner in the `bench-build` job of `ci.yml`. Spec §4.3 stays true (no Valgrind), but "prints each benchmark" does not: Step 2's check that all 16 benchmarks work is the container run below, plus the panic check of item 3.
- **Where the work runs (Step 2).** Callgrind counts a benchmark between the entry and the return of Gungraun's wrapper, and the default toggle `*::__gungraun_wrapper_mod::*` matches every symbol with that path in it. Two effects, both measured: a loop the optimizer inlines into the wrapper was counted as 14 instructions (1,000 allocations); a generic helper taking a closure, whose symbol carries the wrapper's path, turned collection off (1 instruction). The benchmark functions therefore call a plain `#[inline(never)]` `<name>_work` function. `bench-regress/benches/handle.rs` documents it.
- **`pool_iter_1k` is now `pool_iter_10k`.** Summing 1,000 values is vectorized and counted 1,964 instructions, below the 10,000 floor (spec §4.2), so the pool holds 10,000 values.
- **IDs.** Gungraun prints each benchmark as `<file>::<group>::<function> fixed:(<setup>)`; every benchmark has the single case `fixed`.
- **Instruction counts of the first run** (x86_64 counts will differ; aarch64 container): `pool_insert_1k` 55,040; `pool_get_seq_1k` 15,937; `pool_get_random_1k` 15,937; `pool_remove_1k` 64,220; `pool_iter_10k` 10,966; `pool_churn_1k` 62,764; `allocator_allocate_1k` 21,044; `allocator_free_realloc_1k` 26,267; `allocator_is_alive_1k` 13,991; `dense_allocate_1k` 34,046; `dense_free_1k` 45,232; `next_u64_10k` 120,020; `range_u32_10k` 170,023; `range_f64_10k` 170,021; `shuffle_1k` 24,012; `from_seed_1k` 19,289.
- **CI noise floor (Step 8.8).** Changing `--base HEAD^1` to `--base HEAD` would skip the run (the merge checkout equals `HEAD`, so nothing relevant differs). A temporary commit instead used `--base HEAD^2^1` with `fetch-depth: 3` after a comment-only edit in `bench-regress/src/lib.rs`; the x86_64 run reported `+0.0%` for all 16 benchmarks, and the job took 3m16s (the first run, whose base has no crate, 2m48s). The commit was reverted. `timeout-minutes: 30` stays.
- **Local noise floor (Step 8.1)** used `--base 8425fad`, for the same reason: with a clean tree `--base HEAD` skips.
- **`RUSTUP_TOOLCHAIN` (found in review).** `cargo xtask` runs under rustup's proxy, which exports `RUSTUP_TOOLCHAIN`; it outranked the base worktree's `rust-toolchain.toml`. The base's `rustup` and `cargo` commands now remove it (`base_command`).
- **Container git.** In a CLI-built container git needs `safe.directory /workspace`; without it every git call failed with "dubious ownership" (the command now shows that message instead of misreading it).
- **Tools missing in the container.** `cargo-hack` and `typos-cli` are not installed by `install-tools.sh` (before this change too), so `cargo xtask ci` stops there in a fresh container. They were installed by hand for the runs below; adding them to `install-tools.sh` is outside M2.4.
