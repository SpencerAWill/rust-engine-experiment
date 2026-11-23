# M3.20: Planning state on GitHub

- **Status:** Draft (2026-11-23)
- **Milestone:** M3 (not part of its exit criteria) · **Depends on:** M3.18 · **Decides:** no ADR (process and documentation, as M3.18; the Project and the GitHub writes are configuration, see §4.8)
- **Issue:** none yet; this chunk's own issue is created in its first migration step (§4.8), and its pull request closes it
- **Estimated size:** about 450 changed lines of Markdown, about 40 lines in `infra/github/index.ts`

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

Today "what is next" has two partial answers: `docs/roadmap.md` orders the scheduled chunks, and the "Engine backlog" Project ranks everything else, so Ready issues such as #90 to #93 are invisible to `/chunk next`, and milestones and chunk statuses are kept twice (roadmap tables and tracking issues #66 and #67). This chunk moves planning state (what is scheduled, in what order, in which milestone, and its status) to GitHub issues, milestones and the Project, and keeps documents (specs, plans, logs, ADRs, lessons, milestone goals) in the repository. At the end, `/chunk next` answers from GitHub alone, every scheduled chunk is an issue in a milestone with an Order, and the roadmap file holds milestone goals and history only.

## 2. Goals and non-goals

**Goals**

- One answer to "what is next": the scheduled chunk of the current milestone with no open blocker and the lowest Order (§4.4).
- Every chunk is exactly one issue, its chunk issue (maintainer).
- Small work that is not a chunk has its own lane, chores (maintainer, §4.3).
- Milestones are GitHub milestones declared in Pulumi, as the labels are (maintainer).
- `docs/roadmap.md` keeps the milestone goals and exit criteria and, frozen, the history of done chunks, so every existing link still resolves (maintainer).

**Non-goals**

- No change to the seven stages, the two waits, specs, plans, logs, lessons or chunk IDs (§4.2 only says when an ID is given).
- No "In progress" Status option: the linked branch or draft pull request shows it, and adding an option means editing the Project by hand *(my call)*.
- The Project, its fields and views stay outside code (M3.18 §4.6); only milestones join the labels in Pulumi.
- No CI check of planning state.

## 3. Guide-level explanation

**Scheduling.** The maintainer picks a Ready issue and either makes it a chunk (title `M3.10: bevy_ecs and hecs baselines`, milestone `M3`, Status Scheduled, Order 10, "blocked by" links for what must land first) or schedules it as a chore (Status Scheduled, an Order, no chunk ID). An agent may make these writes when the maintainer asks for them.

**What is next.** `/chunk next` reads the Project with `gh`: in the current milestone, the open chunk issues with Status Scheduled and no open blocker, lowest Order first. It names the first and lists the scheduled chores, and the maintainer picks; then stage 1 starts on the chunk issue. A "Next up" view in the Project (filter Status Scheduled, sort by Order) shows the same list in the browser.

**Done.** The chunk's pull request says `Closes #N`; merging closes the issue, the Project shows Done, and the milestone's progress bar moves. Nothing in the repository records the status.

## 4. Reference-level design

### 4.1 Sources of truth

| What | Where |
|---|---|
| Milestone goals and exit criteria | `docs/roadmap.md` (milestone table); the GitHub milestone's description is its goal in one line and a link |
| Milestone membership and progress | GitHub milestone |
| Unscheduled work and its ranking | Project: Status Inbox or Ready, Priority, Size |
| Scheduled work, its order and dependencies | Project: Status Scheduled and Order; "blocked by" links between issues |
| What a chunk means | Its spec, once it exists (the issue is the request and is not edited to match) |
| State of a chunk in flight | Branch, plan checkboxes, `log.md` (unchanged) |
| Done | The issue is closed (by the pull request) |
| History of done chunks up to M3.20 | `docs/roadmap.md` tables, frozen |

Priority ranks unscheduled work; Order ranks scheduled work. Neither is read for the other.

### 4.2 Chunk issues

