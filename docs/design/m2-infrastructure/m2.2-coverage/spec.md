# M2.2: Coverage

- **Status:** Draft (2026-11-12)
- **Milestone:** M2 (process and verification infrastructure) · **Depends on:** M2.0 · **Decides:** no ADR
- **Estimated size:** about 400 changed lines, excluding tests, lockfiles and generated files (`xtask` about 310, workflow and devcontainer about 30, docs about 60)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds line and function coverage of the workspace's library and example code: a `cargo xtask coverage` command built on `cargo-llvm-cov`, and a report-only `coverage` job in `ci.yml` that writes a per-crate table to the job summary and uploads the full report. It comes now because the M2 exit criterion "coverage report in CI" depends on it, and the M2.3 mutation triage needs it to tell code no test runs apart from code whose tests do not notice a change. At the end, `cargo xtask coverage --open` shows the HTML report locally, and every pull request shows the per-crate table on its `coverage` job.

## 2. Goals and non-goals

**Goals**

- One local command that measures coverage with the same tests and features as CI's `test` job (minus doctests) and writes HTML, lcov and a Markdown summary.
- A CI job on every push and pull request that shows a per-crate summary in the GitHub job summary and uploads the HTML and lcov reports as an artifact.
- Summary numbers that count only engine code: library and example sources, not integration tests, benches, `xtask`, or the `#[cfg(test)]` unit-test items inside `src/` files (§4.5).
- A recorded first measurement (in the pull request) that M2.3 can start from.

**Non-goals**

- No threshold, ratchet or failing on a drop. The roadmap says "no threshold gate", and a number that gates invites tests written for the number.
- No third-party service (Codecov, Coveralls): the report stays on GitHub, and the token stays read-only (decided 2026-11-12, §10).
- No pull request comments or line annotations. Both need a token with write access, which `check-workflows` forbids.
- No doctests and no branch coverage: both need a nightly toolchain, and coverage runs on the pinned stable (decided 2026-11-12, §7).
- No region coverage in the summary table. The lcov format the table is computed from has no regions (§4.5); the HTML report still shows them, unfiltered.
- No filtering of `#[cfg(test)]` items in the HTML and lcov reports: they are `cargo-llvm-cov`'s output, unchanged. Only the summary table is filtered.
- No coverage of the fuzz corpus (`cargo fuzz coverage`); a later chunk may add it.
- No coverage on wasm, `no_std` targets, macOS or Windows. CI measures on Ubuntu only; the command is not blocked elsewhere, but only Linux and macOS are verified.
- No `#[coverage(off)]` or other exclusion attributes in library code (R10).
- No step in `cargo xtask ci` (§4.4).
- No change to any library crate, public API, frozen format or `rust-toolchain.toml`.

## 3. Guide-level explanation

To measure coverage and open the report:

```sh
cargo xtask coverage --open
```

The first run needs two things, and the command prints the line for whichever is missing:

```sh
cargo install --locked cargo-llvm-cov
rustup component add llvm-tools-preview
```

The command runs the workspace tests under instrumentation (a separate build in `target/llvm-cov-target/`, so the first run takes about as long as a clean build), then prints a table of this shape (one row per crate; real numbers come from the first run, R15) and writes the reports to `target/llvm-cov/`:

```text
## Coverage

Excluded: doctests (not run); tests/, benches/ and tooling sources; 1234 instrumented lines in #[cfg(test)] items.

| Crate         | Lines              | Functions          |
|---------------|--------------------|--------------------|
| engine        | covered/total (p%) | covered/total (p%) |
| engine_handle | covered/total (p%) | covered/total (p%) |
| ...           |                    |                    |
| Total         | covered/total (p%) | covered/total (p%) |
```

- `target/llvm-cov/html/index.html`: the annotated source, the place to look when triaging. It shows every file `cargo-llvm-cov` measured, including the unit-test modules inside `src/` files, so its percentages are higher than the table's.
- `target/llvm-cov/lcov.info`: for editor plugins that show covered lines in the gutter.
- `target/llvm-cov/summary.md`: the table above.

