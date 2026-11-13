# M2.3 plan: Mutation testing

- **Status:** Historical: executed in PR #17 (2026-11-13); the spec and the code are authoritative. Deviations are in §4.
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both.

## 1. Approach

The work is on branch `m2.3-mutation-testing`, which already has the spec and plan commit. The pure parts of the command come first, with unit tests: the exclusion parser and the summary renderer. They carry the chunk's only real logic and need no tool to test. The command that runs `cargo-mutants` comes next, then the workflow and devcontainer, then the docs. The triage itself comes last, as one commit per crate for the new tests and one for the exclusions and the record, followed by the confirming run and the CI run on the pull request.

Choices the spec leaves to implementation:

- **Probe before code (no commit).** `cargo-mutants` 27.1.0 is already installed locally. Write a draft `.cargo/mutants.toml` with the §4.3 keys and an empty `exclude_re = []`, then run `cargo mutants -p engine_random --output target/mutants` once by hand. This confirms what the spec took from the documentation:
  1. all four outcome files are written even when one is empty (spec §4.7);
  2. each line has the `--list` form `<path>:<line>:<col>: <description>`, with paths relative to the workspace root;
  3. a mutant's log in `mutants.out/log/` shows `cargo nextest run` with `--all-features` and `--run-ignored all` (if `additional_cargo_test_args` does not reach nextest, find the key that does; spec §4.3);
  4. the exit code is 0 or 2;
  5. the wall time and the number of mutants, for the time per mutant.

  Also run `cargo mutants --version` and record its exact output for the version check. Write any finding that contradicts the spec into §4 Deviations. Stop and ask if a finding changes a requirement. Keep the probe's `missed.txt` as a model for the hand-written fixtures. The probe's numbers are not the triage: Step 6 reruns from scratch.
- **Module layout.** `xtask/src/mutants.rs` holds the command and declares `mod excludes;` and `mod summary;` (spec §4.1).
- **Running the tool.** `cmd::run` turns any non-zero exit into an error, but exit codes 2 and 3 are results here. So `mutants.rs` runs `cargo mutants` through `std::process::Command` itself, prints the command line as `cmd::run` does, and passes the status to the pure mapping `fn interpret(code: Option<i32>) -> Result<(), String>` (`None` means killed by a signal). Listing uses `Command::output()` and splits stdout into lines.
- **Reading files.** The renderer takes the four files' contents as strings. Only `mutants()` touches the disk. A missing file is detected there, with `std::fs::read_to_string`'s `NotFound`, and reported by name (R8).
- **`-j` in CI.** Start with `-j 2` as the spec suggests, and change it only on the evidence of the first CI run (spec §4.6).

**Global constraints**

- Every commit passes `cargo xtask ci --quick`. The commits that touch a workflow also pass `cargo xtask check-workflows`.
- Commit messages are conventional, end with ` (M2.3)`, and end with the attribution lines the session requires. Stage by explicit path.
- No new dependency in `xtask/Cargo.toml`.
- Under `libs/`, only `tests/` files and `#[cfg(test)]` item bodies change (R15). No change under `examples/` or `infra/`, nor to `rust-toolchain.toml`, nor to any `Cargo.toml` other than the root `Cargo.lock` (which should not change either).
- Stop and ask the maintainer before:
  - opening any issue (Step 8);
  - the first push of the branch (Step 9 needs the pull request to trigger the workflow);
  - opening the pull request.

**Review focus.** These are failure modes the spec implies but no single requirement's test pins. Each one has a test or check in the step that owns the code.

1. **An exclusion that over-matches** (an unescaped `.` or `(`, a missing anchor) hides mutants that were never triaged. The match check only catches entries that match nothing. To counter it, the `--list` mode prints how many mutants each entry matches, and Step 8 reads those counts against the record. Every entry starts with `^libs/<crate>/src/<file>\.rs:` and names the function. Check in Step 3: `entry_match_counts_are_printed`. Check in Step 8: by hand.
2. **A wrong exit-code mapping** would make a baseline failure look green, or make a missed mutant fail the weekly job. Test in Step 3: `interpret_maps_every_documented_code`.
3. **Ignored tests not running under mutants.** Mutants only the statistical tests catch would then show as missed and be "fixed" with redundant tests. The probe checks this (item 3), and Step 6 reads one `engine_random` sampling mutant's log again.
4. **Lints making mutants unviable.** If `cap_lints` were not applied, the unviable count would be high and include warning-only mutants. Step 6 reads the unviable list and checks that every entry is a type or borrow error, not a lint.
5. **Version drift** between `MUTANTS_VERSION`, the workflow and the devcontainer. Test in Step 4: `mutants_version_is_pinned_everywhere`.
6. **Tests that overfit a mutant** (R19). Step 7 checks each new test against the spec's rule (§4.8) before committing, and the code review checks it again.
7. **A slow test misread as a hang.** For each timeout in Step 6, read its log: a run that was still passing tests when the time ran out is triaged as a missed mutant, not as "detected (hang)" (spec §4.8).

