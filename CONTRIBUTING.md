# Contributing

Thanks for your interest. This is an experimental engine: APIs change, and some decisions are deliberately deferred (see the [roadmap](docs/roadmap.md)). For anything beyond a small fix, **open an issue first** so we can agree on the direction before you spend time on it.

## How work is organized

- Work is planned in [docs/roadmap.md](docs/roadmap.md): milestones, broken into **chunks** of exactly one pull request each.
- Each chunk has a `spec.md` and a `plan.md`, copied from [docs/design/_template/](docs/design/_template/). The maintainer reviews them before implementation starts.
- Pull request titles end with the chunk ID, for example `feat(fuzz): fuzz HandlePool against a model (M2.1)`.
- Ideas, bugs and questions that are not scheduled go into issues. When one is scheduled, a chunk spec links it.

## Licence and originality

The project is dual-licensed `MIT OR Apache-2.0`. Unless you state otherwise, a contribution you submit is licensed under both, with no additional terms (see the README).

The rules that make that safe are in [ADR 0002](docs/adr/0002-originality-and-dependency-licensing.md):

- **Do not copy code from other projects**, including other engines. You may study designs, papers, talks, and blog posts, and cite them. Reading another crate's source to understand its behaviour is fine; transcribing it is not.
- Published algorithms are implemented from their papers and specifications; their published test vectors may be used as golden values.
- Other crates may be used as test oracles or benchmark dev-dependencies, but a new dependency must pass `cargo xtask deny` (allowed licences, no unknown registries or git sources).
- If you used an AI assistant, say so in the pull request. The same rule applies: write from specifications, never paste code from other repositories.

## Setting up

```sh
rustup show                 # installs the pinned toolchain from rust-toolchain.toml
cargo xtask ci              # every check CI gates on, in order (the coverage job, the nightly fuzz run and the weekly mutants run are report-only and run outside it)
cargo xtask ci --quick      # fmt check, clippy, and tests only
```

`cargo xtask` prints an install hint when a tool is missing. The extra tools are `cargo-nextest`, `cargo-deny`, `critcmp`, `wasmtime`, and, for fuzzing, `cargo-fuzz` plus a pinned nightly toolchain, and, for coverage, `cargo-llvm-cov` plus the `llvm-tools-preview` component, and, for mutation testing, `cargo-mutants` (`cargo install --locked cargo-mutants --version 27.1.0`); `.devcontainer/install-tools.sh` installs them.

## What CI enforces

These are policy, not preferences. A pull request that breaks one will not be merged:

- **No `unsafe`.** Every crate root has `#![forbid(unsafe_code)]` (`check-unsafe`). An exception needs an ADR and an entry in `unsafe-allowlist.toml`.
- **Determinism.** No `std` `HashMap`/`HashSet`/`RandomState`, `Instant::now`, `SystemTime::now`, `thread_local!`, or inherent transcendental float methods in library code (see `clippy.toml`; transcendentals go through `engine_math::scalar`). Simulation state must be snapshot-able plain data.
- **No global mutable state** (`check-statics`, [ADR 0008](docs/adr/0008-no-global-state.md)).
- **Frozen formats.** The PRNG output streams and `Handle::to_bits` are frozen for all of `0.x`. Changing one needs an ADR and updated golden tests, not just a code change.
- **Documentation.** Every public item is documented and every public type has a runnable doctest.
- **Panics.** A documented panic needs `#[track_caller]`, a `# Panics` section, an actionable message, and a fallible `try_` twin.
- **Workflows** (`check-workflows`): actions pinned to commit hashes, a read-only token, no persisted checkout credentials, no `pull_request_target`.
- Formatting (`cargo fmt`), clippy with warnings denied, docs without warnings, and `no_std` and WebAssembly builds.

## Tests

Changes come with tests. For a bug fix, write the test first and confirm it fails for the reason you expect, then fix the code. Prefer testing behaviour over implementation. The layers in use are unit tests, proptest model tests against a simple reference, golden values, and doctests. Do not weaken or delete a failing test without saying so in the pull request.

## Benchmarks

Performance claims need measurements. Wall-clock numbers from a laptop or shared CI runner are noisy (we have measured about 5% drift between runs on a quiet machine), so:

- Run on AC power with other apps closed, and record the machine and conditions. On macOS `cargo xtask bench` refuses to start on battery and keeps the machine awake; afterwards, check `pmset -g log` for a power change or sleep during the run.
- Compare against a saved baseline: `cargo xtask bench --save NAME`, then `cargo xtask bench-report --baseline NEW --against OLD`. The report only calls a change real if it exceeds 5% and the confidence intervals do not overlap.
- Say what you measured and what you did not.

## Fuzzing

Coverage-guided fuzz targets for `engine_handle` live in `fuzz/` (outside the workspace). Three drive `HandlePool`, `HandleAllocator` and `DenseHandleAllocator` with arbitrary operation sequences against a simple reference model, checking `check_invariants` after every operation; one checks `Handle::from_bits` and `to_bits`.

