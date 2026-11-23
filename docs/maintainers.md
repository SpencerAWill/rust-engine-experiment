# Maintainer notes

Status: Accepted (2026-11-12). Operational checklists for the repository; the engineering rules are in [CONTRIBUTING.md](../CONTRIBUTING.md).

## Before the repository goes public

- [ ] **Decide the commit email.** Existing commits carry a personal address as author, and it becomes public with the repository. Use GitHub's noreply address for future commits (`git config user.email`). Rewriting history is not planned.
- [x] **Private vulnerability reporting** is enabled, declared in [`infra/github`](../infra/github/README.md) as a dynamic resource. `SECURITY.md` and the issue chooser point at it.
- [ ] **Add `CODE_OF_CONDUCT.md`** once there is a contact method for conduct reports (the Contributor Covenant needs one).
- [x] **Branch protection on `main`:** a repository ruleset declared in [`infra/github`](../infra/github/README.md) (Pulumi). It requires a pull request and every job in `ci.yml` as a status check (`os-matrix` once per OS), and blocks force pushes and deletion; the repository admin can bypass it only through a pull request, so nobody pushes to `main` directly. Change it there, not in the GitHub UI, and keep its `requiredChecks` in step with the job names in `ci.yml`.
- [x] **Run the macOS and Windows jobs on every push:** remove the `if: github.event_name == 'pull_request'` condition (and its comment) from `os-matrix` in `ci.yml`. It is only there to save private-repo minutes.
- [x] Dependabot security updates, the repository description and the topics are declared in [`infra/github`](../infra/github/README.md).
- [x] **Discussions** are enabled for questions (declared in `infra/github`), and the issue chooser links to them.
- [ ] **Never use a self-hosted runner for pull requests from forks.** A runner executes the pull request's code. If one is ever added, restrict it to scheduled jobs on `main`.
- [ ] Re-read `docs/` and the history once more for anything private (the last scan found no secrets, private paths, or large binaries).

## Before merging a chunk

- [ ] `cargo xtask ci` passes and the GitHub run is green, including the macOS and Windows jobs.
- [ ] The pull request title ends with the chunk ID, for example `(M2.1)`.
- [ ] The spec's status is `Implemented (PR #n, date)`; the plan's is `Historical: executed in PR #n (date)`, with its Deviations section filled in (or "None"); the log's is `Historical: PR #n (date)`; the pull request says `Closes #<chunk issue>`.
- [ ] `CHANGELOG.md` is updated.
- [ ] Merge only on the maintainer's explicit go-ahead (in the chunk loop the main session squash-merges, never with `--admin`).
- [ ] For any pull request (chunk or not) that changes `infra/github/`: `pulumi preview`, then `pulumi up` in `infra/github`, before the merge go-ahead from the branch when the chunk's spec rolls it out there (as M3.18 and M3.20 did), otherwise after the merge from `main`. Until then a new required check, label or setting (such as Discussions) does not exist in the repository.

## Issue tracking