## 2. Steps

### Step 1: `.cargo/mutants.toml` and the exclusion parser (R1, R5, R10)

- **Files:**
  - new: `.cargo/mutants.toml` (the §4.3 keys, `exclude_re = []`, and a header comment pointing to CONTRIBUTING); `xtask/src/mutants.rs` (only `mod excludes;` for now); `xtask/src/mutants/excludes.rs`;
  - changed: `xtask/src/main.rs` (`mod mutants;`).
- **Interfaces produced:**
  - `pub(super) struct Entry { pub(super) line: usize, pub(super) regex: String }` (`line` is 1-based);
  - `pub(super) fn parse(text: &str) -> Result<Vec<Entry>, Vec<String>>`, where each error starts with `.cargo/mutants.toml:<line>:`;
  - private `fn has_position(regex: &str) -> bool`.
- **Test first** (each a unit test in `excludes.rs`):
  - `parses_entries_with_reasons`: a list with two entries, each under a comment and with blank lines in between, gives both entries with their line numbers.
  - `empty_list_on_one_line`: `exclude_re = []` gives `Ok(vec![])`.
  - `entry_without_a_reason_is_an_error`: a second entry directly after the first, with no comment between them.
  - `malformed_lines_are_errors`, one case each: two entries on one line; a basic `"..."` string; text after the comma; a missing comma.
  - `list_must_appear_once_and_close`: two `exclude_re` lines; a missing `]`; no `exclude_re` at all.
  - `positions_are_rejected`: `'^libs/a/src/b\.rs:12:'` is rejected; `'replace u64 with 0'` and `'[u64; 4]'` are accepted.
  - `all_failures_are_reported_together`: three bad entries give three errors.

  Run `cargo xtask test`. It fails to compile, because `parse` does not exist.
- **Change:** a line scanner over the rules in spec §4.4. It keeps state for "inside the list" and "a comment since the last entry". A TOML literal string is `'` then any characters except `'`, then `'`. Other keys in the file are ignored, apart from a second `exclude_re`.
- **Verify:** `cargo xtask test` passes the new tests, and `cargo xtask clippy` and `cargo xtask fmt --check` pass. `cargo mutants --list -p engine_random | wc -l` still prints 105, so the config file is accepted and excludes nothing.
- **Commit:** `feat(xtask): parse the cargo-mutants exclusion list (M2.3)`

### Step 2: the summary renderer (R7, R8, R10)

- **Files:** new `xtask/src/mutants/summary.rs`; `xtask/src/mutants.rs` (`mod summary;`).
- **Interfaces produced:**
  - `pub(super) struct Outcomes<'a> { pub(super) caught: &'a str, pub(super) missed: &'a str, pub(super) timeout: &'a str, pub(super) unviable: &'a str }`;
  - `pub(super) struct Header<'a> { pub(super) version: &'a str, pub(super) packages: &'a [String], pub(super) excluded_mutants: usize, pub(super) exclude_entries: usize }`;
  - `pub(super) fn render(header: &Header<'_>, outcomes: &Outcomes<'_>) -> Result<String, String>`;
  - private `fn parse_name(name: &str) -> Result<(String, String, u32, u32), String>` (crate, path, line, column) and `struct Row { caught: usize, missed: usize, timeout: usize, unviable: usize }`.