What is measured: the tests `cargo xtask test --include-ignored` runs, including the `#[ignore]`d statistical tests, but not doctests. Code that only a doctest exercises therefore shows as uncovered. That is deliberate: doctests are documentation examples, and a function with no other test is worth knowing about.

What the table counts: lines and functions in `libs/*/src/` and `examples/*/src/`, minus items marked `#[cfg(test)]` (in practice the `mod tests` block at the end of a file). Code under `#[cfg(any(test, feature = "test-invariants"))]` is counted, because it ships with a feature.

In CI, the `coverage` job's summary page shows the same table, and its `coverage-report` artifact holds the HTML and lcov reports for 30 days. The job fails only if a test fails. There is no target percentage; CONTRIBUTING.md says to read the report for untested behaviour, not to raise the number.

## 4. Reference-level design

### 4.1 Layout

```text
xtask/src/coverage.rs    the `coverage` command: flag parsing, the cargo-llvm-cov command lines,
                         the ignore regex, and the summary (lcov parsing, test-item filtering,
                         rendering)
xtask/src/main.rs        `Command::Coverage`, dispatch, help text
.github/workflows/ci.yml the `coverage` job
.devcontainer/install-tools.sh   installs cargo-llvm-cov and llvm-tools-preview (optional block)
```

If `coverage.rs` grows past about 400 lines with its tests, the plan MAY split the summary into `xtask/src/coverage/summary.rs`.

### 4.2 `cargo xtask coverage`

```text
cargo xtask coverage [--open]
```

The command MUST, in order:

1. Check that `cargo llvm-cov` works (`require_cargo_subcommand("llvm-cov", "cargo install --locked cargo-llvm-cov")`). It SHOULD also check that the `llvm-tools-preview` component is installed for the active toolchain (`rustup component list --installed`), and print `rustup component add llvm-tools-preview` when it is not. If `rustup` itself is not available, the component check is skipped and `cargo-llvm-cov`'s own error is shown.
2. Run `cargo llvm-cov clean --workspace`, so profiles from an earlier run cannot mix into this one.
3. Run `cargo llvm-cov nextest --workspace --all-features --exclude xtask --exclude engine_benches --run-ignored all --no-report`. These are the packages, features and tests of `cargo xtask test --include-ignored` (§4.3), minus doctests (which `cargo-llvm-cov` supports only on nightly) and minus the two packages that are tooling, not engine code. The tests run in the dev profile, not release: release optimisations merge and drop regions, which makes line attribution less accurate.
4. Write two reports from the collected profiles, each with the same `--ignore-filename-regex` (§4.3):
   - `cargo llvm-cov report --html --output-dir target/llvm-cov` (writes `target/llvm-cov/html/`)
   - `cargo llvm-cov report --lcov --output-path target/llvm-cov/lcov.info`
5. Compute the summary from `lcov.info` and the source files it names (§4.5), write it to `target/llvm-cov/summary.md`, and print it.
6. With `--open`, open `target/llvm-cov/html/index.html` (through `cargo llvm-cov report --html --open`, or the platform opener; the plan picks).

Any failing step MUST make the command exit non-zero with the failing command line or the summary error (the existing `run` helper does this for commands). An unknown flag MUST be an error, following the `only` style in `xtask/src/main.rs`.

