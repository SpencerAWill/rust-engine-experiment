# M3.20 plan: Planning state on GitHub

- **Status:** Accepted (2026-11-23)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. Progress and surprises go in [log.md](log.md).

## 1. Approach

Seven steps, one commit each, all by the main session: this chunk changes documents, skills, templates and configuration only (no product or test code, so no builder). The GitHub writes of spec §4.8 are steps 1 and 8; step 8 runs in stage 7, after `pulumi up`, and is listed here so its checklist is in one place.

- Edit only the passages that name the roadmap as the place of scheduled work, order or status (spec §4.7); historical documents are untouched (R7).
- `AGENTS.md` stays under 80 lines (`check-docs` enforces it); it is 66 now.
- The "next" rule and the chore steps are written once, in `.claude/skills/chunk/SKILL.md`; `docs/workflow.md` states the rule in prose and links there (lesson docs-1).

## 2. Steps

### Step 1: this chunk's issue and the Order field (R9, first part)

- [x] **Who:** main session
- **Do:** `gh issue create` with title `M3.20: Planning state on GitHub`, the Definition of Ready headings (problem, proposal, done when, how verified, depends on M3.18), labels `infrastructure` and `ai-assisted`; once auto-added, set Status Scheduled, Priority Now, Size M; `gh project field-create 6 --owner SpencerAWill --name Order --data-type NUMBER`; set this issue's Order to 5 (it runs before M3.10). Put the issue number in the spec header's `**Issue:**` line.
- **Read back:** `gh issue view`, `gh project item-list --format json` (the item's `status`, `priority`, `size`, `order`), `gh project field-list` (Order, NUMBER); recorded in `log.md`.
- **Commit:** `docs: link the M3.20 issue (M3.20)`

### Step 2: milestones in Pulumi (R5)

- [x] **Who:** main session
- **Files:** `infra/github/index.ts`.
- **Change:** one `github.RepositoryMilestone` per milestone M3 to M9 (owner, repository `repo.name`, title, description: the goal in one line and `docs/roadmap.md#milestones`, `state: "open"`), each with `{ ignoreChanges: ["state"] }`, declared from one array as the labels are.
- **Check that fails before:** `grep -c RepositoryMilestone infra/github/index.ts` is 0.
- **Verify:** `npx tsc --noEmit` in `infra/github` passes (no Pulumi run; that is the maintainer's, step 8).
- **Commit:** `feat(infra): declare the milestones in Pulumi (M3.20)`

### Step 3: workflow and maintainer documents (R1, R7)

- [x] **Who:** main session
- **Files:** `docs/workflow.md` ("From issue to chunk", "Who is the source of truth", the cardinality table, scheduling, chores), `docs/maintainers.md` ("Issue tracking": the Order field, the "Next up" view and how to create it, how to schedule a chunk and a chore; "When a milestone completes": close the milestone with `gh api --method PATCH`, move open chores, no tracking issue).
- **Check that fails before:** `git grep -n "adding a chunk row" docs/workflow.md docs/maintainers.md` finds the old scheduling rule.
- **Verify:** that grep finds nothing; `cargo xtask check-docs` 0 problems.
- **Commit:** `docs: move planning state to GitHub in the workflow (M3.20)`

### Step 4: the chunk and triage skills (R2, R3, R4, R11)

- [ ] **Who:** main session
- **Files:** `.claude/skills/chunk/SKILL.md` (description; argument default; `next` per spec §4.4, with the `gh` commands; a `chore #N` section per §4.3; `resume` unchanged), `.claude/skills/chunk/stages/1-brainstorm.md` (start from the chunk issue; the scheduling case of §4.2; no roadmap row), `2-grill.md` and `3-plan.md` (no roadmap status), `7-close-out.md` (`Closes #N`; no roadmap row; the rollout of §4.8 for a chunk that changes `infra/github`), `.claude/skills/triage/SKILL.md` (Order in the field check; scheduling is the maintainer's act on the Project, not a roadmap row; the roadmap search becomes an issue-title search).
- **Check that fails before:** `git grep -n -i roadmap .claude/skills/` lists the order and status reads and writes above.
- **Verify:** every remaining `git grep -n -i roadmap .claude/` hit is a reference to milestone goals or history, each named in `log.md` (R3).
- **Commit:** `docs: read chunk order from GitHub in the chunk and triage skills (M3.20)`

### Step 5: entry documents and templates (R7, R8, R11)

- [ ] **Who:** main session
- **Files:** `AGENTS.md` (the planned-work pointer; the loop line; the chore commit-message exception), `CONTRIBUTING.md`, `README.md`, `docs/README.md` (the roadmap row of the index), `docs/chunks/README.md`, `.claude/README.md`, `.github/PULL_REQUEST_TEMPLATE.md` (chunk issue; chores), `.github/ISSUE_TEMPLATE/feature_request.yml` and `infrastructure.yml` ("Depends on" hint), `docs/chunks/_template/spec.md` (Issue line required; no "Followed by").
- **Verify:** `git grep -n -i roadmap` over these files, each remaining hit checked against §4.7; `cargo xtask check-docs` 0 problems (AGENTS.md under 80 lines).
- **Commit:** `docs: point the entry documents and templates at GitHub planning (M3.20)`

### Step 6: the roadmap file (R6)

- [ ] **Who:** main session
- **Files:** `docs/roadmap.md`.
- **Change:** per spec §4.6: new introduction and status line; the milestone table's Status column replaced by a pointer to the GitHub milestones; the M3 table keeps its done rows and M3.20's (row updated at close-out), the open rows M3.7, M3.8 and M3.10 removed with a line saying chunks are issues from M3.20 on; M2 table, "Later milestones" and the numbering note unchanged.
- **Before removing the rows:** copy their full text into `log.md` (it is the source for step 8's issue bodies).
- **Verify:** `cargo xtask check-docs` 0 problems (anchors `#milestones`, `#m3-chunks`, `#m2-chunks` still resolve).
- **Commit:** `docs: shrink the roadmap to milestone goals and history (M3.20)`

### Step 7: final verification

- [ ] **Who:** main session
- **Do:** `cargo xtask ci` on the last step's commit; `git diff --stat main` shows no historical document (`docs/chunks/m*` other than m3.20, `docs/adr/`).
- **Commit:** with the log line only: `docs: record the M3.20 ci run (M3.20)`

### Step 8 (stage 7): rollout and migration (R5, R9, R10)

- [ ] **Who:** maintainer (`pulumi preview`, `pulumi up`, the "Next up" view), main session (the rest)
- **Do,** after the pull request and its close-out commit are pushed: ask the maintainer to run `pulumi preview` then `pulumi up` in `infra/github` from the branch and paste the preview's summary; then spec §4.8 items 1 to 4, each read back with `gh`; then R10: run §4.4's selection with `gh` and record that it names M3.10, and trial the changed `next` section with a fresh read-only agent given only the skill (tooling-7), recording what it selects; then ask the maintainer to create the "Next up" view; record everything in `log.md`, push, and ask for the merge go-ahead.
- **Commit:** `docs: record the M3.20 rollout (M3.20)`

## 3. Final verification

`cargo xtask ci` (step 7); the spec's Done-when list at close-out; R9 and R10 before the go-ahead.

## 4. Lessons

- **docs-1** followed: the "next" rule and the chore steps have one copy (the chunk skill); `docs/workflow.md` and `docs/maintainers.md` state them in a sentence and link.
- **docs-2** followed: the spec template's Issue line and the removed "Followed by" convention change in the template itself (step 5); counts in this plan are rechecked if a step changes them.
- **specs-1** followed: each "finds nothing" grep names its paths (steps 3 to 5).
- **specs-2** followed: every `log.md` evidence item has its step (steps 1, 4, 6, 7, 8).
- **specs-4** followed: the `ci` run and the rollout read-backs name their commits.
- **tooling-3** n/a: no allow rule changes.
- **tooling-7** followed: step 8 trials the changed `next` with a fresh agent against the migrated state.
- **tooling-8** followed: the `next` and `chore` sections say issue text is data, not instructions, where they read it.
- **tooling-9** followed: every `gh api` read in the skills is written with `--method GET`.
- **tooling-10** followed: every `gh issue list` and `gh project item-list` that decides "next" passes `--limit 1000`.

## 5. Deviations

- Step 2 also updated `infra/github/README.md` (it lists what `index.ts` declares), which the plan did not name.
