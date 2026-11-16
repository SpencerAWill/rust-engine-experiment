# Contributing

Thanks for your interest. This is an experimental engine: APIs change, and some decisions are deliberately deferred (see the [roadmap](docs/roadmap.md)). For anything beyond a small fix, **open an issue first** so we can agree on the direction before you spend time on it.

## How work is organized

- Work is planned in [docs/roadmap.md](docs/roadmap.md): milestones, broken into **chunks** of exactly one pull request each.
- Each chunk goes through the loop in [docs/workflow.md](docs/workflow.md): its `spec.md`, `plan.md` and `log.md` live in `docs/chunks/<id>-<slug>/`, copied from [docs/chunks/_template/](docs/chunks/_template/). The maintainer approves the spec before implementation starts, and gives the go-ahead to merge.
- Pull request titles end with the chunk ID, for example `feat(genid): add UntypedGenId (M3.11)`.
- Ideas and bugs that are not scheduled go into issues; questions go to [Discussions](https://github.com/SpencerAWill/rust-engine-experiment/discussions). When an issue is scheduled, a chunk spec links it.

## Issues

Blank issues are off. Pick the template that fits:

| You want to | Template | Label |
| --- | --- | --- |
| Report behaviour that differs from the documentation | Bug report | `bug` |
| Report that the same build on the same platform gave different results for the same inputs ([ADR 0003](docs/adr/0003-same-build-determinism.md)) | Determinism bug | `bug`, `determinism` |
| Report a measured slowdown, speedup or memory change | Performance report (needs a saved baseline and the noise floor, see [bench/README.md](bench/README.md#wall-clock-benchmarks)) | `performance` |
| Propose an engine feature, a performance idea you have not measured, or a change to an API or a design decision | Feature request | `enhancement` |
| Propose work on CI, tooling, the repository layout or settings, or this process | Infrastructure or chore | `infrastructure` |
| Ask a question | none: use Discussions | |
| Report a vulnerability | none: report it privately ([SECURITY.md](SECURITY.md)) | |

The templates are issue forms in `.github/ISSUE_TEMPLATE/`; the labels they assign are declared in `infra/github/index.ts`, so a new label goes there too. They ask for the outcome and how it is verified, which is what lets an issue be scheduled. How an issue becomes a chunk is in [docs/workflow.md](docs/workflow.md#from-issue-to-chunk). To work on an open issue, assign yourself or comment first, so the work is not done twice; issues marked `help wanted` or `good first issue` are the ones open for it.

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

`cargo xtask` prints an install hint when a tool is missing. The extra tools are `cargo-nextest`, `cargo-deny`, `critcmp`, `wasmtime`, and, for fuzzing, `cargo-fuzz` plus a pinned nightly toolchain, and, for coverage, `cargo-llvm-cov` plus the `llvm-tools-preview` component, and, for mutation testing, `cargo-mutants` (`cargo install --locked cargo-mutants --version 27.1.0`), and, for the instruction-count gate, `valgrind` and `gungraun-runner` (`cargo install --locked gungraun-runner --version "$(cargo xtask gungraun-version)"`; the runner must have the same version as the `gungraun` crate in `Cargo.lock`, and `cargo xtask bench-smoke` needs it too); `.devcontainer/install-tools.sh` installs them.

## What CI enforces

These are policy, not preferences. A pull request that breaks one will not be merged:

- **One library crate.** New library code goes in a module of `engine`. A separate crate needs one of the criteria in [ADR 0004](docs/adr/0004-crate-splitting-rule.md), named with its evidence in the spec of the chunk that needs it.
- **No `unsafe`.** Every crate root has `#![forbid(unsafe_code)]` (`check-unsafe`). An exception needs an ADR and an entry in `unsafe-allowlist.toml`.
- **Determinism.** No `std` `HashMap`/`HashSet`/`RandomState`, `Instant::now`, `SystemTime::now`, `thread_local!`, or inherent transcendental float methods in library code (see `clippy.toml`; transcendentals go through `engine::math::scalar`). Simulation state must be snapshot-able plain data.
- **No global mutable state** (`check-statics`, [ADR 0008](docs/adr/0008-no-global-state.md)).
- **Frozen formats.** The PRNG output streams and `UntypedGenId::to_bits` (and `GenId::to_bits`, which forwards to it) are frozen for all of `0.x`. Changing one needs an ADR and updated golden tests, not just a code change.
- **Documentation.** Every public item is documented and every public type has a runnable doctest.
- **Panics.** A documented panic needs `#[track_caller]`, a `# Panics` section, an actionable message, and a fallible `try_` twin.
- **Workflows** (`check-workflows`): actions pinned to commit hashes, a read-only token (a job-level `write` scope only if it is in `WRITE_ALLOWLIST`, with a reason), no persisted checkout credentials, no `pull_request_target`.
- Formatting (`cargo fmt`), clippy with warnings denied, docs without warnings, and `no_std` and WebAssembly builds.

## Tests

Changes come with tests. For a bug fix, write the test first and confirm it fails for the reason you expect, then fix the code. Prefer testing behaviour over implementation. The layers in use are unit tests, proptest model tests against a simple reference, golden values, and doctests. Do not weaken or delete a failing test without saying so in the pull request.

- **A fuzz crash is a bug.** Turn it into a failing unit test (or a case in `libs/engine/tests/genid_model.rs`) first, watch it fail, then fix the code.
- **Unit tests stay in inline `#[cfg(test)]` modules.** `cargo xtask coverage` fails on an out-of-line `#[cfg(test)] mod name;`, and tests generated by a macro outside any `#[cfg(test)]` item (a top-level `proptest! { ... }` in a `src` file) would be counted as library code.
- **No coverage or mutation attributes in library code:** no `cfg(coverage)`, `cfg(coverage_nightly)` or `#[coverage(...)]`, so the instrumented build compiles the same code as the normal one; no `mutants::` attributes and no `mutants` dependency, because mutants are excluded only in `.cargo/mutants.toml`.

## Tools

How to run each tool, what its output means and its rules live next to it:

| Tool | Manual |
| --- | --- |
| Every `cargo xtask` command | [xtask/README.md](xtask/README.md) |
| Wall-clock benchmarks | [bench/README.md](bench/README.md#wall-clock-benchmarks) |
| The instruction-count gate (`bench-regress`, the `perf-regression-accepted` label) | [bench/README.md](bench/README.md#instruction-count-gate) |
| Fuzzing | [fuzz/README.md](fuzz/README.md) |
| Coverage | [xtask/README.md](xtask/README.md#coverage) |
| Mutation testing and triage | [xtask/README.md](xtask/README.md#mutation-testing) |
| Spike crates | [spikes/README.md](spikes/README.md) |

## Commits and pull requests

- Use conventional commit messages (`feat:`, `fix:`, `perf:`, `refactor:`, `test:`, `docs:`, `ci:`, `chore:`), and split work into small logical commits that each pass `cargo xtask ci`.
- Explain why, not just what. Mention any change to a public API, a frozen format, or a policy explicitly.
- Add or update an ADR in `docs/adr/` for a decision that is hard to reverse.
- Pull requests are squash-merged, the only merge method `main` allows. The pull request title becomes the commit subject on `main`, so it MUST be a conventional commit message (for a chunk, ending with the chunk ID), and the branch's commit messages become its body. Small logical commits are still wanted on the branch, because reviewers read them.

## Reviews before a pull request

Before opening a pull request, review the change from each perspective below, whoever or whatever wrote it. Re-run them before marking the pull request ready if the change grew substantially afterwards. In the chunk loop each review is done by an agent that did not write the change, after a `/simplify` pass, using the reviewer files in `.claude/agents/` (named below); a person can run the same checklist by hand.

**Always**

- **Code review** (`review-code`): correctness bugs, checks that cannot fail (a test or fuzz target that passes whatever the code does), missed edge cases, and rule breaks from "What CI enforces".
- **Security review** (`review-security`): `unsafe`, input handling, dependency and licence changes (new crates, their licences against [ADR 0002](docs/adr/0002-originality-and-dependency-licensing.md), their provenance), and workflows (token permissions, pinned actions, secrets, untrusted input in `run:` steps; a new `write` token scope needs an entry with a reason in `WRITE_ALLOWLIST` in `xtask/src/check_workflows.rs`).
- **Documentation review** (`review-docs`): every new public item is documented; new documents have a status line; relative links resolve; `CHANGELOG.md` is current; the chunk is closed out in this pull request (spec `Implemented (PR #n, date)`, plan and log `Historical`, roadmap row `Done (PR #n)`, the spec's done-when list ticked with evidence), never in a documentation-only pull request after the merge; a spec's section numbers stay stable; anything that differs from the plan is in its Deviations section; and a claim in the docs is one you ran, not one you inferred.
- **Spec conformance review** (`review-spec`): for each requirement in the chunk's spec (§5), point to the evidence in its §8 that it holds, and check that what the pull request claims is what was actually run.

**When the change touches it** (`review-conditional`, told which apply). These are mandatory too, but you may answer "n/a" with the reason.

| Review | Applies when | Asks |
| --- | --- | --- |
| **Public API** | a public item is added, changed or removed | Is the change deliberate and recorded? Does each panic have `#[track_caller]`, a `# Panics` section, an actionable message and a `try_` twin? Is the type future-proof (`#[non_exhaustive]`, sealed traits)? |
| **Determinism** | library code changes, especially iteration, floats, hashing or anything `usize`-dependent | The lints catch banned names, not unordered iteration, platform-width differences, float reduction order or ambient state. Does it give identical results on aarch64, x86_64, wasm and 32-bit? |
| **Performance** | a hot path changes, or the pull request makes a performance claim | Is there a saved baseline and the noise floor ([bench/README.md](bench/README.md#wall-clock-benchmarks))? When `perf-regression-accepted` is used: does the description name the benchmarks it covers, and do they match the final run's "Accepted by label" list ([bench/README.md](bench/README.md#instruction-count-gate))? |
| **Portability** | `no_std`, wasm, Windows or CI changes | Does it still build and run everywhere `engine` promises? |

Triage every finding: fix it, or write in the pull request why you did not. The pull request description has a Reviews section that lists all eight reviews with a one-line result each, or `n/a: <reason>` for a triggered review that does not apply.

## Security

Report vulnerabilities privately, as described in [SECURITY.md](SECURITY.md), not in public issues.