The command line arguments and the regex are built by pure functions, so unit tests can check them without running `cargo-llvm-cov` (R7). The workspace root is `Path::new(env!("CARGO_MANIFEST_DIR")).parent()`, as `xtask/src/fuzz.rs` already does for `fuzz/`, passed through `std::fs::canonicalize` once, so it matches the resolved paths LLVM writes even when the checkout is reached through a symlink (such as macOS's `/tmp` to `/private/tmp`).

### 4.3 What is measured

- **Tests:** every test `cargo nextest run --workspace --all-features --run-ignored all` runs, in the packages listed below. The `#[ignore]`d statistical tests in `engine_random` are included: they take about 0.02 s in a debug build (measured 2026-11-12), so excluding them would save nothing and make the coverage run differ from the `test` job for no reason.
- **Packages built and tested:** every workspace member except `xtask` and `engine_benches`.
- **Files reported:** only files under `libs/<crate>/src/` and `examples/<example>/src/`. The `--ignore-filename-regex` MUST exclude the workspace's `xtask/` and `benches/` directories and every `tests/` and `benches/` directory directly inside a `libs/<crate>/` or `examples/<example>/` directory. Dependency sources are already excluded by `cargo-llvm-cov`. The regex MUST be anchored to the workspace root's absolute path (with the root's regex metacharacters escaped), so a checkout under a directory named, say, `tests` or `benches` does not exclude everything. It is built by one function, `ignore_regex(root)`, shared by both report commands. LLVM's regex is POSIX-extended with no lookahead, so the regex lists what to exclude rather than what to keep; the summary's path check (§4.5) catches anything it misses.
- **Library code MUST NOT use `cfg(coverage)` or `cfg(coverage_nightly)`** (which `cargo-llvm-cov` sets), so the instrumented build compiles the same code as the normal one. This is what makes it safe to leave the coverage build out of `cargo xtask ci` (§4.4).

### 4.4 Relationship to `cargo xtask ci`

`coverage` MUST NOT be a step of `cargo xtask ci`, quick or full. `ci` runs the checks that pass or fail (M0+M1 spec §4.8: "every gating job"); the coverage job has no threshold, so it can only fail when a test fails, which `ci` already runs. Adding it would cost a second full instrumented build of the workspace and the missing-tool skip logic that `fuzz-build` needed, for no additional pass/fail signal. The same reasoning already keeps the nightly fuzz run and the planned `bench-regress` out of `ci`.

The description of `ci` in `AGENTS.md`, `README.md` and `CONTRIBUTING.md` ("everything CI runs") MUST change to say that `ci` runs every check CI gates on, and that report-only jobs (the coverage job, the nightly fuzz run) run separately. The old wording was already inexact: `ci` does not run the nightly jobs or the macOS and Windows legs.

### 4.5 The summary table

Stable Rust cannot exclude code from instrumentation (`#[coverage(off)]` is nightly-only), and this repository keeps its unit tests in `#[cfg(test)] mod tests` blocks inside `src/` files: roughly 2,300 of 8,100 `src` lines on 2026-11-12. Counted as library code, those nearly fully executed lines would push every percentage up. The summary therefore removes them itself, from the lcov data, using the `syn` parser `xtask` already depends on (with `span-locations`).

**Reading lcov.** The renderer MUST read these records from `lcov.info` and ignore every other record type (such as `BRDA`, `LF`, `LH`, `FNF`, `FNH`), so a newer `cargo-llvm-cov` adding records does not break it:

- `SF:<path>` starts a file and `end_of_record` ends it;
- `DA:<line>,<hits>` is an instrumented line;
- `FN:<line>[,<end line>],<name>` declares a function starting at `<line>`;
- `FNDA:<hits>,<name>` gives a function's hit count.

A malformed record of one of these four kinds (a missing field, a non-numeric line or count, a `DA` outside an `SF`) MUST be an error naming the record and its line in `lcov.info`.

**Finding test items.** For each `SF` file the renderer MUST parse the source with `syn` and collect the line range (first attribute to closing brace, inclusive) of every item, at any nesting depth (module items, `impl` and trait items), that carries either:

- `#[test]`, or
- `#[cfg(P)]` where `P` can only be true when `test` is set: `P` is `test`, or `all(...)` with at least one argument that is itself such a predicate. `any(test, ...)` and `not(test)` do not qualify, so `#[cfg(any(test, feature = "test-invariants"))]` code is counted.

An out-of-line `#[cfg(test)] mod name;` declaration MUST be an error naming the file, because the filter does not follow module files; test modules stay inline (none exist out of line today). A source file that does not parse MUST be an error naming it.

**Counting.** Per file, after removing everything inside a test-item range:

- **Lines:** the `DA` lines; covered when `hits > 0`.
- **Functions:** the distinct `FN` start lines (several `FN` records on one line, such as generic instantiations, count as one function); covered when any `FNDA` for a name declared on that line has `hits > 0`. A function whose start line is inside a test-item range is removed.

The number of `DA` lines removed, summed over all files, MUST be reported in the heading line, so the size of the adjustment is visible.

**Grouping and output.** The renderer:

- MUST map each `SF` path to a crate: strip the workspace root, then take the directory name after `libs/` or `examples/` when the next component is `src` (`libs/engine_handle/src/pool.rs` is `engine_handle`). Today every such directory name equals its package name. A path that does not match MUST be an error naming it, because silently dropping a file would make the totals wrong. An `lcov.info` with no counted file at all MUST be an error too: it means the ignore regex or the root is wrong, and an empty table would look like a result.
- MUST sum lines and functions per crate, and add a `Total` row computed from the per-crate sums.
- MUST order rows by crate name (a `BTreeMap`), with `Total` last, so the same input always gives the same table.
- MUST print each cell as `covered/total (p.p%)`, with the percentage rounded to one decimal using integer arithmetic (per-mille, half up), and `—` when the total is 0.
- MUST output a GitHub-flavoured Markdown table under a `## Coverage` heading, preceded by one line naming what is excluded: doctests; `tests/`, `benches/` and tooling sources; and the number of instrumented lines in `#[cfg(test)]` items.

The lcov parsing, the test-item finder and the renderer are pure functions over strings (the caller reads the files), so they are unit-tested without running `cargo-llvm-cov` or touching the disk. No new dependency is added.

### 4.6 CI

A new `coverage` job in `.github/workflows/ci.yml`, triggered like the other jobs (every push, and pull requests to `main`):

```yaml
coverage:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@<pinned>            # persist-credentials: false
    - run: rustup show
    - run: rustup component add llvm-tools-preview
    - uses: Swatinem/rust-cache@<pinned>
    - uses: taiki-e/install-action@<pinned>      # tool: cargo-llvm-cov,cargo-nextest
    - run: cargo xtask coverage
    - run: cat target/llvm-cov/summary.md >> "$GITHUB_STEP_SUMMARY"
    - uses: actions/upload-artifact@<pinned>     # name: coverage-report; retention-days: 30
        # path: target/llvm-cov/html and target/llvm-cov/lcov.info
```

- The pins are the hashes already used in `ci.yml`. The workflow keeps its top-level `permissions: contents: read`, so `cargo xtask check-workflows` keeps passing.
- No `run:` step interpolates an expression from the event (`${{ github.* }}`), so no untrusted input reaches a shell.
- The job is not added to the required checks in `infra/github` (R13). It is not `continue-on-error`: if a test fails under instrumentation, the job is red, which is correct.
- The artifact is uploaded only when `cargo xtask coverage` succeeded (the default `if: success()`), since a failed run has no complete report.

### 4.7 Devcontainer and docs

- `.devcontainer/install-tools.sh` installs `cargo-llvm-cov` and adds `llvm-tools-preview`, in an optional block like the fuzzing one: a failure prints a warning and does not abort the container setup.
- `CONTRIBUTING.md` gets a short "Coverage" section: how to run it, where the reports are, what is measured and excluded and why (doctests, tooling, `#[cfg(test)]` items in the table but not in the HTML), that there is no threshold, that the number is not a target, that unit tests stay in inline `#[cfg(test)]` modules, and the `cfg(coverage)` rule (§4.3). The "Setting up" tool list gains `cargo-llvm-cov`.
- `README.md` lists `coverage [--open]` among the commands and `cargo-llvm-cov` among the tools with install hints.
- `AGENTS.md`, `README.md` and `CONTRIBUTING.md` change the `ci` description (§4.4).
- `docs/roadmap.md` M2.2 goes through the statuses in the M2.0 process; `CHANGELOG.md` gets one line.

## 5. Requirements

- **M2.2-R1** `cargo xtask coverage` MUST run the workspace tests (`--all-features`, ignored tests included, doctests excluded, `xtask` and `engine_benches` excluded) under `cargo-llvm-cov`, after clearing earlier profiles.
- **M2.2-R2** `cargo xtask coverage` MUST write `target/llvm-cov/html/`, `target/llvm-cov/lcov.info` and `target/llvm-cov/summary.md`, and print the summary.
- **M2.2-R3** The reports MUST count only files under `libs/*/src/` and `examples/*/src/`, using an ignore regex anchored to the workspace root.
- **M2.2-R4** The summary MUST have one row per crate with covered/total and percentage for lines and functions, a `Total` row last, rows ordered by crate name, and `—` for a zero total.
- **M2.2-R5** The summary MUST fail, naming the cause, on: a path outside the counted patterns, an `lcov.info` with no counted file, a malformed `SF`/`DA`/`FN`/`FNDA` record, a source file that does not parse, and an out-of-line `#[cfg(test)] mod` declaration. It MUST ignore other lcov record types.
- **M2.2-R6** `cargo xtask coverage` MUST print an actionable install hint when `cargo-llvm-cov` is missing, and SHOULD print one when `llvm-tools-preview` is missing.
- **M2.2-R7** The `xtask` change MUST include unit tests for flag parsing, the `cargo llvm-cov` argument lists, the ignore regex, the lcov parser, the test-item finder and the renderer (grouping, totals, ordering, rounding, zero totals, the error cases).
- **M2.2-R8** `coverage` MUST NOT be a step of `cargo xtask ci` or `cargo xtask ci --quick`.
- **M2.2-R9** `ci.yml` MUST have a `coverage` job that runs `cargo xtask coverage`, appends `summary.md` to the job summary, and uploads the HTML and lcov reports as an artifact with 30-day retention.
- **M2.2-R10** Library code MUST NOT use `cfg(coverage)`, `cfg(coverage_nightly)` or `#[coverage(...)]`.
- **M2.2-R11** The workflow change MUST keep `cargo xtask check-workflows` passing.
- **M2.2-R12** The chunk MUST NOT change any library crate, public API, frozen format or `rust-toolchain.toml`.
- **M2.2-R13** The `coverage` job MUST NOT be a required check and MUST NOT have a threshold.
- **M2.2-R14** `AGENTS.md`, `README.md` and `CONTRIBUTING.md` MUST describe `ci` as running every check CI gates on, and name the report-only jobs that run outside it.
- **M2.2-R15** The pull request MUST record the first measured per-crate table, as the starting point for M2.3.
- **M2.2-R16** The summary MUST exclude lines and functions inside items marked `#[test]` or with a `cfg` predicate that requires `test` (§4.5), MUST count `cfg(any(test, ...))` code, and MUST report the number of excluded lines.

## 6. Cross-cutting concerns

- **Determinism:** n/a for library code, which is unchanged. The summary orders rows with a `BTreeMap` and uses integer rounding, so the same lcov and sources always give the same table (R4). Coverage numbers themselves can differ slightly between toolchain versions; that is expected and not a frozen format.
- **`unsafe`:** n/a, because none is written; `xtask` keeps `#![forbid(unsafe_code)]`.
- **Concurrency:** n/a, because `xtask` stays single-threaded (nextest runs the tests in parallel, as it already does).
- **Heap allocation:** n/a, because only `xtask` changes, and it is not library code.
- **`no_std` and wasm:** n/a, because coverage is measured on the host only, and no library code changes.
- **Frozen formats:** n/a, none touched. `summary.md` is a report, not a format anyone may depend on.
- **Public API changes:** none (R12). `xtask` gains `coverage`, a developer-tool command, not engine API.
- **FFI exposure:** n/a.
- **Performance:** not measured. Cost is CI time: one more Ubuntu job per push, an instrumented build plus a test run; expected to be a few minutes with a warm cache, to be recorded in the pull request. Parsing about 30 source files with `syn` adds well under a second.
- **Documentation:** CONTRIBUTING.md "Coverage" section, README command and tool lists, the `ci` wording in three files, roadmap row, CHANGELOG line, `xtask` help text and doc comments.
- **Policy changes:** two new rules, written in CONTRIBUTING.md: R10 (no coverage `cfg`s or attributes in library code), and unit tests stay in inline `#[cfg(test)]` modules (an out-of-line one fails the coverage command, R5). Both are enforced by review, a `grep` and the coverage command, not by a gating CI check (§7). The `ci` wording change (R14) corrects a description; it does not change what `ci` runs.

## 7. Alternatives considered

- **Codecov or Coveralls.** Rejected (decided 2026-11-12): free for a public repository and they give per-PR diff coverage and history, but they add a third-party action and send data outside GitHub, and the diff view matters most with a threshold, which this chunk does not have.
- **Run coverage on the pinned nightly** for doctests, branch coverage and `#[coverage(off)]`. Rejected (decided 2026-11-12): it ties coverage to the fuzz pin, so a pin bump can break it, and doctests would count examples as tests.
- **Stable in CI with a local `--nightly` opt-in.** Rejected: a second code path in `xtask` to keep working, for a view nobody has asked for yet.
- **Count the in-file `#[cfg(test)]` modules and say so.** Rejected (decided 2026-11-12): about a quarter of `src` lines are test code, so every percentage would be noticeably too high.
- **Show raw and filtered numbers side by side.** Rejected: needs the same filtering code and makes the table harder to read; the heading line states the size of the adjustment instead.
- **Filter from the JSON export instead of lcov,** which would keep a Regions column. Rejected: the per-file region data in the full export is much larger and its segment format is harder to filter by line range correctly than lcov's line and function records. The HTML report still has regions.
- **Move unit tests out of `src/` into `tests/`.** Rejected: it would lose access to private items and change the crates, which this chunk does not touch (R12).
- **Paste `cargo llvm-cov report --summary-only` text into the job summary.** Rejected: a long per-file table with no per-crate totals, and it counts test modules.
- **A third-party action that renders Cobertura XML into Markdown.** Rejected: another pinned dependency and a second report format, to draw one table, and it would count test modules.
- **Add `coverage` to the full `cargo xtask ci`** (lenient locally, strict under `CI`, like `fuzz-build`). Rejected: §4.4.
- **Exclude the ignored statistical tests.** Rejected: they take about 0.02 s in debug, so excluding them saves nothing and makes the coverage test set differ from the `test` job.
- **Measure in release mode.** Rejected: optimisation merges and drops regions, so line attribution gets worse; the dev profile already uses `opt-level = 1`.
- **Add `llvm-tools-preview` to `rust-toolchain.toml`.** Rejected: every job and contributor would download it, though only the coverage job uses it. The `rustup component add` line and the install hint cover it.
- **An unanchored ignore regex** (such as `/(tests|benches)/`). Rejected: a checkout under a directory with one of those names would exclude every file.
- **Enforce R10 in `check-statics` or a new check.** Rejected for now: there is no use of these `cfg`s to catch, and a `grep` in review is enough until there is a reason to expect one.
- **A trend file or long-retention artifact for history.** Rejected: machinery for little gain while the codebase is this small; the per-PR artifact and the M2.3 baseline in the pull request are enough.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | Unit test on the `nextest` argument list (packages, `--all-features`, `--run-ignored all`, `--no-report`, the two excludes); run `cargo xtask coverage` locally and read the commands it prints, including `clean` first |
| R2 | Local run: the three outputs exist under `target/llvm-cov/` and the table is printed |
| R3 | Unit tests on `ignore_regex`: it matches `<root>/xtask/...`, `<root>/benches/...`, `<root>/libs/x/tests/...`, `<root>/examples/x/benches/...`; it does not match `<root>/libs/x/src/...`; with a root containing `tests` and a `.`, a `src` file is still not matched; both report commands use it. Local run: the table lists exactly the five libraries and `fixed_step_sim` |
| R4 | Unit tests on the renderer: two crates given out of order, a zero total, a value needing half-up rounding, the `Total` row |
| R5 | Unit tests: a path outside the patterns, an lcov with no `SF` record, a `DA` with a non-numeric count, a `DA` before any `SF`, an unparsable source, and an out-of-line `#[cfg(test)] mod x;` each give an error naming the cause; a `BRDA` and an `LF` record are ignored |
| R6 | Run with `cargo-llvm-cov` hidden from `PATH` and read the hint; run with the component removed (`rustup component remove llvm-tools-preview`) and read the hint, then re-add it |
| R7 | `cargo xtask test` runs the new unit tests |
| R8 | Unit tests on `ci_steps(false)` and `ci_steps(true)` |
| R9 | The `coverage` job on the pull request: its summary page shows the table, and the `coverage-report` artifact contains `html/` and `lcov.info` |
| R10 | `grep -rnE "cfg\(coverage|coverage\(" libs examples` returns nothing |
| R11 | `cargo xtask check-workflows` |
| R12 | `git diff --stat main -- libs/ examples/ rust-toolchain.toml` is empty |
| R13 | Read the workflow diff (no threshold flag such as `--fail-under-lines`); `git diff --stat main -- infra/` is empty |
| R14 | Read the diffs of the three files |
| R15 | The pull request description contains the table from the CI run |
| R16 | Unit tests on the test-item finder: `#[cfg(test)] mod`, a `#[test]` fn, a `#[cfg(all(test, feature = "x"))]` fn inside an `impl` are excluded; `#[cfg(any(test, feature = "x"))]` and `#[cfg(not(test))]` items are counted; a function whose `FN` line is inside a test range is removed; the excluded-line count is in the heading. Local run: the table's line totals are lower than the HTML report's by the reported count |

## 9. Done when

- [ ] All requirements in §5 are verified as in §8, with the evidence (command output, run links) in the pull request description.
- [ ] `cargo xtask ci` passes locally on macOS and in CI on all legs, and `cargo xtask coverage` passes locally on macOS and in the `coverage` job.
- [ ] CONTRIBUTING.md, README.md, AGENTS.md, the roadmap row (to `Done`), CHANGELOG and this spec's status (`Implemented (PR #n, date)`) are updated.
- [ ] The pull request title is `feat(xtask): coverage report through cargo-llvm-cov (M2.2)` and says an assistant was used.

## 10. Open questions

None. Decided before drafting (2026-11-12): the report stays on GitHub with no third-party service; coverage runs on the pinned stable without doctests or branch coverage; the ignored statistical tests are included; coverage stays out of `cargo xtask ci`, and the `ci` wording in the docs is corrected instead. Decided while planning (same day): the summary filters `#[cfg(test)]` items out of its numbers (§4.5), which drops the Regions column; the ignore regex is anchored to the workspace root.

## 11. Prior art and sources

- [`cargo-llvm-cov` README](https://github.com/taiki-e/cargo-llvm-cov) for the `nextest`, `report`, `clean`, `--no-report` and `--ignore-filename-regex` commands, and the `cfg(coverage)` it sets.
- [The rustc book: Instrumentation-based code coverage](https://doc.rust-lang.org/rustc/instrument-coverage.html) for `-C instrument-coverage`, and why doctest and branch coverage and `#[coverage(off)]` need nightly.
- [LLVM `llvm-cov export`](https://llvm.org/docs/CommandGuide/llvm-cov.html#llvm-cov-export) for the lcov output read in §4.5, and the [`geninfo` tracefile format](https://github.com/linux-test-project/lcov/blob/master/man/geninfo.1) for the `SF`, `DA`, `FN` and `FNDA` records.
- [`syn` documentation](https://docs.rs/syn) for item spans and attribute parsing.
- [GitHub: adding a job summary](https://docs.github.com/en/actions/writing-workflows/choosing-what-your-workflow-does/workflow-commands-for-github-actions#adding-a-job-summary) for `$GITHUB_STEP_SUMMARY`.
- [M0+M1 spec §4.7, §4.8 and §4.12](../../m0-m1-foundations/spec.md) for the original coverage plan; [M2.1 spec](../m2.1-fuzzing/spec.md) for the `xtask` and workflow patterns reused here.
- No code copied from other projects (ADR 0002).
