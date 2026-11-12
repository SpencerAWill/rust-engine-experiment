# M2.3: Mutation testing

- **Status:** Accepted (2026-11-12)
- **Milestone:** M2 (process and verification infrastructure) · **Depends on:** M2.2 · **Decides:** no ADR
- **Estimated size:** about 650 changed lines, excluding tests, lockfiles and generated files (`xtask` about 330, `.cargo/mutants.toml` about 40, workflow and devcontainer about 50, docs about 100, the triage record about 130), plus at most about 400 lines of new tests in `engine_handle` and `engine_random` (§4.8)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds mutation testing through `cargo-mutants`: a shared `.cargo/mutants.toml`, a `cargo xtask mutants` command that checks the exclusion list, runs the tool and prints a per-crate table, and a weekly, report-only `mutants` workflow. It then does the first triage of `engine_handle` and `engine_random`: every mutant the tests miss is classified, small test gaps are closed in this pull request, larger ones become issues, and mutants no test could catch are excluded with a reason. It comes now because "first mutation triage recorded" is an M2 exit criterion, and M2.2's coverage report is in place to tell unexecuted code apart from code whose tests do not notice a change. At the end, `cargo xtask mutants` reports no missed mutant in the two crates other than those with an open issue, and `docs/mutation-triage.md` records how each one was resolved.

## 2. Goals and non-goals

**Goals**

- One configuration that both `cargo xtask mutants` and a plain `cargo mutants` use, so ad hoc runs (`--in-diff`, `--file`, `-F`) test the same way.
- One local command that runs mutation testing on the triaged crates and prints a per-crate summary of outcomes and the missed mutants.
- Exclusions that cannot rot silently: each one has a written reason, and the command fails when one no longer matches any mutant.
- A weekly CI job that shows the summary on its run page and keeps the full output for 30 days, without ever failing because of a missed mutant.
- A recorded first triage of `engine_handle` and `engine_random`, with small test gaps closed.

**Non-goals**

- No threshold, score target, ratchet or failing on a missed mutant (roadmap: "report-only"; decided 2026-11-12, §7). A score that gates invites tests written for the score.
- No mutation testing on pull requests by default, and no `--in-diff` job. The workflow's `pull_request` trigger is limited to changes in the mutants tooling itself (§4.6).
- No triage, and no weekly runs, of `engine_time`, `engine_math`, `fixed_step_sim` or the `engine` facade (decided 2026-11-12, §7). Later chunks add a crate to the triaged set when they triage it (§4.2). The facade has no mutants today (`cargo mutants --list -p engine` is empty).
- No `#[mutants::skip]` and no `mutants` dependency in any library crate (decided 2026-11-12, §7); exclusions live in `.cargo/mutants.toml`.
- No change to non-test library code. A bug the triage finds is recorded as an issue and fixed in its own pull request, test first (§4.8).
- No sharding, custom cargo profile or `--in-place` runs, unless the measured CI time needs them (§4.6, §7).
- No step in `cargo xtask ci` (§4.5).
- No mutation testing of `xtask`, `benches` or `fuzz/`.
- No change to any public API, frozen format or `rust-toolchain.toml`.

## 3. Guide-level explanation

To list the mutants that would be tested, without building anything:

```sh
cargo xtask mutants --list
```

To run them (the first run builds a copy of the workspace per job, so it is as slow as a clean build, then rebuilds incrementally for each mutant):

```sh
cargo xtask mutants                   # the triaged crates: engine_handle and engine_random
cargo xtask mutants -p engine_random  # one crate
cargo xtask mutants -j 2              # two mutants at a time
```

If `cargo-mutants` is missing, the command prints `cargo install --locked cargo-mutants --version 27.1.0`. It first checks the exclusion list, then runs `cargo mutants` with the settings in `.cargo/mutants.toml`, writes its output to `target/mutants/mutants.out/`, and prints a table of this shape (the real numbers come from the first run, R16):

```text
## Mutation testing

cargo-mutants 27.1.0; packages: engine_handle, engine_random; 12 mutants excluded by 5 entries in .cargo/mutants.toml.

| Crate         | Tested | Caught | Missed | Timeout | Unviable |
|---------------|--------|--------|--------|---------|----------|
| engine_handle | ...    | ...    | ...    | ...     | ...      |
| engine_random | ...    | ...    | ...    | ...     | ...      |
| Total         | ...    | ...    | ...    | ...     | ...      |

### Missed (2)

libs/engine_handle/src/pool.rs:412:9: replace HandlePool<T>::shrink_to_fit with ()
...
```

- **Caught:** some test failed with the mutant in place, which is what should happen.
- **Missed:** every test passed with the mutant in place. Either a test is missing, or the mutant changes nothing observable.
- **Timeout:** the tests did not finish in time, usually because the mutant makes a loop run forever. That still detects the mutant.
- **Unviable:** the mutant does not compile, so it says nothing about the tests.