- **Test first:**
  - `rows_are_ordered_by_crate_with_total_last`: outcomes for `engine_random` given before `engine_handle` render `engine_handle`, `engine_random`, `Total`. `Tested` is the sum of the four, and `Total` is the column sums.
  - `selected_crate_without_outcomes_shows_zeros`: packages `[engine_handle, engine_time]` with outcomes only in `engine_handle` give an `engine_time` row of zeros.
  - `missed_list_is_in_file_and_line_order`: missed names from `b.rs:9`, `a.rs:30` and `a.rs:4` are listed `a.rs:4`, `a.rs:30`, `b.rs:9`. Line 30 comes after line 4, so the sort is numeric, not textual. The heading is `### Missed (3)`.
  - `no_missed_says_none`.
  - `heading_line_names_version_packages_and_exclusions`.
  - `examples_paths_map_to_their_crate`: `examples/fixed_step_sim/src/main.rs:1:1: ...` maps to `fixed_step_sim`.
  - `name_outside_the_patterns_is_an_error` (`xtask/src/main.rs:1:1: x`), `line_that_is_not_a_name_is_an_error` (`garbage`); both name the line.
  - `blank_lines_are_ignored`.

  Run `cargo xtask test`. It fails to compile.
- **Change:**
  - `parse_name` splits at the first `: ` after a `<path>:<digits>:<digits>` prefix. Paths have no spaces, and the description may contain `:`, as in `<impl fmt::Debug for Handle<T>>`.
  - The crate comes from `libs/<name>/src/` or `examples/<name>/src/`, read through `Path::components`.
  - Rows are kept in a `BTreeMap<String, Row>`, seeded with the selected packages.
  - The missed list is sorted by `(path, line, column)`.
- **Verify:** `cargo xtask test` passes, and clippy and fmt pass. Render the probe's real outcome files through a temporary, uncommitted test, and read the result.
- **Commit:** `feat(xtask): per-crate mutation testing summary (M2.3)`

### Step 3: `cargo xtask mutants` (R2, R3, R4, R5, R6, R9, R10, R11)

- **Files:** `xtask/src/mutants.rs`, and `xtask/src/main.rs` (`Command::Mutants(mutants::Options)`, `parse`, `execute`, the usage text).
- **Interfaces produced:**
  - `pub(crate) const MUTANTS_VERSION: &str = "27.1.0"` and `const TRIAGED: &[&str] = &["engine_handle", "engine_random"]`;
  - `pub(crate) struct Options { packages: Vec<String>, jobs: Option<NonZeroU32>, list: bool }`, `pub(crate) fn parse_flags(flags: &[&str]) -> Result<Options, String>` and `pub(crate) fn mutants(options: &Options) -> Result<(), String>`;
  - private pure functions: `fn packages(options: &Options) -> Vec<String>` (the default is `TRIAGED`), `fn run_args(options: &Options) -> Vec<String>`, `fn list_args(packages: &[String], use_config: bool) -> Vec<String>`, `fn match_args(regex: &str) -> Vec<String>`, `fn interpret(code: Option<i32>) -> Result<(), String>` and `fn version_matches(version_output: &str) -> bool`.
- **Test first:**
  - `parses_packages_jobs_and_list`: `-p a -p b -j 2 --list`. Errors for `-p` at the end, `-j 0`, `-j x`, `-j 2 -j 3` and `--x`. In `main.rs`, `parse(&["mutants"])` is `Command::Mutants(_)`.
  - `run_args_match_the_spec`: the default is `["mutants", "--output", "target/mutants", "-p", "engine_handle", "-p", "engine_random"]`. `-j 2` appends `["--jobs", "2"]`. No argument is `-F`, `-E`, `-f`, `--re`, `--exclude-re` or `--file`.
  - `list_args_with_and_without_config`: `--list` with `-p` for each package, plus `--no-config` only when asked.
  - `match_args_use_the_workspace_without_config`: `["mutants", "--list", "--no-config", "--workspace", "-F", regex]`.
  - `interpret_maps_every_documented_code` (review focus 2): `Some(0)`, `Some(2)` and `Some(3)` give `Ok`. `Some(1)`, `Some(4)`, `Some(5)`, `Some(6)`, `Some(70)`, `Some(9)` and `None` each give an error containing the code and the meaning from the spec §4.2 table (`baseline` for 4, `signal` for `None`).
  - `version_matches_the_pin`: the exact `--version` output recorded in the probe is accepted. `cargo-mutants 27.0.0` is rejected.
  - `mutants_is_not_a_ci_step` (R11): no `Command::Mutants(_)` in `ci_steps(false)` or `ci_steps(true)`.

  Run `cargo xtask test`. It fails to compile.