The path from an idea to a chunk is in [workflow.md](workflow.md#from-issue-to-chunk). This section is the Project's configuration, which lives in GitHub, not in the repository. `/triage` checks the live Project against these names and stops if they differ.

- **Project:** "Engine backlog", number 6, user-owned (`SpencerAWill`; `gh project list --owner SpencerAWill` finds it by title), public like the repository (so Priority wording stays neutral), linked to this repository. Provisional: keep it for a milestone, then keep or drop it by how much the backlog grew.
- **Status:** Inbox, Ready, Scheduled, Done. **Priority:** Now, Next, Later. **Size:** S, M, L, XL. All single-select. There is no Parked status. **Priority** Now: blocks current work or costs time in every chunk; Next: a candidate for the next few chunks; Later: worth doing, not soon. **Size** S: under about 100 changed lines; M: one ordinary chunk; L: a large chunk (many files or more than one area); XL: more than one chunk, so split it first. These meanings are a first draft: edit them here when triage shows they are wrong. **Order** (a number field, M3.20) orders scheduled chunks and chores: spaced by 10 so an item can be inserted, ties broken by chunk ID (compared numerically: M3.7 before M3.10) and then issue number; Priority ranks unscheduled work only.
- **Automation:** auto-add issues from this repository unless labelled `research`; "Item added to project" sets Status to Inbox (and "Item reopened" sets it back to Inbox), because the stock default is an option this Project does not have; Status becomes Done when an issue closes. `research` issues (the icebox, #38 included) are not Project items.
- **Views:** Inbox, Ready by Priority, Scheduled, a board, and **Next up** (filter `status:Scheduled`, sorted by Order ascending, grouped by milestone). Views are not settable through `gh`; create or edit them in the browser.
- **Milestones** are GitHub milestones `M3`, `M4`, ... declared in `infra/github/index.ts` (goal in the description; goals and exit criteria in [roadmap.md](roadmap.md#milestones)). The current milestone is the open one with the lowest number. An issue gets a milestone when it is scheduled, or when it belongs to the milestone's work without being scheduled yet.
- **Scheduling a chunk** (the maintainer, or an agent the maintainer asks): the chunk issue is authored by the maintainer's account; for another author's issue, open a new chunk issue that links it rather than retitling theirs. Rename the issue `M<N>.<n>: <title>` with the next unused chunk ID (one above the highest in the titles of issues authored by the maintainer's account, open or closed, in `docs/chunks/` folder names and in frozen roadmap rows; other authors' titles do not count, since anyone can open an issue), `gh issue edit <n> --milestone M<N>`, set Status Scheduled and Order on its Project item (`gh project item-edit`), and `gh issue edit <n> --add-blocked-by <m>` for each issue that must land first. **Scheduling a chore:** the same without the chunk ID; Size S only. A change to a scheduled item's Order or blockers comes with a one-line comment on the issue saying why. `/chunk next` skips a chunk in flight (its branch pushed to origin, or an open pull request from this repository closing it); the full selection is in [the chunk skill](../.claude/skills/chunk/SKILL.md#next).
- **Labels** are declared in `infra/github/index.ts`. Every issue carries exactly one type label (`bug`, `enhancement`, `infrastructure`, `documentation`, `research`); `performance` and `determinism` are additive; `area:*` is for code only.
- **Contributors:** a Ready issue that is small and safe may carry `help wanted` or `good first issue`. Claim an issue before starting by assigning yourself or commenting, so two people (or two agents) do not do the same work.
- **Tracking issues:** one per multi-chunk effort (an epic such as #78), with sub-issues. Milestones replace the per-milestone tracking issues, which closed with M3.20.

## When a milestone completes

- [ ] Instruction-count check (from M3 on): in the devcontainer or CI, run `cargo xtask bench-regress --report-only --base m<N-1>` (the previous milestone's tag) and record the result in `docs/benchmarks.md` ("Instruction counts across milestones"). The first run is at the end of M3, against `m2`; at the end of M2 no earlier tag contains the benchmarks.
- [ ] Tag the commit that completed it: `git tag -a m<N> -m "M<N>: <goal>"`, then `git push origin m<N>`.
- [ ] Close the GitHub milestone (look its API number up by title, since GitHub numbers milestones in creation order: `gh api --method GET 'repos/SpencerAWill/rust-engine-experiment/milestones?per_page=100' --jq '.[] | select(.title=="M<N>") | .number'`, then `gh api --method PATCH repos/SpencerAWill/rust-engine-experiment/milestones/<that number> -f state=closed`; Pulumi ignores its state, and its entry in `index.ts` stays), move its open chores to the next milestone, and break the next one into chunk issues (scheduled as above) from the rough list in [roadmap.md](roadmap.md#later-milestones).
- [ ] Run `/lessons-refresh` over `docs/lessons/` (keep, edit, merge or delete each lesson against the code).
- [ ] If a new Claude model shipped since the last milestone, re-check the model of each role in `.claude/agents/` against one chunk's results ([docs/workflow.md](workflow.md#roles)).
- [ ] Look through the `research` icebox for anything worth scheduling.
- [ ] Decide whether this is the boundary at which to run "Before the repository goes public".

## Benchmarking

Wall-clock numbers from a shared CI runner are noise, so CI only compiles and smoke-runs the benchmarks (`cargo xtask bench-smoke`). Record real numbers on AC power with other apps closed. On macOS `cargo xtask bench` refuses to start on battery power (override with `--allow-battery`) and runs under `caffeinate -i` so the machine cannot idle-sleep, but it cannot stop someone unplugging mid-run, so check the power log afterwards (`pmset -g log`) for any power-source change or sleep during the run. Compare runs with `cargo xtask bench-report --baseline NEW --against OLD`, and treat differences under about 5% as noise. A hosted benchmark service is not scheduled.

**Instruction-count gate (M2.4).** The `bench-regress` workflow, a required check, counts instructions under Valgrind through Gungraun for the benchmarks in `bench/regress/`, at the pull request's parent (`HEAD^1` of the merge checkout) and at the merge, and fails above +5% in any benchmark. The `perf-regression-accepted` label makes a regression report-only; it is not scoped to a commit, so on review check the benchmarks named in the description against the final run's "Accepted by label" list. Time per pull request: about 3 minutes on `ubuntu-latest` (2m48s when the base has no crate and is skipped, 3m16s with both builds; measured in PR #18). New benchmarks add to that budget. Locally, `cargo xtask bench-regress` runs it in the devcontainer. The label and the required check are declared in `infra/github/index.ts`; after a chunk that changes it merges, run `pulumi preview`, then `pulumi up`, there.