```sh
cargo xtask fuzz handle_pool_ops --time 60      # one target for 60 s (the default)
cargo xtask fuzz all                            # every target, 10 min each, as the nightly job does
cargo xtask fuzz handle_pool_ops --replay fuzz/artifacts/handle_pool_ops/crash-<hash>
cargo xtask fuzz-build                          # compile only; CI does this on every pull request
```

You need `cargo-fuzz` and the pinned nightly; the command prints the install lines when either is missing. Fuzzing does not work on Windows (libFuzzer needs a non-MSVC toolchain): use the devcontainer or WSL. `cargo xtask ci` skips `fuzz-build` with a loud note when the tools are missing, but fails when `CI` is set.

- **A finding.** The nightly `fuzz` job goes red and uploads `fuzz/artifacts` for 30 days. Following the test-first rule above, first turn the crash into a failing unit test (or a case in `tests/model.rs`) in `libs/engine_handle`, watch it fail, then fix the code. Do not commit crash files or the corpus.
- **Bumping the nightly.** Change `FUZZ_NIGHTLY` in `xtask/src/fuzz.rs` (the only place the date is written; CI and the devcontainer read it through `cargo xtask fuzz-toolchain`), run `cargo xtask fuzz-build`, and mention the bump in the pull request.
- **Why `fuzz/` is outside `cargo xtask deny`.** It is a developer tool that is never built into or shipped with an engine artifact, and `libfuzzer-sys` is licensed `NCSA` (in addition to MIT or Apache-2.0), which is outside the allow list of [ADR 0002](docs/adr/0002-originality-and-dependency-licensing.md). Dependabot watches `fuzz/Cargo.lock` instead, so advisories on its dependencies still surface.

## Coverage

```sh
cargo xtask coverage          # measure, print the per-crate table, write the reports
cargo xtask coverage --open   # the same, then open the HTML report
```

You need `cargo-llvm-cov` and `rustup component add llvm-tools-preview`; the command prints the line for whichever is missing. It runs the workspace tests under instrumentation (the same tests and features as `cargo xtask test --include-ignored`, minus doctests, `xtask` and `engine_benches`) in a separate build under `target/llvm-cov-target/`, so the first run is as slow as a clean build. It writes `target/llvm-cov/html/index.html` (annotated source, for triage), `target/llvm-cov/lcov.info` (for editor gutters) and `target/llvm-cov/summary.md` (the table).

- **What the table counts.** Lines and functions in `libs/*/src/` and `examples/*/src/`. It leaves out doctests (not measured: `cargo-llvm-cov` supports them only on nightly, so code that only a doctest runs shows as uncovered), `tests/`, `benches/` and `xtask/`, and every item marked `#[test]` or `#[cfg(test)]` (including `cfg(all(test, ...))`); the heading says how many instrumented lines that removed. Code under `#[cfg(any(test, feature = "test-invariants"))]` is counted. The HTML and lcov reports are unfiltered apart from the directories, so their percentages are higher.
- **No threshold.** CI's `coverage` job writes the same table to the job summary and uploads the reports for 30 days. It fails only when a test fails, and it is not a required check. Use the report to find behaviour no test exercises, not to raise the number.
- **Two rules.** Library code does not use `cfg(coverage)`, `cfg(coverage_nightly)` or `#[coverage(...)]`, so the instrumented build compiles the same code as the normal one. Unit tests stay in inline `#[cfg(test)]` modules: an out-of-line `#[cfg(test)] mod name;` fails the command, and tests generated by a macro outside any `#[cfg(test)]` item (a top-level `proptest! { ... }` in a `src` file) would be counted as library code.
- **Platforms.** CI measures on Ubuntu only. The command is verified on macOS and Linux, not on Windows.

## Mutation testing

```sh
cargo xtask mutants --list            # check the exclusions and list the mutants; builds nothing
cargo xtask mutants                   # test the triaged crates (engine_handle and engine_random)
cargo xtask mutants -p engine_random  # one crate
cargo xtask mutants -j 2              # two mutants at a time
cargo xtask mutants --in-diff FILE    # only mutants in code that a unified diff changes
```

You need `cargo-mutants` at the pinned version; the command prints the install line when it is missing and warns when the version differs, because results from another version are not comparable. The command checks the exclusion list, runs `cargo mutants` with the settings in `.cargo/mutants.toml` (nextest, all features, ignored tests included, `cap_lints`), writes the output to `target/mutants/mutants.out/` and the table to `target/mutants/summary.md`, and prints it. The first run builds a copy of the workspace per job, so it is as slow as a clean build.

