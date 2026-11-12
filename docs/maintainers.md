# Maintainer notes

Status: Accepted (2026-11-12). Operational checklists for the repository; the engineering rules are in [CONTRIBUTING.md](../CONTRIBUTING.md).

## Before the repository goes public

- [ ] **Decide the commit email.** Existing commits carry a personal address as author, and it becomes public with the repository. Use GitHub's noreply address for future commits (`git config user.email`). Rewriting history is not planned.
- [ ] **Enable private vulnerability reporting** (Settings, Code security). `SECURITY.md` and the issue chooser point at it.
- [ ] **Add `CODE_OF_CONDUCT.md`** once there is a contact method for conduct reports (the Contributor Covenant needs one).
- [ ] **Branch protection on `main`:** require a pull request, require these status checks (the job names in `ci.yml`): `fmt`, `clippy`, `test`, `doc`, `no-std`, `wasm`, `check-unsafe`, `check-statics`, `check-workflows`, `deny`, `bench-build`, `os-matrix`; no force pushes; require branches to be up to date.
- [ ] **Run the macOS and Windows jobs on every push:** remove the `if: github.event_name == 'pull_request'` condition (and its comment) from `os-matrix` in `ci.yml`. It is only there to save private-repo minutes.
- [ ] Enable Dependabot security updates; set the repository description and topics; decide whether to enable Discussions.
- [ ] **Never use a self-hosted runner for pull requests from forks.** A runner executes the pull request's code. If one is ever added, restrict it to scheduled jobs on `main`.
- [ ] Re-read `docs/` and the history once more for anything private (the last scan found no secrets, private paths, or large binaries).

## Before merging a milestone

- [ ] `cargo xtask ci` passes and the GitHub run is green, including the macOS and Windows jobs.
- [ ] **Remove the implementation plan** (`docs/design/<milestone>/plan.md`) in a final commit on the branch, and fix any links to it. The spec and ADRs stay; the plan is recoverable with `git log`.
- [ ] `CHANGELOG.md` is updated and the spec carries a status line.
- [ ] Merge only on the maintainer's explicit go-ahead.

## Benchmarking

Wall-clock numbers from a shared CI runner are noise, so CI only compiles and smoke-runs the benchmarks (`cargo xtask bench-smoke`). Record real numbers on AC power with other apps closed, keep the machine awake (`caffeinate -i` on macOS), and check the power log afterwards for any power-source change or sleep during the run. Compare runs with `cargo xtask bench-report --baseline NEW --against OLD`, and treat differences under about 5% as noise. Instruction-count regression gating (Gungraun) and a hosted service are planned for M2.