- **Change:** `mutants()` does the following, in order.
  1. Call `require_cargo_subcommand("mutants", <hint with MUTANTS_VERSION>)`.
  2. Read `cargo mutants --version` and warn on a mismatch.
  3. Read `.cargo/mutants.toml` from the root, then `excludes::parse` it.
  4. For each entry, run `match_args` and count the lines. Collect every failing entry, either from parsing or because it matches nothing, then return them together. This prints `<n> mutants  <regex>` for each entry (review focus 1).
  5. Compute the excluded count from the two `list_args` listings.
  6. With `--list`, print the configured listing and return.
  7. Otherwise, run `run_args` through `Command::status`, then `interpret`.
  8. On `Ok`, read the four files (a `NotFound` becomes `missing target/mutants/mutants.out/<file>`), `summary::render`, write `target/mutants/summary.md`, and print it.

  Add `mutants [-p PACKAGE]... [-j N] [--list]` to the usage text and to the missing-command hint list.
- **Verify:**
  - `cargo xtask test`, clippy and fmt pass.
  - `cargo xtask mutants --list` prints 458 names and builds nothing. Check that no `target/mutants/` build directory appears.
  - Add a dead entry, such as `# probe` then `'^libs/engine_handle/src/nope\.rs:'`, to `.cargo/mutants.toml`. `cargo xtask mutants` fails at once, naming line and reason. Revert the entry.
  - `cargo xtask mutants -p engine_random` runs, prints the table, and writes `target/mutants/summary.md`. Record the duration.
  - With `cargo-mutants` hidden from `PATH`, the hint is printed (R9).
- **Commit:** `feat(xtask): add the mutants command (M2.3)`

### Step 4: workflow, devcontainer and version pin (R10, R12, R13)

- **Files:** new `.github/workflows/mutants.yml`; changed `.devcontainer/install-tools.sh` and `xtask/src/mutants.rs` (the pin test).
- **Test first:** `mutants_version_is_pinned_everywhere`. It reads `.github/workflows/mutants.yml` and `.devcontainer/install-tools.sh` from the workspace root, and asserts that each contains `MUTANTS_VERSION` in its install form: `cargo-mutants@<v>` and `--version <v>`. Run `cargo xtask test`. It fails, because the files do not have the pin yet.
- **Change:**
  - The workflow exactly as in spec §4.6, with the pins copied from `ci.yml` (`actions/checkout`, `Swatinem/rust-cache`, `taiki-e/install-action`, `actions/upload-artifact`).
  - The summary step has no `if:`, so it runs only on success. The upload step has `if: always()`, `name: mutants-out`, `path: target/mutants/mutants.out` and `retention-days: 30`.
  - The devcontainer gets an optional block: `cargo install --locked cargo-mutants --version 27.1.0 || echo "warning: ..." >&2`.
- **Verify:**
  - `cargo xtask test` passes, and `cargo xtask check-workflows` passes.
  - Read the workflow: no `${{` inside any `run:` line.
  - `git diff --stat main -- infra/` is empty.
- **Commit:** `ci: weekly report-only mutation testing (M2.3)`

### Step 5: documentation (R14, R20)

- **Files:** `CONTRIBUTING.md`, `README.md`, `AGENTS.md` and `CHANGELOG.md`.
- **Test first (the check that fails before the change):**
  - `grep -n "mutants" CONTRIBUTING.md README.md AGENTS.md` finds nothing.
  - `grep -n "Mutation testing" CONTRIBUTING.md` finds nothing.
- **Change:**
  - CONTRIBUTING gets a "Mutation testing" section with everything in spec §4.10, and `cargo-mutants` is added to "Setting up".
  - README gets the command and the tool.
  - The `ci` description in all three files now names the weekly mutants run among the report-only jobs.
  - CHANGELOG gets one `Added` line for M2.3. It is finished in Step 9, once the triage numbers exist.
- **Verify:**
  - `cargo xtask typos` passes.
  - The relative links resolve (open each in the rendered Markdown).
  - `grep -rn "mutants::" libs examples` and `grep -n mutants libs/*/Cargo.toml examples/*/Cargo.toml` are empty (R14).
- **Commit:** `docs: mutation testing workflow and triage rules (M2.3)`

### Step 6: the triage run (no commit)

- **Run:** `cargo xtask mutants -j 2` on a quiet machine, AC power, no other builds. Record the following.
  - The summary and the duration.
  - Copies of `missed.txt`, `timeout.txt` and `unviable.txt` in the scratchpad, as the triage input. Do not keep them in the repository.
  - For one `engine_random` sampling mutant, the log, to confirm that ignored tests ran (review focus 3).
  - The unviable list, checked for lint-only causes (review focus 4).
  - Each timeout's log (review focus 7).