- **What the outcomes mean.** *Caught*: a test failed with the mutant in place, which is what should happen. *Missed*: every test passed with it, so a test is missing or the mutant changes nothing observable. *Timeout*: the tests did not finish in time, usually an endless loop; this detects the mutant. *Unviable*: the mutant does not compile and says nothing about the tests. Doctests do not run (as in coverage), so a mutant only a doctest catches shows as missed.
- **Report only.** A missed mutant never fails anything and there is no target score: a score to hit invites tests written for the score. The command fails only when it could not produce a report. CI's `mutants` workflow runs every mutant on Sundays (on `main`) and on manual dispatch (about 20 minutes). On a pull request that changes the sources of `engine_handle` or `engine_random`, or the mutants tooling, it runs only the mutants in the changed code (`cargo xtask mutants --in-diff FILE`) after checking the exclusion list, which takes a few minutes. Both write the table to the job summary and upload `mutants.out/` for 30 days. Neither is a required check.
- **Ad hoc runs.** A plain `cargo mutants` uses the same settings. For one mutant: `cargo mutants -p engine_handle -F 'shrink_to_fit'`. For changed code: `git diff main > target/changes.diff`, then `cargo mutants --in-diff target/changes.diff`. Read a missed mutant's diff in `target/mutants/mutants.out/diff/`.
- **Triage.** Give each missed mutant exactly one class: *test gap, fixed* (a test at the public-API level that fails with the mutant and passes without it; never one that only pins the mutant's diff), *test gap, deferred* (an issue; the mutant stays missed), *bug* (an issue, fixed in its own pull request, test first), *equivalent* (no input tells it from the original; exclude it) or *not worth a test* (real behaviour no caller depends on, such as a speed-only fast path; exclude it and say why). A readable `Debug` impl is behaviour, not a candidate for exclusion. The record is [docs/mutation-triage.md](docs/mutation-triage.md); it is written by hand.
- **Exclusions.** Only in `exclude_re` in `.cargo/mutants.toml`, never as a `#[mutants::skip]` attribute: library code has no `mutants::` attributes and no `mutants` dependency. The list is written in a restricted form (`exclude_re = [`, one `'single-quoted'` regex per line followed by `,`, `]`), with a comment directly above each entry giving the reason. The command rejects an entry without a reason, an entry that contains a line or column number (a `:` followed by a digit; it would stop matching when the file changes), and an entry that matches no mutant. `cargo xtask mutants --list` prints how many mutants each entry removes; check that against what you meant. `cargo-mutants` never applies a name filter to its "delete field" mutants, so those cannot be excluded.
- **The triaged set** is a constant in `xtask/src/mutants.rs` and grows by chunk: a chunk that triages another crate adds it there and to the record.

## Commits and pull requests

- Use conventional commit messages (`feat:`, `fix:`, `perf:`, `refactor:`, `test:`, `docs:`, `ci:`, `chore:`), and split work into small logical commits that each pass `cargo xtask ci`.
- Explain why, not just what. Mention any change to a public API, a frozen format, or a policy explicitly.
- Add or update an ADR in `docs/adr/` for a decision that is hard to reverse.

## Reviews before a pull request

Before opening a pull request, review the change from each perspective below, whoever or whatever wrote it. Re-run them before marking the pull request ready if the change grew substantially afterwards.

**Always**

- **Code review** (`/code-review`): correctness bugs, checks that cannot fail (a test or fuzz target that passes whatever the code does), missed edge cases, and rule breaks from "What CI enforces".
- **Security review** (`/security-review`): `unsafe`, input handling, dependency and licence changes (new crates, their licences against [ADR 0002](docs/adr/0002-originality-and-dependency-licensing.md), their provenance), and workflows (token permissions, pinned actions, secrets, untrusted input in `run:` steps).
- **Documentation review**, by hand: every new public item is documented; new documents have a status line; relative links resolve; `CHANGELOG.md` and the roadmap row are current; a spec's section numbers stay stable; anything that differs from the plan is in its Deviations section; and a claim in the docs is one you ran, not one you inferred.
- **Spec conformance review**, by hand: for each requirement in the chunk's spec (§5), point to the evidence in its §8 that it holds, and check that what the pull request claims is what was actually run.

**When the change touches it.** These are mandatory too, but you may answer "n/a" with the reason.

| Review | Applies when | Asks |
| --- | --- | --- |
| **Public API** | a public item is added, changed or removed | Is the change deliberate and recorded? Does each panic have `#[track_caller]`, a `# Panics` section, an actionable message and a `try_` twin? Is the type future-proof (`#[non_exhaustive]`, sealed traits)? |
| **Determinism** | library code changes, especially iteration, floats, hashing or anything `usize`-dependent | The lints catch banned names, not unordered iteration, platform-width differences, float reduction order or ambient state. Does it give identical results on aarch64, x86_64, wasm and 32-bit? |
| **Performance** | a hot path changes, or the pull request makes a performance claim | Is there a saved baseline and the noise floor (see "Benchmarks")? |
| **Portability** | `no_std`, wasm, Windows or CI changes | Does it still build and run everywhere the facade promises? |

Triage every finding: fix it, or write in the pull request why you did not. The pull request description has a Reviews section that lists all eight reviews with a one-line result each, or `n/a: <reason>` for a triggered review that does not apply.

## Security

Report vulnerabilities privately, as described in [SECURITY.md](SECURITY.md), not in public issues.