A missed mutant exits 0: the command reports, it does not judge. It exits non-zero only when it could not produce a report (the tool is missing, an exclusion entry is broken, the unmutated tests already fail, or `cargo-mutants` hit an internal error).

`target/mutants/mutants.out/` holds `cargo-mutants`' own output: the outcome lists (`caught.txt`, `missed.txt`, `timeout.txt`, `unviable.txt`), a diff for each mutant in `diff/`, and the build and test log for each in `log/`. To see exactly what a missed mutant changed, read its diff.

Because the settings are in `.cargo/mutants.toml`, a plain `cargo mutants` tests the same way, which is useful while working on one mutant:

```sh
cargo mutants -p engine_handle -F 'shrink_to_fit'   # only the mutants whose name matches
git diff main > target/changes.diff
cargo mutants --in-diff target/changes.diff         # only mutants in changed code
```

**Triaging a missed mutant.** Read its diff, then decide:

1. **A test gap.** Write a test at the public-API level that fails with the mutant in place (`cargo mutants -F '<name>'` shows it caught) and passes without it. Do not write a test that only pins the mutant.
2. **A real bug.** Open an issue; fix it in its own pull request, test first.
3. **Equivalent.** The mutant behaves exactly like the original for every input (for example, `<` against `<=` where the two values can never be equal). Exclude it.
4. **Not worth a test.** The behaviour is real but no caller depends on it. Exclude it, and say why.

An exclusion is a regex in `exclude_re` in `.cargo/mutants.toml`, with the reason in a comment directly above it:

```toml
exclude_re = [
    # Equivalent: `len` is never 0 here (checked two lines above), so `>` and `>=` agree.
    '^libs/engine_handle/src/pool\.rs:.*replace > with >= in HandlePool<T>::grow',
]
```

The regex is matched against the names `--list` prints, which start with the file, line and column. Never put a line or column number in an entry: it would stop matching the next time the file changes. The command rejects an entry that contains one, an entry without a comment above it, and an entry that matches no mutant.

In CI, the `mutants` workflow runs every Sunday on `main` and on manual dispatch. Its summary page shows the same table, and its `mutants-out` artifact holds `mutants.out/` for 30 days. It fails only when the command does; a missed mutant leaves it green. When a weekly run shows a new missed mutant, triage it as above.

## 4. Reference-level design

### 4.1 Layout

```text
.cargo/mutants.toml                  shared cargo-mutants settings and the exclusion list (§4.3)
xtask/src/mutants.rs                 the `mutants` command: flags, the cargo-mutants argument lists,
                                     the exit-code mapping, and the version pin
xtask/src/mutants/excludes.rs        parsing and checking the exclusion list (§4.4)
xtask/src/mutants/summary.rs         the summary table (§4.7)
xtask/src/main.rs                    `Command::Mutants`, dispatch, help text
.github/workflows/mutants.yml        the weekly job (§4.6)
.devcontainer/install-tools.sh       installs cargo-mutants (optional block)
docs/mutation-triage.md              the triage record (§4.9)
```

### 4.2 `cargo xtask mutants`

```text
cargo xtask mutants [-p <package>]... [-j <n>] [--list]
```

- `-p <package>` MAY be repeated. Without it, the packages are the **triaged set**, a constant in `xtask/src/mutants.rs`: `engine_handle` and `engine_random`. A later chunk that triages another crate adds it there and to §4.9's record.
- `-j <n>` is passed through as `--jobs <n>`; `n` MUST be a positive integer. Without it, `cargo-mutants`' default (one job) applies.
- `--list` checks the exclusions and runs `cargo mutants --list` on the packages, then stops.
- An unknown flag, a `-p` or `-j` without a value, or a repeated `-j` MUST be an error. `-p` values are not checked against the workspace: `cargo-mutants` rejects an unknown package itself.

Without `--list`, the command MUST, in order:

1. Check that `cargo mutants` works (`require_cargo_subcommand("mutants", "cargo install --locked cargo-mutants --version <MUTANTS_VERSION>")`). It SHOULD read `cargo mutants --version` and print a warning, without failing, when the version is not `MUTANTS_VERSION` (§4.6), because results from a different version are not comparable.
2. Check the exclusion list (§4.4). A failed check stops the command before any build.
3. Run `cargo mutants --output target/mutants -p <package>... [--jobs <n>]` from the workspace root. All other settings come from `.cargo/mutants.toml`. The command passes no filter (`-F`, `-E`, `-f`), so the configured exclusions apply unchanged.
4. Map the exit code (below). For an exit code that means "a report exists", build the summary from `target/mutants/mutants.out/` (§4.7), write it to `target/mutants/summary.md`, and print it.

`cargo-mutants` 27.1.0 exit codes, and what the command does with each:

| Code | Meaning | `cargo xtask mutants` |
|---|---|---|
| 0 | every tested mutant was caught | summary, exit 0 |
| 2 | some mutants were missed | summary, exit 0 |
| 3 | some tests timed out | summary, exit 0 |
| 1 | usage error | error naming the code and its meaning |
| 4 | the unmutated tests fail or hang | error naming the code and its meaning |
| 5, 6 | `--in-diff` problems (never passed here) | error naming the code and its meaning |
| 70, other, or killed by a signal | internal error | error naming the code (or the signal) |

The mapping is a pure function of the exit status, unit-tested on every row. The argument lists are built by pure functions too, so unit tests check them without running `cargo-mutants`. The workspace root is found as in `xtask/src/coverage.rs`.

### 4.3 `.cargo/mutants.toml`

The file MUST set:

| Key | Value | Why |
|---|---|---|
| `test_tool` | `"nextest"` | The workspace's test runner. Its fail-fast stops at the first failing test, which shortens caught mutants. Doctests do not run, as in M2.2's coverage, so a mutant only a doctest catches shows as missed. |
| `all_features` | `true` | The same features as `cargo xtask test`. The `test-invariants` feature makes the model tests call `check_invariants`. |
| `additional_cargo_test_args` | `["--run-ignored", "all"]` | Runs the `#[ignore]`d statistical tests in `engine_random`, which are the only tests of some sampling distributions. They take about 0.02 s in a debug build (measured for M2.2). |
| `cap_lints` | `true` | CI sets `RUSTFLAGS=-D warnings`, and the workspace denies some lints. Without this, a mutant that only causes a warning (an unused variable, say) would be counted as unviable instead of tested. |
| `gitignore` | `true` | Keeps ignored files (`node_modules/`, `fuzz/corpus/`, build output) out of the per-job copies of the tree. |
| `exclude_re` | the exclusion list (§4.4) | |

