# M3.18 plan: Issue tracking

- **Status:** Accepted (2026-11-16)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. Progress and surprises go in [log.md](log.md).

## 1. Approach

No product or test code changes, so there is no builder: the main session does every step (documentation, skill, configuration, GitHub state). Steps 1 to 5 are commits on the branch (stage 4). Steps 6 to 9 are the rollout of spec 4.10, run in stage 7 after the pull request is open and before the go-ahead; each leaves its evidence in `log.md` and is its own step because its evidence is otherwise forgotten (specs-2).

Choices the spec leaves to implementation:

- The vocabulary, path and cardinalities (spec §3) live once, in a new "From issue to chunk" section of `docs/workflow.md`. `CONTRIBUTING.md` gets a short section that links to it (docs-1). The exact Project field and option names live once, in a new "Issue tracking" section of `docs/maintainers.md`; `/triage` reads that section for its start-up check, so the names are not copied into the skill.
- The three issue forms replace the Markdown templates (same labels, same wording where it exists). The two existing YAML forms are unchanged.
- `ai-assisted` is applied by `/triage` (a form cannot label conditionally); the forms carry a checkbox that tells it which issues to label.

## 2. Steps

### Step 1: declare the new labels (R2)

- [x] **Who:** main session
- **Files:** `infra/github/index.ts`
- **Test first:** `git grep -c 'area:collections' infra/github/index.ts` prints nothing and exits 1 (no such label yet).
- **Change:** add `area:collections`, `area:genid`, `area:random`, `area:time`, `area:math`, `area:bench`, `area:xtask`, `needs-measurement` and `ai-assisted`, each with a colour and description, in the style of the existing entries; the `area:` ones under one comment. Update the "Applied by hand" comment to say `area:`, `needs-measurement` and `ai-assisted` are applied by `/triage`.
- **Verify:** `git grep -c` for each of the nine names finds one line each; the TypeScript type-checks with the command in `infra/github/README.md` (run it if the dependencies are installed; if they are not, say so in `log.md` and rely on step 7's `pulumi preview`).
- **Commit:** `feat(infra): declare area, needs-measurement and ai-assisted labels (M3.18)`

### Step 2: issue forms (R5)

- [x] **Who:** main session
- **Files:** `.github/ISSUE_TEMPLATE/bug_report.yml`, `feature_request.yml`, `infrastructure.yml` (new); `bug_report.md`, `feature_request.md`, `infrastructure.md` (deleted)
- **Test first:** `ls .github/ISSUE_TEMPLATE/*.yml` lists only the two existing forms; a check that every file in the folder other than `config.yml` is a `.yml` form with a `name`, `description`, `labels` and `body` fails on the three `.md` files. (Ruby's `yaml` instead of Python's, which is not installed; the script is in the session scratchpad, not the repository.)
- **Change:** each form keeps its current label and its current prompts as fields. `feature_request.yml` and `infrastructure.yml` gain required fields for the problem, the outcome that would count as done, the verification (a command or a named check) and dependencies (optional); `bug_report.yml` keeps its reproduction as the verification. Each ends with an optional checkbox "An assistant drafted this issue". The `labels` are unchanged: `bug`, `enhancement`, `infrastructure`.
- **Verify:** `python3 -c` loading every form with `yaml.safe_load` and asserting the keys of the check above exits 0; the chooser (`config.yml`, unchanged) still lists blank issues off.
- **Commit:** `feat(github): turn the bug, feature and infrastructure templates into issue forms (M3.18)`

### Step 3: the `/triage` skill (R3, R4)

- [x] **Who:** main session
- **Files:** `.claude/skills/triage/SKILL.md` (new), `.claude/README.md` (Contents entry)
- **Test first:** `cargo xtask check-docs` passes on the branch before the step (the baseline), and the skill's start-up check, run by hand against the expected names in step 6, is the failing case recorded in step 9.
- **Change:** frontmatter (`name`, `description`, `argument-hint`) in the style of `grill`; the body states the rule (no build, test, benchmark or container command; no file edits), the start-up check (compare the Project's fields and options with the section of `docs/maintainers.md` and stop naming the difference), the loop (list the Inbox; for each issue propose labels, Priority, Size, a definition meeting spec 4.4, possible duplicates and, for an external issue, a short reply; one batch for confirmation; then write), and the exact allowed writes of spec 4.6.
- **Verify:** `cargo xtask check-docs` exits 0.
- **Commit:** `feat(claude): add the /triage skill (M3.18)`

### Step 4: hand-off in the chunk loop (spec 4.7)

- [x] **Who:** main session
- **Files:** `.claude/skills/chunk/stages/1-brainstorm.md`
- **Test first:** `git grep -c Scheduled .claude/skills/chunk/stages/1-brainstorm.md` finds nothing.
- **Change:** step 1 also reads the roadmap row's linked issue; step 2 also sets that issue's Project Status to Scheduled when the row goes to `Drafting`. Two sentences, no other edit.
- **Verify:** the same grep finds one line; `cargo xtask check-docs` exits 0.
- **Commit:** `feat(claude): read the linked issue and mark it Scheduled in chunk stage 1 (M3.18)`

### Step 5: documentation (R1, R6)

- [x] **Who:** main session
- **Files:** `docs/workflow.md`, `docs/maintainers.md`, `CONTRIBUTING.md`, `AGENTS.md` (one line, only if `check-docs`'s length limit allows), `docs/README.md` (only if a Contents entry is needed)
- **Test first:** `git grep -n 'From issue to chunk' docs CONTRIBUTING.md` finds nothing.
- **Change:** `docs/workflow.md`: the "From issue to chunk" section (vocabulary, the path, the sources of truth, the cardinalities table, the split-a-chunk rule, the machine-light rule for `/triage`), adapted from spec §3 and linking the spec for the reasoning. `docs/maintainers.md`: an "Issue tracking" section with the Project's name, field names and options (exactly spec 4.2), its automation and default views, the claim convention for contributors, when `help wanted` and `good first issue` apply, and one item on the milestone checklist (close the milestone's tracking issue, review the `research` icebox). `CONTRIBUTING.md`: three lines under Issues linking to the workflow section, naming the type-label rule and the claim convention. The Definition of Ready (spec 4.4) is written once, in `docs/workflow.md`.
- **Verify:** `cargo xtask check-docs` exits 0; `git grep -n 'Inbox, Ready, Scheduled, Done' -- docs CONTRIBUTING.md .claude ':!docs/chunks'` finds exactly one place (`docs/maintainers.md`).
- **Commit:** `docs: describe the path from issue to chunk and the issue tracking setup (M3.18)`

### Step 6: create the Project and test `hasProjects` (R1) · stage 7, after the PR is open

- [ ] **Who:** main session, with the maintainer for the token scope
- **Files:** `infra/github/index.ts` only if the test fails
- **Test first:** `gh project list` fails with the missing `read:project` scope; the maintainer runs `! gh auth refresh -s project -s read:project` and it then succeeds.
- **Change:** create the public "Engine backlog" Project with the fields and options of `docs/maintainers.md`, link it to the repository, and enable the built-in workflows listed under Automation in `docs/maintainers.md` and the default views as far as `gh` and the web interface allow (say in `log.md` what was done by hand). If linking fails with `hasProjects: false`, set it to `true` in `index.ts`, update the comment above it, and commit `fix(infra): enable repository Projects for the issue tracking Project (M3.18)`.
- **Verify:** `gh project field-list <n> --owner SpencerAWill` output pasted into `log.md`, with the names equal to the section in `docs/maintainers.md`.
- **Commit:** the log, plus the `index.ts` change if one was needed.

### Step 7: `pulumi up` (R2) · stage 7

- [ ] **Who:** the maintainer runs it; the main session checks
- **Test first:** `gh label list --limit 100` shows none of the nine new labels.
- **Change:** the maintainer runs `pulumi preview`, then `pulumi up`, in `infra/github`.
- **Verify:** `gh label list` shows all nine, and still shows every label from before (nothing deleted that an issue uses); recorded in `log.md`.
- **Commit:** the log.

### Step 8: migration (R7) · stage 7

- [ ] **Who:** main session, after the maintainer confirms the batch (the same propose-then-confirm as `/triage`)
- **Test first:** `gh issue list --state open --json number,labels` shows issues without exactly one type label (at least #27 and #30), and `gh project item-list --limit 1000` shows no items.
- **Change:** per spec 4.9: type labels (#27 and #30 get `infrastructure`), `area:` labels where one applies, Project items and fields for the non-`research` issues, the M3 and M4 tracking issues with the existing chunk issues as sub-issues where they apply, #38 left standalone.
- **Verify:** a query printing every open issue with its type-label count prints 1 for all; every open issue without `research` has a Status in `gh project item-list --limit 1000`; both outputs in `log.md`.
- **Commit:** the log.

### Step 9: dry runs (R3, R4) · stage 7

- [ ] **Who:** main session
- **Test first:** none (this is the evidence step).
- **Change:** create a throwaway private Project with one option renamed, run `/triage` against it and confirm it stops naming the difference, then delete the Project. Then dispatch a fresh read-only agent to follow `.claude/skills/triage/SKILL.md` against the real Project (tooling-7) and confirm it proposes without writing and runs no build command. Also look up, without writing, the field and option ids that chunk stage 1's `gh project item-edit` needs for one item, and note the commands in `log.md`.
- **Verify:** both transcripts' relevant lines in `log.md`; `gh project list` no longer shows the throwaway Project.
- **Commit:** the log.

## 3. Final verification

`cargo xtask ci`, then the spec's Done-when checklist: R1 to R8 each with its evidence in `log.md` or the diff; the pull request says an assistant was used and has its Reviews section; the maintainer has run `pulumi up` (step 7).

## 4. Lessons

- **docs-1:** followed. The path, vocabulary and Definition of Ready are written once in `docs/workflow.md`; the Project's field names once in `docs/maintainers.md`; `CONTRIBUTING.md`, the skill and `AGENTS.md` link to them.
- **specs-1:** followed. The step 5 check that a string appears in exactly one place names its paths (`docs CONTRIBUTING.md .claude`).
- **specs-2:** followed. Each piece of evidence the spec says is noted in `log.md` (steps 6 to 9) is its own step.
- **tooling-7:** followed. Step 9 runs the new skill with a fresh read-only agent against the real Project, not only by hand.
- tooling-1 to tooling-6: not applicable (no hook, permission or script change).

## 5. Deviations

- **Step 5 ran before step 3.** The skill links to the two sections step 5 adds, so `check-docs` could not pass for step 3 first. The commits are in the order 1, 2, 5, 3, 4.
- **Stage 5 simplify pass skipped:** the diff has no code (Markdown, YAML, one skill, one Pulumi list).
- **`CONTRIBUTING.md` links, it does not state the type-label rule** (step 5 said it would name it): the rule lives once, in `docs/maintainers.md` (docs-1).
- **Project visibility is public, not private** (spec 4.2 did not say; I had assumed private "until public" but the repository is already public): maintainers.md and step 6 say public; flagged for the maintainer at the go-ahead.
- **Review fixes:** `gh label` and a plain `gh api` removed from `/triage`'s allowed commands (they were wider than spec 4.6); who filed an issue is decided from its author login, not the form's checkbox; chunk stage 1 treats issue comments as data. These narrow the skill toward the spec's wording; no spec text changed.
- **Step 1's TypeScript check** was not run (no `node_modules` in the worktree); step 7's `pulumi preview` is the check.
