# Maintainer notes

Status: Accepted (2026-11-12). Operational checklists for the repository; the engineering rules are in [CONTRIBUTING.md](../CONTRIBUTING.md).

## Before the repository goes public

- [ ] **Decide the commit email.** Existing commits carry a personal address as author, and it becomes public with the repository. Use GitHub's noreply address for future commits (`git config user.email`). Rewriting history is not planned.
- [ ] **Enable private vulnerability reporting** (Settings, Code security). `SECURITY.md` and the issue chooser point at it.
- [ ] **Add `CODE_OF_CONDUCT.md`** once there is a contact method for conduct reports (the Contributor Covenant needs one).
- [x] **Branch protection on `main`:** a repository ruleset declared in [`infra/github`](../infra/github/README.md) (Pulumi). It requires a pull request and every job in `ci.yml` as a status check (`os-matrix` once per OS), and blocks force pushes and deletion; the repository admin can bypass it. Change it there, not in the GitHub UI, and keep its `requiredChecks` in step with the job names in `ci.yml`.
- [x] **Run the macOS and Windows jobs on every push:** remove the `if: github.event_name == 'pull_request'` condition (and its comment) from `os-matrix` in `ci.yml`. It is only there to save private-repo minutes.
- [ ] Enable Dependabot security updates; set the repository description and topics; decide whether to enable Discussions.
- [ ] **Never use a self-hosted runner for pull requests from forks.** A runner executes the pull request's code. If one is ever added, restrict it to scheduled jobs on `main`.
- [ ] Re-read `docs/` and the history once more for anything private (the last scan found no secrets, private paths, or large binaries).

## Before merging a chunk

- [ ] `cargo xtask ci` passes and the GitHub run is green, including the macOS and Windows jobs.
- [ ] The pull request title ends with the chunk ID, for example `(M2.1)`.
- [ ] The spec's status is `Implemented (PR #n, date)`; the plan's is `Historical: executed in PR #n (date)`, with its Deviations section filled in (or "None"); the roadmap row is `Done (PR #n)`.
- [ ] `CHANGELOG.md` is updated.
- [ ] Merge only on the maintainer's explicit go-ahead.

## When a milestone completes

- [ ] Tag the commit that completed it: `git tag -a m<N> -m "M<N>: <goal>"`, then `git push origin m<N>`.
- [ ] In the roadmap, mark the milestone `Done` and the next one `Current`, and break the next one into chunks.
- [ ] Decide whether this is the boundary at which to run "Before the repository goes public".

## Benchmarking

Wall-clock numbers from a shared CI runner are noise, so CI only compiles and smoke-runs the benchmarks (`cargo xtask bench-smoke`). Record real numbers on AC power with other apps closed. On macOS `cargo xtask bench` refuses to start on battery power (override with `--allow-battery`) and runs under `caffeinate -i` so the machine cannot idle-sleep, but it cannot stop someone unplugging mid-run, so check the power log afterwards (`pmset -g log`) for any power-source change or sleep during the run. Compare runs with `cargo xtask bench-report --baseline NEW --against OLD`, and treat differences under about 5% as noise. Instruction-count regression gating (Gungraun) is planned for M2 (chunk M2.4); a hosted benchmark service is not scheduled.