- Every chunk MUST have exactly one issue, its chunk issue, titled `M<N>.<n>: <title>`. A chunk is scheduled by: giving its issue the next unused chunk ID of the milestone, one above the highest ID in any issue title (open or closed), chunk folder (`docs/chunks/m<N>.<n>-*`) or frozen roadmap row (IDs are never reused; #41, which reserves M3.12, is retitled `M3.12: ...` in the migration so the rule sees it) *(main session's call)*, the milestone, Status Scheduled, an Order, and its "blocked by" links.
- A chunk issue's body meets the Definition of Ready. Text a later chunk must read (such as the caveats in M3.8's roadmap row) goes in its body, linking the logs it comes from.
- The spec's header `**Issue:**` line names the chunk issue and is no longer optional; the template's "Followed by" convention is replaced by a "blocked by" link or the Order.
- When the maintainer names work for `/chunk` that is not yet a scheduled chunk issue, naming it is the scheduling: stage 1 gives the issue (creating it if there is none) its chunk ID, the current milestone, Status Scheduled and an Order below the current lowest, and says what it wrote. Otherwise stage 1 writes no Project field: Status Scheduled covers queued and in-flight chunks, and the branch shows which *(main session's call)*.
- The pull request says `Closes #<chunk issue>`.
- Sub-issues stay for grouping (an epic such as #78); a tracking issue per milestone is no longer used.

### 4.3 Chores

A chore is a scheduled issue without a chunk ID, scheduled into the current milestone *(main session's call)*: Size S, done as a plain pull request (conventional commit, CI, `Chunk: None (chore, #N)` in the template, a review if not trivial, `Closes #N`), not the seven stages. It has an Order like a chunk. `/chunk next` lists the current milestone's scheduled chores beside the next chunk, and the maintainer picks; an issue bigger than S is scheduled as a chunk. At a milestone boundary, open chores move to the next milestone (`docs/maintainers.md` checklist).

### 4.4 Order, dependencies and "next"

- The Project gains a number field `Order`. Orders SHOULD be spaced (10, 20, 30) so an item can be inserted without renumbering; equal Orders are broken by chunk ID, then issue number.
- A hard dependency is a "blocked by" link (`gh issue edit <n> --add-blocked-by <m>`); it replaces the roadmap's Depends-on column.
- Changing an Order or a dependency of a scheduled item SHOULD come with a one-line comment on the issue saying why, since Project fields have no history *(my call)*.
- **Current milestone:** the open GitHub milestone with the lowest number in its title (`M3` before `M4`).
- **Next chunk:** among the open issues in the current milestone whose title has a chunk ID and whose Status is Scheduled, those with no open "blocked by" issue, lowest Order. If none is eligible, `/chunk next` says so, names the milestone-boundary checklist in `docs/maintainers.md` if every chunk of the milestone is closed, and waits, as today.
- A Scheduled issue in the current milestone with no Order, or a Scheduled issue with no milestone, is not eligible; `/chunk next` lists each such issue as needing a field before it can be next. Scheduled items of later milestones are ignored *(main session's call)*.
- Every read goes through `gh` (`gh project item-list --limit 1000`, `gh issue view --json`, `gh api --method GET .../dependencies/blocked_by`).

### 4.5 Milestones in Pulumi

`infra/github/index.ts` declares one `github.RepositoryMilestone` per open milestone, M3 to M9: title `M<N>`, description the milestone's goal in one line plus a link to `docs/roadmap.md#milestones`, state `open`. Done milestones (M0+M1, M2) are not created. Pulumi creates them and ignores later changes to `state` (`ignoreChanges: ["state"]`), so closing a milestone at its boundary is one command in the checklist (`gh api --method PATCH repos/<owner>/<repo>/milestones/<number> -f state=closed`), not a pull request *(main session's call)*. The maintainer runs `pulumi up`, as for labels (§4.8).

### 4.6 The roadmap file

`docs/roadmap.md` keeps: the introduction (rewritten to say what moved and where), the milestone table with goals and exit criteria (its Status column replaced by a pointer to the GitHub milestones), the M2 and M3 chunk tables frozen as history up to M3.20 (with a line saying new chunks are issues), the "Later milestones" table of rough lists and open questions (read when a milestone becomes current), and the milestone numbering note. The open rows (M3.7, M3.8, M3.10) move to chunk issues (§4.8) and are removed; M3.20's row becomes its last entry, `Done (PR #n)`. Every heading anchor that historical documents link to (`#milestones`, `#m3-chunks`, `#m2-chunks`) MUST still resolve.

### 4.7 Documents and skills

Each file below changes only where it names the roadmap as the place of scheduled work, order or status:

- `docs/workflow.md` ("From issue to chunk", "Who is the source of truth", the cardinality table): §4.1 to §4.4.
- `docs/maintainers.md` ("Issue tracking", "When a milestone completes"): the Order field, the "Next up" view, how to schedule a chunk and a chore, closing a milestone in Pulumi, no tracking issue to close.
- `.claude/skills/chunk/SKILL.md` (`next`) and stages 1, 2, 3 and 7: `next` reads GitHub (§4.4); stage 1 starts from the chunk issue and edits no roadmap row; stages 2 and 3 set no roadmap status; stage 7 closes through `Closes #N` and edits no roadmap row (the spec, plan and log statuses are unchanged).
- `.claude/skills/triage/SKILL.md`: the field check includes `Order` (a number field); triage still never sets Scheduled or Order.
- `AGENTS.md`, `CONTRIBUTING.md`, `README.md`, `docs/README.md`, `docs/chunks/README.md`, `.claude/README.md`, `.github/PULL_REQUEST_TEMPLATE.md`, the issue forms' "Depends on" hints, and `docs/chunks/_template/spec.md`: where they point to the roadmap for planned work.

Historical documents (specs, plans and logs of done chunks, ADRs) are not edited.

### 4.8 Migration (GitHub writes, authorized by this spec)

These writes change GitHub, not the repository, so they are not in the pull request:

- **Before the pull request** (so the pull request can close it): this chunk's own issue, `M3.20: Planning state on GitHub`, created with the Definition of Ready headings and added to the Project; and the `Order` number field created on the Project (`gh project field-create`).
- **After the merge**, once the maintainer has run `pulumi up` (milestones exist), as one checklist in the plan, each item checked by reading it back:
  1. Chunk issues for M3.10, M3.7 and M3.8, their bodies in the Definition of Ready headings: the problem and proposal from the roadmap row's text (M3.8's caveats with their log links), the outcome and verification from the row and the M3 exit criteria, the dependencies from the row; no design beyond the row *(main session's call)*; Status Scheduled, milestone M3, Orders 10 (M3.10), 20 (M3.7), 30 (M3.8), and M3.8 blocked by the other two.
  2. Milestone M3 on every open direct sub-issue of #66, milestone M4 on every open direct sub-issue of #67, and M3 on this chunk's issue; any other issue gets a milestone only when it is scheduled. A milestone says where an issue belongs; only Status Scheduled makes it eligible to be next.
  3. #41 retitled `M3.12: ...` (§4.2), unscheduled.
  4. #66 and #67 closed as completed, each with a comment that milestones replace them.
  5. The "Next up" view: created by the maintainer in the browser (views are not settable through `gh`), as `docs/maintainers.md` describes.

If the pull request is abandoned, only the two "before" writes exist, and both are removed.

## 5. Requirements

- **M3.20-R1** `docs/workflow.md` and `docs/maintainers.md` MUST state §4.1 to §4.4: the sources of truth, chunk issues, chores, the Order field, "blocked by" links and the "next" rule.
- **M3.20-R2** `/chunk next` MUST select the next chunk by §4.4 from GitHub, list the scheduled chores, and stop with the milestone checklist when none is eligible.
- **M3.20-R3** No stage of `/chunk` and no skill MUST read or write the roadmap file for order or status.
- **M3.20-R4** `/triage`'s field check MUST include `Order` as a number field, and `/triage` MUST still never set Status Scheduled or Order.
- **M3.20-R5** `infra/github/index.ts` MUST declare milestones M3 to M9 (§4.5), and `pulumi preview` MUST show seven milestones to create; any other difference it shows is recorded in `log.md`, not fixed by this chunk.
- **M3.20-R6** `docs/roadmap.md` MUST have the shape of §4.6, with every heading anchor that existing documents link to still resolving.
- **M3.20-R7** Every file of §4.7 MUST name GitHub, not the roadmap, as the place of scheduled work, order and status; historical documents MUST be unchanged.
- **M3.20-R8** The spec template's `**Issue:**` line MUST be required, and the PR template MUST name chores.
- **M3.20-R9** The migration of §4.8 MUST be done and read back: the "before" writes before the pull request opens, the "after" checklist after the merge.
- **M3.20-R10** After the migration, a dry run of `/chunk next`'s selection MUST name M3.10.

## 6. Cross-cutting concerns

- **Determinism:** n/a, because no engine code changes.
- **`unsafe`:** n/a, because no Rust code changes.
- **Concurrency:** n/a for code; two sessions changing Orders at once is possible and acceptable for one maintainer (the read-back catches a lost write).
- **Heap allocation:** n/a.
- **`no_std` and wasm:** n/a.
- **Frozen formats:** none.
- **Public API changes:** none.
- **FFI exposure:** none.
- **Performance:** n/a.
- **Documentation:** §4.7; the CHANGELOG gets a "Changed" entry.
- **Policy changes:** yes: the workflow's source of truth for planning (no ADR: process, as M3.18 decided for its own change; the decision is reversible by moving the state back). `infra/github` gains milestones; the maintainer runs `pulumi up`.

## 7. Alternatives considered

- **Delete `roadmap.md`.** Rejected (maintainer): about 60 historical documents link into it, and the milestone goals are documents.
- **Generate `roadmap.md` from GitHub.** Rejected (maintainer): a second copy that goes stale between runs.
- **Order by Priority and issue number only.** Rejected (maintainer): ties decided by issue number, not by the maintainer.
- **The Project's manual drag order.** Rejected (maintainer): `gh` does not expose it reliably, so agents could not read it.
- **Milestones by hand, or kept beside tracking issues.** Rejected (maintainer): unversioned, or two places per milestone again.
- **Chunks without issues.** Rejected (maintainer): "what is next" would miss them.
- **Every small fix a chunk, or folded into nearby chunks.** Rejected (maintainer) for the chore lane.
- **An "In progress" Status.** Rejected *(my call)*: the branch shows it; adding an option means a hand edit of the Project.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | review-docs and review-spec against §4.1 to §4.4 |
| R2 | The `next` section read against §4.4; R10's dry run |
| R3 | `git grep -n roadmap .claude/` lists no read or write of order or status (each remaining mention named in `log.md`) |
| R4 | The triage skill's text; a dry run of its field check after the Order field exists, recorded in `log.md` |
| R5 | `pulumi preview` output in `log.md` (run by the maintainer or with their credentials) |
| R6 | `cargo xtask check-docs` (links and anchors in every Markdown file) |
| R7 | `git grep -n -i roadmap` over the §4.7 files, each remaining mention checked; `git diff --stat main` shows no historical document |
| R8 | The two templates |
| R9 | `gh issue view`, `gh project item-list` and `gh api --method GET` read-backs in `log.md`, per checklist item |
| R10 | The selection run by hand with `gh` after the migration, its result in `log.md` |

## 9. Done when

- [ ] R1 to R8 have their evidence; `cargo xtask ci` passes.
- [ ] The pull request is merged and the maintainer has run `pulumi up`.
- [ ] R9 and R10 have their evidence: the "before" writes in `log.md`; the "after" checklist, which runs after the merge when an agent can no longer commit to the branch, as a comment on this chunk's issue.

## 10. Open questions

None.

## 11. Prior art and sources

GitHub's documentation for milestones, issue dependencies ("blocked by"), sub-issues and Project fields; `gh issue edit --add-blocked-by` and `--milestone` (gh 2.102.0); the Pulumi GitHub provider's `RepositoryMilestone`. M3.18's spec, whose choices this chunk revisits.