The file MAY set other keys, if the plan finds a reason, but MUST NOT set `test_workspace` (each crate is judged by its own tests, which is `cargo-mutants`' default) or `examine_re`, `examine_globs` or `exclude_globs` (the triaged set is chosen by package, and exclusions are by name only). The plan's first step checks that `additional_cargo_test_args` reaches `cargo nextest run` as written. If it does not, the plan finds the setting that does and records it in its Deviations section.

### 4.4 The exclusion list

`xtask` reads `exclude_re` from `.cargo/mutants.toml` by hand, as `check-unsafe` reads its allowlist, so it needs no TOML dependency. The list MUST be written in this restricted form, and anything else MUST be an error naming the line:

- The list opens with a line that is exactly `exclude_re = [` and closes with a line that is exactly `]`. It appears once. An empty list is `exclude_re = []` on one line.
- Each line between them is blank, a comment (first non-blank character `#`), or exactly one entry: a TOML literal string (`'...'`, so a regex backslash needs no escaping) followed by `,`, with nothing after it.
- Each entry MUST have at least one comment line between it and the previous entry, or the opening line for the first entry. That comment is the reason.

Each entry MUST then pass two checks:

- **No position.** An entry MUST NOT contain a `:` directly followed by an ASCII digit. That form only appears in the `file:line:col:` prefix of a name, so an entry containing it depends on a line or column number.
- **Still matches.** `cargo mutants --list --no-config --workspace -F '<entry>'` MUST list at least one mutant. `--no-config` stops the configured exclusions from hiding the entry's own matches, and `--workspace` lets an entry for a crate outside the triaged set stay valid. Listing only parses the source, so this takes well under a second per entry.

All failing entries MUST be reported together, each with its line in `.cargo/mutants.toml` and the reason it failed. The check also yields the number of mutants the configured exclusions remove from the selected packages: the length of `cargo mutants --list --no-config -p ...` minus the length of `cargo mutants --list -p ...`. The summary reports this number (§4.7).

The parser and the position check are pure functions over the file's text, unit-tested. The match check runs `cargo-mutants`, so its regex semantics are the tool's own and `xtask` needs no regex dependency.

### 4.5 Relationship to `cargo xtask ci`

`mutants` MUST NOT be a step of `cargo xtask ci`, quick or full, for the reasons M2.2 §4.4 gives for coverage. It never fails on a result, only on a broken setup, and a full run takes tens of minutes. The exclusion check is not a `ci` step either: it needs `cargo-mutants` installed, and a stale entry is caught by the next weekly run or by the workflow's `pull_request` trigger when the configuration changes. The `ci` descriptions in `AGENTS.md`, `README.md` and `CONTRIBUTING.md`, which name the report-only jobs, MUST add the weekly mutants run to that list.

### 4.6 CI

A new workflow, `.github/workflows/mutants.yml`:

```yaml
name: Mutants
on:
  schedule:
    - cron: "0 8 * * 0"        # Sundays 08:00 UTC, after the nightly run
  workflow_dispatch:
  pull_request:
    branches: [main]
    paths:
      - .cargo/mutants.toml
      - .github/workflows/mutants.yml
      - xtask/src/mutants.rs
      - xtask/src/mutants/**
permissions:
  contents: read
jobs:
  mutants:
    runs-on: ubuntu-latest
    timeout-minutes: 120
    steps:
      - uses: actions/checkout@<pinned>            # persist-credentials: false
      - run: rustup show
      - uses: Swatinem/rust-cache@<pinned>
      - uses: taiki-e/install-action@<pinned>      # tool: cargo-mutants@27.1.0,cargo-nextest
      - run: cargo xtask mutants -j 2
      - run: cat target/mutants/summary.md >> "$GITHUB_STEP_SUMMARY"
      - uses: actions/upload-artifact@<pinned>     # name: mutants-out; path: target/mutants/mutants.out
        # if: always(); retention-days: 30
```

- **Triggers.** The weekly schedule and `workflow_dispatch` are the job's purpose. The `pull_request` trigger, limited to the mutants tooling files, makes a pull request that changes the configuration, the command or the workflow run the job before merge. It also lets this chunk's own pull request run the job: GitHub only lets you dispatch a workflow that already exists on the default branch, and this file does not exist there yet. Changes to library code or tests do not trigger it.
- **The version pin.** `MUTANTS_VERSION` in `xtask/src/mutants.rs` is the one place the version is defined. The workflow's `cargo-mutants@<version>` and the devcontainer's `--version <version>` repeat it, and a unit test in `xtask` MUST fail when either differs from `MUTANTS_VERSION`. The version is pinned because 26.0 and 27.0 each changed defaults (sharding order; how command-line filters combine with the configuration), and an unpinned upgrade could change results without anyone changing the repository. `taiki-e/install-action` has a manifest for `cargo-mutants` 27.1.0 (checked 2026-11-12), so it installs a prebuilt binary.
- **Jobs and time.** Nothing has been run yet; the figures below are an estimate. There are 458 mutants in the triaged set (353 in `engine_handle`, 105 in `engine_random`; `cargo mutants --list`, 27.1.0, 2026-11-12). At an estimated 5 to 10 s each (an incremental build plus the package's tests) and two jobs on a four-vCPU runner, the run should take about 20 to 40 minutes. The plan measures it. If the measured run takes more than 60 minutes, the plan changes `-j` or adds a sharded matrix, records why, and adjusts `timeout-minutes` to about twice the measured time. Both figures go in the pull request (R16).
- **Report-only.** The job is not `continue-on-error`: it goes red when the command fails, which means the setup is broken (§4.2), and that should be visible. A missed mutant leaves it green. It is not added to the required checks in `infra/github`. The artifact is uploaded with `if: always()`, because the logs of a failed run (a baseline failure, say) are what explains it. The summary step runs only on success, since a failed run has no summary.
- **Workflow rules.** The workflow keeps a top-level `permissions: contents: read`, pins every action to a commit hash already used in `ci.yml`, sets `persist-credentials: false`, and interpolates no `${{ github.* }}` expression into a `run:` step. `cargo xtask check-workflows` MUST pass.

### 4.7 The summary table

The summary is built from the four outcome files in `target/mutants/mutants.out/`: `caught.txt`, `missed.txt`, `timeout.txt` and `unviable.txt`. Each line is one mutant name in the `--list` form (`<path>:<line>:<col>: <description>`); blank lines are ignored. The renderer:

- MUST require all four files. A missing file MUST be an error naming it, because an absent file and an empty one mean different things. The plan's first step checks that `cargo-mutants` writes all four even when they are empty. If it does not, an absent file counts as empty, and the plan records that.
- MUST map each name to a crate from its path: `libs/<name>/src/...` or `examples/<name>/src/...`, with the path relative to the workspace root. A name that does not parse or does not match MUST be an error naming it.
- MUST give one row per crate with Tested (the sum of the four), Caught, Missed, Timeout and Unviable, ordered by crate name (a `BTreeMap`), and a `Total` row last.
- MUST list a crate that was selected but has no outcome with zeros, so a crate whose mutants were all excluded is still visible.
- MUST write a GitHub-flavoured Markdown table under a `## Mutation testing` heading. Before the table, one line gives the `cargo-mutants` version, the packages, and the number of mutants removed by how many `exclude_re` entries (§4.4).
- MUST follow the table with a `### Missed (<n>)` section listing the missed mutants in file and line order, inside a `text` code block. When there are none, it says `None.` instead. Timeouts are counted but not listed, because they detect the mutant.

The renderer is a pure function over the four files' contents, the version string, the package list and the exclusion count, unit-tested without running `cargo-mutants`.

### 4.8 The first triage

The triage covers every missed mutant and every timeout from a full run of the triaged set, on the branch, before any test is added. Each missed mutant MUST get exactly one class:

| Class | Action in this chunk |
|---|---|
| **Test gap, fixed** | A new or extended test in the crate's `tests/` directory or its inline `#[cfg(test)]` module, at the public-API level where possible. It fails with the mutant in place and passes without it. |
| **Test gap, deferred** | An issue, opened after the maintainer approves (an issue is outward-facing), naming the mutant and the behaviour no test checks. The mutant stays missed and is not excluded. |
| **Bug** | An issue as above, with a failing test written in its own pull request, not this one. The mutant stays missed. |
| **Equivalent** | An `exclude_re` entry whose comment explains why no input can tell the mutant from the original. |
| **Not worth a test** | An `exclude_re` entry whose comment says what behaviour is unchecked and why no caller depends on it. |

Each timeout MUST be recorded as **detected (hang)**, with no action, unless the triage finds that it hides a missed mutant (a test that is only slow, not stuck). In that case it is triaged like a missed one.

- **What "small" means.** A gap is fixed in this chunk if its test is about 50 lines or fewer. All the new tests together SHOULD stay under about 400 lines. Gaps beyond that become **test gap, deferred**, starting with the ones in the least-used code.
- **Do not overfit.** A fixing test MUST check behaviour a caller could observe through the crate's API, `check_invariants`, or a documented `Debug` or error message. It MUST NOT exist only to pin one mutant's diff.
- **No exclusion by default.** "Not worth a test" is for code no caller depends on, such as an internal fast path whose only effect is speed. It is not a way to clear the list: a `Debug` impl the M0+M1 spec calls "readable" is behaviour, and its gaps are test gaps.
- **Library code stays put.** Changes under `libs/` MUST be limited to `tests/` directories and to the bodies of `#[cfg(test)]` items in `src/` files. Library `Cargo.toml` files MUST NOT change. Code that runs outside tests MUST NOT change, so the run after the fixes measures the same code as the run before them.
- **Confirming.** After the fixes and exclusions, a second full run of the triaged set MUST report as missed only mutants classed **test gap, deferred** or **bug**, each one listed in the record with its issue.

### 4.9 The triage record

`docs/mutation-triage.md` is a living record. It MUST have:

- a status line naming it a living record and the date of the last triage;
- a short preamble on how to read it and how to add a crate (link to CONTRIBUTING);
- a section per triaged crate, giving the commit and `cargo-mutants` version of the triage run, the outcome counts before the fixes and after them, and a table with one row per missed mutant and per timeout: the mutant's name (with the line and column at that commit), its class, and the action (the test's name, the issue link, or "excluded" with the entry's first comment line). Rows are in file and line order.

The record MUST NOT be generated: the classes and reasons are judgements. Its counts MUST match the summaries of the two runs in §4.8, which are pasted into the pull request.

### 4.10 Devcontainer and docs

- `.devcontainer/install-tools.sh` installs `cargo-mutants` at the pinned version in an optional block, like the fuzzing and coverage blocks: a failure prints a warning and does not abort the setup.
- `CONTRIBUTING.md` gets a "Mutation testing" section covering: how to run it and list mutants; what the outcomes mean; the triage steps and the five classes; the exclusion format and its three checks; that a missed mutant never fails anything and there is no target score; that the triaged set grows by chunk; and the "no `mutants::` attributes in library code" rule. The "Setting up" tool list gains `cargo-mutants`.
- `README.md` lists `mutants [-p PACKAGE]... [-j N] [--list]` among the commands and `cargo-mutants` among the tools with install hints.
- `AGENTS.md`, `README.md` and `CONTRIBUTING.md` add the weekly mutants run to the report-only jobs named in the `ci` description (§4.5).
- `docs/roadmap.md` M2.3 goes through the statuses in the M2.0 process; `CHANGELOG.md` gets one line under Added.

## 5. Requirements

- **M2.3-R1** `.cargo/mutants.toml` MUST set `test_tool = "nextest"`, `all_features = true`, test arguments that run ignored tests, `cap_lints = true`, `gitignore = true` and `exclude_re`, and MUST NOT set `test_workspace`, `examine_re`, `examine_globs` or `exclude_globs`.
- **M2.3-R2** `cargo xtask mutants` MUST check the exclusions, then run `cargo mutants --output target/mutants` on the selected packages (default: `engine_handle`, `engine_random`), passing `--jobs` only when `-j` is given and no filter flag.
- **M2.3-R3** `cargo xtask mutants --list` MUST check the exclusions and list the selected packages' mutants without building.
- **M2.3-R4** Flag parsing MUST reject unknown flags, `-p` or `-j` without a value, a non-positive or non-numeric `-j`, and a repeated `-j`.
- **M2.3-R5** The exclusion list MUST be parsed in the restricted form of §4.4. A line outside that form, an entry without a reason comment, an entry containing `:` followed by a digit, and an entry that matches no mutant in the workspace MUST each be an error naming the line and the reason, with all failing entries reported together.
- **M2.3-R6** The command MUST exit 0 with a summary for `cargo-mutants` exit codes 0, 2 and 3, and MUST fail, naming the code and its meaning, for every other code or a signal.
- **M2.3-R7** The summary MUST have one row per selected crate (zeros included) with Tested, Caught, Missed, Timeout and Unviable, ordered by crate name, a `Total` row last, a heading line with the version, packages and excluded count, and a `Missed` section listing the missed mutants in file and line order. It MUST be written to `target/mutants/summary.md` and printed.
- **M2.3-R8** The summary MUST fail, naming the cause, on a missing outcome file and on a name that does not map to `libs/<crate>/src/` or `examples/<crate>/src/`.
- **M2.3-R9** The command MUST print `cargo install --locked cargo-mutants --version <MUTANTS_VERSION>` when `cargo-mutants` is missing, and SHOULD warn when the installed version differs.
- **M2.3-R10** The `xtask` change MUST include unit tests for flag parsing, the argument lists, the exit-code mapping, the exclusion parser and position check, the excluded count, the renderer (grouping, zeros, ordering, totals, the missed list, the error cases), and the version pin in the workflow and the devcontainer script.
- **M2.3-R11** `mutants` MUST NOT be a step of `cargo xtask ci` or `cargo xtask ci --quick`.
- **M2.3-R12** `.github/workflows/mutants.yml` MUST run weekly, on `workflow_dispatch`, and on pull requests that change the mutants tooling files; MUST install `cargo-mutants` at `MUTANTS_VERSION`; MUST run `cargo xtask mutants`, append the summary to the job summary, and upload `target/mutants/mutants.out` with `if: always()` and 30-day retention.
- **M2.3-R13** The workflow MUST keep `cargo xtask check-workflows` passing, MUST NOT be a required check, and MUST NOT fail on a missed mutant.
- **M2.3-R14** Library code MUST NOT use a `mutants::` attribute, and no library `Cargo.toml` may depend on the `mutants` crate.
- **M2.3-R15** Changes under `libs/` MUST be limited to `tests/` directories and the bodies of `#[cfg(test)]` items. No library `Cargo.toml`, public API, frozen format or `rust-toolchain.toml` may change.
- **M2.3-R16** The pull request MUST record the summaries of the triage run and the confirming run (§4.8), the local and CI durations, and the `-j` value used in CI.
- **M2.3-R17** Every missed mutant and timeout from the triage run MUST be classified in `docs/mutation-triage.md` as §4.8 and §4.9 describe, and every `exclude_re` entry MUST appear in it.
- **M2.3-R18** After the chunk, a full run of the triaged set MUST report as missed only mutants classed **test gap, deferred** or **bug**, each with an issue linked from the record.
- **M2.3-R19** Each test added to close a gap MUST fail with its mutant in place and pass without it, and MUST check behaviour observable through the crate's API, `check_invariants`, or a documented `Debug` or error message.
- **M2.3-R20** `AGENTS.md`, `README.md` and `CONTRIBUTING.md` MUST name the weekly mutants run among the report-only jobs outside `ci`, and CONTRIBUTING MUST have the "Mutation testing" section of §4.10.

## 6. Cross-cutting concerns

- **Determinism:** n/a for library code, which does not change outside tests. `cargo-mutants` 26.0 and later run mutants in a fixed order without `--shuffle`, and the summary orders rows with a `BTreeMap` and lists missed mutants in file and line order, so the same run gives the same report. Timeouts can vary with runner load. A mutant near the time limit can flip between caught and timeout, and the record says so if it happens.
- **`unsafe`:** n/a, because none is written. `xtask` keeps `#![forbid(unsafe_code)]`, and `cargo-mutants` does not mutate `unsafe` functions in any case.
- **Concurrency:** n/a, because `xtask` stays single-threaded. `cargo-mutants` runs jobs in separate copies of the tree.
- **Heap allocation:** n/a, because only `xtask` and tests change.
- **`no_std` and wasm:** n/a, because mutants run on the host only. New tests follow the existing test crates' platform rules (statistical tests stay `#[ignore]`d; panic tests stay native-only).
- **Frozen formats:** none touched. A new test MAY read the golden values, but MUST NOT change them. `summary.md` and the triage record are reports, not formats anyone may depend on.
- **Public API changes:** none (R15). `xtask` gains `mutants`, a developer-tool command.
- **FFI exposure:** n/a.
- **Performance:** not a library concern. The cost is CI time: one job a week, estimated at 20 to 40 minutes, plus a run on each pull request that changes the mutants tooling. The measured figures go in the pull request (R16). New tests SHOULD add under a second to `cargo xtask test`.
- **Documentation:** CONTRIBUTING "Mutation testing" section, README command and tool lists, the `ci` wording in three files, `docs/mutation-triage.md` (new, with a status line), roadmap row, CHANGELOG line, `xtask` help text and doc comments.
- **Policy changes:** three new rules, written in CONTRIBUTING.md: exclusions only in `.cargo/mutants.toml`, in the restricted form with a reason (enforced by the command, R5); no `mutants::` attributes or `mutants` dependency in library code (R14, enforced by review and a `grep`); and the triage classes, including that a bug found by mutation testing gets its own pull request (by review). The `ci` wording change (R20) adds a name to a list; it does not change what `ci` runs.

## 7. Alternatives considered

- **`#[mutants::skip]` next to the code.** Rejected (decided 2026-11-12): it needs `mutants` as a normal dependency of each library crate that uses it. That adds a dependency to shipped crates for a test tool, and changes non-test library code. Config regexes keep the libraries unchanged, and the match check (R5) gives them the one property the attribute has for free: they cannot point at nothing.
- **No exclusions, equivalents only in the record.** Rejected: every weekly report would repeat them, and a new miss would be hard to spot among them.
- **Nightly runs.** Rejected (decided 2026-11-12): the triaged code changes rarely, and a nightly run costs seven times the CI minutes for the same information.
- **Fail on a missed mutant that is not in the record** (a ratchet). Rejected (decided 2026-11-12): the roadmap says report-only, a ratchet needs a machine-readable record kept in sync with the code, and a red job for a missed mutant would push people toward tests written for the tool.
- **Run the whole workspace weekly.** Rejected (decided 2026-11-12): that is 962 mutants, about twice the CI time, and a report in which about 500 untriaged mutants in `engine_time`, `engine_math` and `fixed_step_sim` hide the new misses in the triaged crates.
- **Mutation testing on every pull request with `--in-diff`.** Rejected for this chunk: the roadmap scopes M2.3 to a scheduled job. Contributors can run `--in-diff` locally with the shared configuration (§3). It is worth reconsidering once more crates are triaged.
- **The job in `nightly.yml` behind an `if:` on `github.event.schedule`.** Rejected: dispatching `nightly.yml` by hand would run the 75-minute fuzz job and the others too. A separate file keeps the job's triggers and its version pin together.
- **A temporary `pull_request` trigger to test the new workflow.** Rejected: a temporary commit that must be remembered and removed. The permanent trigger, limited to the tooling files, tests every later change to the setup as well.
- **`cargo test` instead of nextest.** Rejected: it runs doctests, so it would catch a few more mutants, but it is slower per mutant, has no fail-fast across test binaries, and would differ from what M2.2's coverage measured. Mutants only a doctest catches show as missed, and are triaged like any other.
- **`test_workspace = true`** (run every crate's tests for every mutant). Rejected: slower, and a crate should be covered by its own tests; the facade and the example have few tests of their own.
- **Parse `outcomes.json` instead of the text files.** Rejected: its per-outcome schema is not documented as stable, while the text files hold exactly the names `--list` prints. `serde_json` would not be a new dependency, but nothing in the summary needs more than the names.
- **A TOML dependency in `xtask` for `mutants.toml`.** Rejected: `check-unsafe` already hand-parses its allowlist, and the restricted form (§4.4) is what lets the command require a reason comment, which a TOML parser would discard.
- **A regex dependency in `xtask` for the match check.** Rejected: `cargo-mutants -F` applies the tool's own regex semantics, which is what matters.
- **A custom `[profile.mutants]`** (no debug info) for faster builds. Deferred: it changes the root `Cargo.toml` for a speed-up nobody has measured. Revisit if the measured run is too slow.
- **Sharding from the start.** Deferred: a matrix with `--baseline=skip` needs a separate baseline job and makes the summary a merge of several outputs. Added only if the measured time needs it (§4.6).
- **Triage record inside the chunk's spec folder.** Rejected: later chunks add crates to it, and specs are not updated after they are implemented.
- **Unpinned `cargo-mutants`**, as the coverage job leaves `cargo-llvm-cov` unpinned. Rejected: the coverage job runs on every push, so a breaking upgrade shows up within a day. A weekly report whose results quietly change with the tool's defaults is harder to notice and harder to trust.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | Read `.cargo/mutants.toml`. The plan's first step confirms from a mutant's log in `mutants.out/log/` that nextest ran with `--run-ignored all` and `--all-features`. |
| R2 | Unit test on the run argument list (default packages, `-p` override, `--jobs` only with `-j`, no `-F`/`-E`/`-f`). Local run: the printed command line. |
| R3 | Unit test on the list argument list; local `cargo xtask mutants --list` prints the mutants and builds nothing. |
| R4 | Unit tests on `parse_flags`. |
| R5 | Unit tests on the parser: a good list; an entry without a comment; two entries on a line; a basic `"..."` string; text after the `,`; a second `exclude_re`; a missing `]`; an entry with `:12`; several failures reported together. Local run with a deliberately dead entry added to the file (reverted): the command fails before building and names the line. |
| R6 | Unit test on the exit-code mapping over 0, 1, 2, 3, 4, 5, 6, 70, an unknown code and a signal. |
| R7 | Unit tests on the renderer: two crates out of order, a selected crate with no outcomes, the `Total` row, the missed list in file and line order across files, `None.` when there are none. Local run: `summary.md` exists and matches the printed table. |
| R8 | Unit tests: a missing `timeout.txt`; a name in `xtask/src/`; a line that is not a name. |
| R9 | Run with `cargo-mutants` hidden from `PATH` and read the hint. The warning path is exercised by a unit test on the version comparison. |
| R10 | `cargo xtask test` runs the new unit tests; the pin test reads `.github/workflows/mutants.yml` and `.devcontainer/install-tools.sh`. |
| R11 | Unit tests on `ci_steps(false)` and `ci_steps(true)`. |
| R12 | Read the workflow diff; the `pull_request` run on this chunk's pull request: green, the job summary shows the table, and the `mutants-out` artifact contains `caught.txt`, `missed.txt`, `diff/` and `log/`. |
| R13 | `cargo xtask check-workflows`; `git diff --stat main -- infra/` is empty; the CI run above is green although it reports missed mutants, if any remain. |
| R14 | `grep -rn "mutants::" libs examples` and `grep -n "mutants" libs/*/Cargo.toml examples/*/Cargo.toml` return nothing. |
| R15 | `git diff --stat main -- libs/` touches only `tests/` files and `src/` files whose diff is inside `#[cfg(test)]` items (read the diff); `git diff --stat main -- libs/*/Cargo.toml rust-toolchain.toml` is empty. |
| R16 | The pull request description has both summaries, both durations and the `-j` value. |
| R17 | Spec conformance review: every name in the triage run's `missed.txt` and `timeout.txt` appears in the record with a class, and every `exclude_re` entry's first comment appears in it. |
| R18 | The confirming run's `missed.txt`, compared with the record's **test gap, deferred** and **bug** rows. |
| R19 | For each new test, `cargo mutants -F '<name>'` with the test in place reports the mutant caught. The triage run had reported it missed. Code review checks each test against the overfitting rule. |
| R20 | Read the diffs of the three files. |

## 9. Done when

- [ ] All requirements in §5 are verified as in §8, with the evidence (command output, run links) in the pull request description.
- [ ] `cargo xtask ci` passes locally on macOS and in CI on all legs; `cargo xtask mutants` passes locally on macOS and in the `mutants` job on the pull request.
- [ ] `docs/mutation-triage.md` records the first triage of `engine_handle` and `engine_random`, and the M2 exit criterion "first mutation triage recorded" is met.
- [ ] Each deferred gap and bug has an issue, opened after the maintainer approved.
- [ ] CONTRIBUTING.md, README.md, AGENTS.md, the roadmap row (to `Done`), CHANGELOG and this spec's status (`Implemented (PR #n, date)`) are updated.
- [ ] The pull request title is `feat(xtask): mutation testing through cargo-mutants (M2.3)` and says an assistant was used.

## 10. Open questions

None. Decided before drafting (2026-11-12):

- **Triage scope:** record every missed mutant, close small gaps in this pull request, and open issues for the rest.
- **Exclusions:** config-file regexes, with a reason and a match check; no `mutants` dependency in library code.
- **CI:** a weekly, report-only workflow that fails only on a broken setup, running only the triaged crates.

Measured while drafting: the per-crate mutant counts in §4.6 (`cargo-mutants` 27.1.0, `--list` only; nothing was run).

## 11. Prior art and sources

- [The cargo-mutants book](https://mutants.rs/), for:
  - exit codes, the `mutants.out` layout and outcome files;
  - `.cargo/mutants.toml` keys (`test_tool`, `all_features`, `additional_cargo_test_args`, `cap_lints`, `gitignore`, `exclude_re`) and `--no-config`;
  - filtering by name (`-F`, `-E`), `--list`, `--in-diff` and `--jobs`;
  - timeouts derived from the baseline;
  - the advice on triaging missed mutants and on not over-fitting tests to them.
  `cargo mutants --emit-schema=config` (27.1.0) lists the configuration keys used here.
- [cargo-mutants changelog](https://mutants.rs/changelog.html) and [NEWS.md](https://github.com/sourcefrog/cargo-mutants/blob/main/NEWS.md), for:
  - 26.0's default changes (no shuffle; `slice` sharding);
  - 27.0's change to how command-line filters combine with the configuration, which is why the version is pinned;
  - which code is never mutated (`#[cfg(test)]`, `#[test]`-like attributes, `unsafe` functions).
- [nextest: cargo-mutants integration](https://nexte.st/docs/integrations/cargo-mutants/), for running mutants under nextest.
- [GitHub: `workflow_dispatch`](https://docs.github.com/en/actions/writing-workflows/choosing-when-your-workflow-runs/events-that-trigger-workflows#workflow_dispatch), for the rule that a workflow must be on the default branch to be dispatched, which is why the job has a `pull_request` trigger (§4.6).
- [M2.2 spec](../m2.2-coverage/spec.md), for the `xtask`, summary and workflow patterns reused here, and for the coverage baseline (PR #16) that tells untested code apart from weakly tested code during triage. [M2.1 spec](../m2.1-fuzzing/spec.md), for the optional devcontainer block and the single-place version pin.
- No code copied from other projects (ADR 0002).