- **Classify:** go through each missed mutant and timeout, in file and line order, and give it one class from spec §4.8. Write the draft table for `docs/mutation-triage.md` as you go. For each mutant:
  - read its diff in `mutants.out/diff/`;
  - check the M2.2 coverage HTML to see whether any test executes the line at all;
  - for **test gap, fixed**, note the planned test and its rough size;
  - check the planned tests against the 400-line budget, and move the excess to **test gap, deferred**, least-used code first.
- **Stop and show the maintainer** the classified list before writing tests, if it has any **bug** row, or more than 40 rows, or more than 10 exclusions. The classes are judgements, and a large or surprising list is worth agreeing on first.

### Step 7: close the small test gaps (R15, R19)

One commit per crate: `engine_random` first (the smaller one), then `engine_handle`.

- **Files:** the crate's `tests/*.rs` files, or the bodies of existing `#[cfg(test)]` modules when the behaviour is only reachable from inside the crate.
- **Test first, for each gap:**
  1. Write the test.
  2. Run `cargo mutants -p <crate> -F '<escaped name without line:col>'`. It reports the mutant **caught**; the Step 6 run had reported it missed.
  3. Run `cargo xtask test`. The test passes on the unmutated code.

  If the new test does not catch the mutant, it does not test what was intended. Fix the test; do not move on.
- **Change:** tests only. Before committing, check each test against the overfitting rule (review focus 6): it would still be a reasonable test if the mutant did not exist.
- **Verify:**
  - `cargo xtask ci --quick` passes.
  - `git diff --stat main -- libs/` lists only `tests/` files and `src/` files whose hunks are all inside `#[cfg(test)]` modules (read the hunks).
- **Commits:** `test(engine_random): close gaps found by mutation testing (M2.3)` and `test(engine_handle): close gaps found by mutation testing (M2.3)`

### Step 8: exclusions, issues and the record (R5, R17, R18)

- **Files:** `.cargo/mutants.toml` (the `exclude_re` entries) and `docs/mutation-triage.md` (new).
- **Issues:** list the proposed issues for every **test gap, deferred** and **bug** row, with titles and bodies, and stop for the maintainer's approval. Then open them with `gh issue create` and put the links in the record.
- **Change:**
  - Each exclusion entry is anchored as `^libs/<crate>/src/<file>\.rs:`, names the function and the replacement, and sits under the reason comment.
  - The record follows spec §4.9.
- **Test first (the check that fails before the change):** before the exclusions are added, `cargo xtask mutants --list` lists each mutant classed as excluded; afterwards it does not.
- **Confirming run:** `cargo xtask mutants -j 2` again. Each part of the check must hold.
  - Every missed mutant is a **test gap, deferred** or **bug** row (R18).
  - The excluded count in the heading equals the number of excluded rows in the record.
  - Each entry's match count, printed by the check, equals the number of rows it is meant to cover (review focus 1).

  Record the summary and duration.
- **Verify:**
  - `cargo xtask typos` passes.
  - Every name from the Step 6 `missed.txt` and `timeout.txt` appears in the record (check with `grep -F` from the scratchpad copies).
- **Commit:** `docs: record the first mutation triage of engine_handle and engine_random (M2.3)`

### Step 9: CI run and closing record (R12, R13, R16)

- **Push:** ask the maintainer, then push the branch and open the pull request as a draft, once they approve opening it. The pull request touches `.cargo/mutants.toml` and the workflow, so the `mutants` job runs.
- **Read the run:**
  - It is green.
  - The job summary shows the table, matching the confirming run's counts (except for timeouts near the limit, which are noted if they occur).
  - The `mutants-out` artifact has the outcome files, `diff/` and `log/`.
  - Note the duration. If it is over 60 minutes, change `-j` or add sharding as spec §4.6 says, record why, and rerun.
  - Set `timeout-minutes` to about twice the measured time if it is far from 120.
- **Change:**
  - The roadmap row becomes `Done (PR #n)` with the spec and plan links.
  - The spec status becomes `Implemented (PR #n, date)`.
  - The plan status becomes `Historical: executed in PR #n (date); ...`, with §4 Deviations filled in.
  - The CHANGELOG line is finished.
  - If `timeout-minutes` changed, the workflow is updated.
