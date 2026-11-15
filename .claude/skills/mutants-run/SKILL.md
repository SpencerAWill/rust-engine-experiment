---
name: mutants-run
description: Run cargo-mutants (cargo xtask mutants) only inside a memory-capped Docker container with a host-side watchdog, never bare on the maintainer's Mac. Use for any mutation-testing run, or any other long job that executes mutated or untrusted code.
argument-hint: "[cargo xtask mutants flags, e.g. --module random | --in-diff FILE]"
---

# Mutation testing in a capped container

A mutant can make an iterator endless, and a test that collects or formats it then grows memory without bound. On 2026-11-14 a bare `cargo xtask mutants -j 2` on the Mac did exactly that, until the editor had to be force-quit. So mutants never run on the host.

What the outcomes mean and how to triage them: CONTRIBUTING.md, "Mutation testing", and [docs/mutation-triage.md](../../../docs/mutation-triage.md). Tests that walk iterators of code under test follow lesson `testing-1` in [docs/lessons/testing.md](../../../docs/lessons/testing.md).

## Run

1. **Say it is starting**, with the flags and a rough duration (a module: minutes; everything: about 20 minutes on CI, longer here at `-j 1`). Docker Desktop must be running (`docker info`).
2. Check the command first: `sh .claude/skills/mutants-run/run.sh --dry-run $ARGUMENTS` prints the `docker` commands without running them.
3. Start the run in the background: `sh .claude/skills/mutants-run/run.sh $ARGUMENTS`. It builds the image once (Debian trixie, the pinned toolchain, `cargo-nextest`, `cargo-mutants` 27.1.0), reuses named volumes for the cargo registry, rustup and `target/`, and runs `cargo xtask mutants -j 1 $ARGUMENTS` with `--memory=4g --memory-swap=4g`. It starts `watchdog.sh` beside it, which kills the container if it uses over 3.5 GB or the Mac has under 1 GB free.
4. **Read the first results** as they appear. A `TIMEOUT` line means a mutant hung a test; check memory is flat before letting it continue.
5. On completion, `target/mutants/summary.md` holds the table and `target/mutants/mutants.out/` the details. If the watchdog fired, say so with its last memory readings; the results are incomplete.

## Afterwards

Triage missed mutants into `docs/mutation-triage.md` as CONTRIBUTING.md describes. A test that hung or grew memory is a `lesson?` for the chunk's `log.md`: the fix is usually `bounded` / `debug_bounded`.