- **Pull request description:** both summaries, the local and CI durations, the `-j` value (R16), the issues opened, the eight reviews from CONTRIBUTING, a note that an assistant was used, and the attribution line.
- **Commit:** `docs: close M2.3 (M2.3)`

## 3. Final verification

- `cargo xtask ci` passes locally on macOS. Every CI leg is green, including the `mutants` job on the pull request.
- Before marking the pull request ready, run the reviews from CONTRIBUTING: `/code-review`, `/security-review`, documentation and spec conformance. Public API is `n/a` (no library API change). Determinism is `n/a` (only tests change in libraries). Performance is `n/a` (no hot path; the CI cost is reported). Portability: the job is Ubuntu-only; the command is checked on macOS.
- Go through the spec's §9 Done-when list item by item, with the evidence in the pull request description.

## 4. Deviations

Everything the implementation did differently from this plan or from the spec as accepted on 2026-11-12:

1. **The match check uses `-E`, not `-F`** (Step 3, spec §4.4). `cargo-mutants` 27.1 never applies a name filter (`-F`, `-E` or `exclude_re`) to its "delete field ... from struct Self expression" mutants (3 in `engine_handle`, 11 in the workspace), so `-F` listed them whatever the regex and a count could never be zero. The check counts the mutants that `-E <entry>` removes from `--list --no-config --workspace`. Such mutants can never be excluded; the one missed in `engine_handle` was closed with a test.
2. **Two equivalent mutants are not excluded** (Step 8, R18). `sample.rs:182` (`<` to `<=`) and `handle.rs:208:33` (`+` to `*`) have the same description as caught mutants in the same function, and an entry without a line or column cannot tell them apart. They stay in the missed list and are recorded in `docs/mutation-triage.md` as equivalent, not excluded; no issue was opened. Four entries exclude four mutants, not the six the first classification proposed.
3. **A test-only helper in `src`** (Step 7). `#[cfg(test)] impl HandlePool<()> { corrupt_dense_length }` in `pool.rs`, so a test can break an invariant of `DenseHandleAllocator`, whose pool fields are private to another module. It is a `#[cfg(test)]` item, so R15 holds.
4. **`DenseHandleAllocator::with_capacity`** has no observable capacity (its pool stores `()`), so its gap is closed by a no-allocation test in `fixed_capacity.rs` (dhat), not by a capacity assertion. That test measures up to three runs on fresh containers, because dhat counts every thread and the harness's main thread made the `cargo-mutants` baseline fail once on a loaded CI runner (13 blocks against 9).
5. **The pull request job** (Step 4, spec §2, §4.2, §4.6, §7, R4, R12), changed at the maintainer's request after the CI run measured 20 minutes: pull requests that change the triaged crates' sources or the mutants tooling run the exclusion check and `cargo xtask mutants --in-diff` on the pull request's own diff (a separate `mutants-changed` job); the full run stays weekly and on dispatch. `xtask` gained `--in-diff FILE` (rejected with `--list`, resolved against the caller's directory), a summary for a diff that contains no mutant, and removal of the previous run's output before a run. `timeout-minutes` is 40 for the full job and 30 for the pull request job.
6. **Step 1 and 2 details.** `parse_name` returns a small struct, not a 4-tuple (clippy `type_complexity`); a temporary `#![allow(dead_code)]` in `mutants.rs` covered the commits before the command used the parsers, and was removed in Step 3.
7. **The probe (Step 0) was cut short.** I stopped the probe run by mistake before it finished; its items were confirmed by later runs instead: all four outcome files are written, each name has the `--list` form, a mutant's log shows `cargo nextest run --all-features --run-ignored all`, `cargo mutants --version` prints `cargo-mutants 27.1.0`, and a baseline failure exits 4.
8. **Also in this pull request, outside the plan:** the nightly `stable-toolchain` job failed with E0463 because a bare `stable` lacks the targets in `rust-toolchain.toml`; `nightly.yml` now installs them (the maintainer asked for the fix here).
9. **Review fixes** from `/code-review`: stale output, relative diff path, `--list` with `--in-diff`, the CHANGELOG usage line, the pin test also covering `CONTRIBUTING.md`, and `if-no-files-found: ignore` on the pull request job's upload. Two findings were declined: `-p` values are not checked against the workspace layout before a run (the spec says `cargo-mutants` rejects an unknown package itself), and the exclusion check spawns one list command per entry (the spec rejected a regex dependency).
